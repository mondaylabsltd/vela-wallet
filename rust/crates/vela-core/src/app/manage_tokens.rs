//! Machine — manual custom-token management (spec `017-crux-wallet-state-complete`,
//! `manage_tokens` P3).
//!
//! ```text
//! Start ──► ReadCustomTokens ──► ledger {custom_tokens}
//! AddressInput ──► validity; found cards cleared
//! DetectRequested{networks} ──► one MulticallErc20Meta PER CHAIN, in parallel
//!        └─ aggregate Map<chain_id, Option<meta>> ──► found cards │ not-found
//! SaveRequested{chain_id} ──► ReadCustomTokens (fresh dedupe read)
//!        ├─ id already stored ──► mark added, NO write
//!        └─ WriteCustomToken ──► Saved ──► mark added + InvalidateTokenCache
//! DeleteRequested{id} ──► RemoveCustomToken ──► Removed ──► drop row + InvalidateTokenCache
//! ```
//!
//! Ported from the ERC-20 tab of `src/components/ui/AddTokenPanel.tsx` (its 13
//! `useState`s collapse into [`Model`]; the custom-NETWORK tab is
//! `network_admin`'s), with the metadata admission semantics of
//! `src/services/token-metadata.ts` / `src/services/tokens.ts` and the storage
//! contract of `src/services/storage.ts:110-124`. The three inventory
//! invariants each have a single owner here:
//!
//! - **① one dedupe implementation** — [`token_id`] is THE
//!   `${chainId}_${addr.toLowerCase()}` key. Today the manual path
//!   (`AddTokenPanel.tsx:188`) and the auto-add path (`token-autoadd.ts:69`)
//!   each hand-roll it; this pub fn is the designated single owner. The
//!   landed `token_trust` machine still formats `"{chain_id}_{addr}"` inline
//!   over an already-lowercased addr (`auto_add_write_phase`) — byte-identical
//!   output; pointing it at this fn is a cross-file follow-up outside this
//!   wave's write fence. The save path re-reads storage *at save time*
//!   exactly as `AddTokenPanel.tsx:192` does, which is what catches a token
//!   the auto-add path listed mid-session.
//! - **② no symbol, no listing** — [`admissible_erc20_meta`] is the
//!   `!name || !symbol → null` gate (`AddTokenPanel.tsx:59`). The `'?'` of the
//!   invariant is the *render fallback* for a missing symbol
//!   (`TokenCard.tsx:78`), not a value to blacklist: a contract whose
//!   `symbol()` literally returns `"?"` is listed today and stays listed —
//!   ported verbatim.
//! - **③ a mutation invalidates the fetchTokens cache** — after a confirmed
//!   save or delete the core issues [`MtokOperation::InvalidateTokenCache`],
//!   so the send/receive token picker sees the change immediately instead of
//!   after the 5-minute TTL (`ReceiveRequestControls.tsx:59-67`,
//!   `token-autoadd.ts:79-81`). Today this is the host's `onChanged` callback
//!   convention; here it is the machine's own rule.
//!
//! The shell owns the network registry (`getAllNetworksSync` — defaults plus
//! custom networks — snapshots into [`Event::DetectRequested`]), the Multicall3
//! encoding/decoding and RPC failover, the storage key, the QR scanner (it
//! feeds the extracted address through [`Event::AddressInput`]), haptics and
//! every word on screen. The core owns validity, the probe orchestration, the
//! admission and dedupe rules, and the cache-invalidation decision.
//!
//! Deviation from verbatim (016 staleness rule, deliberate): today an address
//! edit does NOT retire an in-flight probe — the closure keeps the old address
//! and lands its results under the new input (`AddTokenPanel.tsx:159-185`), so
//! a save could write token A's metadata under address B. Here a probe answer
//! echoes the address it probed (the [`MtokShellResult::Removed`] echo
//! pattern) and is accepted only when that echo matches the current input AND
//! the chain is still owed an answer. The pending set alone is not enough:
//! type A → detect → type B → detect repopulates the SAME chain ids, and an
//! attempt bump can't carry the gate either — it would also strand an
//! unrelated save read in flight. The address echo is exact: an old answer
//! for the same (chain, address) pair is accurate and welcome; one for a
//! superseded address is dropped.

use std::collections::{BTreeMap, BTreeSet};

