//! Machine — the payroll batch importer (spec `016-crux-wallet-state`,
//! inventory `batch_import`).
//!
//! ```text
//! Open{token,currency} ─► fetch USD→fiat rate ─► mirror rate (auto)
//!        │  paste / file ─► parse table ─► preview {valid, dup, convert}
//!        │  edit rate ─► the shown string IS the applied rate
//!        └─► Apply (cap 60, Σ ≤ balance, rate > 0) ─► RecipientDraft[]
//! ```
//!
//! Two incidents shaped this machine and both are pinned here as rules:
//!
//! - **Issue #137** (twice regressed): a digit-bearing NAME (`123123`,
//!   `Alice123`, `团队2024`) must never silently become the payment. The
//!   amount COLUMN is settled per table shape before any row is emitted, a
//!   blank amount cell is an error the operator can see (never a fallback to
//!   another column), and evidence from a 2-column table never decides a
//!   3-column one (`recipient-table.ts:107-283`).
//! - **The rate mirror** (`BatchImportSheet.tsx` header + `:423-443`): the
//!   rate string in the input IS the applied rate — display and conversion
//!   never diverge. The old `toFixed(2)` mirror showed "0" for sub-cent
//!   prices while converting at the true value, and a touch then zeroed
//!   every row via `parseFloat("0")`. `format_rate` therefore uses four
//!   significant digits and never returns "0" for a positive rate.
//!
//! Also ported: the per-batch "Priced in" currency is a scoped override that
//! never touches the app-wide display currency (issue #80 — note this
//! machine's operation vocabulary simply HAS no storage write); the over-cap
//! and rejected notices are both visible at once (`:365-377`); apply is
//! blocked when the capped total exceeds the balance; and every `Open` is a
//! full reset so a stale paste or rate is never reused (`:84-91`, hardened
//! here per inventory invariant ⑤: the USD→fiat rate is also cleared, where
//! the component kept the previous fetch's value for one frame).
//!
//! One ported quirk was REJECTED rather than mirrored — **an unknown rate
//! blocks the import**. `tokenPriceInFiat`'s `usdToFiatRate > 0 ? … : 1`
//! (`fiat-convert.ts`, reached through `usdFiatRate ?? 0` at
//! `BatchImportSheet.tsx:105`) made a rate nobody could fetch convert 1:1, so
//! `5000 CNY` previewed as 5000 USDT — worth ~698 — with `can_apply: true`:
//! one payroll batch at ~7x the intended payout, behind a green button. The
//! owner overturned that decision, so `auto_price_per_token` refuses an
//! unknown (`None`) or invalid (`<= 0`) rate: the mirror goes EMPTY, nothing
//! converts, apply is blocked, and `BatchRateStatus::Failed` sends the user to
//! the manual `rate_input` channel ("Rate unavailable — enter one manually",
//! `send.batchRateFailed`). Same discipline as `enforce_no_unlimited` and the
//! never-assume-18-decimals rule; the native twin
//! (`use-batch-import.ts` + `batch-import-controller-types.ts::autoPricePerToken`)
//! holds it identically, and the shared `fiat-convert.ts` helper is untouched
//! because its fallback is still right for DISPLAY.
//!
//! A second quirk from the same family was REJECTED with it: the component
//! kept mirroring the OLD currency's rate across a "Priced in" switch, until
//! the new fetch landed. That is the identical overpayment wearing a different
//! hat — USD→CNY at the retained rate 1 showed "1 USDT = 1 CNY" with Apply
//! green, and a 5000-a-row payroll went out at ~7.2x for the whole FX
//! round-trip. Unknown and MISLABELLED are the same refusal, so a rate here is
//! never a bare number: [`FiatRate`] carries the code it was fetched FOR and
//! [`auto_price_per_token`] quotes nothing unless that code is the one being
//! priced right now. That single check closes both directions at once — the
//! old rate cannot outlive the switch, and a late answer for the abandoned
//! currency cannot relabel itself as the new one.
//!
//! Quirks ported verbatim (see inventory open questions):
//! - In token mode a pasted amount with more decimals than the token is
//!   silently truncated by `to_base_units` (eip681.ts `toBaseUnits` slice).
//!
//! The shell owns the file picker, SheetJS (`.xlsx` → cell matrix — the text
//! path is parsed entirely in core), the rate source (`resolveRate` — NOT the
//! display `getRate`, whose `?? 1` would answer this machine a lie), haptics,
//! and all formatting/locale; the core owns the table interpreter, the money
//! math and every gate. The produced `recipients` (address, token amount,
//! optional name) are the hand-off to the send machine's split editor —
//! the shell assigns row ids (`makeRecipientId` stays in the shell).

use std::collections::{BTreeMap, BTreeSet};

use crux_core::capability::Operation;
use crux_core::macros::effect;
use crux_core::{render::render, render::RenderOperation, App, Command};
use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

/// One batch's recipient cap, matching `BATCH_MAX_RECIPIENTS`
/// (`batch-send.ts:42`). Passed to `Open` by the shell so the policy stays
/// with batch-send; exported here as the canonical value.
pub const BATCH_MAX_RECIPIENTS: u32 = 60;

/// The downloadable CSV template, byte-for-byte (`BatchImportSheet.tsx:42-46`).
pub const TEMPLATE_CSV: &str = "name,address,amount\n\
Alice,0x1111111111111111111111111111111111111111,5000\n\
Bob,0x2222222222222222222222222222222222222222,8000\n\
Carol,0x3333333333333333333333333333333333333333,6500\n";

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------

/// What this machine asks the platform to do.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "BatchOperation"))]
pub enum BatchOperation {
    /// USD→`code` multiplier (`resolveRate` in currency.ts). `rate: None` is
    /// the honest "no source could price it" — including when the source
    /// threw — and the ONLY thing the shell may answer when it does not know.
    /// Never `getRate`: its `?? 1` is a display fallback and arrives here as
    /// "the rate really is 1", which is what this machine's guard exists to
    /// refuse. Status drives the loading/failed hints so an empty mirror is
    /// never unexplained.
    FetchUsdFiatRate { code: String },
    /// Open the table picker. Text files come back as text (parsed in core);
    /// Excel workbooks are flattened by the shell's lazy SheetJS into a cell
    /// matrix (`recipient-table.ts:294-302` stays shell-side).
    PickFile,
    /// Save the CSV template (`saveTextFile` semantics).
    SaveTemplateFile {
        name: String,
        contents: String,
        mime: String,
    },
}

/// A picked file's content, already shaped for the core.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "BatchFileContent"))]
pub enum BatchFileContent {
    /// CSV / TSV / TXT — the pure text path, interpreted in core.
    Text { text: String },
    /// A workbook's first sheet as an array-of-arrays (SheetJS `header: 1`,
    /// `defval: ''` — column positions stay stable).
    Matrix { rows: Vec<Vec<String>> },
}

/// What the shell observed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "BatchShellResult"))]
pub enum BatchShellResult {
    /// `code` rides along so a result for a currency the user has already
    /// switched away from can be identified and dropped.
    RateResolved {
        code: String,
        rate: Option<f64>,
    },
    FilePicked {
        name: String,
        content: BatchFileContent,
    },
    /// The picker was dismissed — nothing changes.
    FilePickCancelled,
    /// The file could not be read/parsed (the `showAlert` branch).
    FilePickFailed,
    TemplateSaved,
    /// Share sheet dismissed / unavailable — silently keep the plain label.
    TemplateSaveFailed,
}

