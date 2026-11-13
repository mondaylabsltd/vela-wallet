//! The multisig service — the Safe Genesis Registry's HTTP API, where a
//! multisig's genesis is recorded and its proposals are shared. Its address
//! is one of the service endpoints ([`super::network_admin`]'s
//! `GenesisService`: saved, probed and reset as the others are, under the
//! same URL rule).
//!
//! What lives here is how its `GET /api/health` answer reads
//! ([`service_answer`]): the service's when it names itself
//! `safe-genesis-registry` (both shells — genesis-cf and the docker server),
//! with the chains it reads when it lists them (those its registry is on
//! marked `registry`) and its home chain, where it writes records. Its own
//! 503 (`"ok": false`) is still the service, answering: degraded, not away.
//! The shell asks it to probe a service that was away, and to offer a new
//! multisig only the networks the service reads.
//!
//! Nothing the service answers is trusted beyond "it is there": every record
//! and approval it hands out is re-derived and re-verified by
//! [`crate::multisig`].

use serde::Serialize;

/// The `service` a Safe Genesis Registry reports at `/api/health`.
pub const GENESIS_SERVICE_IDENTITY: &str = "safe-genesis-registry";

/// One chain a service says it serves.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceChain {
    pub chain_id: u64,
    /// The service's own name for it; the wallet names its networks itself.
    pub name: Option<String>,
    /// The registry is on this chain and the service reads it there (its
    /// `registry` flag, or its home chain): where a record can be looked up.
    /// The other chains it reads only for deployments and members.
    pub registry: bool,
}

/// What a `GET {service}/api/health` answer says about the service.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ServiceAnswer {
    /// A multisig service.
    Genesis {
        /// The chains it reads; `None` when it lists none readable (nothing
        /// is then narrowed).
        chains: Option<Vec<ServiceChain>>,
        /// The chain its registry lives on, when it says.
        home_chain_id: Option<u64>,
        /// `false` when it says it cannot do all of its job now (`"ok":
        /// false`, with a 503: its home chain's RPCs all failing, or its
        /// record writer stalled). It is THERE — it answered as itself, and
        /// what it reads may still come back — so it is never taken for away.
        ok: bool,
    },
    /// Something answered, but not as a multisig service.
    Other,
    /// A gateway's error (5xx): the service itself did not answer.
    Unreachable,
}

