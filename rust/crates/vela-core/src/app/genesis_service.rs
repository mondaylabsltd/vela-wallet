//! The multisig service — the Safe Genesis Registry's HTTP API, where a
//! multisig's genesis is recorded and its proposals are shared — as a person
//! may set it in Settings (a developer's own, on this computer or elsewhere).
//!
//! Two rules, both pure; the shell fetches and stores:
//!
//! - [`service_address`]: what may be saved as the service's address. The
//!   rule every address typed into Settings follows
//!   ([`is_allowed_endpoint_url`]: `https://`, or plain `http://` to this
//!   computer), trimmed of spaces and trailing slashes — the shell appends
//!   `/api/…` — and with no query or fragment, which would swallow that path.
//! - [`service_answer`]: whether a `GET /api/health` answer is the service's
//!   (`service: "safe-genesis-registry"`, both shells — genesis-cf and the
//!   docker server), and the chains it serves when it lists them.
//!
//! Nothing the service answers is trusted beyond "it is there": every record
//! and approval it hands out is re-derived and re-verified by
//! [`crate::multisig`]. The chain table only narrows which networks a new
//! multisig is offered on — a chain the service does not serve is one its
//! record cannot be written on.

use serde::Serialize;

use super::network_admin::is_allowed_endpoint_url;

/// The `service` a Safe Genesis Registry reports at `/api/health`.
pub const GENESIS_SERVICE_IDENTITY: &str = "safe-genesis-registry";

/// What a typed multisig service address is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ServiceAddress {
    /// It may be saved, as `url` (trimmed, no trailing slash).
    Ok { url: String },
    /// Plain `http://` to another computer: anyone on the way could answer.
    Insecure,
    /// Not an address the wallet fetches at all.
    Invalid,
}

/// The rule for a multisig service address (see the module note).
#[must_use]
pub fn service_address(input: &str) -> ServiceAddress {
    let url = input.trim().trim_end_matches('/');
    if url.contains(['?', '#']) {
        return ServiceAddress::Invalid;
    }
    if is_allowed_endpoint_url(url) {
        return ServiceAddress::Ok { url: url.to_owned() };
    }
    // A real host over plain http: said as insecure, not as nonsense.
    let lower = url.to_ascii_lowercase();
    match lower.strip_prefix("http://") {
        Some(rest) if is_allowed_endpoint_url(&format!("https://{rest}")) => {
            ServiceAddress::Insecure
        }
        _ => ServiceAddress::Invalid,
    }
}

/// One chain a service says it serves.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceChain {
    pub chain_id: u64,
    /// The service's own name for it; the wallet names its networks itself.
    pub name: Option<String>,
}

/// What a `GET {service}/api/health` answer says about the service.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ServiceAnswer {
    /// A multisig service.
    Genesis {
        /// The chains it records on and reads; `None` when it lists none
        /// readable (nothing is then narrowed).
        chains: Option<Vec<ServiceChain>>,
        /// The chain its registry lives on, when it says.
        home_chain_id: Option<u64>,
    },
    /// Something answered, but not as a multisig service.
    Other,
    /// A gateway's error (5xx): the service itself did not answer.
    Unreachable,
}