impl Operation for BatchOperation {
    type Output = BatchShellResult;
}

#[effect]
pub enum BatchEffect {
    Render(RenderOperation),
    Shell(BatchOperation),
}

// ---------------------------------------------------------------------------
// Wire value types
// ---------------------------------------------------------------------------

/// The facts of the token being sent, as the shell maps them from `APIToken`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct BatchToken {
    pub symbol: String,
    pub decimals: u32,
    /// Human decimal string (never a float — money crosses the wire as
    /// decimal text). Empty means zero, matching `token.balance || '0'`.
    pub balance: String,
    pub price_usd: Option<f64>,
}

/// Which unit the pasted numbers are in (`BatchImportSheet.tsx:48`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum BatchUnit {
    /// Amounts are fiat figures converted at the shown rate. Works even for
    /// an unpriced token — the company can pin its own rate, which is the
    /// whole payroll point.
    #[default]
    Fiat,
    Token,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum BatchRateStatus {
    #[default]
    Loading,
    Ok,
    Failed,
}

// ---------------------------------------------------------------------------
// Table interpreter — faithful port of recipient-table.ts
// ---------------------------------------------------------------------------

/// One successfully-read payee row. `line` is the 1-based source row (header
/// excluded).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatchParsedRow {
    pub line: u32,
    pub name: Option<String>,
    /// The address exactly as written; validated/lowercased downstream.
    pub address: String,
    /// A clean numeric string ("5000", "173.88").
    pub raw_amount: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchParseReason {
    NoAddress,
    NoAmount,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatchParseError {
    pub line: u32,
    pub raw: String,
    pub reason: BatchParseReason,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BatchParseResult {
    pub rows: Vec<BatchParsedRow>,
    pub errors: Vec<BatchParseError>,
}

const DELIMITERS: [char; 3] = [',', '\t', ';'];

/// `^0x[0-9a-fA-F]{40}$` — `ADDRESS_RE` (`models/types.ts:383`).
fn is_address(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() == 42 && bytes.starts_with(b"0x") && bytes[2..].iter().all(u8::is_ascii_hexdigit)
}

/// Pick the delimiter: prefer whichever splits the first line into a cell
/// that looks like an address (so `addr;¥5,000.50` chooses `;` over the
/// thousands comma), otherwise the most columns (`recipient-table.ts:50-62`).
fn sniff_delimiter(first_line: &str) -> char {
    let mut best = ',';
    let mut best_score: i64 = -1;
    for delim in DELIMITERS {
        let cells = split_csv_line(first_line, delim);
        let has_address = cells.iter().any(|c| is_address(c.trim()));
        let score = if has_address { 1000 } else { 0 } + (cells.len() as i64 - 1);
        if score > best_score {
            best = delim;
            best_score = score;
        }
    }
    best
}

/// Split one CSV line, honouring simple double-quoted cells with `""`
/// escapes (`recipient-table.ts:66-87`).
fn split_csv_line(line: &str, delim: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                cur.push(ch);
            }
        } else if ch == '"' {
            in_quotes = true;
        } else if ch == delim {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(ch);
        }
    }
    out.push(cur);
    out
}

/// Reduce a cell to a positive decimal string, or `""` if it isn't one
/// (`cleanAmount`, `recipient-table.ts:96-102`). The value CLEANER — never
/// the detector; using it as a detector is what caused issue #137.
fn clean_amount(cell: &str) -> String {
    let stripped: String = cell
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    if stripped.is_empty() {
        return String::new();
    }
    if stripped.matches('.').count() > 1 {
        return String::new(); // "1.2.3" — ambiguous
    }
    match stripped.parse::<f64>() {
        // parseFloat semantics: overflow to infinity fails Number.isFinite.
        Ok(n) if n.is_finite() && n > 0.0 => stripped,
        _ => String::new(),
    }
}

/// Header labels (lowercased) that pin a column's role
/// (`recipient-table.ts:107-118`).
const AMOUNT_LABELS: [&str; 16] = [
    "amount",
    "amt",
    "sum",
    "value",
    "money",
    "pay",
    "salary",
    "wage",
    "金额",
    "数量",
    "工资",
    "薪资",
    "薪酬",
    "数额",
    "转账金额",
    "发放金额",
];
const NAME_LABELS: [&str; 13] = [
    "name",
    "username",
    "nickname",
    "employee",
    "recipient",
    "payee",
    "contact",
    "姓名",
    "名字",
    "名称",
    "收款人",
    "员工",
    "昵称",
];
const ADDRESS_LABELS: [&str; 11] = [
    "address",
    "addr",
    "wallet",
    "account",
    "to",
    "地址",
    "钱包",
    "钱包地址",
    "账户",
    "账号",
    "收款地址",
];

const CURRENCY_SIGNS: [char; 11] = ['¥', '$', '€', '£', '₩', '₽', '₹', '฿', '¢', '￥', '＄'];
const CJK_CURRENCY: [char; 4] = ['元', '円', '圆', '块'];
/// Lowercased codes that may flank a number (`recipient-table.ts:125-129`).
const CURRENCY_CODES: [&str; 33] = [
    "usd", "usdt", "usdc", "busd", "dai", "cny", "rmb", "eur", "gbp", "jpy", "hkd", "krw", "twd",
    "sgd", "aud", "cad", "chf", "inr", "brl", "rub", "thb", "vnd", "myr", "php", "idr", "eth",
    "btc", "bnb", "pol", "matic", "sol", "trx", "ton",
];

/// A known code, or any ALL-CAPS 2–5 letter token (an unlisted ticker) —
/// which a capitalized name like `Alice` or `Team` is not
/// (`recipient-table.ts:146`).
fn is_currency_code(word: &str) -> bool {
    CURRENCY_CODES.contains(&word.to_lowercase().as_str())
        || (word.len() >= 2 && word.len() <= 5 && word.bytes().all(|b| b.is_ascii_uppercase()))
}

/// The captured groups of `AMOUNT_SHAPE` (`recipient-table.ts:139-144`).
struct AmountShape {
    lead: Option<String>,
    lead_sign: bool,
    trail: Option<String>,
}

fn is_group_sep(c: char) -> bool {
    // The regex class `[,\s'’]`.
    c == ',' || c == '\'' || c == '’' || c.is_whitespace()
}

/// Hand-rolled equivalent of the `AMOUNT_SHAPE` regex — same grammar, same
/// alternation order (grouped thousands → plain decimal → bare fraction),
/// same greedy/backtracking outcome (digits are never matchable by the tail,
/// which is what makes the greedy scan exact).
fn match_amount_shape(cell: &str) -> Option<AmountShape> {
    let chars: Vec<char> = cell.trim().chars().collect();
    let n = chars.len();
    let mut i = 0;

    // (?<lead>[A-Za-z]{1,5})? — a longer letter run can never be rescued
    // (nothing after lead matches a letter), so full-run-or-fail is exact.
    let lead_start = i;
    while i < n && chars[i].is_ascii_alphabetic() {
        i += 1;
    }
    let lead_len = i - lead_start;
    if lead_len > 5 {
        return None;
    }
    let lead: Option<String> = (lead_len > 0).then(|| chars[lead_start..i].iter().collect());

    // (?<leadSign>[SIGNS]+)?
    let sign_start = i;
    while i < n && CURRENCY_SIGNS.contains(&chars[i]) {
        i += 1;
    }
    let lead_sign = i > sign_start;

    // \s*
    while i < n && chars[i].is_whitespace() {
        i += 1;
    }

    // (?<num>…) alternatives, each validated against the tail before the
    // next is tried — the regex engine's alternation backtracking.
    for num_end in number_alternatives(&chars, i) {
        if let Some(trail) = match_tail(&chars, num_end) {
            return Some(AmountShape {
                lead,
                lead_sign,
                trail,
            });
        }
    }
    None
}

