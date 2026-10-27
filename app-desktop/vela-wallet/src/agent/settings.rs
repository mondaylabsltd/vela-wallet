//! Settings → AI agent: whether this wallet answers a remote AI agent through
//! a ClientMCP bridge, and which bridge (spec 103 D1). Off unless the person
//! turns it on; an agent can never change it.
//!
//! Stored as the web stores it — `vela.agent`, `{ enabled, url, token,
//! linked }` — in
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
    /// Spec 106: whose the token is, when clientmcp.dev issued it.
    pub linked: Option<AgentLinked>,
    /// Spec 106: the MCP server address clientmcp.dev gave for the person's
    /// AI agent, its token in the path — a credential, kept as the token is.
    /// Empty unless `linked`.
    pub agent_url: String,
    /// Spec 107 E8: the wallets the agent may use; none unless the person
    /// says so.
    pub wallets: Vec<String>,
}

/// Who clientmcp.dev issued the token to — not a secret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentLinked {
    pub site: String,
    pub login: String,
    pub client_id: String,
}

/// A change to the settings: what is `None` stays as it is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgentChange {
    pub enabled: Option<bool>,
    pub url: Option<String>,
    pub token: Option<String>,
    pub wallets: Option<Vec<String>>,
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
        let linked = value.get("linked").and_then(|linked| {
            let text = |key: &str| linked.get(key).and_then(Value::as_str).map(str::to_owned);
            Some(AgentLinked {
                site: text("site")?,
                login: text("login")?,
                client_id: text("clientId")?,
            })
        });
        AgentConfig {
            enabled: value.get("enabled").and_then(Value::as_bool) == Some(true),
            url: text("url"),
            token: text("token"),
            agent_url: if linked.is_some() {
                text("agentUrl")
            } else {
                String::new()
            },
            linked,
            wallets: value
                .get("wallets")
                .and_then(Value::as_array)
                .map(|list| {
                    list.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    fn encode(&self) -> Value {
        let linked = self.linked.as_ref().map(|linked| {
            json!({ "site": linked.site, "login": linked.login, "clientId": linked.client_id })
        });
        json!({
            "enabled": self.enabled,
            "url": self.url,
            "token": self.token,
            "linked": linked,
            "agentUrl": self.agent_url,
            "wallets": self.wallets,
        })
    }

    pub fn apply(&mut self, change: &AgentChange) {
        if let Some(enabled) = change.enabled {
            self.enabled = enabled;
        }
        if let Some(url) = &change.url {
            self.url = url.trim().to_owned();
        }
        if let Some(wallets) = &change.wallets {
            self.wallets = wallets.iter().map(|w| w.trim().to_owned()).collect();
        }
        if let Some(token) = &change.token {
            self.token = token.trim().to_owned();
            // A token from anywhere else is no longer clientmcp.dev's, nor
            // is the agent's address that came with it.
            self.linked = None;
            self.agent_url.clear();
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
            wallets: None,
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
            let mut config = AgentConfig {
                enabled: true,
                url: "wss://b/connect".into(),
                token: "t".into(),
                linked: None,
                agent_url: String::new(),
                wallets: Vec::new(),
            };
            config.save();
            assert_eq!(
                storage::read_value(KEY_AGENT).unwrap(),
                Some(
                    json!({ "enabled": true, "url": "wss://b/connect", "token": "t", "linked": null, "agentUrl": "", "wallets": [] })
                )
            );
            assert_eq!(AgentConfig::load(), config);
            config.linked = Some(AgentLinked {
                site: "https://clientmcp.dev".into(),
                login: "alice".into(),
                client_id: "c-1".into(),
            });
            config.agent_url = "https://bridge.clientmcp.dev/mcp/a".into();
            config.save();
            assert_eq!(
                storage::read_value(KEY_AGENT).unwrap().unwrap()["linked"],
                json!({ "site": "https://clientmcp.dev", "login": "alice", "clientId": "c-1" })
            );
            assert_eq!(AgentConfig::load(), config);
            // A token typed by hand is no longer clientmcp.dev's.
            config.apply(&AgentChange {
                token: Some("mine".into()),
                ..AgentChange::default()
            });
            assert_eq!(config.linked, None);
            assert_eq!(config.agent_url, "");
            storage::write_value(KEY_AGENT, json!("garbage")).unwrap();
            assert_eq!(AgentConfig::load(), AgentConfig::default());
        });
    }
}