use crux_core::capability::Operation;
use crux_core::macros::effect;
use crux_core::{render::render, render::RenderOperation, App, Command};
use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::contacts::is_address;

// ---------------------------------------------------------------------------
// Wire value types
// ---------------------------------------------------------------------------

/// One chain's probe answer — ERC-20 `name()`, `symbol()`, `decimals()` as the
/// shell decoded them from a single Multicall3 `aggregate3` (`AddTokenPanel.tsx:39-61`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct MtokTokenMeta {
    pub name: String,
    pub symbol: String,
    /// `decU8` output — never a bignum, so `u8` is the honest wire type.
    pub decimals: u8,
}

/// One row of the shell's network registry snapshot (`getAllNetworksSync()` —
/// defaults + custom networks). `name` is the display name that gets frozen
/// into a saved token's `networkName`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct MtokNetwork {
    pub chain_id: u32,
    pub name: String,
}

/// The persisted token. Serialises 1:1 to the TS `CustomToken`
/// (`models/types.ts:194-202`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct MtokCustomToken {
    /// `${chainId}_${contractAddress}` — always built by [`token_id`].
    pub id: String,
    pub chain_id: u32,
    /// Lowercased at save time (`AddTokenPanel.tsx:203`).
    pub contract_address: String,
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
    pub network_name: String,
}

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------

/// What this machine asks the platform to do. Multicall3 encoding, the RPC
/// pool, `vela.customTokens` and the fetchTokens cache all live behind these
/// sentences.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum MtokOperation {
    /// Probe `name`/`symbol`/`decimals` for `address` on one chain via a
    /// single Multicall3 `aggregate3` eth_call (`AddTokenPanel.tsx:39-61`).
    /// The address is passed exactly as typed — the probe is case-insensitive
    /// on-chain; only the SAVE lowercases.
    MulticallErc20Meta { chain_id: u32, address: String },
    /// Read the `vela.customTokens` array. Unreadable/corrupt answers as
    /// empty, exactly as the TS loader's `catch {}` does.
    ReadCustomTokens,
    /// Persist one token — replace-by-id then append
    /// (`storage.ts:110-115`).
    WriteCustomToken { token: MtokCustomToken },
    /// Remove one token by id (`storage.ts:121-124`).
    RemoveCustomToken { id: String },
    /// Drop the active account's `fetchTokens` cache so the token picker sees
    /// the change immediately (invariant ③). The cache is keyed by WALLET
    /// address (`wallet-api.ts:130-133`), which this core deliberately does
    /// not hold — the shell knows the active account.
    InvalidateTokenCache,
}

/// What the shell observed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum MtokShellResult {
    /// One chain's probe concluded. `address` echoes the operation's address
    /// verbatim — the correlation key that retires answers for a superseded
    /// input (see the module doc's recorded deviation). `None` covers every
    /// unresolved path the TS collapses to `null`: RPC error, decode failure,
    /// a sub-call that reverted, or a missing name/symbol
    /// (`AddTokenPanel.tsx:49-60`; the rejected `Promise.allSettled` arm at
    /// `:176-178` is the same `None` — fail-closed). The core re-checks
    /// admission regardless.
    ChainMetaResolved {
        chain_id: u32,
        address: String,
        meta: Option<MtokTokenMeta>,
    },
    CustomTokensLoaded {
        tokens: Vec<MtokCustomToken>,
    },
    Saved,
    /// `saveCustomToken` threw — the `showAlert(errorSaveToken)` branch
    /// (`AddTokenPanel.tsx:213-214`).
    SaveFailed,
    /// `id` rides along so the answer correlates with the row it removes.
    Removed {
        id: String,
    },
    /// `removeCustomToken` threw. TS has NO handler here
    /// (`AddTokenPanel.tsx:82-87` — the rejection is unhandled and none of
    /// the follow-ups run), so the port fails closed: the row stays.
    RemoveFailed {
        id: String,
    },
    /// Cache-invalidation acknowledged. Never changes state.
    CacheInvalidated,
}

impl Operation for MtokOperation {
    type Output = MtokShellResult;
}