/// Candidate end positions for the `num` group, in the regex's alternation
/// order: `\d{1,3}(?:[,\s'’]\d{3})+(?:\.\d+)?` | `\d+(?:\.\d+)?` | `\.\d+`.
fn number_alternatives(chars: &[char], start: usize) -> Vec<usize> {
    let n = chars.len();
    let mut out = Vec::new();
    let mut digit_run = 0;
    while start + digit_run < n && chars[start + digit_run].is_ascii_digit() {
        digit_run += 1;
    }

    // Grouped thousands. `\d{1,3}` can only usefully take the whole leading
    // run (a shorter take leaves a digit where the separator must be), so
    // the alternative exists only for runs of 1–3.
    if (1..=3).contains(&digit_run) {
        let mut i = start + digit_run;
        let mut groups = 0;
        while i < n
            && is_group_sep(chars[i])
            && i + 3 < n + 1
            && chars.get(i + 1).is_some_and(char::is_ascii_digit)
            && chars.get(i + 2).is_some_and(char::is_ascii_digit)
            && chars.get(i + 3).is_some_and(char::is_ascii_digit)
        {
            i += 4;
            groups += 1;
        }
        if groups >= 1 {
            out.push(eat_optional_fraction(chars, i));
        }
    }

    // Plain decimal.
    if digit_run >= 1 {
        out.push(eat_optional_fraction(chars, start + digit_run));
    }

    // Bare fraction `.\d+`.
    if chars.get(start) == Some(&'.') && chars.get(start + 1).is_some_and(char::is_ascii_digit) {
        let mut i = start + 2;
        while i < n && chars[i].is_ascii_digit() {
            i += 1;
        }
        out.push(i);
    }
    out
}

/// `(?:\.\d+)?` — greedy, taken only when a digit follows the dot.
fn eat_optional_fraction(chars: &[char], mut i: usize) -> usize {
    if chars.get(i) == Some(&'.') && chars.get(i + 1).is_some_and(char::is_ascii_digit) {
        i += 2;
        while i < chars.len() && chars[i].is_ascii_digit() {
            i += 1;
        }
    }
    i
}

/// The tail `\s*[SIGNS]*\s*(?<trail>[A-Za-z]{1,5}|[CJK])?$`. Returns the
/// captured trail on a full match (`Some(None)` = matched, no trail).
fn match_tail(chars: &[char], mut i: usize) -> Option<Option<String>> {
    let n = chars.len();
    while i < n && chars[i].is_whitespace() {
        i += 1;
    }
    while i < n && CURRENCY_SIGNS.contains(&chars[i]) {
        i += 1;
    }
    while i < n && chars[i].is_whitespace() {
        i += 1;
    }
    if i == n {
        return Some(None);
    }
    let rest_len = n - i;
    let all_letters = chars[i..].iter().all(|c| c.is_ascii_alphabetic());
    if all_letters && (1..=5).contains(&rest_len) {
        return Some(Some(chars[i..].iter().collect()));
    }
    if rest_len == 1 && CJK_CURRENCY.contains(&chars[i]) {
        return Some(Some(chars[i..].iter().collect()));
    }
    None
}

/// Does this cell READ as an amount? (`isAmountCell`,
/// `recipient-table.ts:149-158`.) Value extraction stays with
/// [`clean_amount`]; this shape gate is what keeps `Alice123` / `团队2024` /
/// `1e5` / dates from ever being paid (issue #137).
fn is_amount_cell(cell: &str) -> bool {
    let Some(shape) = match_amount_shape(cell) else {
        return false;
    };
    if let Some(lead) = &shape.lead {
        // A leading word is a currency token only if it is a known /
        // ticker-shaped code, or a 1–3 letter code glued to a sign (`R$`).
        if !is_currency_code(lead) && !(shape.lead_sign && lead.chars().count() <= 3) {
            return false;
        }
    }
    if let Some(trail) = &shape.trail {
        let cjk_single =
            trail.chars().count() == 1 && trail.chars().all(|c| CJK_CURRENCY.contains(&c));
        if !cjk_single && !is_currency_code(trail) {
            return false;
        }
    }
    !clean_amount(cell).is_empty()
}

