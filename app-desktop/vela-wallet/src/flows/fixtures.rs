//! Canonical wallet-flow fixtures (spec 021) — the desktop port of the web's
//! `src/lib/flows/fixtures.ts`, byte-for-byte the same canon.
//!
//! Pure data plus assembly: nothing here fetches, signs, formats a number or
//! decides a business rule. Chain colours are fixture DATA, not theme tokens,
//! exactly as spec 015's are.
//!
//! Where a mock invented content the product already has a canon for, the canon
//! wins: the contact picker uses spec 018's roster and every address is spec
//! 015's or spec 018's, so identicon artwork matches across features and across
//! clients.

use gpui::{Hsla, SharedString, rgb};

use crate::wallet::fill;
use crate::wallet::fixtures::{
    self as wallet, ActivityKind, ActivityRowModel, AssetRowModel, Fiat, chain_arbitrum,
    chain_base, chain_bnb, chain_ethereum, chain_gnosis, chain_polygon,
};

use super::{FlowPanel, FlowStrings};

// -- Canon ------------------------------------------------------------------

/// The receive list is the first screen to draw all eight supported networks,
/// not just the six the home holds balances on.
pub fn chain_optimism() -> Hsla {
    rgb(0xff0420).into()
}
pub fn chain_avalanche() -> Hsla {
    rgb(0xe84142).into()
}

pub struct NetworkFixture {
    pub name: &'static str,
    pub code: &'static str,
    pub color: fn() -> Hsla,
    pub chain_id: &'static str,
}

/// The eight supported networks, in the order R1 lists them.
pub const NETWORKS: [NetworkFixture; 8] = [
    NetworkFixture {
        name: "Ethereum",
        code: "ETH",
        color: chain_ethereum,
        chain_id: "1",
    },
    NetworkFixture {
        name: "BNB Chain",
        code: "BNB",
        color: chain_bnb,
        chain_id: "56",
    },
    NetworkFixture {
        name: "Polygon",
        code: "POL",
        color: chain_polygon,
        chain_id: "137",
    },
    NetworkFixture {
        name: "Arbitrum",
        code: "ARB",
        color: chain_arbitrum,
        chain_id: "42161",
    },
    NetworkFixture {
        name: "Optimism",
        code: "OP",
        color: chain_optimism,
        chain_id: "10",
    },
    NetworkFixture {
        name: "Base",
        code: "BASE",
        color: chain_base,
        chain_id: "8453",
    },
    NetworkFixture {
        name: "Avalanche",
        code: "AVAX",
        color: chain_avalanche,
        chain_id: "43114",
    },
    NetworkFixture {
        name: "Gnosis",
        code: "GNO",
        color: chain_gnosis,
        chain_id: "100",
    },
];

/// USDT on Ethereum — the real contract, as the mocks print it.
pub const USDT_CONTRACT: &str = "0xdAC17F958D2ee523a2206206994597C13D831ec7";
pub const USDT_CONTRACT_SHORT: &str = "0xdAC1…1ec7";

pub const ADDRESS_DISPLAY: &str = "0x14fB1f…D1eA5c";

// Spec 018's roster, reused rather than re-invented.
pub const ALICE_DISPLAY: &str = "0x9F3c…21aE";
pub const ALICE_FULL: &str = "0x9F3cA71b04E82f5C55d9B21aE00734F8Dd8021aE";
pub const A_HAO_FULL: &str = "0x77Bd59A302cC93D23dB0d0BA6a45C6830EF74F02";
pub const HOLD_ON_DISPLAY: &str = "0xCafe…F00d";
pub const HOLD_ON_FULL: &str = "0xCafe9078B1c2A04d33Ff21B0BC934eB8A812F00d";

const TX_HASH_RECEIVED: &str = "0x8f3a…c21d";
const TX_HASH_SENT: &str = "0x3c2d…8e1f";

/// Split a 0x address into the two lines the mocks wrap it into.
/// 42 characters, so 21 and 21 — an even break rather than one that leaves a
/// stub on the second line.
pub fn address_lines(address: &str) -> (String, String) {
    let half = address.len().div_ceil(2);
    let (a, b) = address.split_at(half);
    (a.to_string(), b.to_string())
}

// -- Models -----------------------------------------------------------------

#[derive(Clone)]
pub struct TokenMark {
    pub ticker: SharedString,
    pub badge: Hsla,
    /// The endpoint's logos for this coin (issue 201). Empty on a drawn mark:
    /// the gallery shows the glyph, which is the documented fallback.
    pub logos: crate::marks::Logos,
}

#[derive(Clone)]
pub enum FactLead {
    None,
    Token(TokenMark),
    Identicon(SharedString),
}

/// The single label-value row for the whole feature — DA2L's transaction
/// facts, DSD3L's summary and DT3bL's chain facts are the same row with
/// different leading art.
#[derive(Clone)]
pub struct FactRow {
    pub label: SharedString,
    pub value: SharedString,
    pub lead: FactLead,
    /// Renders the value in the mono face (addresses, hashes).
    pub mono: bool,
    /// What the row's copy button puts on the clipboard — the whole address
    /// or hash, not the shortened one the row shows (the web's
    /// `copyValue`). `None` draws no button.
    pub copy: Option<SharedString>,
    /// One calm sentence under the row, saying why its value is what it is —
    /// the confirm's speed row uses it for a speed taken because it was free
    /// (issue 686), so the tier and its reason reach the last screen together.
    pub note: Option<SharedString>,
}

/// The chip's tone, matching the other three clients' vocabulary.
///
/// The desktop column hosts nineteen of the feature's thirty states, and the
/// ones that carry a warning, an error or a neutral note — the incompatible
/// chain in T3b, the pending and failed transactions in A2 — are in the mobile
/// matrix. The tone stays complete here so the chip does not have to be taught
/// them again when a desktop panel grows one.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code)]
pub enum StatusTone {
    Success,
    Warning,
    Error,
    Info,
}

#[derive(Clone)]
pub struct StatusChip {
    pub text: SharedString,
    pub tone: StatusTone,
}

#[derive(Clone)]
pub struct NetworkRow {
    pub name: SharedString,
    pub code: SharedString,
    pub badge: Hsla,
    pub address: SharedString,
    /// The whole address, for the row's copy button.
    pub address_full: SharedString,
}

#[derive(Clone)]
pub struct AddressCard {
    pub name: SharedString,
    pub seed: SharedString,
    pub lines: (SharedString, SharedString),
}

#[derive(Clone)]
pub struct ReceiveList {
    pub subtitle: SharedString,
    pub search_placeholder: SharedString,
    /// Template carrying `{{query}}`: a search that hides every network.
    pub empty_text: String,
    pub rows: Vec<NetworkRow>,
}

/// One arrival, as the receive screen announces it.
///
/// Ported from `screens/wallet/ReceiveScreen.tsx` (the `depositBox` group): an
/// open, de-boxed section under a hairline, a success dot beside a muted time,
/// then one row per token with the amount in success ink and the network and
/// value muted beside it.
#[derive(Clone)]
pub struct DepositEntry {
    pub time: SharedString,
    /// `(+1.5 xDAI, Gnosis  $1.50)` — the amount, then its context.
    pub rows: Vec<(SharedString, SharedString)>,
}

