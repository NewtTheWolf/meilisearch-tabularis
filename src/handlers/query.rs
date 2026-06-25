//! Connectivity and query execution.
//!
//! Meilisearch has no SQL. `execute_query` is only used for *browsing*: the
//! grid sends `SELECT * FROM "index"`, and we page through that index's
//! documents. Real search happens in the UI extension, not here.

use std::time::Instant;

use serde_json::{json, Value};

use crate::client::Client;
use crate::models::{inner_params, ConnectionParams};
use crate::rpc::{error_response, not_implemented, ok_response};
use crate::utils::{pagination, schema, sql};

pub fn test_connection(id: Value, params: &Value) -> Value {
    match health(params) {
        Ok(()) => ok_response(id, json!({ "success": true })),
        Err((code, msg)) => error_response(id, code, &msg),
    }
}

pub fn ping(id: Value, params: &Value) -> Value {
    match health(params) {
        Ok(()) => ok_response(id, Value::Null),
        Err((code, msg)) => error_response(id, code, &msg),
    }
}

fn health(params: &Value) -> Result<(), (i64, String)> {
    let conn = ConnectionParams::from_value(inner_params(params));
    let client = Client::from_params(&conn).map_err(|e| (e.code, e.message))?;
    client.health().map_err(|e| (e.code, e.message))
}

pub fn execute_query(id: Value, params: &Value) -> Value {
    let conn = ConnectionParams::from_value(inner_params(params));
    let query = params.get("query").and_then(Value::as_str).unwrap_or("");
    let page = params.get("page").and_then(Value::as_u64).unwrap_or(1);
    let page_size = params
        .get("page_size")
        .and_then(Value::as_u64)
        .unwrap_or(100);

    let index = match sql::table_from_query(query) {
        Some(i) => i,
        None => {
            return error_response(
                id,
                -32602,
                "could not determine the index to browse from the query",
            )
        }
    };

    let client = match Client::from_params(&conn) {
        Ok(c) => c,
        Err(e) => return error_response(id, e.code, &e.message),
    };

    let off = pagination::offset(page, page_size);
    let path = format!("/indexes/{index}/documents?offset={off}&limit={page_size}");

    let started = Instant::now();
    let resp = match client.get(&path) {
        Ok(v) => v,
        Err(e) => return error_response(id, e.code, &e.message),
    };
    let elapsed_ms = started.elapsed().as_millis() as u64;

    let docs: Vec<Value> = resp
        .get("results")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let total = resp
        .get("total")
        .and_then(Value::as_u64)
        .unwrap_or(docs.len() as u64);

    let columns = schema::union_columns(&docs);
    let rows: Vec<Value> = docs
        .iter()
        .map(|doc| {
            Value::Array(
                columns
                    .iter()
                    .map(|c| doc.get(c).cloned().unwrap_or(Value::Null))
                    .collect(),
            )
        })
        .collect();

    ok_response(
        id,
        json!({
            "columns": columns,
            "rows": rows,
            "total_count": total,
            "execution_time_ms": elapsed_ms,
        }),
    )
}

pub fn explain_query(id: Value, _params: &Value) -> Value {
    // Meilisearch has no query plan to explain.
    not_implemented(id, "explain_query")
}