/// Interpret an already-split cell matrix (`interpretRows`,
/// `recipient-table.ts:171-283`). The first non-blank row is dropped as a
/// header only when it carries no address; every later address-less row is
/// an error, never a silently-swallowed "header". The amount COLUMN is
/// settled per table shape (cell count + address position) before any row is
/// emitted, so a 2-column `amount,address` row never speaks for a 3-column
/// `name,address,amount` row — which is exactly how vote-mixing reintroduced
/// issue #137.
pub fn interpret_rows(matrix: &[Vec<String>]) -> BatchParseResult {
    struct Row {
        cells: Vec<String>,
        addr_idx: Option<usize>,
        cand: Vec<usize>,
        shape: (usize, i64),
    }

    // Pass 0: normalize, drop blanks, split off the (optional) header row.
    let mut header: Option<Vec<String>> = None;
    let mut seen_any_row = false;
    let mut raw_rows: Vec<Vec<String>> = Vec::new();
    for cells_raw in matrix {
        let cells: Vec<String> = cells_raw.iter().map(|c| c.trim().to_owned()).collect();
        if cells.iter().all(String::is_empty) {
            continue; // blank line
        }
        if !seen_any_row {
            seen_any_row = true;
            if !cells.iter().any(|c| is_address(c)) {
                header = Some(cells);
                continue;
            }
        }
        raw_rows.push(cells);
    }

    let norm = |s: &str| s.trim().to_lowercase();
    let header_cells = header.unwrap_or_default();
    let header_amount_idx = header_cells
        .iter()
        .position(|c| AMOUNT_LABELS.contains(&norm(c).as_str()));
    let header_name_idx = header_cells
        .iter()
        .position(|c| NAME_LABELS.contains(&norm(c).as_str()));
    let header_addr_idx = header_cells
        .iter()
        .position(|c| ADDRESS_LABELS.contains(&norm(c).as_str()));

    // Pass 1: locate the address and every cell that reads as an amount. A
    // labelled address column wins, so an address pasted into the NAME
    // column cannot capture the payment.
    let mut data_rows: Vec<Row> = Vec::new();
    for cells in raw_rows {
        let addr_idx = match header_addr_idx {
            Some(h) if cells.get(h).is_some_and(|c| is_address(c)) => Some(h),
            _ => cells.iter().position(|c| is_address(c)),
        };
        let mut cand: Vec<usize> = Vec::new();
        if let Some(addr) = addr_idx {
            for (i, cell) in cells.iter().enumerate() {
                if i != addr && Some(i) != header_name_idx && is_amount_cell(cell) {
                    cand.push(i);
                }
            }
        }
        let shape = (cells.len(), addr_idx.map_or(-1, |a| a as i64));
        data_rows.push(Row {
            cells,
            addr_idx,
            cand,
            shape,
        });
    }

    // Pass 2: settle one amount column per shape. Rows whose amount sits
    // after the address (the template convention) vote first; only if no row
    // of that shape has one do single-candidate rows — which may be a lone
    // digit-only name on a row whose real amount cell is blank — decide.
    type Tally = (BTreeMap<usize, u32>, BTreeMap<usize, u32>);
    let mut by_shape: BTreeMap<(usize, i64), Tally> = BTreeMap::new();
    for row in &data_rows {
        let Some(addr) = row.addr_idx else { continue };
        if row.cand.is_empty() {
            continue;
        }
        let (after, single) = by_shape.entry(row.shape).or_default();
        if let Some(&first_after) = row.cand.iter().find(|&&i| i > addr) {
            *after.entry(first_after).or_insert(0) += 1;
        }
        if row.cand.len() == 1 {
            *single.entry(row.cand[0]).or_insert(0) += 1;
        }
    }
    // Smallest column wins a tie (entries sorted ascending, strict `>`).
    fn winner(tally: &BTreeMap<usize, u32>) -> Option<usize> {
        let mut best = None;
        let mut best_n = 0;
        for (&col, &n) in tally {
            if n > best_n {
                best_n = n;
                best = Some(col);
            }
        }
        best
    }
    let mut shape_amount_idx: BTreeMap<(usize, i64), usize> = BTreeMap::new();
    for (shape, (after, single)) in &by_shape {
        let settled = if after.is_empty() {
            winner(single)
        } else {
            winner(after)
        };
        if let Some(col) = settled {
            shape_amount_idx.insert(*shape, col);
        }
    }

    // Pass 3: emit rows/errors in source order.
    let mut rows: Vec<BatchParsedRow> = Vec::new();
    let mut errors: Vec<BatchParseError> = Vec::new();
    for (row_idx, row) in data_rows.iter().enumerate() {
        let data_line = (row_idx + 1) as u32;
        let raw = row.cells.join(" , ");
        let Some(addr_idx) = row.addr_idx else {
            errors.push(BatchParseError {
                line: data_line,
                raw,
                reason: BatchParseReason::NoAddress,
            });
            continue;
        };

        // An explicit header is authoritative wherever the row actually has
        // that column; a ragged row without it falls back to inference.
        let pinned = header_amount_idx
            .filter(|&h| h != addr_idx && h < row.cells.len())
            .or_else(|| shape_amount_idx.get(&row.shape).copied());
        let amt_idx = match pinned {
            Some(p) if p != addr_idx => Some(p),
            _ => row
                .cand
                .iter()
                .copied()
                .find(|&i| i > addr_idx)
                .or_else(|| row.cand.first().copied()),
        };
        // Reading only from the settled column is what makes a blank amount
        // cell an error the operator can see, instead of a name silently
        // becoming a payment (issue #137).
        let amount = amt_idx
            .and_then(|i| row.cells.get(i))
            .filter(|c| is_amount_cell(c))
            .map(|c| clean_amount(c))
            .unwrap_or_default();
        if amount.is_empty() {
            errors.push(BatchParseError {
                line: data_line,
                raw,
                reason: BatchParseReason::NoAmount,
            });
            continue;
        }

        // Name: the header-labelled column first; else the first leftover
        // cell that reads as text (so a row-number column never labels the
        // payee); else the first non-empty leftover (a digit-only name is
        // still a name). Empty header cells fall through, as `||` does.
        let leftover = |i: usize, c: &str| i != addr_idx && Some(i) != amt_idx && !c.is_empty();
        let name = header_name_idx
            .filter(|&h| h != addr_idx && Some(h) != amt_idx)
            .and_then(|h| row.cells.get(h))
            .filter(|c| !c.is_empty())
            .cloned()
            .or_else(|| {
                row.cells
                    .iter()
                    .enumerate()
                    .find(|(i, c)| leftover(*i, c) && !is_amount_cell(c))
                    .map(|(_, c)| c.clone())
            })
            .or_else(|| {
                row.cells
                    .iter()
                    .enumerate()
                    .find(|(i, c)| leftover(*i, c))
                    .map(|(_, c)| c.clone())
            });
        rows.push(BatchParsedRow {
            line: data_line,
            name,
            address: row.cells.get(addr_idx).cloned().unwrap_or_default(),
            raw_amount: amount,
        });
    }

    BatchParseResult { rows, errors }
}

/// Parse delimited text — CSV / TSV / TXT / pasted. Pure and synchronous
/// (`parseRecipientTableText`, `recipient-table.ts:286-292`).
pub fn parse_recipient_table_text(text: &str) -> BatchParseResult {
    let clean = text.strip_prefix('\u{feff}').unwrap_or(text); // strip BOM
    let lines: Vec<&str> = clean
        .split(['\r', '\n'])
        .filter(|l| !l.trim().is_empty())
        .collect();
    let Some(first) = lines.first() else {
        return BatchParseResult::default();
    };
    let delim = sniff_delimiter(first);
    let matrix: Vec<Vec<String>> = lines.iter().map(|l| split_csv_line(l, delim)).collect();
    interpret_rows(&matrix)
}

// ---------------------------------------------------------------------------
// Pure money text
// ---------------------------------------------------------------------------

/// `parseFloat(s) || 0` for the strings this machine feeds it (ASCII digits
/// and dots — cleaned amounts and the filtered rate input): the longest
/// valid numeric prefix, `0` when there is none. `"1.2.3"` → 1.2, matching
/// the mirror's quirk of keeping extra dots while applying the prefix.
fn parse_float_prefix(s: &str) -> f64 {
    let mut num = String::new();
    let mut seen_dot = false;
    for c in s.chars() {
        if c.is_ascii_digit() {
            num.push(c);
        } else if c == '.' && !seen_dot {
            seen_dot = true;
            num.push('.');
        } else {
            break;
        }
    }
    if !num.bytes().any(|b| b.is_ascii_digit()) {
        return 0.0;
    }
    num.parse::<f64>().unwrap_or(0.0)
}

/// JS `Number.prototype.toFixed` for non-negative finite inputs: round
/// half-away-from-zero at `prec` decimals of the exact binary value —
/// including exact ties like 0.125 → "0.13", where Rust's own formatter
/// rounds half-to-even. Implemented by formatting 25 digits past `prec`
/// (exact for every dyadic tie in range) and string-rounding.
fn js_to_fixed(x: f64, prec: usize) -> String {
    let wide = format!("{:.*}", prec + 25, x);
    let (int_part, frac) = match wide.split_once('.') {
        Some(parts) => parts,
        None => (wide.as_str(), ""),
    };
    let kept: Vec<char> = frac.chars().take(prec).collect();
    let round_up = frac.chars().nth(prec).is_some_and(|d| d >= '5');
    let mut digits: Vec<char> = int_part.chars().chain(kept).collect();
    if round_up {
        let mut idx = digits.len();
        loop {
            if idx == 0 {
                digits.insert(0, '1');
                break;
            }
            idx -= 1;
            match digits[idx] {
                '9' => digits[idx] = '0',
                d => {
                    digits[idx] = char::from_u32(d as u32 + 1).unwrap_or('9');
                    break;
                }
            }
        }
    }
    if prec == 0 {
        return digits.into_iter().collect();
    }
    let split = digits.len().saturating_sub(prec);
    let int_str: String = digits[..split].iter().collect();
    let frac_str: String = digits[split..].iter().collect();
    format!("{int_str}.{frac_str}")
}

