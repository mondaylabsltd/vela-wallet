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
    /// The wallet's own key backup (first party): the headline row, no
    /// requester. Numbered after web and desktop's cs34/cs35 (the cap being
    /// typed), which this client does not draw as boards.
    case cs36
    var id: String { rawValue }
    /// Gallery chip label — mock naming, not translatable copy.
    var label: String { rawValue.uppercased() }
}

/// Semantic weight. `accent` is the intent sentence; the rest colour warnings.
enum SigningTone { case neutral, accent, success, caution, danger }

struct AmountLine {
    /// Rendered ahead of the value and coloured with it: "−", "+", or "".
    let sign: String
    let value: String
    let symbol: String
    /// The coin's token mark — its logo over its letters, the send flow's
    /// in-line mark, by the core's rule. It was a first letter on a brand-
    /// coloured disc, so USDC and USDT were both "U".
    var token: TokenMarkModel?
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
        custom: AllowanceInput? = nil,
        /// The batch leg this card caps; `nil` = the single approval. The
        /// sheet routes its chips and field to that leg's own events.
        leg: Int? = nil
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
        case .allowance(_, let value, _, _, _, _, _, let leg): "allowance-\(leg.map(String.init) ?? "")-\(value)"
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
    /// What stands beside it, as on a fact row: a token's mark, or a
    /// person's identicon from their address — never a letter.
    var lead: FactLead?
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
    /// The coin's mark — logo over its drawn ticker, the send flow's own.
    let mark: TokenMarkModel
    let name: String
    let balance: String
    let fee: String
    let selected: Bool
    /// The core's `insufficient`: drawn for context, never pickable (issue 211).
    var disabled = false
    /// Issue #408: why a disabled coin cannot pay, drawn under its row — the
    /// core's shortfall, need and have in the coin's own unit. `nil` = none.
    var reason: String?
}

enum FeeModel {
    /// `warning` says why the confirm is shut when the coin that pays is not
    /// there (issue #262); it sits under the row, where the other coins are.
    /// `tappable` is whether the row can DO anything: ask a failed quote
    /// again, or open a list with more than one coin in it. With one coin and
    /// a good quote there is nothing to choose, and the row is a statement —
    /// drawn without a chevron, because a control that cannot act is not
    /// dressed as one. Android has decided this in its builder since 046; web
    /// followed in spec 081. It defaults to `true` so the drawn boards, which
    /// are pictures of the tappable state, are unchanged.
    case onchain(label: String, value: String, selector: (title: String, options: [FeeTokenOption])?,
                 warning: String? = nil, tappable: Bool = true)
    /// Off-chain signature: the ✓ line, in place of a fee row.
    case offchain(note: String)
    /// Nothing at all — cs20–cs22, where there is no fee and no reassurance.
    case hidden
}

/// Spec 079: the send form's refresh control on the signing sheet's fee row.
struct FeeRefreshModel: Equatable {
    let label: String
    /// A measurement is out — the control is dimmed.
    let refreshing: Bool
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
    /// The confirm (issue #461): one tap on the shared primary button, its
    /// label the action alone — "确认兑换", "签名", "授权", "备份公钥". There is
    /// no reject BUTTON anywhere in this vocabulary; the header's ✕ is the
    /// explicit refusal, and since spec 079 nothing else closes the sheet
    /// (owner ruling: no swipe rejection).
    ///
    /// `nil` under a refusal: a dead button reads as an option somebody merely
    /// failed to use, rather than one the wallet never offered.
    let confirm: (action: String, enabled: Bool)?
    /// Spec 099 R7: why the confirm is shut, in the core's words for the part
    /// of the gate that is (`componentsUi.signing.confirmBlock.*`) — one line
    /// under it. `nil` while it may arm, or where the sheet says it its own way.
    var confirmBlockLine: String?
    /// Desktop third-column heading; the phone sheet uses it as its a11y name.
    let panelTitle: String
    /// Spec 079: the ✕'s label — the sheet's one explicit close. Empty in the
    /// gallery, which draws no ✕.
    var closeLabel = ""
    /// Spec 079 (owner: one slide): the account signs on the Trusted Signer's
    /// page, whose slide is the consent — the sheet's confirm goes there and
    /// says so (`confirmButtonLabel`, "去签名页确认") instead of the action.
    var confirmAsButton = false
    var confirmButtonLabel = ""
    /// Spec 102 D4: this account reviews and signs on a trusted page, so the
    /// sheet is the hand-off card — which page, which key, its integrity
    /// line — and not a second preview: the page is the authority.
    var handoff: HandoffCardModel?
    /// What still stands under the card: the request's status, how the page
    /// last ended, and every warning the wallet has about this request.
    var handoffBlocks: [SigningBlock] = []
    /// Spec 079: once the person has approved, the sheet stops being a form
    /// and shows this — the send receipt's own model and words, so a dApp
    /// transaction and a send look the same while they land. Also the ending
    /// the sheet keeps after the core has closed it (`SigningAftercare`).
    var receipt: SendReceiptModel?
    /// The wallet asking ITSELF (the key backup; the core's `first_party`):
    /// not a site, so no requester header — no mark, no name, no chain chip.
    /// The header is one row, `headline` and the ✕.
    var dappOwn = false
    /// The speed control under the fee (spec 069) — the send form's own.
    var feeSpeed: FeeSpeedModel?
    /// The site's own icon, tried in order OVER the letter (founder ruling
    /// 2026-09-19). Https only.
    var dappIconUrls: [String] = []
    /// The chain's logo; the dot shows until it lands, and when there is none.
    var networkLogoUrl: String?
    /// Spec 079: the fee row's refresh (owner: "似乎没有刷新网络费的按钮呀");
    /// `nil` where there is no network fee.
    var feeRefresh: FeeRefreshModel?
    /// The fee row's chevron: only where a tap opens a coin list. A failed
    /// quote is still asked again by a tap, but the refresh control says so.
    var feeChevron = true

    /// The wallet's own request leads with its outcome (issue #314): its
    /// first intent ("备份公钥") is the sheet's title, in the header row
    /// beside the ✕. `nil` on a site's request, whose intent is the eyebrow
    /// over its figure.
    var headline: (text: String, tone: SigningTone)? {
        guard dappOwn, let index = headlineIndex,
              case .intent(let text, let tone) = blocks[index] else { return nil }
        return (text, tone)
    }

    /// What the form draws: every block, less the intent the header carries.
    var formBlocks: [SigningBlock] {
        guard dappOwn, let index = headlineIndex else { return blocks }
        var rest = blocks
        rest.remove(at: index)
        return rest
    }

    private var headlineIndex: Int? {
        blocks.firstIndex { if case .intent = $0 { return true } else { return false } }
    }
}
