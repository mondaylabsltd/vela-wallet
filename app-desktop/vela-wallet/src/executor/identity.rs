//! Who an address belongs to — the waterfall two machines share.
//!
//! **Ported from** `src/services/recipient-identity.ts` @ `c513c4c6` (FR-006).
//!
//! `contacts::ResolveIdentity` and `activity_feed::ResolveRecipientIdentity` ask
//! the same question about the same address, so they ask it here. Writing it
//! twice is how one screen learns a name the other never does.
//!
//! ## The order is the order, and it is not an optimisation
//!
//! 1. **The person's own accounts**, on disk, no network. A wallet that misses
//!    its own account labels it a stranger.
//! 2. **Cache**, positive entries only, 24 hours.
//! 3. **The passkey index** — a Vela user, by `walletRef`.
//! 4. **Name services**, in priority order: `.bnb`, `.arb`, `.g`, Basename, ENS.
//!
//! ## Only positive results are cached
//!
//! The core says so (`ContactShellResult::IdentityResolved`, invariant ⑦) and it
//! matters: a name registered a minute after somebody looked would otherwise be
//! invisible for a day. A miss costs a lookup; a cached miss costs the truth.

use serde_json::{Value, json};

use vela_core::app::contacts::ContactIdentity;
use vela_core::primitives::keccak256;

use crate::executor::{pool, registry, storage};

/// `vela.recipientIdentity` — `{ "0xlowercase": { name, source, at } }`.
///
/// One document rather than the web's key-per-address, because this store is a
/// single JSON file and a key per recipient would grow it without bound.
const CACHE_KEY: &str = "vela.recipientIdentity";
/// 24 hours (`recipient-identity.ts:75`).
const CACHE_TTL_MS: f64 = 24.0 * 60.0 * 60.0 * 1000.0;

/// An ENS-compatible registry: `registry.resolver(node)` then `resolver.name(node)`.
struct NameService {
    /// The words the UI shows. The shell owns these, per `ContactIdentity`.
    label: &'static str,
    chain_id: u32,
    registry: &'static str,
    /// ENSIP-19 chains derive the reverse node from a registrar call instead of
    /// `namehash("<addr>.addr.reverse")`.
    reverse_registrar: Option<&'static str>,
}

/// Priority order. Adding a service is a row here, as long as it follows the
/// ENS registry pattern.
const NAME_SERVICES: &[NameService] = &[
    NameService {
        label: ".bnb",
        chain_id: 56,
        registry: "0x08CEd32a7f3eeC915Ba84415e9C07a7286977956",
        reverse_registrar: None,
    },
    NameService {
        label: ".arb",
        chain_id: 42161,
        registry: "0x4a067EE58e73ac5E4a43722E008DFdf65B2bF348",
        reverse_registrar: None,
    },
    NameService {
        label: ".g",
        chain_id: 1625,
        registry: "0x5dC881dDA4e4a8d312be3544AD13118D1a04Cb17",
        reverse_registrar: None,
    },
    NameService {
        label: "Basename",
        chain_id: 8453,
        registry: "0xb94704422c2a1e396835a571837aa5ae53285a95",
        reverse_registrar: Some("0x79ea96012eea67a83431f1701b3dff7e37f9e282"),
    },
    NameService {
        label: "ENS",
        chain_id: 1,
        registry: "0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e",
        reverse_registrar: None,
    },
];

/// `resolver(bytes32)`, `name(bytes32)`, and ENSIP-19's `node(address)`.
const SEL_RESOLVER: &str = "0178b8bf";
const SEL_NAME: &str = "691f3431";
const SEL_NODE: &str = "bffbe61c";

// ---------------------------------------------------------------------------
// namehash
// ---------------------------------------------------------------------------

