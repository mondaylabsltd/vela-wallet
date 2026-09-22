//
//  SigningModels.swift
//  VelaWallet
//
//  Signing view models (spec 022, data-model.md §3 — the iOS port of the web
//  reference `src/lib/signing/model.ts`): the universal renderer.
//
//  A scenario is a header, an ORDERED list of blocks, and a fixed footer.
//  Every one of the 33 CS mocks is expressible that way, and nothing in the
//  renderer knows what "a swap" is: the six-rung ERC-7730 degradation ladder
//  is made structural, so a deeper rung emits more warning blocks and fewer
//  decoded ones instead of forking the layout.
//

import SwiftUI

enum SigningStateId: String, CaseIterable, Identifiable {
    case cs1, cs2, cs3, cs4, cs5, cs6, cs7, cs8, cs9, cs10, cs11
    case cs12, cs13, cs14, cs15, cs16, cs17, cs18, cs19, cs20, cs21, cs22
    case cs23, cs24, cs25, cs26, cs27, cs28, cs29, cs30, cs31, cs32, cs33
    var id: String { rawValue }
    /// Gallery chip label — mock naming, not translatable copy.
    var label: String { rawValue.uppercased() }
}

/// Semantic weight. `accent` is the intent sentence; the rest colour warnings.
enum SigningTone { case neutral, accent, success, caution, danger }

struct TokenMark {
    let letter: String
    let tint: Color
}

struct AmountLine {
    /// Rendered ahead of the value and coloured with it: "−", "+", or "".
    let sign: String
    let value: String
    let symbol: String
    var token: TokenMark?
    var fiat: String?
    /// "支付" / "最少收到" / "存入资产" — the line's own small label.
    var caption: String?
    var tone: SigningTone = .neutral
}

struct SigningRow: Identifiable {
    let id = UUID()
    let label: String
    let value: String
    var valueTone: SigningTone = .neutral
    var mono = false
}

struct AllowanceChip: Identifiable {
    enum State { case idle, selected, disabled }
    let id: String
    let label: String
    let state: State
}

/// The field a person types their own spending cap into.
///
/// **A deviation from the drawing, and a deliberate one.** The drawn editor
/// has a 自定义 chip and nowhere to type, so choosing it selected a mode that
/// could not be completed — and an unlimited approval must be cappable to the
/// person's own figure, not only to their balance or to zero. Recorded in
/// 053's results for whoever redraws this card.
struct AllowanceInput {
    let value: String
    let symbol: String
    let placeholder: String
    var error: String?
}

struct PartyBadge {
    let text: String
    let tone: SigningTone
}

struct BalanceDeltaRow: Identifiable {
    let id = UUID()
    let symbol: String
    let delta: String
    let tone: SigningTone
}

enum SigningBlock: Identifiable {
    /// The eyebrow above the hero — "发送", "授权", "盲签".
    case intent(text: String, tone: SigningTone)
    /// The hero number. `card` boxes it in its tone (cs28's burn intercept).
    case amount(line: AmountLine, card: Bool = false, note: String? = nil)
    /// Two amount lines with the ↓ badge between them.
    case swap(pay: AmountLine, receive: AmountLine)
    case nft(id: String, collection: String)
    /// The one-sentence plain-language summary.
    case sentence(text: String, tone: SigningTone)
    case allowance(
        label: String, value: String, valueTone: SigningTone,
        chips: [AllowanceChip], note: String? = nil, resultingTotal: SigningRow? = nil,
        custom: AllowanceInput? = nil
    )
    case party(label: String, name: String, address: String? = nil, badge: PartyBadge? = nil)
    case rows([SigningRow])
    case warning(tone: SigningTone, text: String)
    case positive(String)
    /// Message, hex, typed-data JSON or calldata — always monospace.
    case code(lines: [String], note: String? = nil)
    /// A batch step or a Safe inner call.
    case card(title: String?, rows: [SigningRow], tone: SigningTone)
    case balances(title: String, rows: [BalanceDeltaRow], note: String?, noteTone: SigningTone)