/// The pre-receive warning, in the corpus's own words.
#[derive(Clone)]
pub struct ReceiveGate {
    pub title: SharedString,
    pub body: SharedString,
    /// Why one address works everywhere — the sentence that stops somebody
    /// hunting for a per-network address they do not need.
    pub counterfactual: SharedString,
    pub confirm: SharedString,
    /// The flag is still being read: draw the cover, but not the button. A
    /// button that appears a frame later is one somebody clicks twice.
    pub loading: bool,
}

#[derive(Clone)]
pub struct ReceiveQr {
    pub title: SharedString,
    /// DR3L only: the token's contract, above the account card.
    pub contract: Option<(SharedString, SharedString)>,
    pub account: AddressCard,
    /// The mark drawn in the middle of the code — the token, or the network.
    pub centre: TokenMark,
    pub warning: SharedString,
    pub save_image: SharedString,
    pub view_on_explorer: SharedString,
    /// Where "View on Explorer" leads — this account's page on the chain's
    /// explorer (the web's `explorerAddressURL`). `None` in the mocks.
    pub explorer_url: Option<SharedString>,
    /// The whole contract, for the copy beside the contract line — the line
    /// itself is shortened. `None` for the chain's own coin and the mocks.
    pub contract_copy: Option<SharedString>,
    /// What the code actually encodes.
    ///
    /// `None` in every mock, and that is why the gallery still draws the
    /// designed pattern. A signed-in receive screen carries the real payload —
    /// the core's `qr_value` — because a decorative code on a screen whose
    /// whole job is to be scanned is a screen that does not work.
    pub qr_payload: Option<SharedString>,
    /// The warning a person must read before the code is shown.
    ///
    /// `Some` while `payment_request` says the account has not acknowledged it
    /// (and while the flag is still being read, so a first visit never flashes
    /// the code). The gate is the reason `can_copy` and `can_save` exist: a
    /// screen whose whole job is to hand an address over must first say which
    /// networks that address is safe on.
    pub gate: Option<ReceiveGate>,
    /// May the address be copied yet? The core's `can_copy`.
    pub can_copy: bool,
    /// Money that landed while this code was open, newest first.
    ///
    /// Empty in every mock, because the mocks draw the screen before anything
    /// has arrived — this is a state only a live wallet reaches, like the
    /// hero's real figure. A field, not a separate panel: the person is looking
    /// at the code when it happens and must not have to go anywhere.
    pub deposits: Vec<DepositEntry>,
}

#[derive(Clone)]
pub struct HistoryGroup {
    pub label: SharedString,
    pub rows: Vec<ActivityRowModel>,
}

/// DA1L — the history, in one of the web's three modes: still loading
/// (skeleton rows), rows, or empty (one quiet line).
#[derive(Clone)]
pub struct HistoryPanel {
    pub groups: Vec<HistoryGroup>,
    /// Nothing to draw yet and the core has not ruled: skeleton rows.
    pub loading: bool,
    /// Nothing to draw and the core HAS ruled: the line that says so. `None`
    /// draws nothing at all (a record that vanished under an open detail).
    pub empty: Option<SharedString>,
}

#[derive(Clone)]
pub struct TxDetail {
    pub title: SharedString,
    pub status: StatusChip,
    /// Spec 038 #D2 — a folded batch row: its parts, under the facts.
    pub breakdown_title: Option<SharedString>,
    pub breakdown: Vec<BreakdownRow>,
    pub amount: SharedString,
    pub fiat: SharedString,
    pub positive: bool,
    pub facts: Vec<FactRow>,
    pub view_on_explorer: SharedString,
    /// The transaction's page on its chain's explorer (the web's
    /// `explorerTxURL`). `None` without a hash, and in the mocks.
    pub explorer_url: Option<SharedString>,
    /// The record's delete (the web's `deleteLabel`): removes the local
    /// record, not the transaction. `None` in the mocks, which have nothing
    /// to delete.
    pub delete_label: Option<SharedString>,
}

#[derive(Clone)]
pub struct AssetsEmpty {
    pub title: SharedString,
    pub caption: SharedString,
    pub cta: SharedString,
    pub hint_title: SharedString,
    pub hint_body: SharedString,
}

#[derive(Clone)]
pub struct AssetsPanel {
    /// DT1L's filter row: the chain pill and the add action beside it.
    pub filter: Option<(Vec<Hsla>, SharedString, SharedString)>,
    pub search_placeholder: SharedString,
    pub rows: Vec<AssetRowModel>,
    /// A search that hides every row.
    pub no_match: SharedString,
    pub add_by_address: SharedString,
    /// DT4L: the guided-empty body replaces the rows entirely.
    pub empty: Option<AssetsEmpty>,
}

#[derive(Clone)]
pub enum AddTokenResult {
    Token {
        mark: TokenMark,
        name: SharedString,
        detail: SharedString,
    },
    Network {
        mark: TokenMark,
        name: SharedString,
        chip: StatusChip,
        facts: Vec<FactRow>,
    },
}

#[derive(Clone)]
pub struct AddToken {
    pub tab_erc20: SharedString,
    pub tab_native: SharedString,
    pub native: bool,
    /// ERC-20 only: the network the contract is looked up on.
    pub network: Option<(TokenMark, SharedString)>,
    pub field_label: SharedString,
    pub field_value: SharedString,
    pub result: AddTokenResult,
    /// Live only: the write itself failed. A CTA that does nothing and says
    /// nothing is the same defect as a picker that silently drops a file.
    pub notice: Option<SendNotice>,
    pub cta: SharedString,
}

#[derive(Clone)]
pub struct FilterChip {
    pub label: SharedString,
    pub selected: bool,
}

#[derive(Clone)]
pub struct SendPick {
    pub search_placeholder: SharedString,
    /// The class chips. There is no network pill: on the desktop the
    /// sidebar's network filter is the one, and it narrows these rows too.
    pub filters: Vec<FilterChip>,
    pub rows: Vec<AssetRowModel>,
    /// A search that hides every row.
    pub no_match: SharedString,
    pub cta: SharedString,
    /// Spec 033 — the sweep picker (SD1b). `None` is the ordinary
    /// one-token list, which is what the mock draws.
    pub selection: Option<SendSelection>,
    /// The sweep CTA carries a count and wears the accent; the plain one is a
    /// quiet centred link. Two looks, one slot.
    pub cta_accent: bool,
}

/// Which rows a sweep has ticked, and which are on the wrong chain.
///
/// Off-chain rows are DIMMED rather than removed: the person still owns them,
/// and a list that silently shortened would read as a bug (SD1b's own note).
#[derive(Clone)]
pub struct SendSelection {
    pub selected: Vec<bool>,
    pub dimmed: Vec<bool>,
    pub select_all: SharedString,
    /// "Gnosis selected — a multi-token send stays on one network…", with the
    /// chain's own mark beside it. `None` until the first pick names a chain.
    pub notice: Option<(u32, SharedString, SharedString)>,
}

