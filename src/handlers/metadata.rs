//! Schema metadata.
//!
//! Meilisearch maps cleanly onto a flat catalogue:
//!   - one logical "database" (the instance),
//!   - each index is a "table",
//!   - columns are inferred from the index's primary key + a document sample.
//!
//! Everything relational (schemas, FKs, indexes, views, routines) returns
//! empty, which the host renders as "not supported" without errors.

use serde_json::{json, Value};

use crate::client::Client;
use crate::config;
use crate::models::{inner_params, ConnectionParams};
use crate::rpc::{error_response, ok_response};
use crate::utils::schema;

fn connect(params: &Value) -> Result<Client, (i64, String)> {
    let conn = ConnectionParams::from_value(inner_params(params));
    Client::from_params(&conn).map_err(|e| (e.code, e.message))
}

pub fn get_databases(id: Value, _params: &Value) -> Value {
    // Meilisearch has no database hierarchy — one logical catalogue.
    ok_response(id, json!(["meilisearch"]))
}

pub fn get_tables(id: Value, params: &Value) -> Value {
    let client = match connect(params) {
        Ok(c) => c,
        Err((code, msg)) => return error_response(id, code, &msg),
    };

    // Meilisearch caps `limit` at 1000 indexes per page; that's plenty here.
    let resp = match client.get("/indexes?limit=1000") {
        Ok(v) => v,
        Err(e) => return error_response(id, e.code, &e.message),
    };

    let tables: Vec<Value> = resp
        .get("results")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|idx| {
                    let uid = idx.get("uid").and_then(Value::as_str)?;
                    Some(json!({ "name": uid, "schema": Value::Null, "comment": Value::Null }))
                })
                .collect()
        })
        .unwrap_or_default();

    ok_response(id, Value::Array(tables))
}

pub fn get_columns(id: Value, params: &Value) -> Value {
    let table = params.get("table").and_then(Value::as_str).unwrap_or("");
    if table.is_empty() {
        return error_response(id, -32602, "missing 'table' (index name)");
    }

    let client = match connect(params) {
        Ok(c) => c,
        Err((code, msg)) => return error_response(id, code, &msg),
    };

    // Primary key drives is_primary_key (and lets the host do update/delete).
    if !crate::utils::sql::valid_identifier(table) {
        return error_response(id, -32602, "Invalid index name");
    }
    let metadata = match client.get(&format!("/indexes/{table}")) {
        Ok(v) => v,
        Err(e) => return error_response(id, e.code, &e.message),
    };
    let primary_key = metadata
        .get("primaryKey")
        .and_then(Value::as_str)
        .map(str::to_string);
    let sample_size = config::sample_size();
    let docs = match client.get(&format!("/indexes/{table}/documents?limit={sample_size}")) {
        Ok(v) => v
            .get("results")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        Err(e) => return error_response(id, e.code, &e.message),
    };

    let mut columns = schema::union_columns(&docs);

    // Empty index? Fall back to declared attributes so the grid isn't blank.
    if columns.is_empty() {
        if let Ok(settings) = client.get(&format!("/indexes/{table}/settings")) {
            for key in ["displayedAttributes", "searchableAttributes"] {
                if let Some(arr) = settings.get(key).and_then(Value::as_array) {
                    for a in arr.iter().filter_map(Value::as_str) {
                        if a != "*" && !columns.iter().any(|c| c == a) {
                            columns.push(a.to_string());
                        }
                    }
                }
                if !columns.is_empty() {
                    break;
                }
            }
        }
    }

    // Ensure the primary key exists and sits first.
    if let Some(pk) = &primary_key {
        columns.retain(|c| c != pk);
        columns.insert(0, pk.clone());
    }

    let result: Vec<Value> = columns
        .iter()
        .map(|name| {
            let is_pk = primary_key.as_deref() == Some(name.as_str());
            json!({
                "name": name,
                "data_type": schema::infer_column_type(&docs, name),
                "is_nullable": !is_pk,
                "default_value": Value::Null,
                "is_pk": is_pk,
                "is_auto_increment": false,
                "comment": Value::Null,
            })
        })
        .collect();

    ok_response(id, Value::Array(result))
}

pub fn get_schemas(id: Value, _params: &Value) -> Value {
    ok_response(id, json!([]))
}

pub fn get_foreign_keys(id: Value, _params: &Value) -> Value {
    ok_response(id, json!([]))
}

pub fn get_indexes(id: Value, _params: &Value) -> Value {
    ok_response(id, json!([]))
}

pub fn get_views(id: Value, _params: &Value) -> Value {
    ok_response(id, json!([]))
}

pub fn get_view_definition(id: Value, _params: &Value) -> Value {
    ok_response(id, Value::String(String::new()))
}

pub fn get_view_columns(id: Value, _params: &Value) -> Value {
    ok_response(id, json!([]))
}

pub fn get_routines(id: Value, _params: &Value) -> Value {
    ok_response(id, json!([]))
}

pub fn get_routine_parameters(id: Value, _params: &Value) -> Value {
    ok_response(id, json!([]))
}

pub fn get_routine_definition(id: Value, _params: &Value) -> Value {
    ok_response(id, Value::String(String::new()))
}

pub fn get_schema_snapshot(id: Value, _params: &Value) -> Value {
    // ER diagram — Meilisearch has no relationships. ponytail: return empty;
    // wire to get_tables/get_columns if a per-index overview is ever wanted.
    ok_response(
        id,
        json!({
            "tables": [],
            "columns": {},
            "foreign_keys": {},
        }),
    )
}

pub fn get_all_columns_batch(id: Value, _params: &Value) -> Value {
    ok_response(id, json!({}))
}

pub fn get_all_foreign_keys_batch(id: Value, _params: &Value) -> Value {
    ok_response(id, json!({}))
}
