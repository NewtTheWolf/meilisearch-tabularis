//! Row-level CRUD against the Meilisearch documents API.
//!
//! Meilisearch writes are asynchronous (each call enqueues a task and returns
//! a `taskUid`). v1 is fire-and-forget: a 2xx enqueue counts as success. If
//! the document isn't indexed yet when the grid refreshes, a second refresh
//! shows it. ponytail: poll `/tasks/{uid}` if "edited but not visible" confuses
//! users.

use serde_json::{json, Map, Value};

use crate::client::Client;
use crate::models::{inner_params, ConnectionParams};
use crate::rpc::{error_response, ok_response};

fn connect(params: &Value) -> Result<Client, (i64, String)> {
    let conn = ConnectionParams::from_value(inner_params(params));
    Client::from_params(&conn).map_err(|e| (e.code, e.message))
}

fn table_of(params: &Value) -> Option<&str> {
    params.get("table").and_then(Value::as_str)
}

/// Render a primary-key value for use in a URL path.
fn pk_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

pub fn insert_record(id: Value, params: &Value) -> Value {
    let Some(table) = table_of(params) else {
        return error_response(id, -32602, "missing 'table' (index name)");
    };
    let data = params.get("data").cloned().unwrap_or_else(|| json!({}));

    let client = match connect(params) {
        Ok(c) => c,
        Err((code, msg)) => return error_response(id, code, &msg),
    };

    match client.post(&format!("/indexes/{table}/documents"), &json!([data])) {
        Ok(_) => ok_response(id, Value::Null),
        Err(e) => error_response(id, e.code, &e.message),
    }
}

pub fn update_record(id: Value, params: &Value) -> Value {
    let Some(table) = table_of(params) else {
        return error_response(id, -32602, "missing 'table' (index name)");
    };
    let pk_col = params.get("pk_col").and_then(Value::as_str);
    let col_name = params.get("col_name").and_then(Value::as_str);
    let (Some(pk_col), Some(col_name)) = (pk_col, col_name) else {
        return error_response(id, -32602, "missing 'pk_col' or 'col_name'");
    };
    let pk_val = params.get("pk_val").cloned().unwrap_or(Value::Null);
    let new_val = params.get("new_val").cloned().unwrap_or(Value::Null);

    // Partial update: send the primary key plus only the changed field.
    let mut doc = Map::new();
    doc.insert(pk_col.to_string(), pk_val);
    doc.insert(col_name.to_string(), new_val);

    let client = match connect(params) {
        Ok(c) => c,
        Err((code, msg)) => return error_response(id, code, &msg),
    };

    match client.put(
        &format!("/indexes/{table}/documents"),
        &json!([Value::Object(doc)]),
    ) {
        Ok(_) => ok_response(id, json!(1)),
        Err(e) => error_response(id, e.code, &e.message),
    }
}

pub fn delete_record(id: Value, params: &Value) -> Value {
    let Some(table) = table_of(params) else {
        return error_response(id, -32602, "missing 'table' (index name)");
    };
    let pk_val = params.get("pk_val").cloned().unwrap_or(Value::Null);
    if pk_val.is_null() {
        return error_response(id, -32602, "missing 'pk_val'");
    }

    let client = match connect(params) {
        Ok(c) => c,
        Err((code, msg)) => return error_response(id, code, &msg),
    };

    match client.delete(&format!(
        "/indexes/{table}/documents/{}",
        pk_to_string(&pk_val)
    )) {
        Ok(_) => ok_response(id, json!(1)),
        Err(e) => error_response(id, e.code, &e.message),
    }
}