#[derive(Clone)]
pub struct RecipientCard {
    pub ordinal: SharedString,
    pub name: SharedString,
    pub seed: SharedString,
    pub amount: SharedString,
    /// Live only: what the core says about this row — a repeat of an earlier
    /// payee (issue 203), an address or an amount it will not take. Empty
    /// for a row that is fine, and for an unfinished one: an empty field is
    /// not a mistake.
    pub notes: Vec<SharedString>,
}

#[derive(Clone)]
pub struct FeeRow {
    pub label: SharedString,
    pub mark: TokenMark,
    pub value: SharedString,
    /// The refresh control's accessible name (spec 068) — `None` draws none.
    /// The fee is the one figure on the form that moves on its own, and until
    /// 069 a person on the desktop could not re-read it.
    pub refresh: Option<SharedString>,
    /// A measurement is out: the control says so, so a tap is never
    /// ambiguous.
    pub refreshing: bool,
    /// The quote's 30 s TTL elapsed (`FeeView.stale`) — calm wording, never
    /// a fault. `None` keeps the line's room empty so nothing jumps.
    pub stale_note: Option<SharedString>,
}

/// One option of the speed control (spec 068).
#[derive(Clone)]
pub struct FeeSpeedOption {
    /// The SPEED — 超快 / 标准 / 较慢 — never a number.
    pub label: SharedString,
    /// What that speed buys, one line under the name.
    pub detail: SharedString,
    /// This option's OWN fee, or the "…" / "—" standing in for it.
    pub value: SharedString,
    /// Its gas bid as a range, already formatted by the core over the set.
    pub gas_price: Option<SharedString>,
    pub selected: bool,
}

/// The speed control under the fee row, folded until opened (spec 068). Every
/// decision in it is the `fee_speed` core's (spec 069).
#[derive(Clone)]
pub struct FeeSpeedModel {
    pub label: SharedString,
    /// The folded summary: the tier in force for THIS send.
    pub value: SharedString,
    pub open: bool,
    pub once_note: SharedString,
    /// Why the tier in force is the fastest when the default is slower.
    pub free_note: Option<SharedString>,
    /// This network has one speed: the options give way to it.
    pub single_note: Option<SharedString>,
    pub gas_price_label: SharedString,
    /// Whether the options carry a gas-bid line at all.
    pub gas_price_line: bool,
    pub options: Vec<FeeSpeedOption>,
}

/// DSD2L and DSD2bL. Split mode is `!recipients.is_empty()`: the mode IS the
/// list of payees, and a flag beside it could disagree with it.
/// What the core refused, said out loud (spec 032 phase 6).
///
/// One shape for every refusal on the send journey — a live amount warning,
/// the same-asset fee ceiling, a split over balance, an unfulfillable locked
/// request, and the relay's empty float. `action` is the way out the core
/// offers (edit the amount, add the network, check the top-up again); a
/// notice with no action is a statement, not a dead end.
#[derive(Clone)]
pub struct SendNotice {
    pub title: Option<SharedString>,
    pub body: SharedString,
    /// A second line the notice needs: the ceiling's "you can send up to X",
    /// or the top-up address.
    pub detail: Option<SharedString>,
    pub action: Option<SharedString>,
    /// The way out of the notice itself, when it has one — today only the
    /// relay-treasury stop, whose "Close" puts a person back on the form
    /// instead of leaving them staring at a top-up address until it clears.
    pub dismiss: Option<SharedString>,
    /// Red rather than amber: the person cannot proceed as things stand.
    pub error: bool,
}

/// A CTA's three states — the founder's rule: busy is not disabled, and a
/// button the core shut is drawn shut rather than left looking live.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CtaState {
    Enabled,
    Disabled,
    Busy,
}

#[derive(Clone)]
pub struct SendForm {
    pub token: (TokenMark, SharedString, SharedString, Option<SharedString>),
    pub amount: Option<(SharedString, SharedString)>,
    pub recipient: Option<(SharedString, (SharedString, SharedString), SharedString)>,
    pub add_recipient: Option<SharedString>,
    pub recipients: Vec<RecipientCard>,
    pub recipient_actions: Vec<SharedString>,
    pub summary: Option<(SharedString, SharedString)>,
    /// Live only, split: what the balance has left to give out (the core's
    /// `split_remaining`, #265), and whether the rows already outrun it —
    /// the total then draws in the error ink.
    pub remaining: Option<SharedString>,
    pub summary_over: bool,
    /// Live only, split: "Use 0.5 ETH for the empty rows" (the web's
    /// `fillEmpty`) — offered while it would do something.
    pub fill_empty: Option<SharedString>,
    /// Live only: the unit the figure is typed in — the fiat code while the
    /// person types money, the token otherwise (#231). `None` keeps the
    /// token's symbol as the field's label.
    pub amount_unit: Option<SharedString>,
    /// Live only: the ⇄ swap under the figure (#197) — `Some` where the core
    /// offers it, `Some(false)` when it is offered but would change nothing.
    pub denom_toggle: Option<bool>,
    /// Live only: the pill that opens the address book beside a typed field.
    /// The mock's recipient card opens the picker itself, so it has none.
    pub pick_contacts: Option<SharedString>,
    /// Live only: what the core refused, and the way out.
    pub notice: Option<SendNotice>,
    pub fee: FeeRow,
    /// The speed control (spec 068). `None` draws none — a surface with no
    /// quote sessions behind it must not offer a choice it cannot honour.
    pub speed: Option<Box<FeeSpeedModel>>,
    pub cta: SharedString,
    /// The core's `can_continue`, plus the pre-check's busy state. The mock's
    /// button is always armed.
    pub cta_state: CtaState,
}

#[derive(Clone)]
pub struct ContactEntry {
    pub name: SharedString,
    pub group: Option<SharedString>,
    pub address: SharedString,
    pub seed: SharedString,
}

#[derive(Clone)]
pub struct ContactPick {
    pub search_placeholder: SharedString,
    pub scan_row: SharedString,
    pub groups_title: SharedString,
    pub groups: Vec<(SharedString, SharedString, Hsla, Hsla)>,
    pub contacts_title: SharedString,
    pub contacts: Vec<ContactEntry>,
}

#[derive(Clone)]
pub struct FeeTokenRow {
    pub mark: TokenMark,
    pub symbol: SharedString,
    pub balance: SharedString,
    pub fee: SharedString,
    pub selected: bool,
    /// The core's balance<fee gate (invariant ⑧): shown for context, NOT
    /// selectable — paying gas in it would only produce a doomed operation.
    pub insufficient: bool,
}

#[derive(Clone)]
pub struct FeeTokenPick {
    pub hint: SharedString,
    pub estimate_label: SharedString,
    pub rows: Vec<FeeTokenRow>,
}

#[derive(Clone)]
pub struct BatchRow {
    pub ok: bool,
    pub address: SharedString,
    pub conversion: SharedString,
    /// Live only: why the row will not be paid — a duplicate, an address
    /// that is not one, a line the parser refused.
    pub note: Option<SharedString>,
}

