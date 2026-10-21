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

// The bridge's address rules are every shell's: vela-agent's.
pub use vela_agent::bridge_url::{bridge_url_problem, mcp_endpoint};

#[cfg(test)]
mod tests {
    use super::*;

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
