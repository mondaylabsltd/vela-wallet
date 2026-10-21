//! Settings → AI agent: whether this wallet answers a remote AI agent through
//! a ClientMCP bridge, and which bridge (spec 103 D1). Off unless the person
//! turns it on; an agent can never change it.
//!
//! Stored as the web stores it — `vela.agent`, `{ enabled, url, token }` — in
//! `wallet.json`, beside the RPC providers' keys. Erase This Device sweeps the
//! `vela.` namespace, so the token goes with everything else.

use serde_json::{Value, json};

use crate::executor::storage;

pub const KEY_AGENT: &str = "vela.agent";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgentConfig {
    pub enabled: bool,
    /// The bridge's client endpoint, e.g. `wss://bridge.example.com/connect`.
    pub url: String,
    /// A ClientMCP client token for this wallet.
    pub token: String,
}

/// A change to the settings: what is `None` stays as it is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgentChange {
    pub enabled: Option<bool>,
    pub url: Option<String>,
    pub token: Option<String>,
}

impl AgentConfig {
    /// What is stored; anything unreadable reads as off.
    pub fn load() -> AgentConfig {
        storage::read_value(KEY_AGENT)
            .ok()
            .flatten()
            .map(|value| Self::decode(&value))
            .unwrap_or_default()
    }

    fn decode(value: &Value) -> AgentConfig {
        let text = |key: &str| {
            value
                .get(key)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        AgentConfig {
            enabled: value.get("enabled").and_then(Value::as_bool) == Some(true),
            url: text("url"),
            token: text("token"),
        }
    }

    fn encode(&self) -> Value {
        json!({ "enabled": self.enabled, "url": self.url, "token": self.token })
    }

    pub fn apply(&mut self, change: &AgentChange) {
        if let Some(enabled) = change.enabled {
            self.enabled = enabled;
        }
        if let Some(url) = &change.url {
            self.url = url.trim().to_owned();
        }
        if let Some(token) = &change.token {
            self.token = token.trim().to_owned();
        }
    }

    /// Writes it. A storage failure keeps the choice for this run.
    pub fn save(&self) {
        if let Err(error) = storage::write_value(KEY_AGENT, self.encode()) {
            crate::diag::vlog!("agent", "settings not saved: {error:?}");
        }
    }

    /// Whether the bridge can be dialled with what is entered.
    pub fn ready(&self) -> bool {
        self.enabled && bridge_url_problem(&self.url).is_none() && !self.token.trim().is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UrlProblem {
    Invalid,
    /// `ws:` to another machine: the token would cross the network in the clear.
    Insecure,
}

/// Why a bridge URL cannot be used, or `None`. Encrypted (`wss:`) everywhere;
/// plain `ws:` only to this machine — the exception the service endpoints make
/// for a developer's own server.
pub fn bridge_url_problem(url: &str) -> Option<UrlProblem> {
    let Ok(parsed) = url::Url::parse(url.trim()) else {
        return Some(UrlProblem::Invalid);
    };
    match parsed.scheme() {
        "wss" if parsed.host().is_some() => None,
        "ws" if matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")) => None,
        "ws" => Some(UrlProblem::Insecure),
        _ => Some(UrlProblem::Invalid),
    }
}

/// The MCP endpoint an agent connects to, derived from the bridge's client
/// endpoint (`wss://host/connect` → `https://host/mcp`), or `None`.
pub fn mcp_endpoint(url: &str) -> Option<String> {
    if bridge_url_problem(url).is_some() {
        return None;
    }
    let mut parsed = url::Url::parse(url.trim()).ok()?;
    let secure = parsed.scheme() == "wss";
    parsed
        .set_scheme(if secure { "https" } else { "http" })
        .ok()?;
    let path = parsed.path().trim_end_matches('/');
    let base = path.strip_suffix("/connect").unwrap_or(path).to_owned();
    parsed.set_path(&format!("{base}/mcp"));
    parsed.set_query(None);
    Some(parsed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bridge_is_encrypted_unless_it_is_this_machine() {
        assert_eq!(
            bridge_url_problem("wss://bridge.clientmcp.dev/connect"),
            None
        );
        assert_eq!(bridge_url_problem("ws://127.0.0.1:8080/connect"), None);
        assert_eq!(bridge_url_problem("ws://localhost:8080/connect"), None);
        assert_eq!(
            bridge_url_problem("ws://192.168.1.4:8080/connect"),
            Some(UrlProblem::Insecure)
        );
        assert_eq!(
            bridge_url_problem("https://bridge.clientmcp.dev"),
            Some(UrlProblem::Invalid)
        );
        assert_eq!(bridge_url_problem("not a url"), Some(UrlProblem::Invalid));
    }

    #[test]
    fn the_agents_endpoint_is_derived_from_the_bridges() {
        assert_eq!(
            mcp_endpoint("wss://bridge.clientmcp.dev/connect").as_deref(),
            Some("https://bridge.clientmcp.dev/mcp")
        );
        assert_eq!(
            mcp_endpoint("ws://127.0.0.1:8080/connect?x=1").as_deref(),
            Some("http://127.0.0.1:8080/mcp")
        );
        assert_eq!(
            mcp_endpoint("wss://example.com/").as_deref(),
            Some("https://example.com/mcp")
        );
        assert_eq!(mcp_endpoint("ws://10.0.0.1/connect"), None);
    }

    #[test]
    fn ready_needs_the_switch_a_usable_bridge_and_a_token() {
        let mut config = AgentConfig::default();
        config.apply(&AgentChange {
            enabled: Some(true),
            url: Some("  wss://bridge.clientmcp.dev/connect ".into()),
            token: None,
        });
        assert_eq!(config.url, "wss://bridge.clientmcp.dev/connect");
        assert!(!config.ready(), "no token");
        config.apply(&AgentChange {
            token: Some(" t ".into()),
            ..AgentChange::default()
        });
        assert!(config.ready());
        config.apply(&AgentChange {
            enabled: Some(false),
            ..AgentChange::default()
        });
        assert!(!config.ready());
    }

    #[test]
    fn it_is_stored_as_the_web_stores_it() {
        storage::tests::with_temp_state("agent-settings", || {
            assert_eq!(AgentConfig::load(), AgentConfig::default());
            let config = AgentConfig {
                enabled: true,
                url: "wss://b/connect".into(),
                token: "t".into(),
            };
            config.save();
            assert_eq!(
                storage::read_value(KEY_AGENT).unwrap(),
                Some(json!({ "enabled": true, "url": "wss://b/connect", "token": "t" }))
            );
            assert_eq!(AgentConfig::load(), config);
            storage::write_value(KEY_AGENT, json!("garbage")).unwrap();
            assert_eq!(AgentConfig::load(), AgentConfig::default());
        });
    }
}