/// DSD2cL's total line — the web's `total`: what the import sends, in the
/// token and (for a fiat sheet) in the sheet's own currency, against what
/// the account holds.
#[derive(Clone)]
pub struct BatchTotal {
    pub label: SharedString,
    pub value: SharedString,
    pub detail: Option<SharedString>,
    pub balance: SharedString,
    /// The core's `over_balance`, worded; the total then draws in error ink.
    pub over: Option<SharedString>,
}

#[derive(Clone)]
pub struct BatchImport {
    pub unit_fiat: SharedString,
    pub unit_token: SharedString,
    /// Which half of the unit toggle is on. The mock shows fiat.
    pub fiat_on: bool,
    pub paste: SharedString,
    pub import_file: SharedString,
    pub template: SharedString,
    pub rate_section: SharedString,
    pub rate_value: SharedString,
    pub rate_hint: SharedString,
    pub parsed: SharedString,
    /// Live only: the total line (`None` until a row parses).
    pub total: Option<BatchTotal>,
    pub rows: Vec<BatchRow>,
    pub rejected: SharedString,
    /// Live only: the over-cap / over-balance / unreadable-file notice,
    /// beside `rejected` (the two never hide each other).
    pub notice: Option<SendNotice>,
    /// Live only: the "Auto" affordance once the rate was edited by hand.
    pub rate_reset: Option<SharedString>,
    /// Live only: what the import does to the rows already on the form and
    /// the way to choose the other — (sentence, action). `None` when the form
    /// is empty or the import cannot apply (issue #265).
    pub merge: Option<(SharedString, SharedString)>,
    /// The apply gate is the core's; the mock's button is always armed.
    pub cta_enabled: bool,
    pub cta: SharedString,
}

#[derive(Clone)]
pub struct BreakdownRow {
    /// A recipient's address, for the avatar beside the name (spec 038 #D2);
    /// `None` for an asset row, which carries no person.
    pub seed: Option<SharedString>,
    pub label: SharedString,
    pub value: SharedString,
}

#[derive(Clone)]
pub struct SendConfirm {
    /// The coin being sent, drawn above the figure (founder, 2026-09-17). The
    /// confirm page named the asset in words only while every row beneath it
    /// carried art. `None` on a sweep: several coins, no one mark.
    pub mark: Option<TokenMark>,
    pub amount: SharedString,
    pub subline: SharedString,
    pub facts: Vec<FactRow>,
    pub breakdown: Vec<BreakdownRow>,
    /// "First time sending here" — the anti-poisoning tell, on the page that
    /// signs. The core resolves it only while this page is up
    /// (`confirm_probes`), so the form never has it to show. Live only;
    /// `None` on a split.
    pub recipient_tag: Option<SharedString>,
    /// Live only: why the slide is disarmed, when something disarmed it.
    pub notice: Option<SendNotice>,
    pub cta: SharedString,
    /// The core's `can_confirm`, plus signing / submitting.
    pub cta_state: CtaState,
}

#[derive(Clone)]
pub struct SendReceipt {
    /// Which of the four the receipt is in — the disc's colour and mark
    /// (078 F-04, the web's `StatusHero`).
    pub stage: ReceiptStage,
    /// 0–1, how much of the ring round the disc is drawn while submitted;
    /// `None` without an estimate for the chain, and the ring roams instead.
    pub progress: Option<f32>,
    /// "View on Explorer" and where it leads — once there is a hash.
    pub explorer: Option<(SharedString, SharedString)>,
    /// The CTA is the accent "Done" once confirmed; before that it is the
    /// quiet "Close · keep running".
    pub cta_accent: bool,
    pub title: SharedString,
    pub captions: Vec<SharedString>,
    /// Spec 038 #D2 — a split: "N recipients", then every one of them, on
    /// the receipt as on the confirm.
    pub breakdown_title: Option<SharedString>,
    pub breakdown: Vec<BreakdownRow>,
    pub hash: Option<(SharedString, SharedString)>,
    pub cta: SharedString,
}

#[derive(Clone)]
pub struct ScanModal {
    pub title: SharedString,
    pub hint: SharedString,
    /// A desktop webcam has no torch, so the modal offers two tools where the
    /// phone offers three.
    pub tools: Vec<SharedString>,
}

/// The receipt's four states (`ReceiptStage` on the web).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReceiptStage {
    Submitting,
    Submitted,
    Confirmed,
    Failed,
}

/// Everything a flow panel can hold. The page matches on this, so a panel body
/// and its model can never drift apart.
#[derive(Clone)]
pub enum FlowBody {
    Receive(ReceiveList),
    ReceiveQr(ReceiveQr),
    History(HistoryPanel),
    TxDetail(TxDetail),
    Assets(AssetsPanel),
    AddToken(AddToken),
    SendPick(SendPick),
    SendForm(SendForm),
    ContactPick(ContactPick),
    FeeToken(FeeTokenPick),
    BatchImport(BatchImport),
    SendConfirm(SendConfirm),
    SendReceipt(SendReceipt),
    /// DS1L is drawn as a centred modal, not in the column.
    Scan(ScanModal),
}

// -- Assembly ---------------------------------------------------------------

fn mark(ticker: &str, badge: Hsla) -> TokenMark {
    TokenMark {
        ticker: ticker.into(),
        badge,
        logos: crate::marks::Logos::default(),
    }
}

fn fact(label: &SharedString, value: impl Into<SharedString>) -> FactRow {
    FactRow {
        label: label.clone(),
        value: value.into(),
        lead: FactLead::None,
        mono: false,
        copy: None,
        note: None,
    }
}

fn asset(ticker: &str, chain: &str, badge: Hsla, balance: &str, fiat: &str) -> AssetRowModel {
    AssetRowModel {
        ticker: ticker.into(),
        chain: chain.into(),
        badge,
        balance: balance.into(),
        fiat: Fiat::Value(fiat.into()),
        logos: crate::marks::Logos::default(),
    }
}

/// The assets DT1L lists, verbatim from the mock.
fn assets_rows() -> Vec<AssetRowModel> {
    vec![
        asset("BNB", "BNB Chain", chain_bnb(), "0.8533", "$496.46"),
        asset("ETH", "Arbitrum", chain_arbitrum(), "0.2253", "$422.62"),
        asset("ETH", "Ethereum", chain_ethereum(), "0.0689", "$129.25"),
        asset("XDAI", "Gnosis", chain_gnosis(), "74.3965", "$74.38"),
        asset("USDT", "Ethereum", chain_ethereum(), "53.4836", "$53.48"),
        asset("USDC", "Polygon", chain_polygon(), "12.04", "$12.04"),
    ]
}

/// DSD1L's order differs from DT1L's: the picker leads with what you'd send.
fn send_rows() -> Vec<AssetRowModel> {
    vec![
        asset("USDT", "Ethereum", chain_ethereum(), "53.4836", "$53.48"),
        asset("ETH", "Ethereum", chain_ethereum(), "0.0689", "$129.25"),
        asset("USDC", "Ethereum", chain_ethereum(), "18.20", "$18.20"),
        asset("BNB", "BNB Chain", chain_bnb(), "0.8533", "$496.46"),
        asset("XDAI", "Gnosis", chain_gnosis(), "74.3965", "$74.38"),
    ]
}

