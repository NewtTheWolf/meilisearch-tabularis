//! Connectivity and query execution.
//!
//! Meilisearch has no SQL. `execute_query` serves two callers:
//!   - the grid *browsing* a table — `SELECT * FROM "index"` → page documents;
//!   - the search toolbar UI extension — a `@meili-search:{json}` sentinel
//!     (routed here via `usePluginQuery`) → a real Meilisearch search.
//!
//! Both return the same `{ columns, rows, total_count, execution_time_ms }`
//! shape the host grid expects.

use std::time::Instant;

use serde_json::{json, Value};

use crate::client::Client;
use crate::models::{inner_params, ConnectionParams};
use crate::rpc::{error_response, not_implemented, ok_response};
use crate::utils::{pagination, schema, sql};

/// Prefix the UI extension uses to ask for a search instead of a browse.
/// Followed by JSON: `{ "index": string, "q": string, "limit"?, "offset"? }`.
const SEARCH_PREFIX: &str = "@meili-search:";

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

    let client = match Client::from_params(&conn) {
        Ok(c) => c,
        Err(e) => return error_response(id, e.code, &e.message),
    };

    // Search request from the toolbar UI extension.
    if let Some(spec) = query.strip_prefix(SEARCH_PREFIX) {
        return run_search(id, &client, spec);
    }

    // Otherwise: browse documents in the index named by the SELECT.
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

    let off = pagination::offset(page, page_size);
    let path = format!("/indexes/{index}/documents?offset={off}&limit={page_size}");

    let started = Instant::now();
    let resp = match client.get(&path) {
        Ok(v) => v,
        Err(e) => return error_response(id, e.code, &e.message),
    };
    let elapsed_ms = started.elapsed().as_millis() as u64;

    let docs = resp
        .get("results")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let total = resp
        .get("total")
        .and_then(Value::as_u64)
        .unwrap_or(docs.len() as u64);

    ok_response(id, result_payload(docs, total, elapsed_ms))
}

fn run_search(id: Value, client: &Client, spec: &str) -> Value {
    let spec: Value = match serde_json::from_str(spec) {
        Ok(v) => v,
        Err(e) => return error_response(id, -32602, &format!("invalid search spec: {e}")),
    };
    let index = match spec.get("index").and_then(Value::as_str) {
        Some(i) if !i.is_empty() => i,
        _ => return error_response(id, -32602, "search spec missing 'index'"),
    };
    let q = spec.get("q").and_then(Value::as_str).unwrap_or("");
    let limit = spec.get("limit").and_then(Value::as_u64).unwrap_or(20);
    let offset = spec.get("offset").and_then(Value::as_u64).unwrap_or(0);

    let body = json!({
        "q": q,
        "limit": limit,
        "offset": offset,
        "showRankingScore": true,
    });

    let started = Instant::now();
    let resp = match client.post(&format!("/indexes/{index}/search"), &body) {
        Ok(v) => v,
        Err(e) => return error_response(id, e.code, &e.message),
    };
    let elapsed_ms = started.elapsed().as_millis() as u64;

    let hits = resp
        .get("hits")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let total = resp
        .get("estimatedTotalHits")
        .and_then(Value::as_u64)
        .unwrap_or(hits.len() as u64);

    ok_response(id, result_payload(hits, total, elapsed_ms))
}

/// Turn a list of documents into the grid payload: union of keys as columns,
/// each row aligned to those columns (missing keys → null).
fn result_payload(docs: Vec<Value>, total: u64, elapsed_ms: u64) -> Value {
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

    json!({
        "columns": columns,
        "rows": rows,
        "total_count": total,
        "execution_time_ms": elapsed_ms,
    })
}

pub fn explain_query(id: Value, _params: &Value) -> Value {
    // Meilisearch has no query plan to explain.
    not_implemented(id, "explain_query")
}