/// EIP-137 namehash. `namehash("") = 0x00…00`, then fold the labels right to
/// left.
fn namehash(name: &str) -> String {
    let mut node = vec![0u8; 32];
    if name.is_empty() {
        return hex(&node);
    }
    for label in name.split('.').rev() {
        let label_hash = keccak256(label.as_bytes());
        let mut combined = node.clone();
        combined.extend_from_slice(&label_hash);
        node = keccak256(&combined);
    }
    hex(&node)
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(2 + bytes.len() * 2);
    out.push_str("0x");
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

// ---------------------------------------------------------------------------
// Reverse resolution
// ---------------------------------------------------------------------------

fn eth_call(chain_id: u32, to: &str, data: &str) -> Option<String> {
    let result = pool::call(
        chain_id,
        "eth_call",
        json!([{ "to": to, "data": data }, "latest"]),
    )
    .ok()?;
    let hex = result.get("result").and_then(Value::as_str)?;
    (hex != "0x").then(|| hex.to_owned())
}

/// One service's answer for one address, or `None`.
fn reverse_resolve(address: &str, service: &NameService) -> Option<String> {
    let stripped = address.trim_start_matches("0x").to_lowercase();
    let reverse_node = match service.reverse_registrar {
        Some(registrar) => {
            let data = format!("0x{SEL_NODE}{stripped:0>64}");
            let answer = eth_call(service.chain_id, registrar, &data)?;
            // A node is one word. Anything shorter is not one.
            (answer.trim_start_matches("0x").len() >= 64).then_some(answer)?
        }
        None => namehash(&format!("{stripped}.addr.reverse")),
    };
    let node = reverse_node.trim_start_matches("0x");

    let resolver = eth_call(
        service.chain_id,
        service.registry,
        &format!("0x{SEL_RESOLVER}{node}"),
    )?;
    let resolver = resolver.trim_start_matches("0x");
    // The last 20 bytes of the word. A zero resolver means "no record", which
    // is the common answer and not an error.
    let resolver_address = resolver.get(resolver.len().checked_sub(40)?..)?;
    if resolver_address.bytes().all(|b| b == b'0') {
        return None;
    }

    let answer = eth_call(
        service.chain_id,
        &format!("0x{resolver_address}"),
        &format!("0x{SEL_NAME}{node}"),
    )?;
    decode_name(&answer)
}

/// An ABI `string` return, bounded and validated.
///
/// A name is drawn next to somebody's money, so an over-long or non-UTF-8
/// answer is refused rather than truncated into something plausible-looking.
fn decode_name(hex: &str) -> Option<String> {
    let data = hex.trim_start_matches("0x");
    if data.len() < 128 {
        return None;
    }
    let offset = usize::from_str_radix(data.get(..64)?, 16)
        .ok()?
        .checked_mul(2)?;
    let length = usize::from_str_radix(data.get(offset..offset.checked_add(64)?)?, 16)
        .ok()?
        .checked_mul(2)?;
    // The web's cap, in bytes.
    if length == 0 || length > 512 {
        return None;
    }
    let start = offset.checked_add(64)?;
    let body = data.get(start..start.checked_add(length)?)?;
    let bytes: Option<Vec<u8>> = (0..body.len() / 2)
        .map(|i| u8::from_str_radix(body.get(i * 2..i * 2 + 2)?, 16).ok())
        .collect();
    let name = String::from_utf8(bytes?).ok()?;
    let name = name.trim().to_owned();
    (!name.is_empty()).then_some(name)
}

// ---------------------------------------------------------------------------
// Cache
// ---------------------------------------------------------------------------

fn cached(address: &str, now_ms: f64) -> Option<ContactIdentity> {
    let store = storage::read_value(CACHE_KEY).ok().flatten()?;
    let entry = store.get(address.to_lowercase())?;
    let at = entry.get("at").and_then(Value::as_f64)?;
    if now_ms - at > CACHE_TTL_MS {
        return None;
    }
    Some(ContactIdentity {
        name: entry.get("name").and_then(Value::as_str)?.to_owned(),
        source: entry.get("source").and_then(Value::as_str)?.to_owned(),
    })
}

fn remember(address: &str, identity: &ContactIdentity, now_ms: f64) {
    let mut map = match storage::read_value(CACHE_KEY) {
        Ok(Some(Value::Object(map))) => map,
        _ => serde_json::Map::new(),
    };
    map.insert(
        address.to_lowercase(),
        json!({ "name": identity.name, "source": identity.source, "at": now_ms }),
    );
    let _ = storage::write_value(CACHE_KEY, Value::Object(map));
}

// ---------------------------------------------------------------------------
// The waterfall
// ---------------------------------------------------------------------------

/// The person's own name for an address they own, read from disk.
///
/// Checked BEFORE any network lookup, because the answer is already there and
/// because "my other wallet" is a better label than an ENS name for the same
/// address. Compared lowercased: a wallet that misses its own account on casing
/// labels it a stranger.
#[must_use]
pub fn own_account_name(address: &str) -> Option<String> {
    let wanted = address.to_lowercase();
    storage::load_accounts()
        .ok()?
        .into_iter()
        .find_map(|account| {
            (account.address.to_lowercase() == wanted && !account.name.trim().is_empty())
                .then_some(account.name)
        })
}

/// Is this a real address worth asking about?
fn askable(address: &str) -> bool {
    let stripped = address.trim_start_matches("0x");
    if stripped.len() != 40 || !stripped.bytes().all(|b| b.is_ascii_hexdigit()) {
        return false;
    }
    // The zero address is a mint/burn counterparty (EIP-7708 native events),
    // not a recipient. It has no identity and asking 404s the index.
    !stripped.bytes().all(|b| b == b'0')
}

/// The whole waterfall, blocking. `None` is a real answer: nobody knows this
/// address.
#[must_use]
pub fn resolve(address: &str) -> Option<ContactIdentity> {
    if !askable(address) {
        return None;
    }
    let now_ms = crate::executor::now_ms();

    // 1. The person's own accounts, before anything reaches the network.
    if let Some(name) = own_account_name(address) {
        return Some(ContactIdentity {
            name,
            source: "self".to_owned(),
        });
    }

    // 2. Cache — positives only, so a miss simply is not here.
    if let Some(identity) = cached(address, now_ms) {
        return Some(identity);
    }

    // 3. The passkey index.
    if let Some(name) = registry::query_by_wallet_ref(address) {
        let identity = ContactIdentity {
            name,
            source: "passkey".to_owned(),
        };
        remember(address, &identity, now_ms);
        return Some(identity);
    }

    // 4. Name services, in priority order.
    //
    // The web races all five and takes the first match BY PRIORITY, not by
    // arrival. Asked in order here: five sequential lookups against a name
    // nobody has is the slow path, and it is also the rare one — a stranger's
    // address is looked up once and then not asked again for a day.
    for service in NAME_SERVICES {
        if let Some(name) = reverse_resolve(address, service) {
            let identity = ContactIdentity {
                name,
                source: service.label.to_owned(),
            };
            remember(address, &identity, now_ms);
            return Some(identity);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// EIP-137's own worked examples.
    #[test]
    fn namehash_matches_the_specification() {
        assert_eq!(
            namehash(""),
            "0x0000000000000000000000000000000000000000000000000000000000000000"
        );
        assert_eq!(
            namehash("eth"),
            "0x93cdeb708b7545dc668eb9280176169d1c33cfd8ed6f04690a0bcc88a93fc4ae"
        );
        assert_eq!(
            namehash("foo.eth"),
            "0xde9b09fd7c5f901e23a3f19fecc54828e9c848539801e86591bd9801b019f84f"
        );
    }

    /// The addresses that must not reach the network at all.
    #[test]
    fn only_a_real_address_is_worth_asking_about() {
        assert!(askable("0x88cCA0EeDbF2C4426110bbFc998F048689266894"));
        assert!(askable("88cCA0EeDbF2C4426110bbFc998F048689266894"));
        // The zero address is a mint/burn counterparty, not a person.
        assert!(!askable("0x0000000000000000000000000000000000000000"));
        assert!(!askable(""));
        assert!(!askable("0x123"));
        assert!(!askable("0xZZcCA0EeDbF2C4426110bbFc998F048689266894"));
    }

    /// A name is drawn beside somebody's money, so a malformed answer is
    /// refused rather than shown as a plausible-looking fragment.
    #[test]
    fn a_name_decodes_or_is_refused() {
        // offset 0x20, length 7, "vela.eth"[..7] = "vela.et"
        let hex = format!(
            "0x{:064x}{:064x}{:0<64}",
            32,
            7,
            "76656c612e6574" // "vela.et"
        );
        assert_eq!(decode_name(&hex).as_deref(), Some("vela.et"));

        assert_eq!(decode_name("0x"), None, "empty");
        assert_eq!(
            decode_name(&format!("0x{:064x}{:064x}", 32, 0)),
            None,
            "a zero-length name is not a name"
        );
        assert_eq!(
            decode_name(&format!("0x{:064x}{:064x}", 32, 9_999)),
            None,
            "a length past the payload"
        );
        // Non-UTF-8 bytes must not become a mojibake name.
        let bad = format!("0x{:064x}{:064x}{:0<64}", 32, 2, "fffe");
        assert_eq!(decode_name(&bad), None);
    }

    /// The person's own account wins before anything is asked, and casing does
    /// not decide.
    ///
    /// This test moved here from `activity_feed.rs` when the two copies of the
    /// lookup became one — the feed's `AliasResolved` and the contacts sheet's
    /// `IdentityResolved` now call the same function.
    #[test]
    fn an_own_account_is_named_locally_whatever_its_casing() {
        storage::tests::with_temp_state("identity-own", || {
            let account = vela_core::app::Account {
                id: "cred0".to_owned(),
                name: "Everyday wallet".to_owned(),
                address: "0xABCdef0000000000000000000000000000000001".to_owned(),
                public_key_hex: "04aa".to_owned(),
                created_at_iso: "2026-09-04T00:00:00.000Z".to_owned(),
                keys: Vec::new(),
            };
            if storage::save_account(&account).is_err() {
                unreachable!("could not save");
            }
            // Asked in a different case: the address is the key, and a wallet
            // that misses its own account on casing labels it a stranger.
            assert_eq!(
                own_account_name("0xabcdef0000000000000000000000000000000001").as_deref(),
                Some("Everyday wallet")
            );
            assert_eq!(
                own_account_name("0xABCDEF0000000000000000000000000000000001").as_deref(),
                Some("Everyday wallet")
            );
            assert_eq!(own_account_name("0xsomeone-else"), None);

            // And the waterfall stops there, before the network — `source` says
            // where the name came from.
            let identity = resolve("0xabcdef0000000000000000000000000000000001")
                .unwrap_or_else(|| unreachable!("own account not resolved"));
            assert_eq!(identity.name, "Everyday wallet");
            assert_eq!(identity.source, "self");
        });
    }

    /// Positives are cached and negatives are not — a name registered a minute
    /// after somebody looked must not be invisible for a day.
    #[test]
    fn only_a_positive_is_remembered_and_it_expires() {
        storage::tests::with_temp_state("identity-cache", || {
            const ADDR: &str = "0x88cca0eedbf2c4426110bbfc998f048689266894";
            let now = crate::executor::now_ms();
            assert_eq!(cached(ADDR, now), None);

            let identity = ContactIdentity {
                name: "vela.eth".to_owned(),
                source: "ENS".to_owned(),
            };
            remember(ADDR, &identity, now);
            assert_eq!(cached(ADDR, now).as_ref(), Some(&identity));
            // Casing must not miss the entry it just wrote.
            assert_eq!(
                cached("0x88cCA0EeDbF2C4426110bbFc998F048689266894", now).as_ref(),
                Some(&identity)
            );
            assert_eq!(cached(ADDR, now + CACHE_TTL_MS + 1.0), None, "expired");
        });
    }

    /// The live waterfall against a name that exists.
    #[test]
    #[ignore = "reads the passkey index and five name services"]
    fn a_real_ens_name_resolves_and_a_nameless_address_does_not() {
        storage::tests::with_temp_state("identity-live", || {
            // vitalik.eth — a reverse record that has existed for years.
            const NAMED: &str = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045";
            let identity = resolve(NAMED);
            println!("  {NAMED} → {identity:?}");
            let identity = identity.unwrap_or_else(|| unreachable!("no identity"));
            assert!(!identity.name.is_empty());
            assert_eq!(identity.source, "ENS");

            // The second call must come from the cache, not the network.
            let again = resolve(NAMED);
            assert_eq!(
                again.as_ref().map(|i| i.name.as_str()),
                Some(identity.name.as_str())
            );

            // An address nobody has named answers None — and is NOT cached, so
            // a name registered tomorrow is found tomorrow.
            const NAMELESS: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
            let none = resolve(NAMELESS);
            println!("  {NAMELESS} → {none:?}");
            assert_eq!(none, None, "the golden Safe has no reverse record");
            assert_eq!(cached(NAMELESS, crate::executor::now_ms()), None);
        });
    }
}
