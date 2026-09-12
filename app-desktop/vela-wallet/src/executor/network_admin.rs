//! The only place the `network_admin` machine touches the outside world.
//!
//! Fifteen operations in three groups: four stored ledgers, six probes, and the
//! two cache invalidations that have nothing to invalidate yet.
//!
//! ## The probes are LIVE here, unlike everything else network-shaped
//!
//! Spec 030's rule (FR-006) is that an operation goes live in this cut when it
//! is its own self-contained HTTP call *and* the core gates a user-visible
//! outcome on the answer. These probes are both, and web's spec 024 came to the
//! same conclusion the same way — it planned them fail-closed and reversed
//! itself during implementation, because `add_confirmed` **hard-gates** on a
//! verified compatibility verdict. A probe-less settings screen is not a
//! degraded settings screen; it is one where 添加网络 can never enable.
//!
//! They are operation-local HTTP, deliberately not a pool. Web's executor says
//! it plainly — "the probes travel WITH the executor" — and on desktop it also
//! keeps spec 031's boundary crisp: the RPC pool arrives as its own executor,
//! and 031 can then prove it touched nothing here.
//!
//! ## Casing is the shell's job, and that is not a nuisance
//!
//! `NetCustomNetwork` and friends are snake_case Rust; the stored records are
//! camelCase with four irregular names (`logoURL`, `rpcURL`, `explorerURL`,
//! `bundlerURL`) and one renamed field (`added_at_iso` ⇄ `addedAt`). The core's
//! own doc assigns the mapping here — "the shell maps field-name casing" —
//! because the *stored* spelling is a cross-client contract with the web and
//! Expo clients, while the Rust spelling is just Rust.

use std::io::Read as _;
use std::time::{Duration, Instant};

use gpui::App;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use vela_core::app::network_admin::{
    DEFAULT_ETHEREUM_DATA_URL, Event, NetChainIndexEntry, NetCustomNetwork, NetHealthBody,
    NetNetworkConfig, NetOperation, NetProviderKeys, NetRawChainData, NetServiceEndpoints,
    NetShellResult, NetStoredEndpoints, NetworkAdmin, P256_PRECOMPILE, VALID_P256_CALL,
};

use crate::executor::{proxy, storage};
use crate::resident::{Answer, Machine};

/// `NET_TIMEOUTS.networkCheck` — the same ten seconds web gives a probe.
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);
/// `NET_TIMEOUTS.ethereumData`.
const DATA_TIMEOUT: Duration = Duration::from_secs(5);

// ---------------------------------------------------------------------------
// Stored shapes — the camelCase side of the contract
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct StoredNetwork {
    id: String,
    display_name: String,
    chain_id: u32,
    icon_label: String,
    icon_color: String,
    icon_bg: String,
    #[serde(rename = "logoURL")]
    logo_url: String,
    #[serde(rename = "isL2")]
    is_l2: bool,
    #[serde(rename = "rpcURL")]
    rpc_url: String,
    #[serde(rename = "explorerURL")]
    explorer_url: String,
    #[serde(rename = "bundlerURL")]
    bundler_url: String,
    native_symbol: String,
    /// Stored as `addedAt`; the core calls it what it is.
    added_at: String,
}

impl From<&NetCustomNetwork> for StoredNetwork {
    fn from(n: &NetCustomNetwork) -> Self {
        Self {
            id: n.id.clone(),
            display_name: n.display_name.clone(),
            chain_id: n.chain_id,
            icon_label: n.icon_label.clone(),
            icon_color: n.icon_color.clone(),
            icon_bg: n.icon_bg.clone(),
            logo_url: n.logo_url.clone(),
            is_l2: n.is_l2,
            rpc_url: n.rpc_url.clone(),
            explorer_url: n.explorer_url.clone(),
            bundler_url: n.bundler_url.clone(),
            native_symbol: n.native_symbol.clone(),
            added_at: n.added_at_iso.clone(),
        }
    }
}

