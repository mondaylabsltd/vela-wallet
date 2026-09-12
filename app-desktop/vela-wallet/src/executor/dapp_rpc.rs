//! Who answers a request `dapp_permissions` forwarded.
//!
//! The core routes on PERMISSION — connected or not, this frame or another,
//! secure origin or not — and then forwards everything it does not answer
//! itself. That "everything" is three different things: a signature, a fact
//! about the wallet (which chain, which network id), and a read of the chain.
//! Deciding between them is a shell job in every client: the extension's
//! service worker does it, and this is the same table.
//!
//! **Ported from** `app-web/vela-wallet/extension/lib/protocol.js`
//! (`classifyMethod`, `READ_ONLY_RPC_METHODS`, `BUNDLER_METHODS`,
//! `READ_PROXY_METHODS`) as of `6965af54`. That file is compiled into this
//! binary already — the page's own provider is built from it — so
//! [`tests::the_read_allowlist_matches_the_javascript`] parses it and fails if
//! the two ever disagree.
//!
//! ## Why an allowlist
//!
//! The JS says it in its own words and it is worth repeating where the list
//! is duplicated: routing by denylist fails OPEN. `eth_signTransaction` is not
//! caught by any "is this a signing method" test, so a catch-all read bucket
//! would hand it to a public node — and a wallet that proxies arbitrary
//! methods for any site it renders is an open RPC relay wearing a wallet's
//! name.

/// The reads the app itself advertises (`READ_ONLY_RPC_METHODS`).
const READ_ONLY_RPC_METHODS: &[&str] = &[
    "eth_call",
    "eth_estimateGas",
    "eth_getBalance",
    "eth_getCode",
    "eth_getStorageAt",
    "eth_getTransactionCount",
    "eth_getTransactionByHash",
    "eth_getTransactionReceipt",
    "eth_getLogs",
    "eth_blockNumber",
    "eth_getBlockByNumber",
    "eth_getBlockByHash",
    "eth_feeHistory",
    "eth_gasPrice",
    "eth_maxPriorityFeePerGas",
    "eth_newFilter",
    "eth_newBlockFilter",
    "eth_getFilterChanges",
    "eth_uninstallFilter",
    "eth_sendRawTransaction",
    "eth_syncing",
];

/// The 4337 reads (`BUNDLER_METHODS`). Routed to the bundler, not the node.
const BUNDLER_METHODS: &[&str] = &[
    "eth_sendUserOperation",
    "eth_estimateUserOperationGas",
    "eth_getUserOperationReceipt",
    "eth_getUserOperationByHash",
    "pimlico_getUserOperationGasPrice",
];

/// The rest of `READ_PROXY_METHODS`, beyond the two lists above.
const EXTRA_READ_METHODS: &[&str] = &[
    "eth_getBlockReceipts",
    "eth_getProof",
    "eth_createAccessList",
    "eth_getFilterLogs",
    "eth_getTransactionByBlockHashAndIndex",
    "eth_getTransactionByBlockNumberAndIndex",
    "eth_getBlockTransactionCountByHash",
    "eth_getBlockTransactionCountByNumber",
    "web3_clientVersion",
];

/// Who answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// The signing column's four machines.
    Sign,
    /// A fact about the wallet: `eth_chainId`, `net_version`.
    State,
    /// `wallet_switchEthereumChain`.
    Switch,
    /// Acknowledged without changing anything (`wallet_addEthereumChain`,
    /// `wallet_watchAsset`) — the page is told yes, and the wallet's own
    /// settings remain the only place a network or a token is added. Saying
    /// "no" would break sites that switch after adding; saying "yes" and
    /// changing nothing is what the extension ships.
    Ack,
    /// A node or bundler read, on the wallet's current chain.
    Read {
        /// `true` for the 4337 methods, which the pool routes to the bundler.
        bundler: bool,
    },
    /// Refused. Every method outside the allowlist lands here.
    Unsupported,
}

/// `classifyMethod`, with `eth_accounts` / `wallet_getPermissions` /
/// `eth_requestAccounts` absent on purpose: the core answers those, and this
/// is only ever asked about what the core forwarded.
#[must_use]
pub fn classify(method: &str) -> Route {
    // Refused outright, before the signing test that would otherwise catch
    // it: `eth_sign` puts an opaque digest in front of somebody, and the JS
    // refuses it as policy rather than as a missing feature.
    if method == "eth_sign" {
        return Route::Unsupported;
    }
    if vela_core::app::sign_request::is_signing_method(method) {
        return Route::Sign;
    }
    if method == "eth_chainId" || method == "net_version" {
        return Route::State;
    }
    if method == "wallet_switchEthereumChain" {
        return Route::Switch;
    }
    if method == "wallet_addEthereumChain" || method == "wallet_watchAsset" {
        return Route::Ack;
    }
    if BUNDLER_METHODS.contains(&method) {
        return Route::Read { bundler: true };
    }
    if READ_ONLY_RPC_METHODS.contains(&method) || EXTRA_READ_METHODS.contains(&method) {
        return Route::Read { bundler: false };
    }
    Route::Unsupported
}