#[effect]
pub enum MtokEffect {
    Render(RenderOperation),
    Shell(MtokOperation),
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MtokEvent"))]
pub enum Event {
    /// The panel opened — load the already-added tokens (the mount effect at
    /// `AddTokenPanel.tsx:80`). One panel, one core: single-shot.
    Start,
    /// Every keystroke of the contract-address field, or a scanned QR after
    /// the shell's `extractAddress`. Clears the found cards
    /// (`AddTokenPanel.tsx:394-397`) and retires any in-flight probe (see the
    /// module doc's recorded deviation). Deliberately does NOT touch
    /// `added_token_ids` — ported verbatim, see [`Model::added_token_ids`].
    AddressInput { s: String },
    /// The "search networks" button. `networks` is the shell's snapshot of
    /// `getAllNetworksSync()` (`AddTokenPanel.tsx:166`) — the registry itself
    /// is `network_admin` / shell domain, so it rides on the event.
    DetectRequested { networks: Vec<MtokNetwork> },
    /// "Add to wallet" on one found card.
    SaveRequested { chain_id: u32 },
    /// The trash button on an already-added row.
    DeleteRequested { id: String },
    /// Internal: an effect resolved. `attempt` is captured when the request
    /// is made; a result carrying an older attempt is dropped.
    #[serde(skip)]
    ShellCompleted {
        attempt: u64,
        result: MtokShellResult,
    },
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

/// Where the save pipeline is. TS spreads this across `saving` +
/// `addedTokenIds` + an await chain; here it is one place.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum SavePhase {
    #[default]
    Idle,
    /// The fresh dedupe read (`AddTokenPanel.tsx:192`) is in flight. The
    /// candidate token was built when the user tapped save — the id, address
    /// and metadata are frozen there, exactly as the TS closure captures
    /// them, so later typing cannot bend a save in flight.
    Checking { token: MtokCustomToken },
    /// `saveCustomToken` is in flight (TS `saving === true`).
    Writing { token: MtokCustomToken },
}

#[derive(Default)]
pub struct Model {
    started: bool,
    /// As typed — NOT trimmed, NOT lowercased (verbatim: validity and the
    /// probe both see the raw input; only the save lowercases).
    input_address: String,
    address_valid: bool,
    /// chain_id → display name from the latest [`Event::DetectRequested`]
    /// snapshot; frozen into `network_name` at save time.
    networks: BTreeMap<u32, String>,
    /// Registry order of the latest probe — found cards render in this order,
    /// as `Promise.allSettled` preserves it today.
    probe_order: Vec<u32>,
    /// Chains still owing an answer. An answer for a chain not in here is a
    /// superseded probe's — dropped.
    pending_probes: BTreeSet<u32>,
    /// The aggregate: chain_id → admissible metadata or a recorded miss.
    detection: BTreeMap<u32, Option<MtokTokenMeta>>,
    detect_in_flight: bool,
    /// Every id saved (or found already stored) THIS SESSION. Ported
    /// verbatim, quirk included: a delete does NOT remove the id
    /// (`handleDelete` never touches `addedTokenIds`), so a deleted token's
    /// found card keeps reading "added" until the panel is reopened.
    added_token_ids: BTreeSet<String>,
    /// Mirror of `vela.customTokens` — refreshed by every read, trusted by
    /// the view and the delete gate.
    custom_tokens: Vec<MtokCustomToken>,
    customs_loaded: bool,
    save_phase: SavePhase,
    /// Deletes in flight, keyed by id — TS fires them unguarded; the set
    /// keeps one per row and correlates the echo.
    pending_deletes: BTreeSet<String>,
    /// A probe concluded with zero hits — the `showAlert(notFound*)` branch
    /// (`AddTokenPanel.tsx:180-182`). The shell shows the copy; cleared by
    /// the next input or probe.
    not_found: bool,
    /// The address is a chain's native coin behind an ERC-20 interface
    /// (`native_alias_token`) — a refusal with a reason, not a miss.
    native_alias: bool,
    /// A save failed — the `showAlert(errorSaveToken)` branch. Same contract.
    save_error: bool,
    attempt: u64,
}

/// The ERC-20 interface onto a chain's NATIVE balance, where the chain has one.
///
/// Arc's native coin IS USDC: 18 decimals natively, with a 6-decimal ERC-20
/// view of the SAME balance at `0x3600…0000` (spec 060). Adding that address as
/// a token would list one balance twice and double the person's total — money
/// that does not exist, on a screen they are meant to trust.
///
/// Per-chain data, not a global address ban: the same address on another chain
/// is an ordinary unknown contract and is treated as one.
pub fn native_alias_token(chain_id: u32) -> Option<&'static str> {
    match chain_id {
        5_042 | 5_042_002 => Some("0x3600000000000000000000000000000000000000"),
        // Stable: native USDT0 (18 dp) and this 6-dp ERC-20 are one balance —
        // verified address by address, `native ÷ 10¹² == erc20` (spec 061).
        988 => Some("0x779Ded0c9e1022225f8E0630b35a9b54bE713736"),
        _ => None,
    }
}