/// `stripTrailingZeros` (`fiat-convert.ts:17-20`) — integers are left alone,
/// which guards decimals=0 tokens.
fn strip_trailing_zeros(s: &str) -> String {
    if !s.contains('.') {
        return s.to_owned();
    }
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// `tokenPriceInFiat` (`fiat-convert.ts:27-31`) — the shared display-side
/// math, ported verbatim INCLUDING its `usdToFiatRate > 0 ? … : 1` fallback.
///
/// That fallback is a DISPLAY convenience (an unpriceable currency renders the
/// USD figure rather than a blank), and it is why this function must never be
/// handed a rate it cannot vouch for. The importer's only caller,
/// `auto_price_per_token`, therefore screens the rate first and never reaches
/// the `1.0` branch — the discrimination lives at the call site so the shared
/// helper keeps one meaning on both sides of the FFI (the same split
/// `currency.ts` draws between `resolveRate` and the display `getRate`).
// `!(x > 0.0)` is not `x <= 0.0`: it is the one spelling that also catches NaN,
// which is the whole point of the guard below. Do not let clippy talk it into
// the `partial_cmp` rewrite — that hands NaN the priced branch.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
fn token_price_in_fiat(price_usd: Option<f64>, usd_to_fiat_rate: f64) -> f64 {
    let Some(price) = price_usd else { return 0.0 };
    if !(price > 0.0) {
        return 0.0; // JS `!priceUsd || priceUsd <= 0` (NaN lands here too)
    }
    let rate = if usd_to_fiat_rate > 0.0 {
        usd_to_fiat_rate
    } else {
        1.0
    };
    price * rate
}

/// How many decimals a fiat→token conversion keeps (spec 038, founder's
/// ruling "best practice and the person's experience"): six — the precision
/// every balance row prints — or finer when one more place is worth more
/// than a cent of the fiat (a token priced at 60,000 needs seven), never
/// more than the token carries. The phone's port kept the token's full
/// `decimals`, which turned 0.001 USD into `0.001000400160064026 XDAI` sixty
/// times over on a confirm screen.
fn conversion_decimals(price_in_fiat: f64, token_decimals: u32) -> u32 {
    // The place worth a cent: 10^-d × price ≤ 0.01  ⇔  d ≥ log10(price / 0.01).
    let cent_place = (price_in_fiat / 0.01).log10().ceil();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let cent_place = if cent_place.is_finite() && cent_place > 0.0 {
        cent_place as u32
    } else {
        0
    };
    cent_place.max(6).min(token_decimals)
}

/// `fiatToTokenAmount` (`fiat-convert.ts:40-44`), rounded to
/// [`conversion_decimals`] — never more precision than the token can carry
/// (the same guard `to_base_units` applies on-chain-side), and no longer the
/// token's whole eighteen places. Returns "0" for a non-positive fiat OR an
/// unknown (≤0) price.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // NaN must take the "0" branch
fn fiat_to_token_amount(fiat: f64, price_in_fiat: f64, decimals: u32) -> String {
    if !(price_in_fiat > 0.0) || !(fiat > 0.0) {
        return "0".to_owned();
    }
    let places = conversion_decimals(price_in_fiat, decimals) as usize;
    strip_trailing_zeros(&js_to_fixed(fiat / price_in_fiat, places))
}

#[cfg(test)]
mod conversion_precision {
    use super::*;

    #[test]
    fn six_places_or_the_cent_place_whichever_is_finer_capped_by_the_token() {
        assert_eq!(conversion_decimals(1.0, 18), 6);
        assert_eq!(conversion_decimals(7.25, 18), 6);
        assert_eq!(conversion_decimals(60_000.0, 8), 7);
        assert_eq!(conversion_decimals(435_000.0, 8), 8);
        assert_eq!(conversion_decimals(0.000_001, 18), 6);
        assert_eq!(conversion_decimals(1.0, 2), 2);
        assert_eq!(conversion_decimals(1.0, 0), 0);
    }

    #[test]
    fn the_founder_s_sixty_lines_read_as_money_again() {
        // 0.001 USD at 0.9996 USD per XDAI: 0.0010004001… → six places → 0.001.
        assert_eq!(fiat_to_token_amount(0.001, 0.9996, 18), "0.001");
        // 5,000 CNY at 7.25 CNY per USDT.
        assert_eq!(fiat_to_token_amount(5000.0, 7.25, 6), "689.655172");
        // 100 CNY of a coin at 435,000 CNY: a place is worth 0.0435 CNY at
        // six decimals, so the eighth is kept and nothing is lost to rounding.
        assert_eq!(fiat_to_token_amount(100.0, 435_000.0, 8), "0.00022989");
        assert_eq!(fiat_to_token_amount(0.0, 1.0, 18), "0");
        assert_eq!(fiat_to_token_amount(1.0, 0.0, 18), "0");
    }
}

/// `toBaseUnits` (`eip681.ts:48-54`) on u128. `None` when the digit string
/// does not fit u128 or contains a non-digit — the two call sites map both
/// to the fail-closed direction (a saturated total blocks apply; an
/// unreadable balance reads as zero and blocks apply), where the JS BigInt
/// would have thrown mid-render.
fn to_base_units(amount: &str, decimals: u32) -> Option<u128> {
    let cleaned = amount.trim();
    if cleaned.is_empty() {
        return Some(0);
    }
    let (int_part, frac_part) = match cleaned.split_once('.') {
        Some((i, f)) => (i, f),
        None => (cleaned, ""),
    };
    let mut frac: String = frac_part.chars().take(decimals as usize).collect();
    while frac.len() < decimals as usize {
        frac.push('0');
    }
    let int_part = if int_part.is_empty() { "0" } else { int_part };
    format!("{int_part}{frac}").parse::<u128>().ok()
}

/// `fromBaseUnits` (`eip681.ts:57-64`) — trimmed human decimal string.
fn from_base_units(value: u128, decimals: u32) -> String {
    let s = value.to_string();
    if decimals == 0 {
        return s;
    }
    let d = decimals as usize;
    let padded = if s.len() <= d {
        format!("{}{}", "0".repeat(d + 1 - s.len()), s)
    } else {
        s
    };
    let split = padded.len() - d;
    let int_part = &padded[..split];
    let frac = padded[split..].trim_end_matches('0');
    if frac.is_empty() {
        int_part.to_owned()
    } else {
        format!("{int_part}.{frac}")
    }
}

/// `trimNum` (`BatchImportSheet.tsx:424-427`): a float to a compact,
/// trailing-zero-free 2-decimal string — the fiat totals' value shape (the
/// shell adds symbol and locale separators).
fn trim_num(n: f64) -> String {
    if !n.is_finite() {
        return "0".to_owned();
    }
    let fixed = js_to_fixed(n, 2);
    let trimmed = fixed.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() {
        "0".to_owned()
    } else {
        trimmed.to_owned()
    }
}

const RATE_SIG_DIGITS: i32 = 4;

/// `formatRate` (`BatchImportSheet.tsx:436-443`): rate → plain-decimal
/// string with 4 significant digits, trailing zeros trimmed. Never returns
/// "0" for a positive rate — that string, once touched, zeroed every row via
/// `parseFloat` — and the returned string IS the applied rate, so what the
/// user reads is exactly what the conversion uses.
fn format_rate(n: f64) -> String {
    if !n.is_finite() || n <= 0.0 {
        return String::new();
    }
    let exp = n.log10().floor();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let decimals = (f64::from(RATE_SIG_DIGITS) - 1.0 - exp).clamp(0.0, 18.0) as usize;
    let fixed = js_to_fixed(n, decimals);
    let trimmed = if fixed.contains('.') {
        fixed.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        fixed
    };
    if parse_float_prefix(&trimmed) > 0.0 {
        trimmed
    } else {
        String::new()
    }
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "BatchImportEvent"))]
pub enum Event {
    /// The sheet opened. A FULL reset — a stale paste or rate from a
    /// previous open is never reused (`BatchImportSheet.tsx:82-91`,
    /// inventory invariant ⑤). `currency_code` is the app's display
    /// currency, the default the amounts are read as; `max_recipients` is
    /// [`BATCH_MAX_RECIPIENTS`] from batch-send.
    Open {
        token: BatchToken,
        currency_code: String,
        max_recipients: u32,
    },
    SetUnit {
        unit: BatchUnit,
    },
    /// A pick in the scoped per-batch currency sheet (issue #80: this must
    /// never write the app-wide display currency — and this machine's
    /// operation vocabulary has no storage write at all). Re-fetches the
    /// rate and returns the mirror to auto (`BatchImportSheet.tsx:416`).
    SetFiatCode {
        code: String,
    },
    /// Pasted / typed table text. Clears any picked file
    /// (`BatchImportSheet.tsx:228`).
    SetRawText {
        text: String,
    },
    PickFileRequested,
    SaveTemplateRequested,
    /// The rate input edited. The shell dot-normalizes locale input
    /// (`parseLocaleNumber`); the core applies the `[^0-9.]` strip. From
    /// here the typed string is the applied rate.
    EditRate {
        text: String,
    },
    /// The "Auto" button — back to mirroring the fetched rate.
    ResetRateToAuto,
    /// Hand the capped, converted drafts to the send machine. Ignored
    /// unless `can_apply`.
    Apply,
    #[serde(skip)]
    ShellCompleted {
        attempt: u64,
        result: BatchShellResult,
    },
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

/// A fetched USD→fiat multiplier, inseparable from the currency it prices.
///
/// The rate is never stored as a bare `f64`, because a bare `f64` cannot say
/// which currency it belongs to — and every way this machine can be holding a
/// rate for the WRONG currency (a "Priced in" switch whose fetch has not landed,
/// a late answer for a currency already abandoned, a re-open) is a way to pay
/// out the fiat figure at another currency's exchange rate. Pairing the two
/// makes the mistake unrepresentable rather than merely unlikely: the only
/// reader, [`auto_price_per_token`], compares `code` against the currency being
/// priced before it quotes anything.
#[derive(Clone, Debug, PartialEq)]
struct FiatRate {
    /// The currency this rate was fetched FOR — not necessarily the one the
    /// sheet is showing now.
    code: String,
    /// USD → `code` multiplier, exactly as the source answered it (a
    /// non-positive answer is kept as-is and refused at use).
    rate: f64,
}

#[derive(Default)]
pub struct Model {
    /// `None` until the first `Open` — every other event is ignored before.
    token: Option<BatchToken>,
    max_recipients: u32,
    unit: BatchUnit,
    fiat_code: String,
    raw_text: String,
    /// A parsed workbook takes precedence over the paste box, exactly as
    /// `fileParsed ?? parseRecipientTableText(rawText)` does.
    file_parsed: Option<BatchParseResult>,
    file_name: Option<String>,
    busy: bool,
    file_error: bool,
    template_saved: bool,
    /// The last rate the shell answered, tagged with its currency. Only
    /// usable while `code` still equals [`Model::fiat_code`].
    usd_fiat_rate: Option<FiatRate>,
    rate_status: BatchRateStatus,
    /// The single source of the applied rate — the mirror of the auto rate
    /// until the user overrides it (Rate invariant).
    rate_input: String,
    rate_edited: bool,
    applied: bool,
    /// Bumped on every `Open`. A result carrying an older attempt belongs
    /// to a previous open and is dropped — a previous session's rate can
    /// never leak into this one.
    attempt: u64,
}

// ---------------------------------------------------------------------------
// ViewModel
// ---------------------------------------------------------------------------

/// One preview row — validated, de-duplicated, converted. All strings are
/// value shapes; symbols, arrows and locale formatting stay in the shell.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct BatchPreviewRow {
    pub line: u32,
    pub name: Option<String>,
    pub address: String,
    pub valid: bool,
    /// Duplicate of an earlier VALID row (case-insensitive) — the first
    /// occurrence keeps the payment, this one is skipped.
    pub dup: bool,
    /// The amount as pasted (a fiat figure in fiat mode, a token figure in
    /// token mode).
    pub raw_amount: String,
    /// The converted token amount; empty when no positive rate is set in
    /// fiat mode.
    pub token_amount: String,
    pub ok: bool,
}

