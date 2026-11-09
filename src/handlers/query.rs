//! Connectivity and query execution.
//!
//! Meilisearch has no SQL. `execute_query` serves two callers:
//!   - the grid *browsing* a table — `SELECT * FROM "index"` → page documents;
//!   - the search toolbar UI extension — a `@meili-search:{json}` sentinel
//!     entered in the normal query editor for a native Meilisearch search.
//!
//! Both return the host QueryResult and pagination
//! shape the host grid expects.

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
    let query = params
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();

    let client = match Client::from_params(&conn) {
        Ok(c) => c,
        Err(e) => return error_response(id, e.code, &e.message),
    };

    // Search request from the toolbar UI extension.
    if let Some(spec) = query.strip_prefix(SEARCH_PREFIX) {
        return run_search(id, &client, spec);
    }

    // Otherwise: browse documents in the index named by the SELECT.
    let page = params
        .get("page")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .max(1);
    let page_size = params
        .get("limit")
        .filter(|v| !v.is_null())
        .or_else(|| params.get("page_size"))
        .and_then(Value::as_u64)
        .unwrap_or(100);

    if !(1..=1000).contains(&page_size) {
        return error_response(id, -32602, "Page size must be between 1 and 1000");
    }
    let index = match sql::table_from_query(query) {
        Some(i) => i,
        None => {
            return error_response(
                id,
                -32602,
                "Only SELECT * FROM index is supported for browsing. Use @meili-search:{\"index\":\"movies\",\"q\":\"text\"} for native search, filters, facets and sorting.",
            )
        }
    };

    let off = pagination::offset(page, page_size);
    let path = format!("/indexes/{index}/documents?offset={off}&limit={page_size}");

    let resp = match client.get(&path) {
        Ok(v) => v,
        Err(e) => return error_response(id, e.code, &e.message),
    };

    let docs = resp
        .get("results")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let total = resp
        .get("total")
        .and_then(Value::as_u64)
        .unwrap_or(docs.len() as u64);

    ok_response(id, result_payload(docs, total, page, page_size, off))
}

fn run_search(id: Value, client: &Client, spec: &str) -> Value {
    let spec: Value = match serde_json::from_str(spec) {
        Ok(v) => v,
        Err(e) => return error_response(id, -32602, &format!("invalid search spec: {e}")),
    };
    let index = match spec.get("index").and_then(Value::as_str) {
        Some(i) if sql::valid_identifier(i) => i,
        _ => return error_response(id, -32602, "search spec missing 'index'"),
    };
    if !spec.is_object() {
        return error_response(id, -32602, "Search spec must be an object");
    }
    let limit = spec.get("limit").and_then(Value::as_u64).unwrap_or(20);
    let offset = spec.get("offset").and_then(Value::as_u64).unwrap_or(0);

    if !(1..=1000).contains(&limit) {
        return error_response(id, -32602, "Search limit must be between 1 and 1000");
    }
    let mut body = spec.as_object().cloned().unwrap_or_default();
    body.remove("index");
    body.insert("limit".into(), json!(limit));
    body.insert("offset".into(), json!(offset));
    let body = Value::Object(body);

    let resp = match client.post(&format!("/indexes/{index}/search"), &body) {
        Ok(v) => v,
        Err(e) => return error_response(id, e.code, &e.message),
    };

    let hits = resp
        .get("hits")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let total = resp
        .get("estimatedTotalHits")
        .and_then(Value::as_u64)
        .unwrap_or(hits.len() as u64);

    let page = offset.checked_div(limit).map(|p| p + 1).unwrap_or(1);
    ok_response(id, result_payload(hits, total, page, limit, offset))
}

/// Turn a list of documents into the host `QueryResult` shape: union of keys as
/// columns, each row aligned to those columns (missing keys → null), plus the
/// pagination block the grid needs. `affected_rows` is 0 — these are reads.
fn result_payload(docs: Vec<Value>, total: u64, page: u64, page_size: u64, offset: u64) -> Value {
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

    let has_more = !rows.is_empty() && offset.saturating_add(rows.len() as u64) < total;

    json!({
        "columns": columns,
        "rows": rows,
        "affected_rows": 0,
        "truncated": false,
        "pagination": {
            "page": page,
            "page_size": page_size,
            "total_rows": total,
            "has_more": has_more,
        },
    })
}

pub fn explain_query(id: Value, _params: &Value) -> Value {
    // Meilisearch has no query plan to explain.
    not_implemented(id, "explain_query")
}