fn receive_list(s: &FlowStrings) -> ReceiveList {
    ReceiveList {
        subtitle: fill(
            &s.networks_line,
            "count",
            &wallet::NETWORK_COUNT.to_string(),
        )
        .into(),
        search_placeholder: s.receive_search.clone(),
        empty_text: s.search_empty.clone(),
        rows: NETWORKS
            .iter()
            .map(|n| NetworkRow {
                name: n.name.into(),
                code: n.code.into(),
                badge: (n.color)(),
                address: ADDRESS_DISPLAY.into(),
                address_full: crate::wallet::fixtures::ADDRESS_FULL.into(),
            })
            .collect(),
    }
}

fn receive_qr(s: &FlowStrings, asset_mode: bool) -> ReceiveQr {
    let network = &NETWORKS[0];
    let lines = address_lines(wallet::ADDRESS_FULL);
    ReceiveQr {
        title: if asset_mode {
            fill(
                &fill(&s.qr_title_asset, "network", network.name),
                "symbol",
                "USDT",
            )
            .into()
        } else {
            fill(&s.qr_title_network, "network", network.name).into()
        },
        contract: asset_mode.then(|| (s.token_contract.clone(), USDT_CONTRACT_SHORT.into())),
        account: AddressCard {
            name: wallet::WALLET_NAME.into(),
            seed: wallet::ADDRESS_FULL.into(),
            lines: (lines.0.into(), lines.1.into()),
        },
        centre: if asset_mode {
            mark("USDT", chain_gnosis())
        } else {
            mark(network.code, (network.color)())
        },
        warning: s.warning_reminder.clone(),
        save_image: s.save_image.clone(),
        view_on_explorer: s.view_on_explorer.clone(),
        explorer_url: None,
        contract_copy: None,
        // The mocks draw the design, not a wallet: no payload, so the card
        // keeps the pattern the drawing shows.
        qr_payload: None,
        // The gallery draws the code, not the cover: the drawn scenarios are
        // reviewed for the code's own composition, and a live screen shows
        // the warning first (spec 032 phase 38).
        gate: None,
        can_copy: false,
        // The mocks draw the screen before anything has arrived.
        deposits: Vec::new(),
    }
}

fn history(s: &FlowStrings) -> Vec<HistoryGroup> {
    let row = |kind: ActivityKind,
               title: &SharedString,
               subtitle: String,
               amount: &str,
               unit: &str,
               positive: bool,
               badge: Hsla| ActivityRowModel {
        kind,
        title: title.clone(),
        subtitle: subtitle.into(),
        amount: amount.into(),
        unit: unit.into(),
        positive,
        badge,
        badge_logo: None,
        day: None,
    };
    let to = |name: &str, clock: &str| format!("{} · {clock}", fill(&s.to_name, "name", name));
    let from = |name: &str, clock: &str| format!("{} · {clock}", fill(&s.from_name, "name", name));

    vec![
        HistoryGroup {
            label: s.today.clone(),
            rows: vec![
                row(
                    ActivityKind::Sent,
                    &s.label_sent,
                    to("hold on", "14:02"),
                    "−2",
                    "POL",
                    false,
                    chain_polygon(),
                ),
                row(
                    ActivityKind::Received,
                    &s.label_received,
                    from(ALICE_DISPLAY, "11:20"),
                    "+120",
                    "USDT",
                    true,
                    chain_ethereum(),
                ),
            ],
        },
        HistoryGroup {
            label: s.yesterday.clone(),
            rows: vec![
                row(
                    ActivityKind::Received,
                    &s.label_received,
                    from("Alice", "20:15"),
                    "+50",
                    "USDC",
                    true,
                    chain_base(),
                ),
                row(
                    ActivityKind::Sent,
                    &s.label_sent,
                    to("Bob", "09:12"),
                    "−0.4",
                    "XDAI",
                    false,
                    chain_gnosis(),
                ),
            ],
        },
        // A literal date once the run of named days ends — the mock's 8月12日.
        HistoryGroup {
            label: "8/12".into(),
            rows: vec![row(
                ActivityKind::Received,
                &s.label_received,
                from("0x21aE…9F3c", "08:44"),
                "+0.9",
                "BNB",
                true,
                chain_bnb(),
            )],
        },
    ]
}

fn tx_detail(s: &FlowStrings, received: bool) -> TxDetail {
    let network = if received { &NETWORKS[0] } else { &NETWORKS[2] };
    let mut facts = vec![
        FactRow {
            label: if received {
                s.detail_from.clone()
            } else {
                s.detail_to.clone()
            },
            value: if received {
                ALICE_DISPLAY.into()
            } else {
                "hold on".into()
            },
            lead: FactLead::Identicon(if received {
                ALICE_FULL.into()
            } else {
                HOLD_ON_FULL.into()
            }),
            mono: received,
            copy: Some(if received { ALICE_FULL } else { HOLD_ON_FULL }.into()),
            note: None,
        },
        FactRow {
            label: s.detail_chain.clone(),
            value: network.name.into(),
            lead: FactLead::Token(mark(network.code, (network.color)())),
            mono: false,
            copy: None,
            note: None,
        },
    ];
    // Only an ERC-20 transfer has a contract. DA3L's native coin does not, and
    // an empty row there invites "which contract?".
    if received {
        facts.push(FactRow {
            // 代币合约, not 合约: the token panel is already about a token, so
            // it says "contract"; a transaction has to say WHICH contract.
            label: s.token_contract.clone(),
            value: USDT_CONTRACT_SHORT.into(),
            lead: FactLead::None,
            mono: true,
            copy: Some(USDT_CONTRACT.into()),
            note: None,
        });
    }
    facts.push(fact(
        &s.detail_date,
        format!("{} {}", s.today, if received { "11:20" } else { "14:02" }),
    ));
    facts.push(FactRow {
        label: s.detail_hash.clone(),
        value: if received {
            TX_HASH_RECEIVED.into()
        } else {
            TX_HASH_SENT.into()
        },
        lead: FactLead::None,
        mono: true,
        copy: Some(
            if received {
                TX_HASH_RECEIVED
            } else {
                TX_HASH_SENT
            }
            .into(),
        ),
        note: None,
    });

    TxDetail {
        breakdown_title: None,
        breakdown: Vec::new(),
        title: if received {
            fill(&s.tx_label_received, "symbol", "USDT").into()
        } else {
            fill(&s.tx_label_sent, "symbol", "POL").into()
        },
        status: StatusChip {
            text: s.status_confirmed.clone(),
            tone: StatusTone::Success,
        },
        amount: if received {
            "+120 USDT".into()
        } else {
            "−2 POL".into()
        },
        fiat: if received {
            "≈ $120.00".into()
        } else {
            "≈ $0.98".into()
        },
        positive: received,
        facts,
        view_on_explorer: s.view_on_explorer.clone(),
        explorer_url: None,
        delete_label: None,
    }
}

