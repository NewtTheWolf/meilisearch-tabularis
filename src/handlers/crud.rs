//! Document CRUD waits for indexing and accepts the host's pk_map contract.
use crate::client::Client;
use crate::error::PluginError;
use crate::models::{inner_params, ConnectionParams};
use crate::rpc::{error_response, ok_response};
use crate::utils::sql::valid_identifier;
use serde_json::{json, Value};

fn context(params: &Value) -> Result<(Client, String, String), PluginError> {
    let table = params
        .get("table")
        .and_then(Value::as_str)
        .filter(|s| valid_identifier(s))
        .ok_or_else(|| PluginError::invalid_params("Invalid or missing index name"))?
        .to_string();
    let client = Client::from_params(&ConnectionParams::from_value(inner_params(params)))?;
    let metadata = client.get(&format!("/indexes/{table}"))?;
    let pk = metadata
        .get("primaryKey")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            PluginError::invalid_params(
                "Index has no primary key; create it with an explicit primary key first",
            )
        })?
        .to_string();
    Ok((client, table, pk))
}

fn document_id(value: &Value) -> Result<String, PluginError> {
    let id = match value {
        Value::String(s) => s.clone(),
        Value::Number(n) if n.is_i64() || n.is_u64() => n.to_string(),
        _ => {
            return Err(PluginError::invalid_params(
                "Primary key must be a string or integer",
            ))
        }
    };
    if !valid_identifier(&id) {
        return Err(PluginError::invalid_params("Invalid document ID"));
    }
    Ok(id)
}

fn primary_value<'a>(params: &'a Value, pk: &str) -> Result<&'a Value, PluginError> {
    if let Some(map) = params.get("pk_map").and_then(Value::as_object) {
        if map.len() == 1 {
            if let Some(value) = map.get(pk) {
                return Ok(value);
            }
        }
        return Err(PluginError::invalid_params(
            "pk_map must contain exactly the index primary key",
        ));
    }
    if params.get("pk_col").and_then(Value::as_str) == Some(pk) {
        return params
            .get("pk_val")
            .ok_or_else(|| PluginError::invalid_params("Missing primary key value"));
    }
    Err(PluginError::invalid_params("Missing pk_map"))
}

fn respond(id: Value, result: Result<(), PluginError>) -> Value {
    match result {
        Ok(()) => ok_response(id, json!(1)),
        Err(e) => error_response(id, e.code, &e.message),
    }
}

pub fn insert_record(id: Value, params: &Value) -> Value {
    respond(
        id,
        (|| {
            let (client, table, pk) = context(params)?;
            let data = params
                .get("data")
                .and_then(Value::as_object)
                .filter(|d| !d.is_empty())
                .ok_or_else(|| {
                    PluginError::invalid_params("Document data must be a non-empty object")
                })?;
            let value = data.get(&pk).ok_or_else(|| {
                PluginError::invalid_params("Document must include the index primary key")
            })?;
            let key = document_id(value)?;
            // Meilisearch POST replaces documents. Reject duplicates to preserve INSERT semantics.
            match client.get(&format!("/indexes/{table}/documents/{key}")) {
                Ok(_) => {
                    return Err(PluginError::invalid_params(
                        "Document already exists; use Update instead",
                    ))
                }
                Err(e) if e.message.contains("document_not_found") => {}
                Err(e) => return Err(e),
            }
            client.wait_task(client.post(&format!("/indexes/{table}/documents"), &json!([data]))?)
        })(),
    )
}

pub fn update_record(id: Value, params: &Value) -> Value {
    respond(
        id,
        (|| {
            let (client, table, pk) = context(params)?;
            let value = primary_value(params, &pk)?;
            let key = document_id(value)?;
            let col = params
                .get("col_name")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| PluginError::invalid_params("Missing column name"))?;
            if col == pk {
                return Err(PluginError::invalid_params(
                    "Changing a Meilisearch primary key is not supported",
                ));
            }
            client.get(&format!("/indexes/{table}/documents/{key}"))?;
            let mut data = serde_json::Map::new();
            data.insert(pk, value.clone());
            data.insert(
                col.into(),
                params.get("new_val").cloned().unwrap_or(Value::Null),
            );
            client.wait_task(client.put(&format!("/indexes/{table}/documents"), &json!([data]))?)
        })(),
    )
}

pub fn delete_record(id: Value, params: &Value) -> Value {
    respond(
        id,
        (|| {
            let (client, table, pk) = context(params)?;
            let key = document_id(primary_value(params, &pk)?)?;
            client.get(&format!("/indexes/{table}/documents/{key}"))?;
            client.wait_task(client.delete(&format!("/indexes/{table}/documents/{key}"))?)
        })(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keys_follow_host_contract_and_reject_paths_and_composites() {
        let p = json!({"pk_map":{"id":42}});
        assert_eq!(primary_value(&p, "id").unwrap(), &json!(42));
        for p in [
            json!({"pk_map":{}}),
            json!({"pk_map":{"id":1,"x":2}}),
            json!({"pk_map":{"other":1}}),
        ] {
            assert!(primary_value(&p, "id").is_err());
        }
        for v in [
            json!(null),
            json!(true),
            json!({}),
            json!("../keys"),
            json!("a/b"),
        ] {
            assert!(document_id(&v).is_err());
        }
    }
}