/// The hand-off draft for the send machine's split editor
/// (`RecipientDraft` minus the row id, which the shell assigns).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct BatchRecipient {
    pub address: String,
    /// Human token amount, decimal string.
    pub amount: String,
    pub name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct BatchView {
    pub opened: bool,
    pub unit: BatchUnit,
    pub fiat_code: String,
    pub raw_text: String,
    pub file_name: Option<String>,
    pub busy: bool,
    /// A picked file could not be read — the shell shows the alert copy.
    pub file_error: bool,
    pub template_saved: bool,
    /// `!!token.priceUsd && token.priceUsd > 0` — drives which rate hints
    /// show and the default unit.
    pub priced: bool,
    pub rate_status: BatchRateStatus,
    /// The rate string, and it always belongs to `fiat_code`. What is
    /// displayed here IS what converts every row — and, in fiat mode, EMPTY
    /// means nothing converts and `can_apply` is false. `Loading`/`Failed`
    /// both land here empty (the rate is unknown), and so does the whole
    /// round-trip after a "Priced in" switch: the previous currency's rate is
    /// not this currency's rate, so it is refused rather than re-shown under
    /// the new code. The user's own typing is the way out of all three, which
    /// is what the "Rate unavailable — enter one manually" hint asks for.
    pub rate_input: String,
    pub rate_edited: bool,
    pub preview: Vec<BatchPreviewRow>,
    /// More ok rows than the cap — only the first `max_recipients` are
    /// kept. Shown TOGETHER with `rejected` when both hold; the notices
    /// never hide each other (`BatchImportSheet.tsx:365-377`).
    pub over_cap: bool,
    /// Rows that will not be sent: invalid + duplicate + unconverted rows
    /// plus the parser's error lines.
    pub rejected: u32,
    /// `capped.len()` — what the apply button counts.
    pub recipient_count: u32,
    /// Σ of the capped token amounts, human decimal string.
    pub total_token: String,
    /// Σ of the capped fiat figures (fiat mode, > 0 only), trimmed to two
    /// decimals — the shell prefixes the symbol.
    pub total_fiat: Option<String>,
    /// The capped total exceeds the balance — apply is blocked and the
    /// total renders in the error color.
    pub over_balance: bool,
    pub can_apply: bool,
    /// The drafts `Apply` hands over (already capped and converted).
    pub recipients: Vec<BatchRecipient>,
    /// Flipped by a successful `Apply` — the shell seeds the send machine's
    /// split editor with `recipients` and closes the sheet.
    pub applied: bool,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct BatchImport;

impl App for BatchImport {
    type Event = Event;
    type Model = Model;
    type ViewModel = BatchView;
    type Effect = BatchEffect;

    fn update(&self, event: Event, model: &mut Model) -> Command<BatchEffect, Event> {
        match event {
            Event::Open {
                token,
                currency_code,
                max_recipients,
            } => {
                // Full per-open reset (invariant ⑤). The attempt bump is
                // what makes any in-flight result from a previous open
                // identifiable and droppable.
                model.attempt += 1;
                let priced = token.price_usd.is_some_and(|p| p > 0.0);
                model.unit = if priced {
                    BatchUnit::Fiat
                } else {
                    BatchUnit::Token
                };
                model.fiat_code = currency_code;
                model.raw_text.clear();
                model.file_parsed = None;
                model.file_name = None;
                model.busy = false;
                model.file_error = false;
                model.template_saved = false;
                model.usd_fiat_rate = None;
                model.rate_status = BatchRateStatus::Loading;
                model.rate_edited = false;
                model.applied = false;
                model.max_recipients = max_recipients;
                model.token = Some(token);
                sync_auto_rate(model);
                let code = model.fiat_code.clone();
                request(model, BatchOperation::FetchUsdFiatRate { code })
            }
            Event::SetUnit { unit } => {
                if model.token.is_none() {
                    return Command::done();
                }
                model.unit = unit;
                render()
            }
            Event::SetFiatCode { code } => {
                if model.token.is_none() {
                    return Command::done();
                }
                let changed = code != model.fiat_code;
                model.fiat_code = code;
                model.rate_edited = false;
                // The previous currency's rate is still in `usd_fiat_rate`,
                // and it does NOT price this one: `sync_auto_rate` finds the
                // tag no longer matches `fiat_code` and empties the mirror,
                // which blocks apply for the whole FX round-trip. (The
                // component mirrored the old rate through the switch instead
                // — USD→CNY at rate 1 paid every row ~7.2x, button green.)
                sync_auto_rate(model);
                if !changed {
                    // Re-picking the same code doesn't re-run the fetch
                    // effect (React state identity) — only the mirror reset.
                    return render();
                }
                model.rate_status = BatchRateStatus::Loading;
                let code = model.fiat_code.clone();
                request(model, BatchOperation::FetchUsdFiatRate { code })
            }
            Event::SetRawText { text } => {
                if model.token.is_none() {
                    return Command::done();
                }
                model.raw_text = text;
                model.file_parsed = None;
                model.file_name = None;
                render()
            }
            Event::PickFileRequested => {
                if model.token.is_none() || model.busy {
                    return Command::done();
                }
                model.busy = true;
                model.file_error = false;
                request(model, BatchOperation::PickFile)
            }
            Event::SaveTemplateRequested => {
                if model.token.is_none() {
                    return Command::done();
                }
                request(
                    model,
                    BatchOperation::SaveTemplateFile {
                        name: "vela-payroll-template.csv".to_owned(),
                        contents: TEMPLATE_CSV.to_owned(),
                        mime: "text/csv".to_owned(),
                    },
                )
            }
            Event::EditRate { text } => {
                if model.token.is_none() {
                    return Command::done();
                }
                // The `[^0-9.]` strip (`BatchImportSheet.tsx:300`); extra
                // dots survive here, and the applied value is the numeric
                // prefix — exactly `parseFloat(rateInput) || 0`.
                model.rate_input = text
                    .chars()
                    .filter(|c| c.is_ascii_digit() || *c == '.')
                    .collect();
                model.rate_edited = true;
                render()
            }
            Event::ResetRateToAuto => {
                if model.token.is_none() {
                    return Command::done();
                }
                model.rate_edited = false;
                sync_auto_rate(model);
                render()
            }
            Event::Apply => {
                let Some(token) = model.token.clone() else {
                    return Command::done();
                };
                let derived = derived(model, &token);
                if !derived.can_apply {
                    return Command::done();
                }
                model.applied = true;
                render()
            }
            Event::ShellCompleted { attempt, result } => {
                if attempt != model.attempt {
                    // A previous open's result — dropping it IS the
                    // "stale paste/rate never reused" rule.
                    return Command::done();
                }
                accept(model, result)
            }
        }
    }

    fn view(&self, model: &Model) -> BatchView {
        let Some(token) = &model.token else {
            return BatchView {
                opened: false,
                unit: BatchUnit::Fiat,
                fiat_code: String::new(),
                raw_text: String::new(),
                file_name: None,
                busy: false,
                file_error: false,
                template_saved: false,
                priced: false,
                rate_status: BatchRateStatus::Loading,
                rate_input: String::new(),
                rate_edited: false,
                preview: Vec::new(),
                over_cap: false,
                rejected: 0,
                recipient_count: 0,
                total_token: "0".to_owned(),
                total_fiat: None,
                over_balance: false,
                can_apply: false,
                recipients: Vec::new(),
                applied: false,
            };
        };
        let derived = derived(model, token);
        BatchView {
            opened: true,
            unit: model.unit,
            fiat_code: model.fiat_code.clone(),
            raw_text: model.raw_text.clone(),
            file_name: model.file_name.clone(),
            busy: model.busy,
            file_error: model.file_error,
            template_saved: model.template_saved,
            priced: token.price_usd.is_some_and(|p| p > 0.0),
            rate_status: model.rate_status,
            rate_input: model.rate_input.clone(),
            rate_edited: model.rate_edited,
            preview: derived.preview,
            over_cap: derived.over_cap,
            rejected: derived.rejected,
            recipient_count: derived.recipients.len() as u32,
            total_token: derived.total_token,
            total_fiat: derived.total_fiat,
            over_balance: derived.over_balance,
            can_apply: derived.can_apply,
            recipients: derived.recipients,
            applied: model.applied,
        }
    }
}

// ---------------------------------------------------------------------------
// Shell results
// ---------------------------------------------------------------------------

fn accept(model: &mut Model, result: BatchShellResult) -> Command<BatchEffect, Event> {
    match result {
        BatchShellResult::RateResolved { code, rate } => {
            if model.token.is_none() || code != model.fiat_code {
                // A same-session fetch for a currency the user has already
                // switched away from — its rate must not label the current
                // currency (the effect-cleanup `cancelled` flag's job), and
                // above all must not move `rate_status` off Loading for a
                // fetch that is still in flight. Storing it would be harmless
                // now that the rate carries its own code, but dropping it is
                // the honest record.
                return Command::done();
            }
            match rate {
                Some(rate) => {
                    // Tagged with the code it was fetched FOR — never with
                    // "whatever the sheet happens to show later".
                    model.usd_fiat_rate = Some(FiatRate { code, rate });
                    model.rate_status = BatchRateStatus::Ok;
                }
                None => {
                    model.usd_fiat_rate = None;
                    model.rate_status = BatchRateStatus::Failed;
                }
            }
            sync_auto_rate(model);
            render()
        }
        BatchShellResult::FilePicked { name, content } => {
            if !model.busy {
                return Command::done();
            }
            model.busy = false;
            model.file_name = Some(name);
            match content {
                BatchFileContent::Text { text } => {
                    // Text files flow through the same pure parser as paste.
                    model.raw_text = text;
                    model.file_parsed = None;
                }
                BatchFileContent::Matrix { rows } => {
                    model.raw_text.clear();
                    model.file_parsed = Some(interpret_rows(&rows));
                }
            }
            render()
        }
        BatchShellResult::FilePickCancelled => {
            if !model.busy {
                return Command::done();
            }
            model.busy = false;
            render()
        }
        BatchShellResult::FilePickFailed => {
            if !model.busy {
                return Command::done();
            }
            model.busy = false;
            model.file_error = true;
            render()
        }
        BatchShellResult::TemplateSaved => {
            model.template_saved = true;
            render()
        }
        // Share sheet dismissed / unavailable — keep the plain label.
        BatchShellResult::TemplateSaveFailed => Command::done(),
    }
}

// ---------------------------------------------------------------------------
// Derivation
// ---------------------------------------------------------------------------

struct Derived {
    preview: Vec<BatchPreviewRow>,
    over_cap: bool,
    rejected: u32,
    recipients: Vec<BatchRecipient>,
    total_token: String,
    total_fiat: Option<String>,
    over_balance: bool,
    can_apply: bool,
}

/// The preview pipeline (`BatchImportSheet.tsx:117-154`): parse, validate,
/// de-dupe by lowercase address (first occurrence keeps the payment),
/// convert fiat→token at the DISPLAYED rate, cap, total in base units.
fn derived(model: &Model, token: &BatchToken) -> Derived {
    let parsed = match &model.file_parsed {
        Some(parsed) => parsed.clone(),
        None => parse_recipient_table_text(&model.raw_text),
    };
    // The displayed string is the single source of the applied rate.
    let eff_price_per_token = parse_float_prefix(&model.rate_input);

    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut preview: Vec<BatchPreviewRow> = Vec::new();
    for row in &parsed.rows {
        let address = row.address.trim().to_owned();
        let valid = is_address(&address);
        let low = address.to_lowercase();
        let dup = valid && seen.contains(&low);
        if valid {
            seen.insert(low);
        }
        let fiat_num = parse_float_prefix(&row.raw_amount);
        let token_amount = match model.unit {
            BatchUnit::Fiat => {
                if eff_price_per_token > 0.0 {
                    fiat_to_token_amount(fiat_num, eff_price_per_token, token.decimals)
                } else {
                    String::new()
                }
            }
            BatchUnit::Token => row.raw_amount.clone(),
        };
        let ok = valid && !dup && parse_float_prefix(&token_amount) > 0.0;
        preview.push(BatchPreviewRow {
            line: row.line,
            name: row.name.clone(),
            address,
            valid,
            dup,
            raw_amount: row.raw_amount.clone(),
            token_amount,
            ok,
        });
    }

    let ok_rows: Vec<&BatchPreviewRow> = preview.iter().filter(|r| r.ok).collect();
    let cap = model.max_recipients as usize;
    let capped = &ok_rows[..ok_rows.len().min(cap)];
    let over_cap = ok_rows.len() > cap;

    // Base-unit totals in u128 (BigInt-equivalent). Overflow saturates to
    // MAX, which trips `over_balance` and blocks apply — explicit
    // fail-closed instead of a JS throw.
    let mut total_base: u128 = 0;
    for row in capped {
        let add = to_base_units(&row.token_amount, token.decimals).unwrap_or(u128::MAX);
        total_base = total_base.saturating_add(add);
    }
    let total_fiat_sum: f64 = if model.unit == BatchUnit::Fiat {
        capped
            .iter()
            .map(|r| parse_float_prefix(&r.raw_amount))
            .sum()
    } else {
        0.0
    };
    // An unreadable balance reads as zero — apply stays blocked.
    let bal_base = to_base_units(&token.balance, token.decimals).unwrap_or(0);
    let over_balance = total_base > bal_base;

    let rejected = (preview.len() - ok_rows.len() + parsed.errors.len()) as u32;
    let can_apply = !capped.is_empty()
        && !over_balance
        && (model.unit == BatchUnit::Token || eff_price_per_token > 0.0);

    let recipients: Vec<BatchRecipient> = capped
        .iter()
        .map(|row| BatchRecipient {
            address: row.address.clone(),
            amount: row.token_amount.clone(),
            name: row.name.clone(),
        })
        .collect();

    Derived {
        preview,
        over_cap,
        rejected,
        recipients,
        total_token: from_base_units(total_base, token.decimals),
        total_fiat: (total_fiat_sum > 0.0).then(|| trim_num(total_fiat_sum)),
        over_balance,
        can_apply,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// The auto rate for the mirror — `for_code` per 1 token, or `0.0` when it
/// cannot be known.
///
/// This is the guard that keeps a rate this machine cannot vouch for from
/// spending money. FOUR cases the mirror must NOT collapse into one:
///
/// - `None` — UNKNOWN: no source could price the currency (or the fetch has
///   not landed yet). Nothing is asserted about the rate.
/// - `Some(r)` with `r.rate <= 0` — INVALID: a source answered something that
///   is not a rate.
/// - `Some(r)` with `r.code != for_code` — MISLABELLED: a perfectly good rate
///   for a DIFFERENT currency. Just as unusable as unknown, and far more
///   dangerous, because it is a plausible number: it renders, it converts, and
///   it turns Apply green.
/// - `Some(r)` with `r.code == for_code && r.rate > 0` — KNOWN: the only case
///   that converts.
///
/// The first three return `0.0`, which empties `rate_input` and — because the
/// displayed string IS the applied rate — blocks apply until the user types a
/// rate by hand (`rate_input` is that manual channel; `BatchRateStatus::Failed`
/// is what tells them to use it).
///
/// This deliberately does NOT use `token_price_in_fiat`'s `?: 1` fallback.
/// Doing so treated an unpriceable currency as 1:1, so a `5000 CNY` payroll
/// line previewed as 5000 USDT instead of ~698 with `can_apply` still true —
/// a ~7x overpayment behind a green button. The `code` check is the same
/// refusal reached by the other road: carrying USD's rate of 1 into CNY prices
/// that identical line identically wrong. Same discipline as
/// `enforce_no_unlimited` and the "never assume 18 decimals" rule: when the
/// number that moves money is unknown — or is known to be about something
/// else — stop, do not guess.
fn auto_price_per_token(
    price_usd: Option<f64>,
    usd_fiat_rate: Option<&FiatRate>,
    for_code: &str,
) -> f64 {
    match usd_fiat_rate {
        Some(quote) if quote.code == for_code && quote.rate > 0.0 => {
            token_price_in_fiat(price_usd, quote.rate)
        }
        _ => 0.0,
    }
}

/// Keep the editable rate field mirroring the auto rate until the user
/// overrides it (`BatchImportSheet.tsx:108-111`). Significant-digit
/// formatting: a positive rate never mirrors as "0"; an unknown, invalid or
/// other-currency rate mirrors as "" (see [`auto_price_per_token`]).
///
/// Every path that can change WHICH currency is being priced runs through
/// here — `Open`, `SetFiatCode`, `ResetRateToAuto`, and each landing rate —
/// so the tag check is applied on every one of them rather than at any single
/// call site that could be forgotten.
fn sync_auto_rate(model: &mut Model) {
    if model.rate_edited {
        return;
    }
    let price_usd = model.token.as_ref().and_then(|t| t.price_usd);
    let auto = auto_price_per_token(price_usd, model.usd_fiat_rate.as_ref(), &model.fiat_code);
    model.rate_input = if auto > 0.0 {
        format_rate(auto)
    } else {
        String::new()
    };
}

/// Issue one operation whose answer must match the current attempt.
fn request(model: &mut Model, operation: BatchOperation) -> Command<BatchEffect, Event> {
    let attempt = model.attempt;
    Command::all([
        Command::request_from_shell(operation)
            .then_send(move |result| Event::ShellCompleted { attempt, result }),
        render(),
    ])
}

impl super::SplitEffect for BatchEffect {
    type Op = BatchOperation;
    fn into_shell(self) -> Option<crux_core::Request<BatchOperation>> {
        match self {
            BatchEffect::Render(_) => None,
            BatchEffect::Shell(request) => Some(request),
        }
    }
}