fn assets_panel(s: &FlowStrings, empty: bool) -> AssetsPanel {
    AssetsPanel {
        filter: Some((
            NETWORKS[..3].iter().map(|n| (n.color)()).collect(),
            s.pill_all.clone(),
            s.assets_add.clone(),
        )),
        search_placeholder: s.assets_search.clone(),
        no_match: s.no_matching_tokens.clone(),
        rows: if empty { Vec::new() } else { assets_rows() },
        add_by_address: s.add_by_address.clone(),
        empty: empty.then(|| AssetsEmpty {
            title: s.assets_empty_title.clone(),
            caption: s.assets_empty_caption.clone(),
            cta: s.add_token_title.clone(),
            hint_title: s.not_showing_title.clone(),
            hint_body: s.not_showing_body.clone(),
        }),
    }
}

fn add_token(s: &FlowStrings, native: bool) -> AddToken {
    let avax = &NETWORKS[6];
    AddToken {
        tab_erc20: s.tab_erc20.clone(),
        tab_native: s.tab_native.clone(),
        native,
        network: (!native).then(|| {
            (
                mark(NETWORKS[0].code, (NETWORKS[0].color)()),
                NETWORKS[0].name.into(),
            )
        }),
        field_label: if native {
            s.net_search_label.clone()
        } else {
            s.token_address_label.clone()
        },
        field_value: if native {
            "Avalanche".into()
        } else {
            USDT_CONTRACT.into()
        },
        result: if native {
            AddTokenResult::Network {
                mark: mark(avax.code, (avax.color)()),
                name: avax.name.into(),
                chip: StatusChip {
                    text: s.compatible.clone(),
                    tone: StatusTone::Success,
                },
                facts: vec![
                    fact(&s.label_chain_id, avax.chain_id),
                    fact(&s.label_native_token, avax.code),
                ],
            }
        } else {
            AddTokenResult::Token {
                mark: mark("USDT", chain_ethereum()),
                name: "Tether USD".into(),
                detail: format!("USDT · {} 6 · Ethereum", s.label_decimals).into(),
            }
        },
        notice: None,
        cta: if native {
            s.add_network_btn.clone()
        } else {
            s.add_to_wallet.clone()
        },
    }
}

fn send_pick(s: &FlowStrings) -> SendPick {
    SendPick {
        // The mock is the one-token list; the sweep is a live-only state.
        selection: None,
        cta_accent: false,
        search_placeholder: s.send_search.clone(),
        no_match: s.no_matching_tokens.clone(),
        filters: vec![
            FilterChip {
                label: s.filter_all.clone(),
                selected: true,
            },
            FilterChip {
                label: s.filter_stable.clone(),
                selected: false,
            },
            FilterChip {
                label: s.filter_gas.clone(),
                selected: false,
            },
            FilterChip {
                label: s.filter_other.clone(),
                selected: false,
            },
        ],
        rows: send_rows(),
        cta: s.multi_send_title.clone(),
    }
}

fn send_form(s: &FlowStrings, split: bool) -> SendForm {
    let balance = fill(&s.balance_label, "amount", "53.4836");
    let fee = FeeRow {
        label: s.network_fee.clone(),
        mark: mark("ETH", chain_ethereum()),
        value: if split {
            "0.0034 ETH · ≈$0.89".into()
        } else {
            "0.0021 ETH · ≈$0.55".into()
        },
        refresh: Some(s.fee_refresh.clone()),
        refreshing: false,
        stale_note: None,
    };
    // Folded, as every send starts: the word and the tier in force.
    let speed = FeeSpeedModel {
        label: s.fee_speed_label.clone(),
        value: s.gas_tier_fast.clone(),
        open: false,
        once_note: s.fee_speed_once.clone(),
        free_note: None,
        single_note: None,
        gas_price_label: s.gas_price_label.clone(),
        gas_price_line: false,
        options: Vec::new(),
    };
    let token = (
        mark("USDT", chain_ethereum()),
        SharedString::from("USDT"),
        SharedString::from(format!("Ethereum · {balance}")),
        (!split).then(|| s.max.clone()),
    );
    let alice_lines = address_lines(ALICE_FULL);

    if split {
        return SendForm {
            token,
            amount: None,
            recipient: None,
            add_recipient: None,
            recipients: vec![
                RecipientCard {
                    ordinal: fill(&s.recipient_n, "n", "1").into(),
                    name: ALICE_DISPLAY.into(),
                    seed: ALICE_FULL.into(),
                    amount: "50".into(),
                    notes: Vec::new(),
                },
                RecipientCard {
                    ordinal: fill(&s.recipient_n, "n", "2").into(),
                    name: "Alice".into(),
                    seed: A_HAO_FULL.into(),
                    amount: "30".into(),
                    notes: Vec::new(),
                },
                RecipientCard {
                    ordinal: fill(&s.recipient_n, "n", "3").into(),
                    name: "hold on".into(),
                    seed: HOLD_ON_FULL.into(),
                    amount: "40".into(),
                    notes: Vec::new(),
                },
            ],
            recipient_actions: vec![
                s.add_recipient.clone(),
                s.from_contacts.clone(),
                s.batch_import.clone(),
            ],
            summary: Some((
                format!(
                    "{} · {}",
                    s.split_total,
                    fill(&s.recipient_count, "count", "3")
                )
                .into(),
                "120 USDT · ≈$120.00".into(),
            )),
            remaining: None,
            summary_over: false,
            fill_empty: None,
            amount_unit: None,
            denom_toggle: None,
            pick_contacts: None,
            notice: None,
            fee,
            speed: Some(Box::new(speed.clone())),
            cta: s.continue_btn.clone(),
            cta_state: CtaState::Enabled,
        };
    }

    SendForm {
        token,
        amount: Some(("120".into(), "≈ $120.00".into())),
        recipient: Some((
            s.recipient_label.clone(),
            (alice_lines.0.into(), alice_lines.1.into()),
            ALICE_FULL.into(),
        )),
        add_recipient: Some(s.add_recipient.clone()),
        recipients: Vec::new(),
        recipient_actions: Vec::new(),
        summary: None,
        remaining: None,
        summary_over: false,
        fill_empty: None,
        amount_unit: None,
        denom_toggle: None,
        pick_contacts: None,
        notice: None,
        fee,
        speed: Some(Box::new(speed)),
        cta: s.continue_btn.clone(),
        cta_state: CtaState::Enabled,
    }
}

fn contact_pick(s: &FlowStrings) -> ContactPick {
    ContactPick {
        search_placeholder: s.pick_contact_search.clone(),
        scan_row: s.scan_to_fill.clone(),
        groups_title: s.contacts_groups.clone(),
        groups: vec![
            (
                "家人".into(),
                fill(&s.group_members, "count", "3").into(),
                chain_polygon(),
                chain_bnb(),
            ),
            (
                "工作".into(),
                fill(&s.group_members, "count", "5").into(),
                chain_gnosis(),
                chain_arbitrum(),
            ),
        ],
        contacts_title: s.contacts_title.clone(),
        contacts: vec![
            ContactEntry {
                name: "Alice".into(),
                group: Some("家人".into()),
                address: ALICE_DISPLAY.into(),
                seed: ALICE_FULL.into(),
            },
            ContactEntry {
                name: "阿豪".into(),
                group: None,
                address: "0x77Bd…4F02".into(),
                seed: A_HAO_FULL.into(),
            },
            ContactEntry {
                name: "hold on".into(),
                group: None,
                address: HOLD_ON_DISPLAY.into(),
                seed: HOLD_ON_FULL.into(),
            },
        ],
    }
}