    var id: String {
        switch self {
        case .intent(let text, _): "intent-\(text)"
        case .amount(let line, _, _): "amount-\(line.value)-\(line.symbol)"
        case .swap(let pay, let receive): "swap-\(pay.value)-\(receive.value)"
        case .nft(let id, _): "nft-\(id)"
        case .sentence(let text, _): "sentence-\(text.prefix(24))"
        case .allowance(_, let value, _, _, _, _, _): "allowance-\(value)"
        case .party(let label, let name, _, _): "party-\(label)-\(name)"
        case .rows(let rows): "rows-\(rows.first?.label ?? "")"
        case .warning(_, let text): "warning-\(text.prefix(24))"
        case .positive(let text): "positive-\(text.prefix(24))"
        case .code(let lines, _): "code-\(lines.first ?? "")"
        case .card(let title, _, _): "card-\(title ?? "")"
        case .balances(let title, let rows, _, _): "balances-\(title)-\(rows.count)"
        }
    }
}

struct TechIdentity: Identifiable {
    let id = UUID()
    let role: String
    let name: String
    let address: String
    var mark: TokenMark?
}

struct TechModel {
    let title: String
    /// Byte count shown on the collapsed row when there is one ("· 412 字节").
    var summary: String?
    var fn: (label: String, signature: String)?
    var params: [SigningRow] = []
    var identities: [TechIdentity] = []
    var simResult: SigningRow?
    var raw: (label: String, hex: String)?
    let copyLabel: String
    let explorerLabel: String

    /// Nothing to disclose. A refused request nulls every field this card
    /// would show (spec 081); drawing the row anyway promises content and then
    /// opens on an empty panel. Found on an Android device, fixed on both.
    var isEmpty: Bool {
        summary == nil && fn == nil && params.isEmpty && identities.isEmpty
            && simResult == nil && raw == nil
    }
}

struct FeeTokenOption: Identifiable {
    let id: String
    let mark: TokenMark
    let name: String
    let balance: String
    let fee: String
    let selected: Bool
    /// The core's `insufficient`: drawn for context, never pickable (issue 211).
    var disabled = false
}

enum FeeModel {
    /// `warning` says why the slide is shut when the coin that pays is not
    /// there (issue #262); it sits under the row, where the other coins are.
    case onchain(label: String, value: String, selector: (title: String, options: [FeeTokenOption])?,
                 warning: String? = nil)
    /// Off-chain signature: the ✓ line, in place of a fee row.
    case offchain(note: String)
    /// Nothing at all — cs20–cs22, where there is no fee and no reassurance.
    case hidden
}

struct SigningModel {
    let id: SigningStateId
    let dapp: (name: String, host: String, letter: String, tint: Color)
    let network: (name: String, dot: Color)
    let blocks: [SigningBlock]
    let tech: TechModel
    /// cs29 ships the disclosure open — the whole point of that mock.
    let techOpen: Bool
    /// `nil` under a refusal (spec 081): a fee for a transaction nobody will
    /// send is a number about nothing.
    let fee: FeeModel?
    var signer: (label: String, name: String, seed: String)
    /// The slide. There is no reject button anywhere in this vocabulary:
    /// closing the sheet is the rejection (product contract, SPEC 签名).
    ///
    /// `nil` under a refusal: a dead slide reads as an option somebody merely
    /// failed to use, rather than one the wallet never offered.
    let confirm: (hint: String, action: String, enabled: Bool)?
    /// Desktop third-column heading; the phone sheet uses it as its a11y name.
    let panelTitle: String
    /// The wallet asking ITSELF (the key backup): its own mark and name, and
    /// no host — it is not a site.
    var dappOwn = false
    /// The speed control under the fee (spec 069) — the send form's own.
    var feeSpeed: FeeSpeedModel?
    /// The site's own icon, tried in order OVER the letter (founder ruling
    /// 2026-09-19). Https only.
    var dappIconUrls: [String] = []
    /// The chain's logo; the dot shows until it lands, and when there is none.
    var networkLogoUrl: String?
    /// "Sign with · Automatic ›" — where the passkey that signs this is. Live only.
    var signWith: SignWithModel?
}

/// The per-request choice of WHERE the signing passkey is; opens in place.
struct SignWithModel: Equatable {
    struct Option: Equatable, Identifiable {
        let id: String
        let title: String
        let selected: Bool
    }

    let label: String
    let value: String
    let open: Bool
    let options: [Option]
}