/// Read a health answer: its HTTP status and its body as text.
#[must_use]
pub fn service_answer(status: u16, body: &str) -> ServiceAnswer {
    if status >= 500 {
        return ServiceAnswer::Unreachable;
    }
    if status != 200 {
        return ServiceAnswer::Other;
    }
    let Ok(serde_json::Value::Object(json)) = serde_json::from_str::<serde_json::Value>(body)
    else {
        return ServiceAnswer::Other;
    };
    if json.get("service").and_then(|s| s.as_str()) != Some(GENESIS_SERVICE_IDENTITY) {
        return ServiceAnswer::Other;
    }
    let chains = json.get("chains").and_then(|c| c.as_array()).map(|list| {
        let mut out: Vec<ServiceChain> = Vec::new();
        for entry in list {
            let Some(chain_id) = entry.get("chainId").and_then(serde_json::Value::as_u64) else {
                continue;
            };
            if chain_id == 0 || out.iter().any(|c| c.chain_id == chain_id) {
                continue;
            }
            out.push(ServiceChain {
                chain_id,
                name: entry
                    .get("name")
                    .and_then(|n| n.as_str())
                    .map(str::to_owned),
            });
        }
        out
    });
    ServiceAnswer::Genesis {
        chains,
        home_chain_id: json
            .get("homeChainId")
            .and_then(serde_json::Value::as_u64)
            .filter(|id| *id > 0),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn ok(url: &str) -> ServiceAddress {
        ServiceAddress::Ok { url: url.into() }
    }

    #[test]
    fn an_address_is_https_or_this_computer() {
        assert_eq!(
            service_address(" https://safe-genesis.getvela.app/ "),
            ok("https://safe-genesis.getvela.app")
        );
        assert_eq!(service_address("http://127.0.0.1:8788"), ok("http://127.0.0.1:8788"));
        assert_eq!(service_address("http://localhost:8787//"), ok("http://localhost:8787"));
        assert_eq!(
            service_address("https://example.com/genesis"),
            ok("https://example.com/genesis")
        );
        assert_eq!(service_address("http://192.168.1.5:8788"), ServiceAddress::Insecure);
        assert_eq!(service_address("http://example.com"), ServiceAddress::Insecure);
        // A familiar prefix on a remote host is not this computer.
        assert_eq!(service_address("http://127.0.0.1.evil.com"), ServiceAddress::Insecure);
        for bad in [
            "",
            "   ",
            "safe-genesis.getvela.app",
            "ftp://example.com",
            "javascript:alert(1)",
            "https://",
            "http://",
            "https://example.com?x=1",
            "https://example.com/#a",
            "https://exa mple.com",
            "https://exa\nmple.com",
        ] {
            assert_eq!(service_address(bad), ServiceAddress::Invalid, "{bad:?}");
        }
    }

    #[test]
    fn a_health_answer_names_the_service_and_its_chains() {
        let body = r#"{"service":"safe-genesis-registry","shell":"docker","homeChainId":5042002,
            "chains":[{"chainId":5042002,"name":"Arc Testnet"},{"chainId":100},{"chainId":"x"},
            {"chainId":0},{"chainId":5042002,"name":"again"}]}"#;
        assert_eq!(
            service_answer(200, body),
            ServiceAnswer::Genesis {
                chains: Some(vec![
                    ServiceChain { chain_id: 5_042_002, name: Some("Arc Testnet".into()) },
                    ServiceChain { chain_id: 100, name: None },
                ]),
                home_chain_id: Some(5_042_002),
            }
        );
        // No chain table: nothing to narrow by.
        assert_eq!(
            service_answer(200, r#"{"service":"safe-genesis-registry"}"#),
            ServiceAnswer::Genesis { chains: None, home_chain_id: None }
        );
    }

    #[test]
    fn anything_else_is_not_the_service() {
        assert_eq!(service_answer(200, r#"{"service":"vela-relay"}"#), ServiceAnswer::Other);
        assert_eq!(service_answer(200, "<html>hello</html>"), ServiceAnswer::Other);
        assert_eq!(service_answer(200, "[]"), ServiceAnswer::Other);
        assert_eq!(service_answer(404, r#"{"error":"not found"}"#), ServiceAnswer::Other);
        assert_eq!(
            service_answer(301, r#"{"service":"safe-genesis-registry"}"#),
            ServiceAnswer::Other
        );
        assert_eq!(service_answer(502, "Bad gateway"), ServiceAnswer::Unreachable);
        assert_eq!(
            service_answer(503, r#"{"service":"safe-genesis-registry"}"#),
            ServiceAnswer::Unreachable
        );
    }

    #[test]
    fn answers_serialize_as_the_shell_reads_them() {
        let json = serde_json::to_value(service_answer(
            200,
            r#"{"service":"safe-genesis-registry","homeChainId":100,"chains":[{"chainId":100,"name":"Gnosis"}]}"#,
        ))
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({"kind":"genesis","chains":[{"chainId":100,"name":"Gnosis"}],"homeChainId":100})
        );
        assert_eq!(
            serde_json::to_value(service_address("http://example.com")).unwrap(),
            serde_json::json!({"kind":"insecure"})
        );
        assert_eq!(
            serde_json::to_value(service_address("https://a.example")).unwrap(),
            serde_json::json!({"kind":"ok","url":"https://a.example"})
        );
    }
}