fn fee_token(s: &FlowStrings) -> FeeTokenPick {
    let row = |ticker: &str, amount: &str, fee: &str, selected: bool| FeeTokenRow {
        mark: mark(ticker, chain_ethereum()),
        symbol: ticker.into(),
        balance: fill(&s.balance_label, "amount", amount).into(),
        fee: fee.into(),
        selected,
        insufficient: false,
    };
    FeeTokenPick {
        hint: s.fee_token_hint.clone(),
        estimate_label: s.fee_token_estimate.clone(),
        rows: vec![
            row("ETH", "0.0689", "~0.0021 ETH", true),
            row("USDC", "18.20", "~0.55 USDC", false),
            row("USDT", "53.4836", "~0.55 USDT", false),
        ],
    }
}

fn batch_import(s: &FlowStrings) -> BatchImport {
    BatchImport {
        unit_fiat: fill(&s.batch_unit_fiat, "code", "CNY").into(),
        unit_token: fill(&s.batch_unit_token, "sym", "USDT").into(),
        fiat_on: true,
        paste: "0xabc… , 5000\n0xdef… , 8000".into(),
        import_file: format!("{} (xlsx / csv / txt)", s.batch_import_file).into(),
        template: s.batch_template.clone(),
        rate_section: s.batch_rate_section.clone(),
        rate_value: format!("{} 7.25 CNY", fill(&s.batch_rate_label, "sym", "USDT")).into(),
        rate_hint: fill(&fill(&s.batch_rate_hint, "code", "CNY"), "sym", "USDT").into(),
        parsed: fill(&s.batch_parsed, "n", "3").into(),
        total: None,
        rows: vec![
            BatchRow {
                ok: true,
                address: ALICE_DISPLAY.into(),
                conversion: "5,000 CNY → 689.66".into(),
                note: None,
            },
            BatchRow {
                ok: true,
                address: "0x21aE…9F3c".into(),
                conversion: "8,000 CNY → 1,103.45".into(),
                note: None,
            },
            BatchRow {
                ok: false,
                address: format!("0x12zz…{}", s.batch_bad_address).into(),
                conversion: "—".into(),
                note: None,
            },
        ],
        rejected: fill(&s.batch_rejected_one, "count", "1").into(),
        // Two of three rows parsed, so the button offers two — never three.
        notice: None,
        rate_reset: None,
        merge: None,
        cta_enabled: true,
        cta: fill(&s.batch_apply, "count", "2").into(),
    }
}

fn send_confirm(s: &FlowStrings) -> SendConfirm {
    SendConfirm {
        mark: Some(mark("USDT", (NETWORKS[0].color)())),
        amount: "120 USDT".into(),
        subline: "≈ $120.00".into(),
        facts: vec![
            FactRow {
                label: s.from_label.clone(),
                value: wallet::WALLET_NAME.into(),
                lead: FactLead::Identicon(wallet::ADDRESS_FULL.into()),
                mono: false,
                copy: None,
                note: None,
            },
            FactRow {
                label: s.to_label.clone(),
                value: ALICE_DISPLAY.into(),
                lead: FactLead::Identicon(ALICE_FULL.into()),
                mono: true,
                copy: None,
                note: None,
            },
            FactRow {
                label: s.detail_chain.clone(),
                value: NETWORKS[0].name.into(),
                lead: FactLead::Token(mark(NETWORKS[0].code, (NETWORKS[0].color)())),
                mono: false,
                copy: None,
                note: None,
            },
            fact(&s.est_fee, "~0.0021 ETH · ≈$0.55"),
        ],
        breakdown: Vec::new(),
        recipient_tag: None,
        notice: None,
        cta: s.confirm_send.clone(),
        cta_state: CtaState::Enabled,
    }
}

fn send_receipt(s: &FlowStrings) -> SendReceipt {
    SendReceipt {
        stage: ReceiptStage::Submitted,
        // The mock has no estimate for its chain, so its ring roams — as the
        // web's `dsd4` does.
        progress: None,
        explorer: None,
        cta_accent: false,
        breakdown_title: None,
        breakdown: Vec::new(),
        title: s.tx_submitted_title.clone(),
        captions: vec![
            s.tx_waiting_confirm.clone(),
            fill(
                &fill(&s.tx_typical_time, "chainName", NETWORKS[0].name),
                "estSecs",
                "12",
            )
            .into(),
        ],
        hash: Some((s.tx_hash.clone(), TX_HASH_RECEIVED.into())),
        cta: s.tx_close_background.clone(),
    }
}

fn scan(s: &FlowStrings) -> ScanModal {
    ScanModal {
        title: s.scan_title.clone(),
        hint: s.scan_hint.clone(),
        tools: vec![s.scan_from_gallery.clone(), s.scan_flip.clone()],
    }
}

/// The panel's title, and whether it offers a way back one level.
pub fn panel_title(panel: FlowPanel, s: &FlowStrings) -> SharedString {
    match panel {
        FlowPanel::Dr1 | FlowPanel::Dr2 | FlowPanel::Dr3 => s.receive_title.clone(),
        FlowPanel::Ds1 => s.scan_title.clone(),
        FlowPanel::Da1 => s.history_title.clone(),
        FlowPanel::Da2 | FlowPanel::Da3 => s.detail_section_title.clone(),
        FlowPanel::Dt1 | FlowPanel::Dt4 => s.assets_title.clone(),
        FlowPanel::Dt3 | FlowPanel::Dt3b => s.add_token_title.clone(),
        FlowPanel::Dsd1 => s.send_action.clone(),
        FlowPanel::Dsd2 | FlowPanel::Dsd2b => fill(&s.send_title, "symbol", "USDT").into(),
        FlowPanel::Dsd2c => s.batch_title.clone(),
        FlowPanel::Dsd2e => s.pick_contact_title.clone(),
        FlowPanel::Dsd2f => s.fee_token_label.clone(),
        FlowPanel::Dsd3 => s.confirm_title.clone(),
        FlowPanel::Dsd4 => s.send_action.clone(),
    }
}