/// Read a health answer: its HTTP status and its body as text.
#[must_use]
pub fn service_answer(status: u16, body: &str) -> ServiceAnswer {
    let json = match serde_json::from_str::<serde_json::Value>(body) {
        Ok(serde_json::Value::Object(json))
            if json.get("service").and_then(|s| s.as_str()) == Some(GENESIS_SERVICE_IDENTITY) =>
        {
            json
        }
        _ if status >= 500 => return ServiceAnswer::Unreachable,
        _ => return ServiceAnswer::Other,
    };
    // A 503 is the service's own "not all is well" only with its own body
    // saying so; any other error with its name on it is no health answer.
    let degraded = status == 503 && json.get("ok") == Some(&serde_json::Value::Bool(false));
    if status != 200 && !degraded {
        return if status >= 500 {
            ServiceAnswer::Unreachable
        } else {
            ServiceAnswer::Other
        };
    }
    let home_chain_id = json
        .get("homeChainId")
        .and_then(serde_json::Value::as_u64)
        .filter(|id| *id > 0);
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
                registry: entry.get("registry").and_then(serde_json::Value::as_bool) == Some(true)
                    || home_chain_id == Some(chain_id),
            });
        }
        out
    });
    ServiceAnswer::Genesis {
        chains,
        home_chain_id,
        ok: !degraded,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn a_health_answer_names_the_service_and_its_chains() {
        let body = r#"{"service":"safe-genesis-registry","shell":"docker","homeChainId":5042002,
            "chains":[{"chainId":5042002,"name":"Arc Testnet"},{"chainId":100},{"chainId":"x"},
            {"chainId":0},{"chainId":5042002,"name":"again"}]}"#;
        assert_eq!(
            service_answer(200, body),
            ServiceAnswer::Genesis {
                chains: Some(vec![
                    ServiceChain {
                        chain_id: 5_042_002,
                        name: Some("Arc Testnet".into()),
                        registry: true
                    },
                    ServiceChain {
                        chain_id: 100,
                        name: None,
                        registry: false
                    },
                ]),
                home_chain_id: Some(5_042_002),
                ok: true,
            }
        );
        // No chain table: nothing to narrow by.
        assert_eq!(
            service_answer(200, r#"{"service":"safe-genesis-registry"}"#),
            ServiceAnswer::Genesis {
                chains: None,
                home_chain_id: None,
                ok: true
            }
        );
    }

    /// The v4 service's table: every chain it reads, the registry's marked.
    #[test]
    fn the_registry_chains_are_marked() {
        let body = r#"{"service":"safe-genesis-registry","ok":true,"homeChainId":100,"chains":[
            {"chainId":100,"name":"Gnosis","registry":true},{"chainId":1,"name":"Ethereum","registry":true},
            {"chainId":8453,"name":"Base","registry":false},{"chainId":10,"name":"OP Mainnet"}]}"#;
        let ServiceAnswer::Genesis {
            chains: Some(chains),
            ok: true,
            ..
        } = service_answer(200, body)
        else {
            panic!("a genesis answer");
        };
        let marked: Vec<(u64, bool)> = chains.iter().map(|c| (c.chain_id, c.registry)).collect();
        assert_eq!(
            marked,
            vec![(100, true), (1, true), (8453, false), (10, false)]
        );
    }

    /// Its own 503 (`"ok": false`: the home chain's RPCs failing, the writer
    /// stalled) is the service, there, saying it is not all well — not away.
    #[test]
    fn its_own_503_is_the_service_degraded() {
        let body = r#"{"service":"safe-genesis-registry","ok":false,"homeChainId":100,
            "chains":[{"chainId":100,"name":"Gnosis (fork)","registry":true}]}"#;
        assert_eq!(
            service_answer(503, body),
            ServiceAnswer::Genesis {
                chains: Some(vec![ServiceChain {
                    chain_id: 100,
                    name: Some("Gnosis (fork)".into()),
                    registry: true
                }]),
                home_chain_id: Some(100),
                ok: false,
            }
        );
        // Not its 503 without its own "ok": false.
        assert_eq!(
            service_answer(503, r#"{"service":"safe-genesis-registry"}"#),
            ServiceAnswer::Unreachable
        );
        assert_eq!(
            service_answer(500, r#"{"service":"safe-genesis-registry","ok":false}"#),
            ServiceAnswer::Unreachable
        );
    }

    #[test]
    fn anything_else_is_not_the_service() {
        assert_eq!(
            service_answer(200, r#"{"service":"vela-relay"}"#),
            ServiceAnswer::Other
        );
        assert_eq!(
            service_answer(200, "<html>hello</html>"),
            ServiceAnswer::Other
        );
        assert_eq!(service_answer(200, "[]"), ServiceAnswer::Other);
        assert_eq!(
            service_answer(404, r#"{"error":"not found"}"#),
            ServiceAnswer::Other
        );
        assert_eq!(
            service_answer(301, r#"{"service":"safe-genesis-registry"}"#),
            ServiceAnswer::Other
        );
        assert_eq!(
            service_answer(429, r#"{"service":"safe-genesis-registry","ok":false}"#),
            ServiceAnswer::Other
        );
        assert_eq!(
            service_answer(502, "Bad gateway"),
            ServiceAnswer::Unreachable
        );
        assert_eq!(
            service_answer(503, r#"{"service":"vela-relay","ok":false}"#),
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
            serde_json::json!({"kind":"genesis","chains":[{"chainId":100,"name":"Gnosis","registry":true}],"homeChainId":100,"ok":true})
        );
    }
}
