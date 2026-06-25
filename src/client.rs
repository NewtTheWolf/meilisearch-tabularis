//! Meilisearch HTTP client.
//!
//! Meilisearch is a plain REST API, so this is a thin blocking wrapper over
//! `ureq`: build a base URL + optional API key from the connection params,
//! then GET/POST/PUT/DELETE JSON. Errors map to `PluginError` with the HTTP
//! status and response body so they surface usefully in the Tabularis log.

use std::time::Duration;

use serde_json::Value;

use crate::error::PluginError;
use crate::models::ConnectionParams;

pub struct Client {
    base: String,
    api_key: Option<String>,
    agent: ureq::Agent,
}

impl Client {
    /// Build a client from the host's connection params. Never opens a socket
    /// here — the first request does that.
    pub fn from_params(params: &ConnectionParams) -> Result<Self, PluginError> {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(20))
            .build();
        Ok(Self {
            base: base_url(params),
            // Treat the password field as the Meilisearch API/master key.
            api_key: params.password.clone().filter(|s| !s.is_empty()),
            agent,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    fn auth(&self, req: ureq::Request) -> ureq::Request {
        match &self.api_key {
            Some(key) => req.set("Authorization", &format!("Bearer {key}")),
            None => req,
        }
    }

    pub fn get(&self, path: &str) -> Result<Value, PluginError> {
        send(self.auth(self.agent.get(&self.url(path))).call())
    }

    pub fn post(&self, path: &str, body: &Value) -> Result<Value, PluginError> {
        let req = self
            .auth(self.agent.post(&self.url(path)))
            .set("Content-Type", "application/json");
        send(req.send_string(&body.to_string()))
    }

    pub fn put(&self, path: &str, body: &Value) -> Result<Value, PluginError> {
        let req = self
            .auth(self.agent.put(&self.url(path)))
            .set("Content-Type", "application/json");
        send(req.send_string(&body.to_string()))
    }

    pub fn delete(&self, path: &str) -> Result<Value, PluginError> {
        send(self.auth(self.agent.delete(&self.url(path))).call())
    }

    /// `GET /health` — cheap connectivity probe for `test_connection`/`ping`.
    pub fn health(&self) -> Result<(), PluginError> {
        self.get("/health").map(|_| ())
    }
}

fn send(result: Result<ureq::Response, ureq::Error>) -> Result<Value, PluginError> {
    match result {
        Ok(resp) => {
            let text = resp
                .into_string()
                .map_err(|e| PluginError::internal(format!("read response body: {e}")))?;
            if text.trim().is_empty() {
                return Ok(Value::Null);
            }
            serde_json::from_str(&text)
                .map_err(|e| PluginError::internal(format!("parse response JSON: {e}")))
        }
        Err(ureq::Error::Status(code, resp)) => {
            let body = resp.into_string().unwrap_or_default();
            Err(PluginError {
                code: -32000,
                message: format!("Meilisearch returned HTTP {code}: {body}"),
            })
        }
        Err(ureq::Error::Transport(t)) => {
            Err(PluginError::internal(format!("connection failed: {t}")))
        }
    }
}

/// Derive the Meilisearch base URL from connection params.
///
/// A full URL in the host field (`http://…`/`https://…`) is used verbatim;
/// otherwise scheme is chosen from the SSL mode and the port is appended.
fn base_url(params: &ConnectionParams) -> String {
    let host = params
        .host
        .clone()
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let host = host.trim().trim_end_matches('/');

    if host.starts_with("http://") || host.starts_with("https://") {
        return host.to_string();
    }

    let scheme = match params.ssl_mode.as_deref() {
        Some(m) if !m.is_empty() && m != "disable" && m != "disabled" => "https",
        _ => "http",
    };
    let port = params.port.unwrap_or(7700);
    format!("{scheme}://{host}:{port}")
}

#[cfg(test)]
mod tests {
    use super::base_url;
    use crate::models::ConnectionParams;

    fn params(host: Option<&str>, port: Option<u16>, ssl: Option<&str>) -> ConnectionParams {
        ConnectionParams {
            driver: None,
            host: host.map(str::to_string),
            port,
            database: None,
            username: None,
            password: None,
            ssl_mode: ssl.map(str::to_string),
        }
    }

    #[test]
    fn defaults_to_local_http() {
        assert_eq!(base_url(&params(None, None, None)), "http://127.0.0.1:7700");
    }

    #[test]
    fn uses_https_when_ssl_enabled() {
        assert_eq!(
            base_url(&params(
                Some("search.example.com"),
                Some(443),
                Some("require")
            )),
            "https://search.example.com:443"
        );
    }

    #[test]
    fn full_url_in_host_is_verbatim() {
        assert_eq!(
            base_url(&params(Some("https://m.example.com/"), Some(7700), None)),
            "https://m.example.com"
        );
    }
}
