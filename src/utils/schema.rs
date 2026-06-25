//! Schema inference for schemaless documents.
//!
//! Meilisearch indexes have no fixed schema, so we derive "columns" from a
//! sample of documents: the ordered union of their keys, with a best-effort
//! Tabularis data type per key.

use std::collections::HashSet;

use serde_json::Value;

/// Map a JSON value to a Tabularis `data_type` (must match the names declared
/// in `.tabularium`). Objects/arrays become `JSON` so the grid renders them
/// with the JSON editor.
pub fn infer_type(value: &Value) -> &'static str {
    match value {
        Value::Bool(_) => "BOOLEAN",
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                "INTEGER"
            } else {
                "DOUBLE"
            }
        }
        Value::Array(_) | Value::Object(_) => "JSON",
        // Null and string both fall back to TEXT.
        _ => "TEXT",
    }
}

/// Ordered union of object keys across the sampled documents (first-seen
/// order is stable, which keeps column order predictable in the grid).
pub fn union_columns(docs: &[Value]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut cols = Vec::new();
    for doc in docs {
        if let Some(obj) = doc.as_object() {
            for key in obj.keys() {
                if seen.insert(key.clone()) {
                    cols.push(key.clone());
                }
            }
        }
    }
    cols
}

/// Infer a column's type from the first non-null sampled value; `TEXT` if the
/// column is absent or always null across the sample.
pub fn infer_column_type(docs: &[Value], column: &str) -> &'static str {
    for doc in docs {
        if let Some(value) = doc.get(column) {
            if !value.is_null() {
                return infer_type(value);
            }
        }
    }
    "TEXT"
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn infers_scalar_types() {
        assert_eq!(infer_type(&json!(true)), "BOOLEAN");
        assert_eq!(infer_type(&json!(42)), "INTEGER");
        assert_eq!(infer_type(&json!(2.5)), "DOUBLE");
        assert_eq!(infer_type(&json!("hi")), "TEXT");
        assert_eq!(infer_type(&json!(null)), "TEXT");
        assert_eq!(infer_type(&json!([1, 2])), "JSON");
        assert_eq!(infer_type(&json!({ "a": 1 })), "JSON");
    }

    #[test]
    fn unions_keys_in_first_seen_order() {
        let docs = vec![
            json!({ "id": 1, "title": "a" }),
            json!({ "id": 2, "year": 2020 }),
        ];
        assert_eq!(union_columns(&docs), vec!["id", "title", "year"]);
    }

    #[test]
    fn column_type_skips_nulls() {
        let docs = vec![json!({ "x": null }), json!({ "x": 7 })];
        assert_eq!(infer_column_type(&docs, "x"), "INTEGER");
        assert_eq!(infer_column_type(&docs, "missing"), "TEXT");
    }
}
