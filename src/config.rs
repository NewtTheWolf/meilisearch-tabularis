//! Plugin settings received via the `initialize` RPC call.
//!
//! Tabularis sends the user-configured settings (declared in `.tabularium`)
//! once at startup. We stash them in a process-global so handlers can read
//! them without threading config through every call.

use std::sync::RwLock;

use serde_json::Value;

static SETTINGS: RwLock<Option<Value>> = RwLock::new(None);

const DEFAULT_SAMPLE_SIZE: usize = 20;

/// Store the `settings` object from an `initialize` request.
pub fn store_from_initialize(params: &Value) {
    if let Some(settings) = params.get("settings") {
        if let Ok(mut guard) = SETTINGS.write() {
            *guard = Some(settings.clone());
        }
    }
}

/// How many documents to sample per index when inferring columns.
pub fn sample_size() -> usize {
    let Ok(guard) = SETTINGS.read() else {
        return DEFAULT_SAMPLE_SIZE;
    };
    guard
        .as_ref()
        .and_then(|v| v.get("sample_size"))
        .and_then(Value::as_u64)
        .filter(|n| *n > 0)
        .map(|n| n.min(1000) as usize)
        .unwrap_or(DEFAULT_SAMPLE_SIZE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Single test: `SETTINGS` is a process-global, so split tests would race
    // under parallel execution.
    #[test]
    fn sample_size_reads_settings_with_fallback() {
        // Fresh process: nothing stored yet.
        assert_eq!(sample_size(), DEFAULT_SAMPLE_SIZE);
        store_from_initialize(&json!({ "settings": { "sample_size": 50 } }));
        assert_eq!(sample_size(), 50);
        // Zero is treated as unset.
        store_from_initialize(&json!({ "settings": { "sample_size": 0 } }));
        assert_eq!(sample_size(), DEFAULT_SAMPLE_SIZE);
    }
}