impl From<StoredNetwork> for NetCustomNetwork {
    fn from(s: StoredNetwork) -> Self {
        Self {
            id: s.id,
            display_name: s.display_name,
            chain_id: s.chain_id,
            icon_label: s.icon_label,
            icon_color: s.icon_color,
            icon_bg: s.icon_bg,
            logo_url: s.logo_url,
            is_l2: s.is_l2,
            rpc_url: s.rpc_url,
            explorer_url: s.explorer_url,
            bundler_url: s.bundler_url,
            native_symbol: s.native_symbol,
            added_at_iso: s.added_at,
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct StoredConfig {
    chain_id: u32,
    #[serde(rename = "rpcURL")]
    rpc_url: String,
    #[serde(rename = "explorerURL")]
    explorer_url: String,
    #[serde(rename = "bundlerURL")]
    bundler_url: String,
}

impl From<StoredConfig> for NetNetworkConfig {
    fn from(s: StoredConfig) -> Self {
        Self {
            chain_id: s.chain_id,
            rpc_url: s.rpc_url,
            explorer_url: s.explorer_url,
            bundler_url: s.bundler_url,
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct StoredEndpoints {
    #[serde(rename = "ethereumDataURL", skip_serializing_if = "Option::is_none")]
    ethereum_data_url: Option<String>,
    #[serde(rename = "passkeyIndexURL", skip_serializing_if = "Option::is_none")]
    passkey_index_url: Option<String>,
    #[serde(rename = "bundlerServiceURL", skip_serializing_if = "Option::is_none")]
    bundler_service_url: Option<String>,
    #[serde(rename = "fiatRatesURL", skip_serializing_if = "Option::is_none")]
    fiat_rates_url: Option<String>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct StoredProviderKeys {
    /// A CLEARED key is REMOVED, never stored as `""` or `null` — the core's
    /// invariant ⑦, and the reason these skip when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    alchemy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    drpc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ankr: Option<String>,
}

/// Decode a stored array, dropping records that will not parse.
///
/// A rejected `StoreLoaded` would strand the core unloaded **forever** — every
/// subsequent write is dropped — so one bad record must never fail the load.
///
/// Deviation from web, recorded rather than accidental: web coerces an
/// unparseable record to zeros and keeps it; this drops it. A network whose
/// `chainId` did not parse has chain 0, and chain 0 is the key the core dedups
/// and routes on. Keeping it puts a colliding ghost in the ledger; dropping it
/// loses a record that was already unusable.
fn decode_list<S, T>(key: &str) -> Vec<T>
where
    S: for<'de> Deserialize<'de>,
    T: From<S>,
{
    let Ok(Some(Value::Array(items))) = storage::read_value(key) else {
        return Vec::new();
    };
    items
        .into_iter()
        .filter_map(|item| serde_json::from_value::<S>(item).ok())
        .map(T::from)
        .collect()
}

fn encode_list<T, S: Serialize>(items: &[T], to_stored: impl Fn(&T) -> S) -> Value {
    Value::Array(
        items
            .iter()
            .map(|item| serde_json::to_value(to_stored(item)).unwrap_or(Value::Null))
            .collect(),
    )
}

/// Flatten `/chains/eip155-{id}.json` into the shape the core reads.
///
/// The served document is the community chain-list format: `chainId`,
/// `nativeCurrency` as a nested object, `explorers` as objects carrying a `url`.
/// `NetRawChainData` is flat, so this is the flattening — and it is the SHELL's
/// job by the core's own instruction: "All parsing decisions (defaults, HTTPS
/// filtering, placeholder rejection) happen in the core."
///
/// Deserializing the document straight into `NetRawChainData` looks like it
/// works and silently yields defaults, which is how the first version of this
/// file could not add a network at all while every unit test passed. Only the
/// live test saw it.
fn decode_raw_chain_data(value: &Value) -> Option<NetRawChainData> {
    let object = value.as_object()?;
    let native = object.get("nativeCurrency").and_then(Value::as_object);
    let text = |v: Option<&Value>| {
        v.and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    Some(NetRawChainData {
        chain_id: object
            .get("chainId")
            .and_then(Value::as_u64)
            .and_then(|id| u32::try_from(id).ok()),
        name: text(object.get("name")),
        short_name: text(object.get("shortName")),
        native_currency_name: native.and_then(|n| text(n.get("name"))),
        native_currency_symbol: native.and_then(|n| text(n.get("symbol"))),
        native_currency_decimals: native
            .and_then(|n| n.get("decimals"))
            .and_then(Value::as_u64)
            .and_then(|d| u32::try_from(d).ok()),
        // Unfiltered on purpose: ws://, http:// and placeholder URLs all pass
        // through, because filtering them is the core's invariant ⑧.
        rpc: object
            .get("rpc")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
        // Each explorer's `url`, or an empty string where an entry has none —
        // the position matters, so a missing url is a blank, not a skip.
        explorers: object
            .get("explorers")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .map(|item| {
                        item.get("url")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned()
                    })
                    .collect()
            })
            .unwrap_or_default(),
        testnet: object.get("testnet") == Some(&Value::Bool(true)),
    })
}

/// The search index rows, skipping any without a usable chain id.
fn decode_search_index(value: &Value) -> Vec<NetChainIndexEntry> {
    let Some(rows) = value.as_array() else {
        return Vec::new();
    };
    rows.iter()
        .filter_map(|row| {
            let object = row.as_object()?;
            let chain_id = object
                .get("chainId")
                .and_then(Value::as_u64)
                .and_then(|id| u32::try_from(id).ok())?;
            let text = |key: &str| {
                object
                    .get(key)
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned()
            };
            Some(NetChainIndexEntry {
                chain_id,
                name: text("name"),
                short_name: text("shortName"),
                native_currency_symbol: text("nativeCurrencySymbol"),
                has_logo: object.get("hasLogo") == Some(&Value::Bool(true)),
            })
        })
        .collect()
}

/// The chain ids of every network somebody added.
///
/// Shared with the signing host, which must tell `sign_request` the same list
/// the settings screen shows — a request for a chain the settings say is
/// present, refused as absent, is the wallet disagreeing with itself.
pub fn read_store_custom_chain_ids() -> Vec<u32> {
    decode_list::<StoredNetwork, NetCustomNetwork>(storage::KEY_CUSTOM_NETWORKS)
        .into_iter()
        .map(|network| network.chain_id)
        .collect()
}

fn read_store() -> NetShellResult {
    let endpoints = storage::read_value(storage::KEY_SERVICE_ENDPOINTS)
        .ok()
        .flatten()
        .and_then(|value| serde_json::from_value::<StoredEndpoints>(value).ok())
        .unwrap_or_default();
    let provider_keys = storage::read_value(storage::KEY_RPC_PROVIDERS)
        .ok()
        .flatten()
        .and_then(|value| serde_json::from_value::<StoredProviderKeys>(value).ok())
        .unwrap_or_default();

    NetShellResult::StoreLoaded {
        custom_networks: decode_list::<StoredNetwork, NetCustomNetwork>(
            storage::KEY_CUSTOM_NETWORKS,
        ),
        network_configs: decode_list::<StoredConfig, NetNetworkConfig>(storage::KEY_NETWORK_CONFIG),
        // Absent fields stay absent: the core applies the same defaults merge
        // the services always did, and a shell that filled them in would hide
        // "never configured" behind "configured to the default".
        endpoints: NetStoredEndpoints {
            ethereum_data_url: endpoints.ethereum_data_url,
            passkey_index_url: endpoints.passkey_index_url,
            bundler_service_url: endpoints.bundler_service_url,
            fiat_rates_url: endpoints.fiat_rates_url,
        },
        provider_keys: NetProviderKeys {
            alchemy: provider_keys.alchemy,
            drpc: provider_keys.drpc,
            ankr: provider_keys.ankr,
        },
    }
}

// ---------------------------------------------------------------------------
// HTTP — operation-local, never a pool
// ---------------------------------------------------------------------------

/// Where `/index/fuse-chains.json` and `/chains/eip155-*.json` live: the
/// configured ethereum-data endpoint, or its default.
fn data_base() -> String {
    storage::read_value(storage::KEY_SERVICE_ENDPOINTS)
        .ok()
        .flatten()
        .and_then(|value| serde_json::from_value::<StoredEndpoints>(value).ok())
        .and_then(|stored| stored.ethereum_data_url)
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| DEFAULT_ETHEREUM_DATA_URL.to_owned())
}

fn get_json(url: &str, timeout: Duration) -> Option<Value> {
    let mut response = proxy::agent(timeout).get(url).call().ok()?;
    let mut body = String::new();
    response
        .body_mut()
        .as_reader()
        .read_to_string(&mut body)
        .ok()?;
    serde_json::from_str(&body).ok()
}

/// One JSON-RPC call. `None` covers every failure the core treats alike:
/// unreachable, timed out, HTTP error, unparseable, or an RPC `error` member.
fn json_rpc(url: &str, method: &str, params: Value) -> Option<Value> {
    let payload = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
    let mut response = proxy::agent(PROBE_TIMEOUT)
        .post(url)
        .header("content-type", "application/json")
        .send_json(&payload)
        .ok()?;
    let mut body = String::new();
    response
        .body_mut()
        .as_reader()
        .read_to_string(&mut body)
        .ok()?;
    let parsed: Value = serde_json::from_str(&body).ok()?;
    if parsed.get("error").is_some() {
        return None;
    }
    parsed.get("result").cloned()
}

/// Split a `ureq` failure into the two things the core distinguishes.
///
/// `ureq` 3 treats a non-2xx response as an **`Err`**, not an `Ok` with a
/// status — `registry.rs:112` says the same thing in its own words: "StatusCode
/// is the ONLY variant that means the server answered." Reading the status off
/// an `Ok` response, as the first version of this file did, meant the
/// `HttpError` arm was unreachable and every 4xx/5xx reported as `Failed`. The
/// core shows those differently ("HTTP 502" against "Connection failed"), so
/// the bug would have been a settings screen that says the wrong true thing.
fn http_error(error: ureq::Error) -> NetHealthBody {
    match error {
        ureq::Error::StatusCode(status) => NetHealthBody::HttpError {
            status: u32::from(status),
        },
        _ => NetHealthBody::Failed,
    }
}

/// `/api/health`, with the cache-buster the TS always sent.
fn service_health(base_url: &str) -> (NetHealthBody, f64) {
    let started = Instant::now();
    let url = format!(
        "{}/api/health?_t={}",
        base_url.trim_end_matches('/'),
        started.elapsed().as_nanos()
    );
    let body = match proxy::agent(PROBE_TIMEOUT).get(&url).call() {
        Ok(mut response) => {
            let mut text = String::new();
            match response.body_mut().as_reader().read_to_string(&mut text) {
                Ok(_) => match serde_json::from_str::<Value>(&text) {
                    Ok(value) => NetHealthBody::Identity {
                        service: value
                            .get("service")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                        status: value
                            .get("status")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                    },
                    // Not JSON — the TS lands this in the same `catch` as a
                    // thrown fetch, and so does this.
                    Err(_) => NetHealthBody::Failed,
                },
                Err(_) => NetHealthBody::Failed,
            }
        }
        Err(error) => http_error(error),
    };
    (body, started.elapsed().as_secs_f64() * 1000.0)
}

/// The fiat endpoint, reported as a COUNT. How many currencies is the shell's
/// observation; whether that is enough is the core's ruling.
fn fiat_rates(url: &str) -> (NetHealthBody, f64) {
    let started = Instant::now();
    let body = match proxy::agent(PROBE_TIMEOUT).get(url).call() {
        Ok(mut response) => {
            let mut text = String::new();
            match response.body_mut().as_reader().read_to_string(&mut text) {
                Ok(_) => match serde_json::from_str::<Value>(&text) {
                    // Both `normalizeRates` shapes: a bare array, or an object
                    // with a `rates` map.
                    Ok(Value::Array(items)) => NetHealthBody::Rates {
                        rate_count: u32::try_from(items.len()).unwrap_or(u32::MAX),
                    },
                    Ok(Value::Object(map)) => {
                        let count = match map.get("rates") {
                            Some(Value::Object(rates)) => rates.len(),
                            Some(Value::Array(rates)) => rates.len(),
                            _ => map.len(),
                        };
                        NetHealthBody::Rates {
                            rate_count: u32::try_from(count).unwrap_or(u32::MAX),
                        }
                    }
                    _ => NetHealthBody::Failed,
                },
                Err(_) => NetHealthBody::Failed,
            }
        }
        Err(error) => http_error(error),
    };
    (body, started.elapsed().as_secs_f64() * 1000.0)
}

impl Machine for NetworkAdmin {
    const LABEL: &'static str = "network_admin";

    fn boot_event(_cx: &App) -> Event {
        Event::Started
    }

    fn perform(operation: &NetOperation) -> Answer<NetShellResult, Self::Event> {
        match operation {
            NetOperation::ReadStore => Answer::Now(read_store()),

            // Best-effort writes: the in-memory ledger stays authoritative, so
            // a failed persist must not stop the person editing.
            NetOperation::WriteCustomNetworks { networks } => {
                let _ = storage::write_value(
                    storage::KEY_CUSTOM_NETWORKS,
                    encode_list(networks, |n: &NetCustomNetwork| StoredNetwork::from(n)),
                );
                Answer::Now(NetShellResult::Written)
            }
            NetOperation::WriteNetworkConfigs { configs } => {
                let _ = storage::write_value(
                    storage::KEY_NETWORK_CONFIG,
                    encode_list(configs, |c: &NetNetworkConfig| StoredConfig {
                        chain_id: c.chain_id,
                        rpc_url: c.rpc_url.clone(),
                        explorer_url: c.explorer_url.clone(),
                        bundler_url: c.bundler_url.clone(),
                    }),
                );
                Answer::Now(NetShellResult::Written)
            }
            NetOperation::WriteServiceEndpoints { endpoints } => {
                let _ = write_service_endpoints(endpoints);
                Answer::Now(NetShellResult::Written)
            }
            NetOperation::WriteRpcProviders { keys } => {
                let stored = StoredProviderKeys {
                    alchemy: keys.alchemy.clone(),
                    drpc: keys.drpc.clone(),
                    ankr: keys.ankr.clone(),
                };
                let _ = storage::write_value(
                    storage::KEY_RPC_PROVIDERS,
                    serde_json::to_value(stored).unwrap_or(Value::Null),
                );
                Answer::Now(NetShellResult::Written)
            }

            // A timer, not a thread. This one fires on every keystroke.
            NetOperation::StartSearchDebounce { ms } => Answer::After(
                Duration::from_millis(u64::from(*ms)),
                NetShellResult::DebounceElapsed,
            ),

            NetOperation::FetchSearchIndex => Answer::Blocking(Box::new(|| {
                let url = format!("{}/index/fuse-chains.json", data_base());
                let chains = get_json(&url, DATA_TIMEOUT)
                    .as_ref()
                    .map(decode_search_index)
                    .unwrap_or_default();
                NetShellResult::SearchIndex { chains }
            })),

            NetOperation::FetchChainInfo { chain_id } => {
                let chain_id = *chain_id;
                Answer::Blocking(Box::new(move || {
                    let url = format!("{}/chains/eip155-{chain_id}.json", data_base());
                    NetShellResult::ChainInfo {
                        chain_id,
                        data: get_json(&url, DATA_TIMEOUT)
                            .as_ref()
                            .and_then(decode_raw_chain_data),
                    }
                }))
            }

            NetOperation::ProbeRpc { url } => {
                let url = url.clone();
                Answer::Blocking(Box::new(move || {
                    let started = Instant::now();
                    // A chain id arrives as a hex quantity; anything else — an
                    // error, a timeout, a decimal string — is "no answer".
                    let reported_chain_id = json_rpc(&url, "eth_chainId", json!([]))
                        .and_then(|value| value.as_str().map(str::to_owned))
                        .and_then(|hex| u32::from_str_radix(hex.trim_start_matches("0x"), 16).ok());
                    NetShellResult::Probed {
                        url,
                        reported_chain_id,
                        latency_ms: started.elapsed().as_secs_f64() * 1000.0,
                    }
                }))
            }

            NetOperation::ProbeReachable { url } => {
                let url = url.clone();
                Answer::Blocking(Box::new(move || {
                    // An explorer is a website, not an API. On the WEB this has
                    // to be a `no-cors` request whose only honest signal is
                    // "resolved without throwing", because the browser hides the
                    // status. A desktop has no CORS, so it can read the status
                    // and say something true: 2xx/3xx is up. Recorded as a
                    // deliberate divergence — the desktop's answer is strictly
                    // better information, and the core already accepts a bool.
                    let started = Instant::now();
                    let ok = proxy::agent(PROBE_TIMEOUT)
                        .get(&url)
                        .call()
                        .is_ok_and(|response| {
                            let status = response.status().as_u16();
                            (200..400).contains(&status)
                        });
                    NetShellResult::Reachable {
                        url,
                        ok,
                        latency_ms: started.elapsed().as_secs_f64() * 1000.0,
                    }
                }))
            }

            NetOperation::RpcGetCode { url, address } => {
                let (url, address) = (url.clone(), address.clone());
                Answer::Blocking(Box::new(move || {
                    let code = json_rpc(&url, "eth_getCode", json!([address, "latest"]))
                        .and_then(|value| value.as_str().map(str::to_owned));
                    NetShellResult::Code { url, address, code }
                }))
            }

            NetOperation::RpcCallP256 { url } => {
                let url = url.clone();
                Answer::Blocking(Box::new(move || {
                    // `gas: 0x100000` is the zkSync-compatibility quirk of the
                    // legacy call, carried over verbatim.
                    let params = json!([
                        { "to": P256_PRECOMPILE, "data": VALID_P256_CALL, "gas": "0x100000" },
                        "latest"
                    ]);
                    let result = json_rpc(&url, "eth_call", params)
                        .and_then(|value| value.as_str().map(str::to_owned));
                    NetShellResult::P256Call { url, result }
                }))
            }

            NetOperation::FetchServiceHealth { field, base_url } => {
                let (field, base_url) = (*field, base_url.clone());
                Answer::Blocking(Box::new(move || {
                    let (body, latency_ms) = service_health(&base_url);
                    NetShellResult::ServiceHealth {
                        field,
                        body,
                        latency_ms,
                    }
                }))
            }

            NetOperation::FetchFiatRates { url } => {
                let url = url.clone();
                Answer::Blocking(Box::new(move || {
                    let (body, latency_ms) = fiat_rates(&url);
                    NetShellResult::FiatRates { body, latency_ms }
                }))
            }

            // Live since 031. Editing an endpoint in Settings must drop what
            // the pool measured about the old one — otherwise the new URL
            // inherits the old one's latency, its failures and its ban.
            //
            // The read path's two caches go with it. Both are keyed by nothing
            // but a chain id, so a document fetched from the OLD ethereum-data
            // host and a price read through the OLD RPC would outlive the edit
            // that replaced them and look like fresh answers from the new one.
            NetOperation::InvalidatePools { chain_id } => {
                crate::executor::pool::refresh(*chain_id);
                crate::executor::chain_tokens::invalidate();
                crate::executor::chainlink::invalidate();
                Answer::Now(NetShellResult::Invalidated)
            }
            // Live since 032: the relay client's account-info and quote caches
            // for this chain go with the endpoint edit that made them stale.
            NetOperation::ClearBundlerCache { chain_id } => {
                crate::executor::relay::clear_cache(*chain_id, None);
                Answer::Now(NetShellResult::BundlerCacheCleared)
            }
        }
    }
}

/// Write the four service endpoints, MERGING rather than replacing.
///
/// `vela.serviceEndpoints` has a second writer — onboarding's passkey-index
/// override — and a whole-value write from either erases the other's fields.
/// See `storage::save_registry_endpoint`, which is the same hazard from the
/// other side.
fn write_service_endpoints(endpoints: &NetServiceEndpoints) -> Result<(), storage::StorageError> {
    let mut fields = serde_json::Map::new();
    fields.insert(
        "ethereumDataURL".to_owned(),
        json!(endpoints.ethereum_data_url),
    );
    fields.insert(
        "passkeyIndexURL".to_owned(),
        json!(endpoints.passkey_index_url),
    );
    fields.insert(
        "bundlerServiceURL".to_owned(),
        json!(endpoints.bundler_service_url),
    );
    fields.insert("fiatRatesURL".to_owned(), json!(endpoints.fiat_rates_url));
    storage::merge_value(storage::KEY_SERVICE_ENDPOINTS, fields)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_host::CoreHost;
    use vela_core::app::network_admin::NetOverrideField;

    fn network(chain_id: u32) -> NetCustomNetwork {
        NetCustomNetwork {
            id: format!("custom-{chain_id}"),
            display_name: "Testnet".to_owned(),
            chain_id,
            icon_label: "T".to_owned(),
            icon_color: "#fff".to_owned(),
            icon_bg: "#000".to_owned(),
            logo_url: "https://logo.example/t.svg".to_owned(),
            is_l2: true,
            rpc_url: "https://rpc.example".to_owned(),
            explorer_url: "https://scan.example".to_owned(),
            bundler_url: "https://bundler.example".to_owned(),
            native_symbol: "TST".to_owned(),
            added_at_iso: "2026-09-04T00:00:00.000Z".to_owned(),
        }
    }

    /// The cross-client contract, asserted field by field on the RAW JSON.
    ///
    /// Not a round-trip through our own codec — that would pass just as happily
    /// with every name misspelled, because it would be misspelling them
    /// symmetrically. What matters is the exact spelling another client reads:
    /// four irregular names (`logoURL`, `rpcURL`, `explorerURL`, `bundlerURL`)
    /// and one renamed field (`addedAt`, not `addedAtIso`).
    #[test]
    fn a_stored_network_uses_the_field_names_every_other_client_reads() {
        let stored = serde_json::to_value(StoredNetwork::from(&network(1234)))
            .unwrap_or_else(|_| unreachable!("a network must serialize"));
        let object = stored
            .as_object()
            .unwrap_or_else(|| unreachable!("a network is an object"));

        for (field, expected) in [
            ("id", "custom-1234"),
            ("displayName", "Testnet"),
            ("iconLabel", "T"),
            ("logoURL", "https://logo.example/t.svg"),
            ("rpcURL", "https://rpc.example"),
            ("explorerURL", "https://scan.example"),
            ("bundlerURL", "https://bundler.example"),
            ("nativeSymbol", "TST"),
            ("addedAt", "2026-09-04T00:00:00.000Z"),
        ] {
            assert_eq!(
                object.get(field).and_then(Value::as_str),
                Some(expected),
                "`{field}` is the name other clients read"
            );
        }
        assert_eq!(object.get("chainId").and_then(Value::as_u64), Some(1234));
        assert_eq!(object.get("isL2").and_then(Value::as_bool), Some(true));

        // The snake_case Rust spellings must NOT appear — their presence would
        // mean a second, silent copy of every field.
        for absent in ["logo_url", "rpc_url", "added_at_iso", "chain_id", "is_l2"] {
            assert!(
                !object.contains_key(absent),
                "`{absent}` leaked the Rust spelling into the stored record"
            );
        }
    }

    /// A record another client wrote must come back whole.
    #[test]
    fn a_record_written_by_another_client_reads_back() {
        storage::tests::with_temp_state("net-foreign-record", || {
            let foreign = json!([{
                "id": "custom-100", "displayName": "Gnosis", "chainId": 100,
                "iconLabel": "G", "iconColor": "#fff", "iconBg": "#04795b",
                "logoURL": "https://logo.example/g.svg", "isL2": false,
                "rpcURL": "https://rpc.gnosischain.com",
                "explorerURL": "https://gnosisscan.io",
                "bundlerURL": "https://relay.example",
                "nativeSymbol": "xDAI", "addedAt": "2026-01-01T00:00:00.000Z"
            }]);
            if storage::write_value(storage::KEY_CUSTOM_NETWORKS, foreign).is_err() {
                unreachable!("could not seed");
            }
            match read_store() {
                NetShellResult::StoreLoaded {
                    custom_networks, ..
                } => {
                    assert_eq!(custom_networks.len(), 1);
                    assert_eq!(custom_networks[0].chain_id, 100);
                    assert_eq!(custom_networks[0].display_name, "Gnosis");
                    assert_eq!(custom_networks[0].native_symbol, "xDAI");
                    assert_eq!(custom_networks[0].added_at_iso, "2026-01-01T00:00:00.000Z");
                }
                other => unreachable!("wrong variant: {other:?}"),
            }
        });
    }

    /// One unparseable record must not cost the whole ledger. A rejected
    /// `StoreLoaded` strands the core unloaded forever, and every subsequent
    /// write is then dropped.
    #[test]
    fn one_corrupt_record_does_not_strand_the_load() {
        storage::tests::with_temp_state("net-corrupt-record", || {
            let mixed = json!([
                { "id": "custom-1", "chainId": "not-a-number" },
                { "id": "custom-100", "displayName": "Gnosis", "chainId": 100 }
            ]);
            if storage::write_value(storage::KEY_CUSTOM_NETWORKS, mixed).is_err() {
                unreachable!("could not seed");
            }
            match read_store() {
                NetShellResult::StoreLoaded {
                    custom_networks, ..
                } => {
                    assert_eq!(
                        custom_networks.len(),
                        1,
                        "the good record must survive its neighbour"
                    );
                    assert_eq!(custom_networks[0].chain_id, 100);
                }
                other => unreachable!("wrong variant: {other:?}"),
            }
        });
    }

    /// A cleared provider key is REMOVED, not stored as null or "" — the core's
    /// invariant ⑦. A stored empty string reads back as "configured with a blank
    /// key", which is a different fact.
    #[test]
    fn a_cleared_provider_key_is_removed_rather_than_blanked() {
        let stored = serde_json::to_value(StoredProviderKeys {
            alchemy: Some("abc".to_owned()),
            drpc: None,
            ankr: None,
        })
        .unwrap_or_else(|_| unreachable!("keys must serialize"));
        let object = stored
            .as_object()
            .unwrap_or_else(|| unreachable!("keys are an object"));
        assert_eq!(object.get("alchemy").and_then(Value::as_str), Some("abc"));
        assert!(!object.contains_key("drpc"), "a cleared key must be absent");
        assert!(!object.contains_key("ankr"));
    }

    /// Absent endpoint fields stay absent, so the core applies its own defaults
    /// merge. A shell that filled them in would hide "never configured" behind
    /// "configured to the default" — and the person could never tell.
    #[test]
    fn unconfigured_endpoints_stay_absent_rather_than_defaulted() {
        storage::tests::with_temp_state("net-endpoints-absent", || match read_store() {
            NetShellResult::StoreLoaded { endpoints, .. } => {
                assert_eq!(endpoints.ethereum_data_url, None);
                assert_eq!(endpoints.passkey_index_url, None);
                assert_eq!(endpoints.bundler_service_url, None);
                assert_eq!(endpoints.fiat_rates_url, None);
            }
            other => unreachable!("wrong variant: {other:?}"),
        });
    }

    /// Writing the four endpoints must not erase onboarding's index override,
    /// and vice versa. The two writers share one key; this is the same hazard
    /// `storage::save_registry_endpoint` guards from the other side.
    #[test]
    fn the_two_writers_of_service_endpoints_do_not_erase_each_other() {
        storage::tests::with_temp_state("net-endpoints-two-writers", || {
            if storage::save_registry_endpoint("https://idx.example").is_err() {
                unreachable!("could not save the index override");
            }
            if write_service_endpoints(&NetServiceEndpoints {
                ethereum_data_url: "https://data.example".to_owned(),
                passkey_index_url: "https://idx.example".to_owned(),
                bundler_service_url: "https://bundler.example".to_owned(),
                fiat_rates_url: "https://rates.example".to_owned(),
            })
            .is_err()
            {
                unreachable!("could not save the endpoints");
            }
            assert_eq!(
                storage::load_registry_endpoint().as_deref(),
                Some("https://idx.example"),
                "network_admin's write erased onboarding's index endpoint"
            );
            match read_store() {
                NetShellResult::StoreLoaded { endpoints, .. } => assert_eq!(
                    endpoints.ethereum_data_url.as_deref(),
                    Some("https://data.example")
                ),
                other => unreachable!("wrong variant: {other:?}"),
            }
        });
    }

    /// Drive the machine to quiescence, performing every operation for real.
    ///
    /// Unlike the local-only drivers elsewhere in this file, this one performs
    /// `Blocking` work inline and takes `After` answers immediately — it is the
    /// whole stack minus gpui's scheduling, which is what an end-to-end claim
    /// needs.
    fn drive_fully(host: &mut CoreHost<NetworkAdmin>, event: Event) {
        let mut pending = host.dispatch(event);
        while let Some(next) = pending.pop() {
            // What the core asked for, in order. The only readable record of a
            // live run, and the thing that turned "it did not add" into "it never
            // got past FetchChainInfo" in one line.
            eprintln!("[op] {:?}", next.operation);
            let result = match NetworkAdmin::perform(&next.operation) {
                Answer::Now(result) => result,
                Answer::Blocking(work) => work(),
                // The reports a streaming operation makes on its way. Dispatched
                // BEFORE its result, which is the order the async pump
                // guarantees and the order `balance_dashboard` depends on.
                Answer::Streaming(work) => {
                    let (reports, result) = crate::resident::run_streaming(work);
                    for report in reports {
                        pending.extend(host.dispatch(report));
                    }
                    result
                }
                // A debounce answers instantly here; no test should sit through
                // a timer it did not come to measure.
                Answer::After(_, result) => result,
            };
            pending.extend(host.resolve(next.id, result));
        }
    }

    /// SC-001, end to end: a network added against the REAL chain index and the
    /// REAL endpoint survives a relaunch.
    ///
    /// `AddByChainIdRequested` is the same pipeline the wizard's Add button
    /// runs — resolve the chain, probe its RPC, gate on the core's
    /// compatibility verdict, dedup, persist — with the UI's confirmation step
    /// removed. So this exercises every rule and every operation that matters,
    /// against the network, and the only thing it does not cover is which
    /// button a person pressed.
    ///
    /// `#[ignore]`d with the probes, for the same reason.
    #[test]
    #[ignore = "hits the real chain index and a real RPC"]
    fn a_network_added_against_the_real_chain_survives_a_relaunch() {
        storage::tests::with_temp_state("net-live-add", || {
            let mut host = CoreHost::<NetworkAdmin>::new();
            drive_fully(&mut host, Event::Started);
            let before = host.view().networks.len();

            // Zora, NOT Gnosis. The first version of this test used chain 100
            // and failed in the most useful way available: the count stayed at
            // 12 and the "added" row turned out to be the BUILT-IN Gnosis,
            // because the core's dedup gate (invariant ①) refuses to add a
            // chain the wallet already ships. The test was wrong; the core was
            // right. Zora is a real chain that is not a default, which is what
            // makes this an add rather than a no-op.
            const ZORA: u32 = 7_777_777;
            drive_fully(
                &mut host,
                Event::AddByChainIdRequested {
                    chain_id: ZORA,
                    now_iso: "2026-09-04T00:00:00.000Z".to_owned(),
                },
            );

            let view = host.view();
            println!("  networks: {before} -> {}", view.networks.len());
            println!(
                "  wizard: phase={:?} error={:?} can_add={}",
                view.wizard.phase, view.wizard.error, view.wizard.can_add
            );
            println!("  chain_info: {:?}", view.wizard.chain_info);
            println!("  compat: {:?}", view.wizard.compat);
            let added = view
                .networks
                .iter()
                .find(|row| row.chain_id == ZORA)
                .unwrap_or_else(|| unreachable!("chain {ZORA} was not added: {:?}", view.networks));
            println!("  added: {} ({})", added.display_name, added.rpc_url);
            assert!(
                added.is_custom,
                "a chain the wallet does not ship must arrive as CUSTOM"
            );
            assert!(
                !added.rpc_url.is_empty(),
                "an added network must carry an endpoint"
            );

            // What actually reached the disk.
            let stored =
                decode_list::<StoredNetwork, NetCustomNetwork>(storage::KEY_CUSTOM_NETWORKS);
            assert!(
                stored.iter().any(|n| n.chain_id == ZORA),
                "the network did not reach storage"
            );

            // The launch that matters: a fresh core over the same directory.
            let mut relaunched = CoreHost::<NetworkAdmin>::new();
            drive_fully(&mut relaunched, Event::Started);
            assert!(
                relaunched
                    .view()
                    .networks
                    .iter()
                    .any(|row| row.chain_id == ZORA),
                "the added network did not survive the relaunch"
            );
            println!("  survived the relaunch");
        });
    }

    /// SC-001's other half: the invariant-④ refusal, against real endpoints.
    ///
    /// Point Ethereum's RPC field at a **Gnosis** endpoint. The endpoint is
    /// perfectly healthy — that is the point. It answers `eth_chainId` with
    /// 100, the card is Ethereum's, and the core refuses the write rather than
    /// quietly routing chain-1 traffic to another chain's node.
    ///
    /// The refusal is the interesting direction. A shell that saved on blur and
    /// let the core "fix it later" would have written an override that silently
    /// breaks every balance read on that network, and nothing about the screen
    /// would look wrong.
    #[test]
    #[ignore = "hits two real RPC endpoints"]
    fn an_endpoint_serving_another_chain_is_refused() {
        storage::tests::with_temp_state("net-mismatch", || {
            const ETHEREUM: u32 = 1;
            // Healthy, real, and serving the WRONG chain for this card.
            const GNOSIS_RPC: &str = "https://gnosis-rpc.publicnode.com";

            let mut host = CoreHost::<NetworkAdmin>::new();
            drive_fully(&mut host, Event::Started);
            drive_fully(&mut host, Event::OverrideExpanded { chain_id: ETHEREUM });
            drive_fully(
                &mut host,
                Event::OverrideFieldEdited {
                    chain_id: ETHEREUM,
                    field: NetOverrideField::Rpc,
                    value: GNOSIS_RPC.to_owned(),
                },
            );
            drive_fully(&mut host, Event::OverrideBlurred { chain_id: ETHEREUM });

            let view = host.view();
            let row = view
                .networks
                .iter()
                .find(|row| row.chain_id == ETHEREUM)
                .unwrap_or_else(|| unreachable!("Ethereum vanished"));

            let mismatch = row
                .rpc_chain_mismatch
                .clone()
                .unwrap_or_else(|| unreachable!("the write was NOT refused: {row:?}"));
            println!(
                "  refused: card is chain {}, endpoint reported chain {}",
                mismatch.expected_chain_id, mismatch.reported_chain_id
            );
            assert_eq!(mismatch.expected_chain_id, ETHEREUM);
            assert_eq!(
                mismatch.reported_chain_id, 100,
                "the endpoint really does serve Gnosis"
            );

            // And nothing reached the disk. The pool still serves whatever it
            // served before, which is the whole point of refusing.
            let stored = decode_list::<StoredConfig, NetNetworkConfig>(storage::KEY_NETWORK_CONFIG);
            assert!(
                !stored.iter().any(|c| c.rpc_url == GNOSIS_RPC),
                "a refused override was written anyway: {stored:?}"
            );
            println!("  nothing was written");
        });
    }

    /// The probes, against a real chain.
    ///
    /// `#[ignore]` for the same reason the registry's health probe is: a CI
    /// runner is not promised a network, and a gate that fails on a flaky
    /// connection is a gate people learn to re-run rather than read. This is
    /// the spec-030 SC-001 evidence, run on demand:
    ///
    /// ```text
    /// cargo test executor::network_admin::tests::the_probes_answer -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "hits the real Gnosis network"]
    fn the_probes_answer_from_a_real_chain() {
        // NOT rpc.gnosischain.com. That endpoint answers curl with 200 and
        // this client with **403**, with and without a proxy — measured by
        // running the same request through a bare `ureq::Agent` and through
        // `proxy::agent`, against all three Gnosis URLs. It is a property of
        // that host, not of this code, and it is exactly why the core routes a
        // POOL of endpoints rather than one. (Recorded because
        // `registry.rs:362` pins that host FIRST for onboarding's legacy-name
        // lookup, where it therefore always burns a request before falling
        // through.)
        const GNOSIS: &str = "https://gnosis-rpc.publicnode.com";

        let Answer::Blocking(work) = NetworkAdmin::perform(&NetOperation::ProbeRpc {
            url: GNOSIS.to_owned(),
        }) else {
            unreachable!("a probe is network work");
        };
        match work() {
            NetShellResult::Probed {
                reported_chain_id,
                latency_ms,
                ..
            } => {
                assert_eq!(
                    reported_chain_id,
                    Some(100),
                    "Gnosis must report chain 100 — the core gates `add_confirmed` on this"
                );
                assert!(latency_ms > 0.0, "a real round trip takes measurable time");
                println!("  eth_chainId -> 100 in {latency_ms:.0}ms");
            }
            other => unreachable!("wrong variant: {other:?}"),
        }

        // The deployment probe, against a Safe that is definitely deployed.
        let Answer::Blocking(work) = NetworkAdmin::perform(&NetOperation::RpcGetCode {
            url: GNOSIS.to_owned(),
            address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894".to_owned(),
        }) else {
            unreachable!("a probe is network work");
        };
        match work() {
            NetShellResult::Code { code, .. } => {
                let code = code.unwrap_or_default();
                assert!(
                    code.len() > 2,
                    "a deployed Safe must return runtime code, got {code:?}"
                );
                println!("  eth_getCode -> {} bytes of runtime code", code.len() / 2);
            }
            other => unreachable!("wrong variant: {other:?}"),
        }

        // The P-256 precompile probe. Whether the ANSWER means "supported" is
        // the core's ruling; the shell only reports what came back.
        let Answer::Blocking(work) = NetworkAdmin::perform(&NetOperation::RpcCallP256 {
            url: GNOSIS.to_owned(),
        }) else {
            unreachable!("a probe is network work");
        };
        match work() {
            NetShellResult::P256Call { result, .. } => {
                println!("  eth_call(P256) -> {result:?}");
            }
            other => unreachable!("wrong variant: {other:?}"),
        }
    }

    /// The store survives a relaunch, driven through the real loop.
    #[test]
    fn an_added_network_survives_a_relaunch() {
        storage::tests::with_temp_state("net-relaunch", || {
            if storage::write_value(
                storage::KEY_CUSTOM_NETWORKS,
                encode_list(&[network(4242)], |n: &NetCustomNetwork| {
                    StoredNetwork::from(n)
                }),
            )
            .is_err()
            {
                unreachable!("could not write");
            }

            let mut host = CoreHost::<NetworkAdmin>::new();
            let mut pending = host.dispatch(Event::Started);
            // Drain only the LOCAL operations; the probes are network work and
            // this test is about the ledger, not the wire.
            while let Some(next) = pending.pop() {
                match NetworkAdmin::perform(&next.operation) {
                    Answer::Now(result) => pending.extend(host.resolve(next.id, result)),
                    _ => continue,
                }
            }
            let view = host.view();
            assert!(
                format!("{view:?}").contains("4242"),
                "the stored network did not reach the view"
            );
        });
    }
}