/// The chain a `wallet_switchEthereumChain` names (`switchChainParam`).
///
/// `[{ chainId: "0x…" }]` per EIP-3326. `None` when the payload names none,
/// which is a malformed request rather than a request for chain zero.
#[must_use]
pub fn switch_chain_param(params_json: &str) -> Option<u32> {
    let params: serde_json::Value = serde_json::from_str(params_json).ok()?;
    let named = params.get(0)?.get("chainId")?;
    let text = named.as_str()?;
    let digits = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X"));
    match digits {
        Some(hex) => u32::from_str_radix(hex, 16).ok(),
        None => text.parse().ok(),
    }
}

/// EIP-1193 §12.4's minimal lowercase hex — `1` becomes `"0x1"`, never
/// `"0x01"`, because a padded chain id is a string a dApp's `===` will miss.
#[must_use]
pub fn hex_chain_id(chain_id: u32) -> String {
    format!("0x{chain_id:x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The allowlist in this file IS the allowlist in the JavaScript.
    ///
    /// Both are compiled into this binary — the page's provider is built from
    /// that same file — so the duplication can be checked rather than
    /// trusted. A method added to the extension's read proxy and not here
    /// would be refused on the desktop for no stated reason; one added here
    /// and not there would be a hole this project never opened on purpose.
    #[test]
    fn the_read_allowlist_matches_the_javascript() {
        let js = crate::webview::protocol_js();
        let mine: std::collections::BTreeSet<&str> = READ_ONLY_RPC_METHODS
            .iter()
            .chain(BUNDLER_METHODS)
            .chain(EXTRA_READ_METHODS)
            .copied()
            .collect();

        // Every name the JS lists in the three sources of READ_PROXY_METHODS.
        let mut theirs = std::collections::BTreeSet::new();
        for block in [
            "READ_ONLY_RPC_METHODS",
            "BUNDLER_METHODS",
            "READ_PROXY_METHODS",
        ] {
            let start = js
                .find(&format!("export const {block}"))
                .unwrap_or_else(|| unreachable!("{block} is gone from protocol.js"));
            let body = &js[start..];
            let end = body
                .find("]);")
                .or_else(|| body.find("];"))
                .unwrap_or_else(|| unreachable!("{block} has no end"));
            for quoted in body[..end].split('\'').skip(1).step_by(2) {
                theirs.insert(quoted);
            }
        }

        let missing: Vec<&&str> = theirs.iter().filter(|m| !mine.contains(*m)).collect();
        let extra: Vec<&&str> = mine.iter().filter(|m| !theirs.contains(*m)).collect();
        assert!(
            missing.is_empty() && extra.is_empty(),
            "the desktop's read allowlist drifted from protocol.js — missing {missing:?}, extra {extra:?}"
        );
    }

    /// The two refusals that must never become a read.
    #[test]
    fn the_dangerous_methods_are_not_proxied() {
        // Not caught by any "is this signing" test, and the reason the list
        // is an allowlist: proxied, it would let a site broadcast a signed
        // transaction through the wallet's own node budget.
        assert_eq!(classify("eth_signTransaction"), Route::Unsupported);
        // Policy, not omission.
        assert_eq!(classify("eth_sign"), Route::Unsupported);
        // An invented method is refused rather than forwarded.
        assert_eq!(classify("vela_stealEverything"), Route::Unsupported);
    }

    #[test]
    fn the_buckets_route_where_the_javascript_routes() {
        assert_eq!(classify("eth_sendTransaction"), Route::Sign);
        assert_eq!(classify("personal_sign"), Route::Sign);
        assert_eq!(classify("eth_signTypedData_v4"), Route::Sign);
        assert_eq!(classify("eth_chainId"), Route::State);
        assert_eq!(classify("net_version"), Route::State);
        assert_eq!(classify("wallet_switchEthereumChain"), Route::Switch);
        assert_eq!(classify("wallet_addEthereumChain"), Route::Ack);
        assert_eq!(classify("eth_getBalance"), Route::Read { bundler: false });
        assert_eq!(
            classify("eth_sendUserOperation"),
            Route::Read { bundler: true }
        );
    }

    #[test]
    fn a_switch_names_its_chain_in_either_notation() {
        assert_eq!(switch_chain_param(r#"[{"chainId":"0x64"}]"#), Some(100));
        assert_eq!(switch_chain_param(r#"[{"chainId":"100"}]"#), Some(100));
        // Malformed is None, never zero: chain zero is a chain, and answering
        // as though a page asked for one is how a switch lands somewhere
        // nobody named.
        assert_eq!(switch_chain_param("[]"), None);
        assert_eq!(switch_chain_param(r#"[{}]"#), None);
        assert_eq!(switch_chain_param("not json"), None);
    }

    #[test]
    fn a_chain_id_is_minimal_hex() {
        assert_eq!(hex_chain_id(1), "0x1");
        assert_eq!(hex_chain_id(100), "0x64");
        assert_eq!(hex_chain_id(11_155_111), "0xaa36a7");
    }
}