/// Build one panel's body (spec.md's desktop state matrix).
pub fn body(panel: FlowPanel, s: &FlowStrings) -> FlowBody {
    match panel {
        FlowPanel::Dr1 => FlowBody::Receive(receive_list(s)),
        FlowPanel::Dr2 => FlowBody::ReceiveQr(receive_qr(s, false)),
        FlowPanel::Dr3 => FlowBody::ReceiveQr(receive_qr(s, true)),
        FlowPanel::Ds1 => FlowBody::Scan(scan(s)),
        FlowPanel::Da1 => FlowBody::History(HistoryPanel {
            groups: history(s),
            loading: false,
            empty: None,
        }),
        FlowPanel::Da2 => FlowBody::TxDetail(tx_detail(s, true)),
        FlowPanel::Da3 => FlowBody::TxDetail(tx_detail(s, false)),
        FlowPanel::Dt1 => FlowBody::Assets(assets_panel(s, false)),
        FlowPanel::Dt4 => FlowBody::Assets(assets_panel(s, true)),
        FlowPanel::Dt3 => FlowBody::AddToken(add_token(s, false)),
        FlowPanel::Dt3b => FlowBody::AddToken(add_token(s, true)),
        FlowPanel::Dsd1 => FlowBody::SendPick(send_pick(s)),
        FlowPanel::Dsd2 => FlowBody::SendForm(send_form(s, false)),
        FlowPanel::Dsd2b => FlowBody::SendForm(send_form(s, true)),
        FlowPanel::Dsd2c => FlowBody::BatchImport(batch_import(s)),
        FlowPanel::Dsd2e => FlowBody::ContactPick(contact_pick(s)),
        FlowPanel::Dsd2f => FlowBody::FeeToken(fee_token(s)),
        FlowPanel::Dsd3 => FlowBody::SendConfirm(send_confirm(s)),
        FlowPanel::Dsd4 => FlowBody::SendReceipt(send_receipt(s)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loc::Loc;

    fn strings() -> FlowStrings {
        FlowStrings::resolve(&Loc::from_env())
    }

    #[test]
    fn every_panel_builds_a_body() {
        let s = strings();
        for (panel, label) in FlowPanel::ALL {
            let built = body(panel, &s);
            // Exhaustive by construction — the match has no fallback arm — but
            // this proves each id reaches a body rather than panicking.
            match built {
                FlowBody::Scan(_) => assert_eq!(panel, FlowPanel::Ds1, "{label}"),
                _ => assert_ne!(panel, FlowPanel::Ds1, "{label}"),
            }
            assert!(!panel_title(panel, &s).is_empty(), "{label} has no title");
        }
    }

    #[test]
    fn r1_lists_the_eight_networks_with_one_shared_address() {
        let s = strings();
        let FlowBody::Receive(list) = body(FlowPanel::Dr1, &s) else {
            panic!("expected the receive list");
        };
        assert_eq!(list.rows.len(), 8);
        let names: Vec<_> = list.rows.iter().map(|r| r.name.as_ref()).collect();
        assert_eq!(
            names,
            [
                "Ethereum",
                "BNB Chain",
                "Polygon",
                "Arbitrum",
                "Optimism",
                "Base",
                "Avalanche",
                "Gnosis"
            ]
        );
        // The point of the panel: every row is the SAME address.
        let addresses: std::collections::HashSet<_> =
            list.rows.iter().map(|r| r.address.as_ref()).collect();
        assert_eq!(addresses.len(), 1);
    }

    #[test]
    fn only_the_asset_qr_carries_a_contract() {
        let s = strings();
        let FlowBody::ReceiveQr(dr2) = body(FlowPanel::Dr2, &s) else {
            panic!()
        };
        let FlowBody::ReceiveQr(dr3) = body(FlowPanel::Dr3, &s) else {
            panic!()
        };
        assert!(dr2.contract.is_none());
        assert!(dr3.contract.is_some());
    }

    #[test]
    fn da2_carries_a_contract_row_and_da3_does_not() {
        let s = strings();
        let FlowBody::TxDetail(da2) = body(FlowPanel::Da2, &s) else {
            panic!()
        };
        let FlowBody::TxDetail(da3) = body(FlowPanel::Da3, &s) else {
            panic!()
        };
        assert_eq!(da2.amount.as_ref(), "+120 USDT");
        assert!(da2.positive);
        assert!(da2.facts.iter().any(|f| f.label == s.token_contract));

        assert_eq!(da3.amount.as_ref(), "−2 POL");
        assert!(!da3.positive);
        // A native coin has no contract — the row must be absent, not empty.
        assert!(!da3.facts.iter().any(|f| f.label == s.token_contract));
    }

    #[test]
    fn dt1_lists_the_six_assets_and_dt4_replaces_them_with_guidance() {
        let s = strings();
        let FlowBody::Assets(dt1) = body(FlowPanel::Dt1, &s) else {
            panic!()
        };
        let FlowBody::Assets(dt4) = body(FlowPanel::Dt4, &s) else {
            panic!()
        };
        assert_eq!(dt1.rows.len(), 6);
        assert!(dt1.empty.is_none());
        assert!(dt4.rows.is_empty());
        assert!(dt4.empty.is_some());
    }

    #[test]
    fn the_split_form_totals_to_what_the_single_form_sends() {
        let s = strings();
        let FlowBody::SendForm(single) = body(FlowPanel::Dsd2, &s) else {
            panic!()
        };
        let FlowBody::SendForm(split) = body(FlowPanel::Dsd2b, &s) else {
            panic!()
        };
        // Split mode is the payee list; the single form has none.
        assert!(single.recipients.is_empty());
        assert_eq!(split.recipients.len(), 3);
        let total: f64 = split
            .recipients
            .iter()
            .map(|r| r.amount.parse::<f64>().unwrap())
            .sum();
        assert_eq!(total, 120.0);
    }

    #[test]
    fn the_importer_counts_only_the_rows_it_can_import() {
        let s = strings();
        let FlowBody::BatchImport(batch) = body(FlowPanel::Dsd2c, &s) else {
            panic!()
        };
        assert_eq!(batch.rows.iter().filter(|r| r.ok).count(), 2);
        // The CTA promises what it delivers — three parsed, two importable.
        assert!(batch.parsed.contains('3'));
        assert!(batch.cta.contains('2'));
    }

    #[test]
    fn one_fee_token_is_chosen_and_the_rest_are_not() {
        let s = strings();
        let FlowBody::FeeToken(fees) = body(FlowPanel::Dsd2f, &s) else {
            panic!()
        };
        assert_eq!(fees.rows.iter().filter(|r| r.selected).count(), 1);
        assert_eq!(fees.rows[0].symbol.as_ref(), "ETH");
    }

    #[test]
    fn the_desktop_scanner_drops_the_torch_a_webcam_does_not_have() {
        let s = strings();
        let FlowBody::Scan(modal) = body(FlowPanel::Ds1, &s) else {
            panic!()
        };
        assert_eq!(modal.tools.len(), 2);
    }

    #[test]
    fn splits_the_account_address_into_two_even_lines() {
        assert_eq!(
            address_lines("0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c"),
            (
                "0x14fB1fB21751E29F7Ec".to_string(),
                "48dC450017552E3D1eA5c".to_string()
            )
        );
    }

    #[test]
    fn pins_the_chain_ids_the_add_network_card_prints() {
        let avax = NETWORKS.iter().find(|n| n.name == "Avalanche").unwrap();
        let gnosis = NETWORKS.iter().find(|n| n.name == "Gnosis").unwrap();
        assert_eq!(avax.chain_id, "43114");
        assert_eq!(gnosis.chain_id, "100");
    }
}