/// Whether this address is the chain's native coin wearing an ERC-20 interface.
fn is_native_alias(chain_id: u32, address: &str) -> bool {
    native_alias_token(chain_id)
        .is_some_and(|alias| alias.eq_ignore_ascii_case(address.trim()))
}

// ---------------------------------------------------------------------------
// ViewModel
// ---------------------------------------------------------------------------

/// One found card (`AddTokenPanel.tsx:417-456`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct MtokFound {
    pub chain_id: u32,
    pub network_name: String,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    /// The card's "added ✓" state — recomputed live against the CURRENT
    /// input, exactly as `addedTokenIds.has(\`${chainId}_${addr}\`)` is.
    pub added: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct MtokView {
    pub input_address: String,
    /// Drives the search button's enabled state.
    pub address_valid: bool,
    pub detecting: bool,
    /// Registry order; only chains whose metadata resolved (invariant ②).
    pub found: Vec<MtokFound>,
    /// TS `saving` — true only while the write itself is in flight.
    pub saving: bool,
    /// The manage/delete list below the form.
    pub custom_tokens: Vec<MtokCustomToken>,
    pub not_found: bool,
    /// The searched address is this network's native coin wearing an ERC-20
    /// interface. It holds the same balance the wallet already shows, so it is
    /// refused with that reason rather than reported as "not found".
    pub native_alias: bool,
    pub save_error: bool,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct ManageTokens;

impl App for ManageTokens {
    type Event = Event;
    type Model = Model;
    type ViewModel = MtokView;
    type Effect = MtokEffect;

    fn update(&self, event: Event, model: &mut Model) -> Command<MtokEffect, Event> {
        match event {
            Event::Start => {
                if model.started {
                    return Command::done();
                }
                model.started = true;
                model.attempt += 1;
                request(model, MtokOperation::ReadCustomTokens)
            }

            Event::AddressInput { s } => {
                model.address_valid = is_address(&s);
                model.input_address = s;
                // `setFoundTokens([])` — and, per the recorded deviation,
                // retire any probe still in flight for the old address.
                clear_probe(model);
                model.not_found = false;
                model.native_alias = false;
                model.save_error = false;
                render()
            }

            Event::DetectRequested { networks } => {
                // Button gate: `disabled={!isValidAddress || loading}`.
                if !model.address_valid || model.detect_in_flight {
                    return Command::done();
                }
                clear_probe(model);
                model.not_found = false;
                model.native_alias = false;
                model.save_error = false;
                model.networks.clear();
                for network in networks {
                    // Keep-first on a duplicate chain id, one probe per chain.
                    if model.networks.contains_key(&network.chain_id) {
                        continue;
                    }
                    model.networks.insert(network.chain_id, network.name);
                    model.probe_order.push(network.chain_id);
                    model.pending_probes.insert(network.chain_id);
                }
                if model.pending_probes.is_empty() {
                    // Zero networks ⇒ zero results ⇒ the not-found alert,
                    // exactly as an empty `allSettled` would conclude.
                    model.not_found = true;
                    return render();
                }
                model.detect_in_flight = true;
                let attempt = model.attempt;
                let address = model.input_address.clone();
                let mut commands: Vec<Command<MtokEffect, Event>> = model
                    .probe_order
                    .iter()
                    .map(|&chain_id| {
                        Command::request_from_shell(MtokOperation::MulticallErc20Meta {
                            chain_id,
                            address: address.clone(),
                        })
                        .then_send(move |result| Event::ShellCompleted { attempt, result })
                    })
                    .collect();
                commands.push(render());
                Command::all(commands)
            }

            Event::SaveRequested { chain_id } => {
                // One save at a time. TS gates this visually (the shared
                // `saving` flag puts every card's button in its loading
                // state); the phase gate closes the double-entry window the
                // TS `await loadCustomTokens()` leaves open — the second
                // save was a no-op-by-replace there anyway.
                if model.save_phase != SavePhase::Idle {
                    return Command::done();
                }
                // A card only exists for a chain whose metadata resolved; a
                // stray event for anything else must not save (fail-closed).
                let Some(Some(meta)) = model.detection.get(&chain_id) else {
                    return Command::done();
                };
                let Some(network_name) = model.networks.get(&chain_id) else {
                    return Command::done();
                };
                let id = token_id(chain_id, &model.input_address);
                // `if (addedTokenIds.has(tokenId)) return` — session guard.
                if model.added_token_ids.contains(&id) {
                    return Command::done();
                }
                let token = MtokCustomToken {
                    id,
                    chain_id,
                    contract_address: model.input_address.to_lowercase(),
                    symbol: meta.symbol.clone(),
                    name: meta.name.clone(),
                    decimals: meta.decimals,
                    network_name: network_name.clone(),
                };
                model.save_phase = SavePhase::Checking { token };
                // The fresh read at save time (`AddTokenPanel.tsx:192`) —
                // NOT the mirror. This is what catches an id the auto-add
                // path stored since our last read (invariant ①).
                request(model, MtokOperation::ReadCustomTokens)
            }

            Event::DeleteRequested { id } => {
                // Only rendered rows are deletable; one delete per row.
                if !model.custom_tokens.iter().any(|token| token.id == id)
                    || model.pending_deletes.contains(&id)
                {
                    return Command::done();
                }
                model.pending_deletes.insert(id.clone());
                request(model, MtokOperation::RemoveCustomToken { id })
            }

            Event::ShellCompleted { attempt, result } => {
                if attempt != model.attempt {
                    return Command::done();
                }
                accept(model, result)
            }
        }
    }

    fn view(&self, model: &Model) -> MtokView {
        let found = model
            .probe_order
            .iter()
            .filter_map(|chain_id| {
                let meta = model.detection.get(chain_id)?.as_ref()?;
                Some(MtokFound {
                    chain_id: *chain_id,
                    network_name: model.networks.get(chain_id).cloned().unwrap_or_default(),
                    name: meta.name.clone(),
                    symbol: meta.symbol.clone(),
                    decimals: meta.decimals,
                    added: model
                        .added_token_ids
                        .contains(&token_id(*chain_id, &model.input_address)),
                })
            })
            .collect();
        MtokView {
            input_address: model.input_address.clone(),
            address_valid: model.address_valid,
            detecting: model.detect_in_flight,
            found,
            saving: matches!(model.save_phase, SavePhase::Writing { .. }),
            custom_tokens: model.custom_tokens.clone(),
            not_found: model.not_found,
            native_alias: model.native_alias,
            save_error: model.save_error,
        }
    }
}

// ---------------------------------------------------------------------------
// Shell results
// ---------------------------------------------------------------------------

fn accept(model: &mut Model, result: MtokShellResult) -> Command<MtokEffect, Event> {
    match result {
        MtokShellResult::ChainMetaResolved {
            chain_id,
            address,
            meta,
        } => {
            // Keyed staleness gate, both keys exact (module doc deviation):
            // an answer whose echoed address is not the current input belongs
            // to a superseded probe — the live probe's address always equals
            // the input, because editing it clears the pending set. Checked
            // FIRST, without touching the set: the live probe for this chain
            // is still owed its own answer.
            if address != model.input_address {
                return Command::done();
            }
            // And an answer for a chain not pending is a duplicate — dropped.
            if !model.pending_probes.remove(&chain_id) {
                return Command::done();
            }
            // The native coin is not a token. On a chain whose native balance
            // also answers an ERC-20 interface (Arc), that contract resolves
            // perfectly well — name, symbol, decimals — and listing it would
            // show one balance as two rows and double the total. Refused before
            // admission, and said out loud (`native_alias`) rather than
            // reported as "not found", which would be a different and untrue
            // reason.
            if is_native_alias(chain_id, &address) {
                model.native_alias = true;
                model.detection.insert(chain_id, None);
            } else {
                // Admission (invariant ②): a missing name or symbol is a miss no
                // matter what the shell sent — the one gate for `!name || !symbol`.
                model
                    .detection
                    .insert(chain_id, meta.filter(admissible_erc20_meta));
            }
            if model.pending_probes.is_empty() {
                model.detect_in_flight = false;
                if model.detection.values().all(Option::is_none) {
                    model.not_found = true;
                }
            }
            render()
        }

        MtokShellResult::CustomTokensLoaded { tokens } => {
            // Every read refreshes the mirror — initial load and save-time
            // re-read hit the same storage, so either answer is current.
            model.customs_loaded = true;
            model.custom_tokens = tokens;
            let SavePhase::Checking { token } = model.save_phase.clone() else {
                return render();
            };
            if model
                .custom_tokens
                .iter()
                .any(|existing| existing.id == token.id)
            {
                // Already stored (by an earlier session or the auto-add
                // path): mark added, save nothing (`AddTokenPanel.tsx:193-196`).
                model.added_token_ids.insert(token.id);
                model.save_phase = SavePhase::Idle;
                return render();
            }
            model.save_phase = SavePhase::Writing {
                token: token.clone(),
            };
            request(model, MtokOperation::WriteCustomToken { token })
        }

        MtokShellResult::Saved => {
            let SavePhase::Writing { token } = std::mem::take(&mut model.save_phase) else {
                return Command::done();
            };
            model.added_token_ids.insert(token.id.clone());
            // Mirror the storage semantics: replace-by-id, then append.
            model
                .custom_tokens
                .retain(|existing| existing.id != token.id);
            model.custom_tokens.push(token);
            // Invariant ③ — the picker must see the token NOW, not after
            // the 5-minute fetchTokens TTL.
            request(model, MtokOperation::InvalidateTokenCache)
        }

        MtokShellResult::SaveFailed => {
            if !matches!(model.save_phase, SavePhase::Writing { .. }) {
                return Command::done();
            }
            model.save_phase = SavePhase::Idle;
            // Not marked added — the card keeps its save button, so the user
            // can retry, exactly as after today's alert.
            model.save_error = true;
            render()
        }

        MtokShellResult::Removed { id } => {
            if !model.pending_deletes.remove(&id) {
                return Command::done();
            }
            model.custom_tokens.retain(|token| token.id != id);
            // `added_token_ids` deliberately keeps the id — ported verbatim
            // (see Model::added_token_ids). The host's onChanged refresh
            // becomes the same invalidation the save path issues.
            request(model, MtokOperation::InvalidateTokenCache)
        }

        MtokShellResult::RemoveFailed { id } => {
            // Fail-closed for the TS unhandled rejection: the row stays (the
            // storage write never happened) and the delete may be retried.
            model.pending_deletes.remove(&id);
            render()
        }

        // An invalidation acknowledged. Never changes state.
        MtokShellResult::CacheInvalidated => Command::done(),
    }
}

// ---------------------------------------------------------------------------
// Pure policy (pub where another machine must share the rule)
// ---------------------------------------------------------------------------

/// THE `${chainId}_${addr.toLowerCase()}` key — invariant ①'s single
/// implementation. `AddTokenPanel.tsx:188` and `token-autoadd.ts:69` each
/// build this string by hand today; every writer converges here. The landed
/// `token_trust` machine still formats it inline over an already-lowercased
/// addr (byte-identical); rewiring it onto this fn is a follow-up outside
/// this wave's write fence.
pub fn token_id(chain_id: u32, address: &str) -> String {
    format!("{chain_id}_{}", address.to_lowercase())
}

/// Invariant ② — a token whose name or symbol did not resolve is never
/// listed, so nothing can render as the `'?'` fallback. Verbatim
/// `if (!name || !symbol) return null` (`AddTokenPanel.tsx:59`); a literal
/// `"?"` symbol from a contract passes today and still passes.
pub fn admissible_erc20_meta(meta: &MtokTokenMeta) -> bool {
    !meta.name.is_empty() && !meta.symbol.is_empty()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Forget the current probe entirely: cards, aggregate, pending answers.
fn clear_probe(model: &mut Model) {
    model.probe_order.clear();
    model.pending_probes.clear();
    model.detection.clear();
    model.detect_in_flight = false;
}

/// Issue one operation whose answer must match the current attempt.
fn request(model: &Model, operation: MtokOperation) -> Command<MtokEffect, Event> {
    let attempt = model.attempt;
    Command::all([
        Command::request_from_shell(operation)
            .then_send(move |result| Event::ShellCompleted { attempt, result }),
        render(),
    ])
}

impl super::SplitEffect for MtokEffect {
    type Op = MtokOperation;
    fn into_shell(self) -> Option<crux_core::Request<MtokOperation>> {
        match self {
            MtokEffect::Render(_) => None,
            MtokEffect::Shell(request) => Some(request),
        }
    }
}
