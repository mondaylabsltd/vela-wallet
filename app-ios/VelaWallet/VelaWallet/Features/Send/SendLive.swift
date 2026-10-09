//
//  SendLive.swift
//  VelaWallet
//
//  The send journey, wearing real state.
//
//  A sibling of `WalletFlowFixtures`, exactly as `WalletLive` is of
//  `WalletFixtures` and `FlowsLive` is of the flows: each builder takes the
//  drawn model as a **fallback** and replaces only the fields the core owns.
//  That is what keeps the gallery and the screenshot sweep pixel-identical, and
//  it is also what keeps the corpus delta at zero — every string still comes
//  from the fixture that resolved it.
//
//  ## What this file is not allowed to do
//
//  Decide. The amount is the core's (`token_amount`), the total is the core's
//  (`confirm_amount` — a split's sum comes from the same function the money
//  gates read, not from the shell adding rows up a second time), the fee is the
//  core's, the refusals are the core's words, and both CTAs are gated by
//  `can_continue` / `can_confirm`. What this file does is FORMAT, because the
//  core's own doc says base units cross the wire and the shell renders them.
//

import SwiftUI
import VelaCore

/// The fee's failure in words (PR 2 note 1) — what every fee row draws: the
/// Send form's, the confirm's and the signing sheet's.
extension FeeFailureViewWire {
    /// The figures this build draws: "Tap to retry" (only when a tap is the
    /// one way) and "Pay with another coin" (a tap opens the coins, PR 2
    /// polish). Any other key is the dash — never a dotted key path.
    static let figureKeys: Set<String> = [
        I18nKeys.CoreRound.estimateFailed, I18nKeys.CoreRound.payWithAnotherCoin,
    ]

    /// The row's figure: the core's key, saying what a tap does, else the dash.
    func figure(_ loc: Loc) -> String {
        guard let key = figureKey, Self.figureKeys.contains(key) else { return "—" }
        return loc.t(key)
    }

    /// `would_fail`'s own sentence — the failure's `footer_key`, "This would
    /// fail if sent as it is." — for a surface with no held confirm to say it
    /// under (the Send form's row). `nil` for every other failure.
    func wouldFailLine(_ loc: Loc) -> String? {
        guard failure == "would_fail", reasonKey == nil,
              footerKey == I18nKeys.CoreRound.feeWouldFail
        else { return nil }
        return loc.t(footerKey)
    }

    /// What a tap on the row does — exactly the core's `tap` (PR 2 polish).
    var rowTap: FeeRowTap {
        switch tap {
        case .retry: .retry
        case .chooseCoin: .chooseCoin
        case .nothing: .nothing
        }
    }

    /// The row's reason line, `{{chain}}` filled with the chain's name; `nil`
    /// when the core gives none (not the network's doing).
    func reason(_ loc: Loc, chain: String) -> String? {
        self.reasonKey.map { loc.t($0, vars: ["chain": chain]) }
    }
}

enum SendLive {

    /// Which drawn state the live view is in.
    ///
    /// The flow host renders by this rather than by remembering where it
    /// navigated from — a back-swipe and a core-driven stage change must agree,
    /// and only one of them can be the authority.
    static func flowState(_ view: SendViewWire, feeSheetOpen: Bool) -> FlowStateId {
        switch view.stage {
        case .selectToken, .lockResolving, .lockError:
            return .sd1
        case .enterDetails:
            if view.showContactPicker { return .sd2e }
            if view.showBatchImport { return .sd2c }
            if feeSheetOpen { return .sd2f }
            if view.multiSelectMode { return .sd2d }
            return view.splitMode ? .sd2b : .sd2
        case .confirm:
            if feeSheetOpen { return .sd2f }
            if view.multiSelectMode { return .sd3c }
            return view.splitMode ? .sd3b : .sd3
        case .receipt:
            switch view.receipt?.status {
            case "confirmed": return .sd4c
            case "submitted", "failed", "maybe_sent", "not_sent": return .sd4b
            default: return .sd4a
            }
        }
    }

    /// The drawn states the send machine owns. Anything else is still a
    /// drawing.
    static let flowStates: Set<FlowStateId> = [
        .sd1, .sd1b, .sd2, .sd2b, .sd2c, .sd2d, .sd2e, .sd2f, .sd3, .sd3b, .sd3c, .sd4a, .sd4b, .sd4c,
    ]

    /// What the header's back arrow asks of the send machine (087 F27).
    enum Back: Equatable {
        /// `back`: the core steps back — confirm → form, form → picker (a
        /// recipient handed in survives that, #332) — and from the picker it
        /// closes the journey itself.
        case step
        /// `done`: nothing in this journey lies behind the screen — a receipt,
        /// or a request still resolving or one this wallet cannot honour,
        /// which are drawn AS the picker. The core closes the journey.
        case leave
    }

    /// The arrow, by the core's stage — never by the shell's stack.
    ///
    /// The arrow popped the shell's `FlowNav` and the machine never heard it:
    /// from the form it dropped the person on the home (the stack holds `.sd1`
    /// for the whole journey), and the next 转账 resumed the abandoned journey
    /// — the last code's recipient and network scope on a new send. The lock
    /// stages leave rather than step: their `back` moves the machine between
    /// steps the person cannot see, and the same screen would need a second
    /// tap.
    static func back(_ view: SendViewWire) -> Back {
        switch view.stage {
        case .selectToken, .enterDetails, .confirm: return .step
        case .receipt, .lockResolving, .lockError: return .leave
        }
    }

    // MARK: - SD1, the token picker

    static func pick(
        _ view: SendViewWire, on model: SendPickModel, picking: Bool = false,
        classFilter: String = "all", loc: Loc, networks: WalletNetworks = .builtin
    ) -> SendPickModel {
        // Which class of token is on screen. The CORE lists every holding —
        // which subset a person is looking at is a render decision, like the
        // sweep's picking flag, and the chips were drawn in 021 with nothing
        // behind them.
        let shown = view.tokens.filter { matches(classFilter, token: $0) }
        // Issue #312: no network pill while a scanned code names the network —
        // the notice says which one, and a pill that opened the network sheet
        // there could choose nothing the list would follow.
        var header = model.header
        if view.requestChainId != nil { header.pill = nil }
        return SendPickModel(
            header: header,
            recipient: pickRecipient(view, loc: loc),
            searchPlaceholder: model.searchPlaceholder,
            // Exactly one chip lit, and it is the one in force.
            filters: model.filters.map { chip in
                FilterChipModel(id: chip.id, label: chip.label, selected: chip.id == classFilter)
            },
            // Once a chain is pinned, say which — the dimmed rows are the
            // consequence and this is the reason. The corpus has no sentence
            // for "locked to", so the summary line's own words carry it:
            // "N 个代币 · Gnosis" is exactly what has been chosen.
            // A scanned request this wallet cannot fulfil says so HERE — on the
            // screen the core put the person on. Until 055 the flow landed on
            // the picker with nothing to explain why the code did not work.
            //
            // The core's own `添加该网络` affordance has no drawn home on this
            // client; recorded in results rather than invented.
            notice: lockNotice(view, loc: loc, networks: networks)
                ?? requestNotice(view, loc: loc, networks: networks)
                ?? (picking ? view.multiChainId.map { chainId in
                    SendNoticeModel(
                        // The chain the sweep is locked to, in the network's
                        // own mark (the kind rule) — never the drawing's
                        // Ethereum, never a blank disc.
                        mark: networkMark(chainId, networks: networks),
                        text: loc.t("send.multiSendSummary", vars: [
                            "n": String(view.multiSelectedIds.count),
                            "chain": networks.meta(chainId)?.displayName ?? "",
                        ])
                    )
                } : nil),
            rows: shown.map { assetRow($0, networks: networks) },
            selection: picking ? SendSelectionModel(
                selected: shown.map { view.multiSelectedIds.contains($0.id) },
                // A row on a chain the pick has left behind is drawn dimmed and
                // is not tappable: a tappable row is an invitation the wallet
                // will not honour.
                dimmed: shown.map { SweepPick.dimmed(view: view, chainId: $0.chainId) },
                selectAll: model.selection?.selectAll ?? loc.t("send.selectAllValuable")
            ) : nil,
            cta: picking
                ? SendCtaModel(
                    label: loc.t("send.multiSendContinue", vars: [
                        "n": String(view.multiSelectedIds.count),
                        "chain": view.multiChainId
                            .flatMap { networks.meta($0)?.displayName } ?? "",
                    ]),
                    // Nothing ticked is nothing to send.
                    accent: !view.multiSelectedIds.isEmpty
                )
                : model.cta,
            // An empty list says why, once the core has looked: on a network a
            // scanned code named, "nothing here" is the answer (issue #312).
            empty: view.loading ? nil
                : loc.t(view.tokens.isEmpty ? "send.noTokensWithBalance" : "send.noMatchingTokens")
        )
    }



    /// Issue #312: the network a scanned code named, in the receive card's own
    /// words ("BNB Chain payments only"). Above an empty list it is also why
    /// the list is empty.
    static func requestNotice(
        _ view: SendViewWire, loc: Loc, networks: WalletNetworks = .builtin
    ) -> SendNoticeModel? {
        guard let chainId = view.requestChainId else { return nil }
        return SendNoticeModel(
            mark: networkMark(chainId, networks: networks),
            text: loc.t("receive.shareCardNetworkNote", vars: [
                "network": networks.meta(chainId)?.displayName ?? "Chain \(chainId)",
            ])
        )
    }

    /// A NETWORK in a line of text — a notice that locks a chain, the
    /// confirm's Network row: its own logo over its coin's letters, never a
    /// badge, never its coin's logo (the kind rule: ETH on Base is Base's
    /// logo here). `nil` for a chain the wallet cannot name, which has no
    /// letters to fall back on — no mark rather than an empty disc.
    static func networkMark(_ chainId: Int, networks: WalletNetworks) -> TokenMarkModel? {
        guard let chain = networks.meta(chainId) else { return nil }
        return TokenMarkModel.chain(chainId: chainId, symbol: chain.nativeSymbol,
                                    color: chainColor(chainId))
    }

    /// Issue #332: the picker's "To" line — the recipient the core already
    /// holds, worded as the confirm page words it (spec 097 F), so the person
    /// sees whom they are paying while they choose what. Nobody held, no line.
    static func pickRecipient(_ view: SendViewWire, loc: Loc) -> FactRowModel? {
        let address = view.recipient.trimmingCharacters(in: .whitespaces)
        guard !address.isEmpty else { return nil }
        return payeeFact(
            label: loc.t("send.toLabel"), address: address, payee: singlePayee(view), loc: loc
        )
    }

    // MARK: - Who is paid (spec 097 F, S2)

    /// The one payee of a single send or a sweep, as the core named them —
    /// never a split's first row.
    static func singlePayee(_ view: SendViewWire) -> SendPayeeWire? {
        view.splitMode ? nil : view.payees?.first
    }

    /// Whose word a payee's name is, when it is not the person's own: "Vela
    /// User" for the public registry, where anyone can register any name; a
    /// name service's own label ("ENS") as it is.
    static func payeeTag(_ payee: SendPayeeWire, loc: Loc) -> String? {
        switch payee.nameSource {
        case .registry: loc.t("send.velaUser")
        case .service(let label): label
        case .own, .unknown, nil: nil
        }
    }

    /// The payee's name, when the core gave one and says whose word it is.
    /// `nil` when nobody named them — or when nobody can say whose word the
    /// name is, which untagged would read as the person's own.
    static func payeeKnownName(_ payee: SendPayeeWire?) -> String? {
        guard let payee, let name = payee.name, let source = payee.nameSource,
              source != .unknown else { return nil }
        return name
    }

    /// "Wallet · Vela User", "bob.eth · ENS", "Savings" — the form's line,
    /// which wraps rather than cuts.
    static func payeeName(_ payee: SendPayeeWire?, loc: Loc) -> String? {
        guard let payee, let name = payeeKnownName(payee) else { return nil }
        return payeeTag(payee, loc: loc).map { "\(name) · \($0)" } ?? name
    }

    /// The line under a named payee on a row that may cut its name: whose
    /// word the name is and the short address together, "Vela User ·
    /// 0x14fB…eA5c" — never cut. A long registry name must not push the tag
    /// out of sight with itself.
    static func payeeDetail(_ payee: SendPayeeWire, short: String, loc: Loc) -> String {
        payeeTag(payee, loc: loc).map { "\($0) · \(short)" } ?? short
    }

    /// The To row: a name never stands in for the address on the page that
    /// signs (S2 — "Wallet", from the public registry, with the address one
    /// tap away). Named, the name alone (it may be cut) over the tag and the
    /// short address (never cut); unnamed, the short address in mono. The
    /// face is the full address's, and a tap shows it whole. Artwork only for
    /// a real address (the founder's anti-poisoning rule).
    static func payeeFact(
        label: String, address: String, payee: SendPayeeWire?, loc: Loc
    ) -> FactRowModel {
        let address = payee?.address ?? address
        let short = AddressText.short(address)
        let name = payeeKnownName(payee)
        return FactRowModel(
            label: label,
            value: name ?? short,
            lead: isAddress(address) ? .identicon(address) : nil,
            mono: name == nil,
            detail: payee.flatMap { name == nil ? nil : payeeDetail($0, short: short, loc: loc) }
        )
    }

    /// Which class a holding belongs to.
    ///
    /// The same three the other clients use: a stablecoin by symbol, the
    /// chain's own coin (what pays for gas), and everything else. A chip nobody
    /// can act on would be worse than none, so `all` matches everything.
    static func matches(_ filter: String, token: SendTokenWire) -> Bool {
        // **Gas wins.** A chain's own coin can also be a stablecoin — xDAI on
        // Gnosis is both — and a token in two classes means the chips are not a
        // partition: tap 稳定币 then Gas and the same coin is in both lists,
        // which reads as a filter that does not work. Being the coin that pays
        // the fee is the more actionable fact on a send screen, so it is the
        // class the coin gets.
        let isNative = token.tokenAddress == nil
        switch filter {
        case "gas": return isNative
        case "stable": return !isNative && stableSymbols.contains(token.symbol.uppercased())
        case "other":
            return !isNative && !stableSymbols.contains(token.symbol.uppercased())
        default: return true
        }
    }

    /// The symbols a wallet treats as dollars. Not a price judgement — a list,
    /// so the chip means the same thing on every client.
    private static let stableSymbols: Set<String> = [
        "USDC", "USDT", "DAI", "XDAI", "USDC.E", "USDBC", "FDUSD", "TUSD",
        "USDE", "PYUSD", "USDS", "PATHUSD",
    ]

    /// Why a scanned request cannot be fulfilled, in the core's words.
    ///
    /// `lock_error` is a field the core has always computed and this client was
    /// not reading: a code for a chain the wallet does not have put the send
    /// flow into a stage with nothing drawn on it at all.
    ///
    /// A notice about a NETWORK wears that network's mark — when the wallet
    /// can name it; a code for a chain it does not have gets none. One about
    /// a token, or about an add-network attempt, names no network and wears
    /// no mark: these were an empty grey disc.
    static func lockNotice(
        _ view: SendViewWire, loc: Loc, networks: WalletNetworks = .builtin
    ) -> SendNoticeModel? {
        switch view.lockError {
        case .network(let chainId):
            return SendNoticeModel(
                mark: networkMark(chainId, networks: networks),
                text: "\(loc.t("send.lock.netTitle")) · "
                    + loc.t("send.lock.netBody", vars: ["chainId": String(chainId)])
            )
        case .token:
            return SendNoticeModel(
                mark: nil,
                text: "\(loc.t("send.lock.tokenTitle")) · \(loc.t("send.lock.tokenBody"))"
            )
        case nil:
            // An add-network attempt that FAILED still owes a sentence — the
            // person asked for something and it did not happen.
            switch view.addNetworkMsg {
            case .netNotFound:
                return SendNoticeModel(mark: nil, text: loc.t("send.lock.netNotFound"))
            case .netNotCompatible:
                return SendNoticeModel(mark: nil, text: loc.t("send.lock.netNotCompatible"))
            case .netAddError:
                return SendNoticeModel(mark: nil, text: loc.t("send.lock.netAddError"))
            case nil:
                return nil
            }
        }
    }

    private static func assetRow(_ token: SendTokenWire, networks: WalletNetworks) -> AssetRowModel {
        AssetRowModel(
            ticker: token.symbol,
            chain: networks.meta(token.chainId)?.displayName ?? token.network,
            badgeColor: chainColor(token.chainId),
            // The asset list's own formatter (`WalletLive.trimBalance`): the
            // picker lists the same holdings, so it prints them the same way.
            balance: "\(WalletLive.tokenAmountText(token.balance)) \(token.symbol)",
            // The picker's job is to choose an asset, so the row states the
            // holding. A priced row's fiat line is the home's business.
            fiat: token.priceUsd == nil ? .noPrice("") : .value(""),
            masked: false,
            // The coin's own mark, as the home's row draws the same holding:
            // its logo, and the badge only where it would not repeat it. With
            // none, the row fell back to the drawing's letters and an
            // always-on dot — BNB on BNB Chain as "BNB" with a yellow dot.
            mark: coinMark(token)
        )
    }

    /// A held coin's mark: the core's rule on its chain, contract and the
    /// logo URLs the core already named for it.
    static func coinMark(_ token: SendTokenWire) -> TokenMarkModel {
        TokenMarkModel.of(
            chainId: token.chainId, symbol: token.symbol,
            tokenAddress: token.tokenAddress, color: chainColor(token.chainId),
            named: token.logoUrls
        )
    }

    // MARK: - SD2, the form

    /// The speed control's inputs (spec 069): the `fee_speed` core's view, and
    /// the fee session pricing each tier — whose fee-coin options format that
    /// option's fee, as the fee row formats its own.
    struct SpeedInputs {
        let view: FeeSpeedViewWire
        let feeView: (String) -> FeeViewWire?
    }

    static func form(
        _ view: SendViewWire,
        fee: FeeViewWire?,
        display: WalletLive.Display,
        on model: SendFormModel,
        loc: Loc,
        speed: SpeedInputs? = nil,
        networks: WalletNetworks = .builtin
    ) -> SendFormModel {
        var live = model
        let token = view.selectedToken
        let symbol = token?.symbol ?? ""
        let chain = token.map { networks.meta($0.chainId)?.displayName ?? $0.network } ?? ""

        // **No token, no card** (issue #209). The drawn SD2 arrives with the
        // mocks' "USDT · Ethereum · Balance 53.4836" already in it, and
        // falling back to it whenever the core has not named a token quoted a
        // balance to somebody who may hold nothing — the web hand-off found
        // it on an account showing $0.00. `selected_token` is null for
        // reachable reasons: the form opens for a handed-off recipient before
        // the token list answers, and a load that fails never names one. The
        // body draws the card only when there IS one.
        live.token = token.map { held in
            SendTokenCardModel(
                // The held token's own logo (058), with its lettermark behind.
                mark: coinMark(held),
                symbol: held.symbol,
                // "Gnosis · Balance 0.53097" — labelled, as the other shells
                // say it, and formatted by the very call the asset list's row
                // makes for this balance (spec 078): the figure here and the
                // figure on the home row are one number, digit for digit. The
                // core's full precision used to go on this line unrounded.
                detail: "\(chain) · " + loc.t("send.balanceLabel", vars: ["amount": WalletLive.tokenAmountText(held.balance)]),
                max: model.token?.max,
                // Issue #326: the card is the way to another asset, where the
                // core says the asset is the payer's to change.
                change: view.canChangeToken == true ? loc.t("send.selectTokenTitle") : nil
            )
        }

        // The field shows what was typed; the line under it shows the OTHER
        // unit. When the figure is already fiat the other unit is the token's,
        // which is `token_amount` — the very number the signed batch is built
        // from, so the two can never disagree.
        let unit = unitAdornment(code: view.amountFiatCode, symbol: symbol)
        live.amount = model.amount.map { drawn in
            AmountFieldModel(
                value: view.amount.isEmpty ? "0" : view.amount,
                // The token figure behind a typed currency amount reads like
                // every other token figure — the core's exact `token_amount`
                // runs to eighteen places, which is not something to glance at.
                fiat: view.amountFiatCode != nil
                    ? "≈ \(WalletLive.tokenAmountText(view.tokenAmount.isEmpty ? "0" : view.tokenAmount)) \(symbol)"
                    : fiatLine(view, token: token, display: display),
                // What the figure is typed IN: its own code, or the token's
                // symbol — never the display currency, which is not the unit
                // of a figure typed in token units at all.
                denomLabel: view.amountFiatCode ?? symbol,
                // The core's three judgements about the ⇄ control, all of
                // which this client was dropping (spec 056's dropped-judgement
                // ruler). Absent, refused-with-a-reason and offered are three
                // states, and a chevron that silently does nothing is the one
                // nobody can act on.
                denomShown: view.denomToggleShown,
                denomEnabled: view.denomToggleEnabled,
                denomReason: view.denomToggleReason.map { issue in
                    loc.t("send.warnCannotConvert", vars: [
                        "code": issue.code, "symbol": issue.symbol,
                    ])
                },
                // A scanned or linked amount is not the person's to change.
                locked: view.amountLocked,
                unitPrefix: unit.prefix,
                unitSuffix: unit.suffix
            )
        }

        live.recipient = model.recipient.map { drawn in
            RecipientFieldModel(
                label: drawn.label,
                lines: AddressText.lines(view.recipient),
                // Only a real address earns an identicon — the founder's
                // anti-poisoning rule: a seed drawn from half-typed text is a
                // picture that changes as somebody types and means nothing.
                // Only a real address earns a face. An empty seed draws a
                // themed circle, which is what "nobody yet" looks like.
                identiconSeed: isAddress(view.recipient) ? view.recipient : "",
                pickLabel: drawn.pickLabel,
                scanLabel: drawn.scanLabel,
                // A token's own contract (spec 096 F12) is said first, in the
                // warning tone; else who the core says this is, and whose word
                // that is (spec 097 F) — never the resolver's own label.
                note: view.recipientIsTokenContract == true
                    ? loc.t("send.recipientTokenContract")
                    : payeeName(singlePayee(view), loc: loc) ?? drawn.note,
                noteWarn: view.recipientIsTokenContract == true
            )
        }

        // The sweep's rows: **what actually moves**, not what the picker showed.
        //
        // A sweep is not "the whole balance" — the core reserves what the fee
        // needs on the asset that pays it, so a row that showed 0.5 xDAI in the
        // picker sends slightly less. Reading the balances here would promise a
        // figure the operation does not carry.
        if view.multiSelectMode {
            let ticked = view.tokens.filter { view.multiSelectedIds.contains($0.id) }
            live.sweepRows = ticked.map { token in
                let spec = view.multiSpecs.first {
                    ($0.tokenAddress ?? "") == (token.tokenAddress ?? "")
                }
                return SweepRowModel(
                    mark: coinMark(token),
                    symbol: token.symbol,
                    balanceLabel: "\(WalletLive.tokenAmountText(token.balance)) \(token.symbol)",
                    // **No spec, no figure.** Falling back to the balance would
                    // print a number the operation does not carry — the whole
                    // reason these rows read `multi_specs` at all. An empty
                    // amount is "not worked out yet", which is true.
                    amount: spec.map { "\(WalletLive.tokenAmountText($0.amount)) \(token.symbol)" } ?? "",
                    // No Max on a sweep row: the sweep already moves the
                    // maximum of each one, and the only event behind that chip
                    // is `tap_max`, which acts on the SINGLE selected token —
                    // tapping it on the third row would silently change a
                    // different amount. Android draws the chip and drops the
                    // index; this is the recorded deviation.
                    max: ""
                )
            }
            live.sweepSummary = loc.t("send.multiSendSummary", vars: [
                "n": String(ticked.count),
                "chain": view.multiChainId
                    .flatMap { networks.meta($0)?.displayName } ?? "",
            ])
            // A sweep has no single amount and no single token: the rows are
            // the amount, and the header card would name one of several.
            live.amount = nil
            live.token = nil
        }

        // The split's rows, each with the core's verdict on it.
        let issues = view.splitMode ? view.splitRowIssues ?? [] : []
        live.recipients = view.splitMode ? view.recipients.enumerated().map { index, row in
            let issue = issues.first { $0.id == row.id }
            return RecipientCardModel(
                ordinal: loc.t("send.recipientN", vars: ["n": String(index + 1)]),
                name: row.name ?? (row.address.isEmpty
                    ? loc.t("send.recipientPlaceholder")
                    : AddressText.short(row.address)),
                // Artwork is for an ADDRESS, and the core says when the field
                // is not one yet — a half-typed "0x1234" draws nobody's face.
                identiconSeed: !row.address.isEmpty && issue?.address != .invalid ? row.address : "",
                amount: "\(trim(row.amount)) \(symbol)",
                removeLabel: loc.t("send.removeRecipient"),
                pickLabel: loc.t("send.recipientPickAria"),
                scanLabel: loc.t("send.scanAria"),
                rowId: row.id,
                problem: rowProblem(row, in: view, loc: loc)
            )
        } : live.recipients

        // A split's sweep-style summary: how many people, and the sum.
        live.summary = view.splitMode
            ? SummaryLineModel(
                // The PLURAL key. `send.recipientCount` on its own does not
                // exist in the corpus — only `_one` and `_other` — so asking
                // for the bare name printed the literal string
                // "send.recipientCount" on the confirm summary, which is what
                // the device's accessibility dump showed.
                label: loc.t(
                    view.recipients.count == 1
                        ? "send.recipientCount_one"
                        : "send.recipientCount_other",
                    vars: ["count": String(view.recipients.count)]
                ),
                // A row that cannot be summed yet leaves the total blank — a
                // dash, not a bare symbol with no figure in front of it.
                value: view.confirmAmount.isEmpty ? "—" : "\(trim(view.confirmAmount)) \(symbol)",
                over: view.splitOverBalance,
                remaining: view.splitRemaining.map { left in
                    loc.t("send.splitRemaining", vars: ["amount": "\(trim(left)) \(symbol)"])
                }
            )
            : live.summary

        return SendFormModel(
            // The title names the token being SENT. The fixture's said USDT,
            // which on a wallet holding xDAI is a sentence about somebody
            // else's money — and with no token at all, "Send " names nothing,
            // so the plain verb stands (#209).
            header: FlowHeaderModel(
                title: token == nil
                    ? loc.t("tokenDetail.send")
                    : loc.t("send.sendTitle", vars: ["symbol": symbol]),
                backLabel: model.header.backLabel,
                action: model.header.action,
                pill: model.header.pill
            ),
            mode: live.mode,
            token: live.token,
            sweepSummary: live.sweepSummary,
            sweepRows: live.sweepRows,
            amount: live.amount,
            recipient: live.recipient,
            addRecipient: live.addRecipient,
            recipients: live.recipients,
            recipientActions: live.recipientActions,
            summary: live.summary,
            fee: feeRow(model.fee, view: view, fee: fee, display: display, loc: loc, speed: speed,
                        networks: networks),
            speed: speed.map { speedModel($0, view: view, display: display, loc: loc, networks: networks) },
            cta: stopRetry(view, loc: loc) ?? live.cta,
            // While the pre-check is out the button is busy, not unfinished.
            hint: view.splitMode && !view.estimatingGas ? splitHint(issues, loc: loc) : nil,
            fillEmpty: view.splitMode ? fillEmpty(view, issues: issues, symbol: symbol, loc: loc) : nil
        )
    }

    /// "Use 0.5 ETH for the empty rows" (the web's `fillEmpty`): offered while
    /// the core flags a row's amount as empty and another row has a figure the
    /// core accepts. The figure is the first such row's, exactly as typed —
    /// nothing is computed.
    static func fillEmpty(
        _ view: SendViewWire, issues: [SendSplitRowIssueWire], symbol: String, loc: Loc
    ) -> FillEmptyModel? {
        guard issues.contains(where: { $0.amount == .empty }) else { return nil }
        let flagged = Dictionary(issues.map { ($0.id, $0.amount) }, uniquingKeysWith: { first, _ in first })
        guard let source = view.recipients.first(where: { row in
            !row.amount.trimmingCharacters(in: .whitespaces).isEmpty && (flagged[row.id] ?? .ok) == .ok
        }) else { return nil }
        return FillEmptyModel(
            label: loc.t("send.splitFillEmpty", vars: ["amount": "\(trim(source.amount)) \(symbol)".trimmingCharacters(in: .whitespaces)]),
            amount: source.amount
        )
    }

    /// "Recipient 2 needs an amount." — the first unfinished row, and what it
    /// still needs, from the core's list. The core says WHICH row; this only
    /// picks the sentence.
    static func splitHint(_ issues: [SendSplitRowIssueWire], loc: Loc) -> String? {
        guard let first = issues.first else { return nil }
        return loc.t(
            first.address == .ok ? "send.splitNeedsAmount" : "send.splitNeedsAddress",
            vars: ["n": String(first.ordinal)]
        )
    }

    /// The unit drawn beside a figure being typed (issue 231, the web's
    /// `unitAdornment`). `code` is the figure's OWN currency
    /// (`amount_fiat_code`); `nil` means token units, and the unit is the
    /// token's symbol. A currency with a symbol leads the figure, one the
    /// catalog has no symbol for follows it as its code, a token always
    /// follows. Nothing defaults to "$".
    static func unitAdornment(code: String?, symbol: String) -> (prefix: String?, suffix: String?) {
        guard let code else { return (nil, symbol.isEmpty ? nil : symbol) }
        if let glyph = CurrencyCatalog.entry(code)?.glyph, !glyph.isEmpty {
            return (glyph, nil)
        }
        return (nil, code)
    }

    /// What is wrong with **this** row, in the core's words.
    ///
    /// A list of six rows with one sentence underneath makes somebody count
    /// rows to find the bad one. Every verdict here is the core's
    /// (`split_row_issues`, `split_duplicates`) — the shell used to compare
    /// addresses itself, a second rule to keep in step with the importer's,
    /// and called a repeat "skipped" when the batch still pays it twice.
    /// Only a field with something IN it can be wrong: an empty one is
    /// unfinished, and the hint above Continue already says so.
    static func rowProblem(
        _ row: SendRecipientDraftWire, in view: SendViewWire, loc: Loc
    ) -> String? {
        let issue = view.splitRowIssues?.first { $0.id == row.id }
        let notes = [
            issue?.address == .invalid ? loc.t("send.batchBadAddress") : nil,
            duplicateNote(view, id: row.id, loc: loc),
            issue?.amount == .invalid ? loc.t("send.badAmount") : nil,
        ].compactMap { $0 }
        return notes.isEmpty ? nil : notes.joined(separator: " · ")
    }

    /// "Same address as recipient 2", for a row the core flagged as a repeat
    /// of an earlier one. `nil` for every other row — including the FIRST
    /// occurrence, which is not the mistake.
    static func duplicateNote(_ view: SendViewWire, id: String, loc: Loc) -> String? {
        (view.splitDuplicates ?? []).first { $0.id == id }.map { flagged in
            loc.t("send.recipientDuplicate", vars: ["n": String(flagged.firstOrdinal)])
        }
    }

    /// The form's repeat warnings, said again on the page that signs: one line
    /// per repeating row, "Recipient 3 · Same address as recipient 1".
    static func confirmRepeatNote(_ view: SendViewWire, loc: Loc) -> String? {
        guard view.splitMode else { return nil }
        let lines = view.recipients.enumerated().compactMap { index, row in
            duplicateNote(view, id: row.id, loc: loc).map { note in
                "\(loc.t("send.recipientN", vars: ["n": String(index + 1)])) · \(note)"
            }
        }
        return lines.isEmpty ? nil : lines.joined(separator: "\n")
    }

    /// The ≈ line beside a token-denominated figure.
    private static func fiatLine(
        _ view: SendViewWire, token: SendTokenWire?, display: WalletLive.Display
    ) -> String {
        guard let price = token?.priceUsd, let typed = Double(view.tokenAmount) else { return "" }
        let converted = typed * price * display.rate
        // A figure this large is not a price, it is a parse going wrong
        // somewhere upstream — a device run typed an address into the amount
        // field and this line rendered ¥1.9e49. A fiat line nobody could read
        // is worse than none, and printing it lends the garbage authority.
        guard converted.isFinite, converted < 1e15 else { return "" }
        // No rate means the figure is still USD, and it says so rather than
        // wearing another currency's glyph (FR-009, since 050).
        return "≈ \(display.glyph)\(Formats.number(converted, minimumFractionDigits: 2, maximumFractionDigits: 2))"
    }

    private static func feeRow(
        _ fallback: FeeRowModel, view: SendViewWire, fee: FeeViewWire?,
        display: WalletLive.Display, loc: Loc, speed: SpeedInputs? = nil,
        networks: WalletNetworks = .builtin
    ) -> FeeRowModel {
        // A figure switched to another coin is measured again before it can
        // be confirmed (PR 2 §4): the row stays measuring until it has.
        let busy = view.estimatingGas || view.feeBusy || (fee?.busy ?? false) || (fee?.provisional ?? false)
        // NEVER ANOTHER TIER'S FIGURE WEARING THIS TIER'S NAME (issue 681): on
        // the path where a pick re-measures, the estimate in hand still belongs
        // to the speed just left — the send machine keeps it across a tier
        // change — and "measuring" is the honest thing to say.
        let ofAnotherTier = view.fee.map { estimate in
            speed.map { offered(estimate.tier) != $0.view.tier } ?? false
        } ?? false
        let text = ofAnotherTier ? nil : view.fee.map {
            feeLine($0, view: view, fee: fee, display: display, networks: networks)
        }
        // With no estimate, the row says so — it does NOT fall back to the
        // drawing's value. The fixture's "0.0021 ETH · ≈$0.55" appeared on a
        // Gnosis send on the founder's iPhone while the quote was still in
        // flight: a number in the fee slot is a promise about what this costs,
        // and the drawing's number is a promise about somebody else's send.
        let waiting = busy ? loc.t("send.estimatingFee") : "—"
        // The fee machine's failure, said ONCE for the row and the footer
        // (PR 2 note 1): its figure ("Tap to retry" only when a tap is the
        // one way, else the dash) and its reason, kept through the core's own
        // re-ask with the measuring sign turning — never "Estimating…" in its
        // place, so the row does not flip every few seconds. A failure means
        // no figure stands (the machine's own `fee` is gone), so it wins
        // over the send machine's last estimate.
        //
        // Only this form's own question (PR 2 polish): right after a token
        // switch the form names another chain before the fee machine has
        // been asked about it, and the old chain's failure is not drawn —
        // no figure, no reason, no footer — as another chain's estimate is not.
        let failure = ofAnotherTier ? nil : formFailure(fee, view: view)
        let chainName = view.selectedToken.flatMap { networks.meta($0.chainId)?.displayName } ?? ""
        return FeeRowModel(
            label: fallback.label,
            // The fee is paid on THIS chain, in THIS coin — the core's answer.
            // The drawing's mark was ETH, the wrong coin on every network but
            // one, and it stood in whenever no estimate was in hand. With no
            // chain known there is no coin to name: an empty disc.
            mark: feeCoinMark(view)
                ?? TokenMarkModel(ticker: "", badgeColor: fallback.mark.badgeColor, badgeHidden: true),
            value: ofAnotherTier ? loc.t("send.estimatingFee") : (failure?.figure(loc) ?? text ?? waiting),
            openLabel: fallback.openLabel,
            refreshLabel: speed.map { _ in loc.t("send.feeRefresh") },
            // A measurement is out — whoever started it — the same fact the
            // "measuring" text reads, so the row is never settled and busy at once.
            refreshing: busy,
            // `FeeView.stale` had no visible consumer on iOS: the 30 s TTL ran
            // out and nothing said so. Not while a fresh measurement is out, and
            // not over a row with no figure of its own on it.
            staleNote: (speed != nil && fee?.stale == true && !busy && text != nil && failure == nil)
                ? loc.t("send.feeStale") : nil,
            // Why there is no fee, in the core's words (`failure.reason_key`):
            // the chain by name for a chain read, Vela's own fault for an
            // internal one (issue #483) — never a blank "—" with nothing said.
            //
            // A fee that would fail has no reason of the network's — and the
            // form has no held confirm to carry the failure's line — so the
            // row's own line says it: "This would fail if sent as it is."
            // (`footer_key`, PR 2 polish; the confirm and the sheet say it
            // under their held button).
            failNote: failure.flatMap { failure in
                failure.reason(loc, chain: chainName) ?? failure.wouldFailLine(loc)
            },
            // A tap on a failed row does what its figure says (the core's
            // `failure.tap`): asks again at once (`requote`), opens the coins
            // after `would_fail`, or — no coin left — nothing.
            tap: failure?.rowTap ?? .open
        )
    }

    /// The chain the form is on: the selected token's, else the sweep's —
    /// the core's `form_chain`.
    static func formChain(_ view: SendViewWire?) -> Int? {
        view?.selectedToken?.chainId ?? view?.multiChainId
    }

    /// The fee's failure as the Send form and its confirm may draw it — and
    /// as the bridge tells the send machine (`fee_failed_changed`): `nil`
    /// when the failure answered another chain's question (PR 2 polish,
    /// `FeeFailureView::is_for_chain`), as another chain's estimate is
    /// withheld.
    static func formFailure(_ fee: FeeViewWire?, view: SendViewWire?) -> FeeFailureViewWire? {
        guard let failure = fee?.failure, failure.isFor(chain: formChain(view)) else { return nil }
        return failure
    }

    /// A tier as one this build offers: the dead `rapid` reads as the factory
    /// default, `standard` — the core's own answer for it.
    static func offered(_ tier: String) -> String {
        ["fast", "standard", "slow"].contains(tier) ? tier : "standard"
    }

    /// A tier's NAME — the speed itself, never a number.
    static func tierName(_ tier: String, loc: Loc) -> String {
        loc.t("send.gasTier.\(offered(tier))")
    }

    /// The folded speed control (spec 068), drawn from the `fee_speed` core's
    /// view (spec 069). Every figure is that tier's OWN settled quote, echoed by
    /// the core; only the words and the fee line are made here.
    ///
    /// The dApp signing sheet draws the same control (spec 069) and passes no
    /// send view: each option is then written the way that sheet writes its
    /// own fee row.
    static func speedModel(
        _ speed: SpeedInputs, view: SendViewWire?, display: WalletLive.Display, loc: Loc,
        networks: WalletNetworks = .builtin
    ) -> FeeSpeedModel {
        let core = speed.view
        return FeeSpeedModel(
            label: loc.t("send.feeSpeedLabel"),
            // THEIR default (or their pick for this send), never a hardcoded one.
            value: tierName(core.tier, loc: loc),
            open: core.open,
            onceNote: loc.t("send.feeSpeedOnce"),
            freeNote: core.freeNote ? loc.t("send.feeSpeedFree") : nil,
            singleNote: core.single ? loc.t("send.feeSpeedSingle") : nil,
            gasPriceLabel: loc.t("send.gasPriceLabel"),
            gasPriceLine: core.gasPriceLine,
            options: core.options.map { option in
                FeeSpeedOptionModel(
                    id: option.tier,
                    label: tierName(option.tier, loc: loc),
                    // "…" while this tier's own quote is out, "—" when there is
                    // none to be had.
                    value: option.fee.map {
                        feeLine($0, view: view, fee: speed.feeView(option.tier), display: display,
                                networks: networks)
                    }
                        ?? (option.measuring ? "…" : "—"),
                    gasPrice: option.gasPrice,
                    selected: option.selected
                )
            }
        )
    }

    /// "0.0021 XDAI" from the estimate — the fee asset's own units, **never
    /// re-priced here**.
    ///
    /// `total_wei` is the native figure even when the fee is paid in an ERC-20;
    /// the token figure is the asset's own `amount` in its own decimals.
    /// Reading the first where the second belongs prints a six-decimal
    /// stablecoin fee as an eighteen-decimal number.
    ///
    /// The native coin is named from the wallet's networks, the person's own
    /// included: a fee on a network they added had a figure and no unit.
    static func feeText(_ estimate: FeeEstimateWire, networks: WalletNetworks = .builtin) -> String {
        switch estimate.feeAsset {
        case .native:
            let symbol = networks.meta(estimate.chainId)?.nativeSymbol ?? ""
            return "\(feeFromBase(estimate.totalWei, decimals: 18)) \(symbol)"
        case .erc20(_, let decimals, let amount, let symbol):
            return "\(feeFromBase(amount, decimals: decimals)) \(symbol ?? "TOKEN")"
        }
    }

    /// Below half a cent the coin amount is the honest primary and the fiat
    /// half is left off (`03-domain-components.md` §3.1): a real fee rounded to
    /// "$0.00" reads as free, which is a worse answer than no figure at all.
    static let feeFiatMinUSD = 0.005

    /// The whole-token figure a price multiplies, and which coin to price.
    private static func feeUnits(_ estimate: FeeEstimateWire) -> (units: Double?, contract: String?) {
        switch estimate.feeAsset {
        case .native:
            return (Double(estimate.totalWei).map { $0 / 1e18 }, nil)
        case .erc20(let token, let decimals, let amount, _):
            return (Double(amount).map { $0 / pow(10, Double(decimals)) }, token)
        }
    }

    /// The unit price, in USD, of the coin a quote is denominated in.
    ///
    /// The relay's published row first — it priced the quote, so its number is
    /// the one the estimate converted through — then the balances the form
    /// already carries, which is where the amount's own "≈" line gets its
    /// price. `nil` when neither knows the coin: a fee row that invents a
    /// price is worse than one that shows only the coin.
    static func feePriceUSD(
        contract: String?, chainId: Int, view: SendViewWire?, fee: FeeViewWire?
    ) -> Double? {
        func same(_ other: String?) -> Bool {
            guard let contract else { return other == nil }
            return other?.lowercased() == contract.lowercased()
        }
        if let published = fee?.options.first(where: { same($0.contract) })?.usdPrice,
           let price = Double(published), price > 0 {
            return price
        }
        guard let view else { return nil }
        let held = ([view.selectedToken].compactMap { $0 } + view.tokens)
            .first { $0.chainId == chainId && same($0.tokenAddress) }
        guard let price = held?.priceUsd, price > 0 else { return nil }
        return price
    }

    /// "0.0021 XDAI · ≈$0.55" — the quote's own amount, and what it costs
    /// (issue 201). The fee was the one figure on the send screens with no
    /// money beside it; the amount is never re-derived here, only the price is
    /// looked up.
    static func feeLine(
        _ estimate: FeeEstimateWire, view: SendViewWire?, fee: FeeViewWire?,
        display: WalletLive.Display, networks: WalletNetworks = .builtin
    ) -> String {
        let coin = feeText(estimate, networks: networks)
        let (units, contract) = feeUnits(estimate)
        guard let units,
              let price = feePriceUSD(
                  contract: contract, chainId: estimate.chainId, view: view, fee: fee
              )
        else { return coin }
        let usd = units * price
        guard usd >= feeFiatMinUSD, usd.isFinite else { return coin }
        let money = usd * display.rate
        return "\(coin) · ≈\(display.glyph)\(Formats.number(money, minimumFractionDigits: 2, maximumFractionDigits: 2))"
    }

    /// The coin the fee row names, in the core's token mark — the core's
    /// `SendView.fee_coin`, one answer on all four shells whether or not a
    /// figure is beside it: the estimate in hand (this speed's, else the speed
    /// just left's while a new one is measured — the coin does not change with
    /// the speed), else the coin in force (the fee card's, mirrored by
    /// `fee_token_changed`, else the person's pick), else the chain's own. An
    /// ERC-20 by its CONTRACT, on the send's chain — never chain 0. `nil` only
    /// while no chain is known.
    ///
    /// Each shell used to answer this itself while no estimate was in hand,
    /// and a failed quote on BNB Chain with USDT chosen was drawn three ways.
    static func feeCoinMark(_ view: SendViewWire) -> TokenMarkModel? {
        guard let coin = view.feeCoin else { return nil }
        return TokenMarkModel.of(chainId: coin.chainId, symbol: coin.symbol,
                                 tokenAddress: coin.contract, color: chainColor(coin.chainId))
    }

    /// The core's live refusal on the form: the same-asset ceiling first, then
    /// the amount warning.
    ///
    /// **Every figure in the ceiling is a base-unit decimal string** and the
    /// shell formats it. Android's first cut printed `5000000000000000000 XDAI`
    /// on a phone; this is where that does not happen again.
    static func formWarning(_ view: SendViewWire, loc: Loc) -> String? {
        if let issue = view.sameAssetFeeIssue {
            let decimals = view.selectedToken?.decimals ?? 18
            // The asset list's figures (spec 078), not 18-digit remainders:
            // "you have" is the balance on the token card above, digit for
            // digit. The most that can be sent is rounded DOWN — typed back,
            // it has to fit (the web's `sameFeeWords`).
            let human = { (base: String, rounding: WalletLive.TokenRounding) in
                WalletLive.tokenAmountText(fromBase(base, decimals: decimals), rounding: rounding)
            }
            let body = loc.t("send.sameFeeTokenBody", vars: [
                "amount": human(issue.transferAmount, .halfUp),
                "fee": human(issue.feeAmount, .halfUp),
                "total": human(issue.total, .halfUp),
                "symbol": issue.symbol,
                "balance": human(issue.balance, .halfUp),
            ])
            let most = loc.t("send.sameFeeTokenMax", vars: [
                "amount": human(issue.maxTransferAmount, .down),
                "symbol": issue.symbol,
            ])
            return "\(body) \(most)"
        }
        // A split has its own live verdict, `split_over_balance` — the same
        // predicate Continue refuses on. It does not take `amount_warning`:
        // that is derived from the single form's figure, which a split leaves
        // behind, so it would judge a number no longer on the screen.
        if view.splitMode {
            return view.splitOverBalance ? loc.t("send.alertInsufficientBalanceBody") : nil
        }
        return view.amountWarning.map { warningText($0, loc: loc) }
    }

    /// One sentence per warning, in the core's own keys.
    static func warningText(_ warning: SendAmountWarningWire, loc: Loc) -> String {
        switch warning {
        case .notEnoughToken:
            return loc.t("send.alertInsufficientBalanceBody")
        case .insufficientForGas(let symbol):
            return loc.t("send.warnInsufficientForGas", vars: ["sym": symbol ?? ""])
        case .insufficientGas(let symbol):
            return loc.t("send.warnInsufficientGas", vars: ["sym": symbol ?? ""])
        case .needGas(let symbol):
            return loc.t("send.warnNeedGas", vars: ["sym": symbol ?? ""])
        case .cannotConvert(let code, let symbol):
            return loc.t("send.warnCannotConvert", vars: ["code": code, "symbol": symbol])
        }
    }

    // MARK: - SD3, the confirmation

    static func confirm(
        _ view: SendViewWire,
        from: (address: String, name: String?),
        display: WalletLive.Display,
        on model: SendConfirmModel,
        loc: Loc,
        fee: FeeViewWire? = nil,
        speed: FeeSpeedViewWire? = nil,
        networks: WalletNetworks = .builtin
    ) -> SendConfirmModel {
        let live = model
        let token = view.selectedToken
        let symbol = token?.symbol ?? ""
        let chain = token.map { networks.meta($0.chainId)?.displayName ?? $0.network } ?? ""
        // The network the signature moves money on: a sweep's locked chain,
        // else the token's. Never 0.
        let networkChainId = (view.multiSelectMode ? view.multiChainId : nil) ?? token?.chainId
        // The fee machine's failure (PR 2 note 1): the row's figure and
        // reason, the same one state the form's row and the footer read —
        // only when it answered this send's chain (PR 2 polish).
        let failure = formFailure(fee, view: view)
        let failureChain = networkChainId.flatMap { networks.meta($0)?.displayName } ?? chain

        var facts = [
            // Who pays, with the face every other shell draws beside it.
            FactRowModel(
                label: model.facts.first?.label ?? "",
                value: from.name ?? AddressText.short(from.address),
                lead: isAddress(from.address) ? .identicon(from.address) : nil
            ),
            // Who is paid, as the core names them (spec 097 F): a single send
            // and a sweep alike. A split's has no one payee and goes below.
            payeeFact(
                label: model.facts.count > 1 ? model.facts[1].label : "",
                address: view.recipient.trimmingCharacters(in: .whitespaces),
                payee: singlePayee(view), loc: loc
            ),
            // The NETWORK, in the network's own mark (the kind rule): "Base"
            // beside Base's logo, whatever coin is sent on it.
            FactRowModel(
                label: model.facts.count > 2 ? model.facts[2].label : "",
                value: networkChainId.flatMap { networks.meta($0)?.displayName } ?? chain,
                lead: networkChainId.flatMap { networkMark($0, networks: networks) }.map { .token($0) }
            ),
            FactRowModel(
                label: model.facts.count > 3 ? model.facts[3].label : "",
                // With no estimate the row says so, as the form's row does —
                // never the drawing's "~0.0021 ETH · ≈$0.55", a promise about
                // somebody else's send (the web's `feeText`: `send.fee ?? fee.fee`).
                // A failure is no figure at all: its own (the dash, or "Tap to
                // retry" only when a tap is the one way), kept through the
                // core's re-ask — never "Estimating…" and back.
                value: failure?.figure(loc)
                    ?? (view.fee ?? fee?.fee).map {
                        "~\(feeLine($0, view: view, fee: fee, display: display, networks: networks))"
                    }
                    ?? (view.estimatingGas || view.feeBusy || (fee?.busy ?? false)
                        ? loc.t("send.estimatingFee") : "—"),
                // Why, under the row: the chain by name, the relay, or Vela's
                // own fault — the core's words.
                note: failure?.reason(loc, chain: failureChain),
                // The coin's figure and its price in the person's currency:
                // a second line between the two, never a figure cut short.
                wraps: true,
                // A failed fee's row does what its figure says (PR 2
                // polish): "Tap to retry" asks again, "Pay with another
                // coin" opens the coins, the dash with none left is no
                // control. A settled fee's row is a fact here.
                feeTap: failure?.rowTap
            ),
        ]
        // The speed, but only when it was CHOSEN for this send, or taken
        // because it was free (spec 068 / issue 686): the last screen before a
        // signature says so, and a free upgrade says why. A send at the stored
        // default adds no row.
        if let speed, speed.picked || speed.free {
            facts.append(FactRowModel(
                label: loc.t("send.feeSpeedLabel"),
                value: tierName(speed.tier, loc: loc),
                note: speed.picked ? nil : loc.t("send.feeSpeedFree")
            ))
        }

        // **The parts, from the core's own rows — never the drawing's.** The
        // drawn SD3b/SD3c carry "Alice 50 USDT" and a three-coin sweep, and a
        // confirm page is the one screen that must not show a payee nobody
        // typed: it is what the person reads before they sign.
        // A single send's figure through the asset list's formatter: a Max
        // is exact to the wei (`0.043790209243313861`), and the page someone
        // reads before signing must not be where that runs off the screen. A
        // split's total stays exact — it is the sum of the rows beneath it.
        var amount = view.splitMode
            ? "\(trim(view.confirmAmount)) \(symbol)"
            : WalletLive.tokenAmountText(view.confirmAmount)
        // A single send's coin is its own piece, drawn smaller and muted on
        // the figure's baseline (round 2) — a split's total and a sweep's
        // count keep one string.
        var amountUnit: String? = view.splitMode || symbol.isEmpty ? nil : symbol
        var subline = view.confirmAmountIssue.map { issue in
            loc.t("send.warnCannotConvert", vars: ["code": issue.code, "symbol": issue.symbol])
        } ?? fiatLine(view, token: token, display: display)
        var breakdown: [BreakdownRowModel] = []
        if view.multiSelectMode {
            // A sweep: N assets on one network. Each row is the amount the
            // signature will move (`multi_specs`), and the total is those rows
            // priced — never the picker's balances.
            let sweep = sweepBreakdown(view, display: display)
            breakdown = sweep.rows
            amount = loc.t("componentsTx.receipt.assetsCount", vars: ["n": String(sweep.rows.count)])
            amountUnit = nil
            subline = loc.t("send.confirmTotalLine", vars: [
                "fiat": money(sweep.totalUsd, display: display),
                "network": view.multiChainId
                    .flatMap { networks.meta($0)?.displayName } ?? chain,
            ])
        } else if view.splitMode, !view.recipients.isEmpty {
            // A split: every payee by name and face, and how many there are.
            // The single "To" row has no one address to name, so it goes —
            // the count and the list below say who instead (web 038 #D2).
            breakdown = splitBreakdown(view, loc: loc)
            facts.remove(at: 1)
            let count = loc.t(
                view.recipients.count == 1 ? "send.recipientCount_one" : "send.recipientCount_other",
                vars: ["count": String(view.recipients.count)]
            )
            let fiat = token?.priceUsd.flatMap { price in
                Double(view.confirmAmount).map { $0 * price }
            }
            subline = [count, chain].filter { !$0.isEmpty }.joined(separator: " · ")
                + (fiat.map { " · ≈ \(money($0, display: display))" } ?? "")
        }

        return SendConfirmModel(
            header: live.header,
            // A sweep moves several coins; one mark would name the wrong one.
            mark: view.multiSelectMode ? nil : token.map { token in
                TokenMarkModel.of(
                    chainId: token.chainId, symbol: token.symbol,
                    tokenAddress: token.tokenAddress, color: chainColor(token.chainId),
                    named: token.logoUrls
                )
            },
            // `confirm_amount` and nothing else. It is the figure the money
            // gates measured and the batch is built from — a shell that
            // re-derived it would put a number on the signing page nothing
            // else in the flow had agreed to.
            amount: amount,
            amountUnit: amountUnit,
            subline: subline,
            facts: facts,
            breakdown: breakdown,
            notice: confirmNotice(view, loc: loc),
            noticeTitle: confirmNoticeTitle(view, loc: loc),
            noticeFund: fundAddress(view, loc: loc),
            // The treasury pause has TWO exits (spec 054 US4): the core's
            // retry, and 暂不 — which keeps the facts on screen rather than
            // throwing the attempt away. A submit the relay refused has one,
            // and a passkey prompt that is up has only cancel, which is the
            // core's own checkpoint.
            noticeAction: {
                if view.relayUnreachable != nil {
                    loc.t("componentsUi.relayUnreachable.retryBtn")
                } else if view.treasuryBootstrap != nil {
                    loc.t("componentsUi.treasuryBootstrap.retryBtn")
                } else if view.txError != nil, view.txError != "venue_blocked" {
                    // A venue that cannot be used here is not retried: the
                    // same attempt would be refused the same way.
                    loc.t("send.txRetryBtn")
                } else if view.txStatus == "signing" {
                    loc.t("componentsUi.funding.cancel")
                } else {
                    nil
                }
            }(),
            noticeSecondary: view.relayUnreachable != nil
                ? loc.t("componentsUi.relayUnreachable.closeBtn")
                : view.treasuryBootstrap != nil
                    ? loc.t("componentsUi.funding.cancel")
                    : nil,
            noticeReport: reportLabel(view, loc: loc),
            repeatNote: confirmRepeatNote(view, loc: loc),
            // The core's own verdicts: a token's own contract (spec 096 F12)
            // first, else the first time, resolved on this page only (single
            // recipient).
            recipientTag: view.recipientIsTokenContract == true
                ? loc.t("send.recipientTokenContract")
                : !view.splitMode && view.recipientRisk?.firstTime == true
                    ? loc.t("componentsUi.signing.firstTimeTag") : nil,
            cta: live.cta,
            heldNote: heldNote(view, fee: fee, loc: loc)
        )
    }

    /// The ONE line under the held confirm: the previous transaction on this
    /// network first (PR 2 §3: the core's key, no timer; it does not move
    /// with the fee's busy state), else the fee's failure in the line the
    /// signing sheet draws for it (PR 2 note 1, `failure.footer_key`):
    /// "Retrying…" while the core asks again by itself, "Tap it to retry"
    /// only when a tap is the one way. Else nothing.
    ///
    /// Said once: while the relay's own refusal of the same kind is the
    /// notice ("Not sent yet", `tx_error` `previous_pending`), the previous
    /// transaction's line is not said again under the button. A failure of
    /// another chain's question says nothing here (PR 2 polish).
    static func heldNote(_ view: SendViewWire, fee: FeeViewWire?, loc: Loc) -> String? {
        if let held = view.previousPending, view.txError != "previous_pending" { return loc.t(held.key) }
        return formFailure(fee, view: view).map { loc.t($0.footerKey) }
    }

    /// A split's payees, one row each: the amount from the core's drafts, who
    /// from its `payees` (same index, spec 097 F).
    ///
    /// A name never stands in for the address on the page that signs: a named
    /// row is the name over its tag and short address; an unnamed one the
    /// short address in mono. Only a real address earns a face.
    static func splitBreakdown(_ view: SendViewWire, loc: Loc) -> [BreakdownRowModel] {
        let symbol = view.selectedToken?.symbol ?? ""
        let payees = view.payees ?? []
        return view.recipients.enumerated().map { index, row in
            let payee = payees.indices.contains(index) ? payees[index] : nil
            let address = payee?.address ?? row.address
            let short = AddressText.short(address)
            let name = payeeKnownName(payee)
            return BreakdownRowModel(
                identiconSeed: isAddress(address) ? address : "",
                label: name ?? short,
                value: "\(trim(row.amount)) \(symbol)",
                detail: payee.flatMap { name == nil ? nil : payeeDetail($0, short: short, loc: loc) },
                mono: name == nil
            )
        }
    }

    /// A sweep's assets, one row each, and what they come to in USD.
    ///
    /// The amount is the core's reserved spec (net of what the fee coin pays)
    /// when it has one, else the balance that spec will become — the web's
    /// `sweepAmount`. A row with no price adds nothing to the total rather
    /// than a guess.
    static func sweepBreakdown(
        _ view: SendViewWire, display: WalletLive.Display
    ) -> (rows: [BreakdownRowModel], totalUsd: Double) {
        var total = 0.0
        let ticked = view.tokens.filter { view.multiSelectedIds.contains($0.id) }
        let rows = ticked.map { token in
            let amount = view.multiSpecs.first {
                ($0.tokenAddress ?? "") == (token.tokenAddress ?? "")
            }?.amount ?? token.balance
            let usd = token.priceUsd.flatMap { price in Double(amount).map { $0 * price } }
            if let usd { total += usd }
            let value = "\(WalletLive.tokenAmountText(amount)) \(token.symbol)"
            return BreakdownRowModel(
                lead: coinMark(token),
                label: token.symbol,
                value: usd.map { "\(value) · ≈\(money($0, display: display))" } ?? value
            )
        }
        return (rows, total)
    }

    /// A USD figure in the display currency, glyph first — the form's own
    /// "≈" arithmetic, without the "≈".
    private static func money(_ usd: Double, display: WalletLive.Display) -> String {
        let converted = usd * display.rate
        guard converted.isFinite else { return "" }
        return "\(display.glyph)\(Formats.number(converted, minimumFractionDigits: 2, maximumFractionDigits: 2))"
    }

    /// What stopped the confirm page: the relay's treasury, or a submit the
    /// relay refused.
    static func confirmNotice(_ view: SendViewWire, loc: Loc) -> String? {
        if let stop = stopNotice(view, loc: loc) { return stop }
        switch view.txError {
        case "bundler_fund": return loc.t("send.txErrorBundlerFund")
        case "generic": return loc.t("send.txErrorGeneric")
        // Spec 102: not the network's failure — this account cannot sign
        // here, and the core says why.
        case "venue_blocked": return view.txVenueBlock?.text(loc) ?? loc.t("send.txErrorGeneric")
        // PR 2 §3: the relay holds this account's nonce for an earlier op —
        // nothing was sent and nothing went wrong; Try again sends once it
        // has landed. Said calmly, under "Not sent yet" (PR 2 polish).
        case "previous_pending": return loc.t(I18nKeys.CoreRound.notSentBody)
        default: return view.txStatus == "signing" ? loc.t("send.txPreparingBiometric") : nil
        }
    }

    /// The notice's title, when it has one: "Not sent yet" over a submit the
    /// relay turned back because the previous transaction on this network
    /// still holds the nonce (PR 2 polish) — no failure word, no red. `nil`
    /// for every other notice, which is one line.
    static func confirmNoticeTitle(_ view: SendViewWire, loc: Loc) -> String? {
        guard stopNotice(view, loc: loc) == nil, view.txError == "previous_pending" else { return nil }
        return loc.t(I18nKeys.CoreRound.notSentTitle)
    }

    /// The form's button gate: the core's `can_continue`, the whole of it.
    ///
    /// Issue #424: this used to OR the relay stops on, and the confirm gate
    /// below AND them (and a signature under way, and a refused submit) — a
    /// predicate of the shell's own beside the core's, which the desktop and
    /// the web did not share. The core says all of it now: while a stop is up
    /// the button is its retry and the core arms it; on confirm the core shuts
    /// the slide, and refuses a slide, on the same reasons.
    static func formCtaDisabled(_ view: SendViewWire) -> Bool { !view.canContinue }

    /// The confirm slide's gate: the core's `can_confirm`, the whole of it.
    static func confirmCtaDisabled(_ view: SendViewWire) -> Bool { !view.canConfirm }

    /// The relay's two stops, in words — on the form AND on confirm.
    ///
    /// The core opens both at Continue, while the stage is still the form
    /// (spec 098): until 098 this app drew them on the confirm page only, so
    /// pressing Continue into a relay with no gas did nothing anyone could see.
    static func stopNotice(_ view: SendViewWire, loc: Loc) -> String? {
        if let sheet = view.relayUnreachable {
            // Whose it is to fix is the core's verdict (spec 098 §2).
            let lead = sheet.operatorServed
                ? loc.t("componentsUi.relayUnreachable.operatorLead")
                : loc.t("componentsUi.relayUnreachable.customLead") + " "
                    + loc.t("componentsUi.relayUnreachable.settingsHint")
            return loc.t("componentsUi.relayUnreachable.title") + " · " + lead
        }
        if let status = view.treasuryBootstrap {
            // Two situations, one symptom: on a network Vela ships the
            // operator owns that relayer and telling them is the fix; on one
            // the person added, there may be nobody else who can hold gas
            // there at all (spec 060).
            let lead = status.operatorServed
                ? loc.t("componentsUi.treasuryBootstrap.operatorLead")
                : loc.t("componentsUi.treasuryBootstrap.customLead")
            // Issue #422: the coin and every figure in it are the core's —
            // the stop's own chain's coin, the relay's shortfall for that
            // chain. No figures the core could read: no amount, rather than
            // one nobody measured.
            var figures = ""
            if let coin = status.coin {
                let symbol = coin.symbol ?? ""
                let hint = loc.t("componentsUi.treasuryBootstrap.amountHint", vars: [
                    "amount": coin.suggested, "symbol": symbol,
                ]).trimmingCharacters(in: .whitespaces)
                // Spec 098 §4: what it has against what it needs.
                let balance = loc.t("componentsUi.treasuryBootstrap.balanceLine", vars: [
                    "balance": coin.balance, "floor": coin.floor, "symbol": symbol,
                ])
                figures = " " + hint + "\n" + balance
            }
            // …the line that must not be missed (non-refundable, not Vela's),
            // and that the sheet watches — the core closes it once funded.
            return loc.t("componentsUi.treasuryBootstrap.title") + " · "
                + lead + figures
                + "\n" + loc.t("componentsUi.treasuryBootstrap.disclaimer")
                + "\n" + loc.t("componentsUi.treasuryBootstrap.watching")
        }
        return nil
    }

    /// The form's button while a relay stop is up: its retry, which is
    /// Continue again (the core clears the stop and re-runs the pre-check).
    /// Spec 098 §4: the treasury stop's address and its copy button — the one
    /// thing a person needs to fund it.
    static func fundAddress(_ view: SendViewWire, loc: Loc) -> FundAddressModel? {
        guard let status = view.treasuryBootstrap else { return nil }
        return FundAddressModel(
            label: loc.t("componentsUi.treasuryBootstrap.addressLabel"),
            address: status.address,
            copy: loc.t("componentsUi.treasuryBootstrap.copyBtn"),
            copied: loc.t("componentsUi.treasuryBootstrap.copied")
        )
    }

    /// Issue #466: "Report this" on either relay stop — shown exactly while
    /// the core built a report for it (`relay_report`: a stop up on a network
    /// Vela ships, whose operator is the one to tell). The words are the
    /// stop's own key; what the button files is the core's.
    static func reportLabel(_ view: SendViewWire, loc: Loc) -> String? {
        guard view.relayReport != nil else { return nil }
        if view.relayUnreachable != nil { return loc.t("componentsUi.relayUnreachable.reportBtn") }
        if view.treasuryBootstrap != nil { return loc.t("componentsUi.treasuryBootstrap.reportBtn") }
        return nil
    }

    /// What "Report this" files, snapshotted at the tap: the core's words,
    /// area and fingerprint. The stop closes itself once the relay is funded,
    /// and a watch refresh rewrites its figures — the sheet keeps what the
    /// person was shown when they asked.
    static func reportSeed(_ view: SendViewWire) -> BugReport.Seed? {
        guard let report = view.relayReport else { return nil }
        return BugReport.Seed(what: report.what, steps: report.steps,
                              area: report.area, fingerprint: report.fingerprint)
    }

    static func stopRetry(_ view: SendViewWire, loc: Loc) -> String? {
        if view.relayUnreachable != nil { return loc.t("componentsUi.relayUnreachable.retryBtn") }
        if view.treasuryBootstrap != nil { return loc.t("componentsUi.treasuryBootstrap.retryBtn") }
        return nil
    }

    /// Title and body for every alert the core raises, in the core's words.
    /// `chain`: the selected token's network by name, for a body that names
    /// the chain it could not reach.
    static func alertText(_ kind: [String: Any], loc: Loc, chain: String = "") -> (title: String, body: String) {
        switch kind["type"] as? String ?? "" {
        case "invalid_address":
            return (loc.t("send.alertInvalidAddressTitle"), loc.t("send.alertInvalidAddressBody"))
        case "invalid_amount":
            return (loc.t("send.alertInvalidAmountTitle"), loc.t("send.alertInvalidAmountBody"))
        case "insufficient_balance", "split_over_balance":
            return (loc.t("send.alertInsufficientBalanceTitle"), loc.t("send.alertInsufficientBalanceBody"))
        case "load_tokens_failed":
            return (loc.t("send.alertLoadTokensError"), "")
        case "estimate_failed":
            // Worded by its cause (PR 2 note 13), the core's choice
            // (`sendEstimateFailureBodyKey`): the chain out of reach by its
            // name, a fault inside Vela as that — never "can't reach the
            // chain" — else the general sentence.
            return (
                loc.t("send.alertEstimateFailedTitle"),
                loc.t(sendEstimateFailureBodyKey(failure: estimateFailureText(kind["kind"])), vars: ["chain": chain])
            )
        case "account_unavailable":
            return (loc.t("send.alertEstimateFailedTitle"), loc.t("send.alertAccountUnavailableBody"))
        default:
            return (loc.t("send.alertEstimateFailedTitle"), "")
        }
    }

    /// A `SendEstimateFailure` as the core's functions take it back: the wire
    /// name, or the JSON of one that carries data (`{"chain_read":{…}}`).
    static func estimateFailureText(_ raw: Any?) -> String {
        switch raw {
        case let name as String: return name
        case let object as [String: Any]: return CoreJSON.string(object)
        default: return "other"
        }
    }

    // MARK: - SD4, the receipt

    /// Which drawn state the receipt is in. The core's status, never a guess
    /// from whether a hash happens to be present.
    ///
    /// Spec 082: `maybe_sent` is a submitted op whose reply was lost — drawn
    /// as on its way, never as failed; `not_sent` is an op the relay never
    /// had — failed, with nothing spent.
    static func receiptStage(_ view: SendViewWire) -> ReceiptStage {
        switch view.receipt?.status {
        case "confirmed": return .confirmed
        case "failed", "not_sent": return .failed
        case "submitted", "maybe_sent": return .submitted
        default: return .submitting
        }
    }

    /// The receipt, in whichever state the transaction is in.
    ///
    /// The hash shown is the **transaction** hash once there is one, and the
    /// operation hash before that: a user operation has an id from the moment
    /// the relay accepts it, and showing nothing until it lands would leave a
    /// person with a submitted payment and no reference at all.
    static func receipt(
        _ view: SendViewWire,
        display: WalletLive.Display,
        on model: SendReceiptModel,
        loc: Loc,
        /// When the relay put this send on the network — the tracker's entry
        /// for its op (`relay_sent_at_ms`, spec 099 R6). `nil`: not yet.
        relaySentAtMs: Double? = nil,
        nowMs: Double = Date().timeIntervalSince1970 * 1000,
        networks: WalletNetworks = .builtin
    ) -> SendReceiptModel {
        let token = view.selectedToken
        let symbol = token?.symbol ?? ""
        let chain = token.map { networks.meta($0.chainId)?.displayName ?? $0.network } ?? ""
        let stage = receiptStage(view)
        let hash = (view.txHash?.isEmpty == false ? view.txHash : view.userOpHash)
            .flatMap { $0.isEmpty ? nil : $0 }

        let title: String
        var captions: [String]
        var eta: ReceiptEtaModel?
        let parts = receiptParts(view, symbol: symbol, loc: loc)
        switch stage {
        case .submitting:
            // Three states, three sentences — the Android receipt's own
            // distinction (058 US2). One sentence for all three left a person
            // looking at 正在提交 while the phone was actually waiting for
            // their finger, which is the moment they most need telling.
            switch view.txStatus {
            case "signing": title = loc.t("send.txSigning")
            case "submitting": title = loc.t("send.txSubmitting")
            default: title = loc.t("send.txPreparing")
            }
            captions = [loc.t("send.txPreparingBiometric"), loc.t("send.txBackgroundHint")]
        case .submitted where view.receipt?.status == "maybe_sent":
            // The reply was lost (spec 082 RA10): it may have been sent, Vela
            // keeps checking, and it must not be sent again. No clock — the
            // relay has not shown it holds the op — and no Retry anywhere.
            title = loc.t("send.txSubmitting")
            captions = [loc.t("componentsUi.signing.maybeSent")]
        case .submitted:
            title = loc.t("send.txSubmittedTitle")
            // Held for fees: the payment is not stuck and not lost — it goes
            // out by itself when fees settle, and saying so is the difference
            // between waiting and sending it twice.
            if view.receipt?.holdReason == "fee_hold" {
                captions = [loc.t("send.txHeldFees")]
                break
            }
            // The relay holds it while it tops up its gas on the chain (098
            // follow-up): why it waits — and no confirmation clock, since
            // nothing is on the network yet.
            if view.receipt?.holdReason == "relay_funding" {
                captions = [loc.t("send.txRelayFunding")]
                break
            }
            // Spec 099 R6: the core's one countdown, from the relay's send.
            // Before the relay has sent it the landing says the relay is
            // sending it — never a chain's countdown over the relay's own
            // queue, which read "taking longer than usual" within seconds.
            let typicalS = view.receipt?.typicalInclusionS
            let pace = LandingPaceWire.of(sentAtMs: relaySentAtMs, typicalS: typicalS, nowMs: nowMs)
            if pace.waiting {
                captions = [loc.t("send.txRelaySending")]
                break
            }
            captions = [loc.t("send.txWaitingConfirm")]
            if let seconds = typicalS {
                let typical = loc.t("send.txTypicalTime", vars: [
                    "chainName": chain, "estSecs": String(seconds),
                ])
                // The screen counts (web `live-send.ts`, #199); the typical
                // line leads the clock.
                eta = ReceiptEtaModel.counting(
                    sentAtMs: relaySentAtMs, typicalS: seconds, typicalLine: typical, loc: loc, nowMs: nowMs
                )
                if eta == nil { captions.append(typical) }
            }
        case .confirmed:
            // Every coin the operation sent, as the core summed them (spec
            // 097 F, S3). One heads with its figure — a split's is its total;
            // several have no one figure and are listed below: "Sent 0.000418
            // ETH" over a sweep that also moved USDC said less than happened.
            let coins = view.receipt?.coins ?? []
            if coins.count > 1 {
                title = loc.t("componentsTx.detail.sent")
            } else {
                let sent = coins.first?.amount ?? view.receipt?.amount ?? view.confirmAmount
                title = loc.t("send.txConfirmedTitle", vars: [
                    // As on the confirm page: the asset list's formatter for
                    // one send, the exact sum for a split.
                    "amount": view.splitMode ? trim(sent) : WalletLive.tokenAmountText(sent),
                    "symbol": coins.first?.symbol ?? symbol,
                ])
            }
            // A split names its count here and its people below; "To " with
            // nobody after it was what the single-recipient line read as. A
            // sweep's one recipient stays named: its parts are its coins.
            let to = parts.people ?? view.recipientIdentity?.name ?? AddressText.short(view.recipient)
            captions = ["\(to) · \(chain)"]
        case .notSent:
            // A submit the relay turned back for the previous transaction's
            // nonce: nothing was sent, nothing went wrong (PR 2 polish).
            title = loc.t(I18nKeys.CoreRound.notSentTitle)
            captions = [loc.t(I18nKeys.CoreRound.notSentBody)]
        case .failed where view.receipt?.status == "not_sent":
            // The relay never had it (RA4): nothing moved, nothing was spent,
            // and "try again" is true — so the generic sentence, never the
            // revert's "the fee may still have been taken".
            title = loc.t("componentsTx.receipt.statusFailed")
            captions = [loc.t("send.txErrorGeneric")]
        case .failed:
            title = loc.t("componentsTx.receipt.statusFailed")
            // A fee-rejected send is not a failure of the transfer: the fee
            // rose above what was approved and NOTHING was sent, which is a
            // different sentence and a different next step. `hold_reason` was
            // on this client's wire and read by no Swift at all until 058.
            // Only the fee refusal: a fee hold survives into a later failure,
            // and "the fee rose" over a transfer that reverted is untrue.
            //
            // PR 2 §3: a refusal is told by its REASON — the core's key (the
            // fee words only for a fee refusal; "another transaction went
            // first" for a spent nonce; else "the network refused it, nothing
            // was sent"). Nothing was sent, so no revert hint follows.
            if let key = view.receipt?.refusalKey {
                captions = [loc.t(key)]
                break
            }
            if view.receipt?.holdReason == "fee_rejected" {
                captions = [loc.t("send.txRejectedFees")]
                break
            }
            // The core's reason, then the corpus's explanation of what a
            // reverted transfer actually costs — the money did not move and the
            // network fee may still have been taken, which is the one thing a
            // person needs to know and a generic apology does not say.
            captions = [
                SendLive.confirmNotice(view, loc: loc) ?? loc.t("send.txErrorGeneric"),
                loc.t("componentsTx.receipt.failedHint"),
            ]
        }

        return SendReceiptModel(
            header: FlowHeaderModel(
                // A sweep is several coins: "Send ETH" named only the first
                // (spec 097 F).
                title: view.multiSelectMode
                    ? loc.t("send.multiSendTitle")
                    : loc.t("send.sendTitle", vars: ["symbol": symbol]),
                backLabel: model.header.backLabel,
                action: model.header.action,
                pill: model.header.pill
            ),
            stage: stage,
            title: title,
            captions: captions,
            hash: hash.map { value in
                ReceiptHashModel(
                    label: loc.t(
                        view.txHash?.isEmpty == false
                            ? "componentsTx.receipt.txHash"
                            : "componentsTx.receipt.userOpHash"
                    ),
                    value: value,
                    copyLabel: model.hash?.copyLabel
                        ?? loc.t("componentsUi.identiconViewer.copyAddress")
                )
            },
            // A chain with no explorer gets no button — sending somebody to the
            // wrong explorer is the misleading link 051 refused to port.
            viewOnExplorer: view.txHash?.isEmpty == false ? model.viewOnExplorer : nil,
            cta: {
                if stage == .confirmed || stage == .failed || stage == .notSent {
                    return loc.t("componentsTx.receipt.done")
                }
                // The ceremony's only honest button.
                return view.txStatus == "signing"
                    ? loc.t("componentsUi.funding.cancel")
                    : loc.t("send.txCloseBackground")
            }(),
            ctaAccent: stage == .confirmed || stage == .failed || stage == .notSent,
            ctaCancels: stage == .submitting && view.txStatus == "signing",
            eta: eta,
            breakdownTitle: parts.title,
            breakdown: parts.rows
        )
    }

    /// A receipt's parts (web `receiptParts`). A sweep's are its coins —
    /// every one the core says the operation sent (spec 097 F, S3); its one
    /// recipient stays the caption. A split's are its people, as on the
    /// confirm (#261): from the receipt's own frozen transfers once the core
    /// has them, from the drafts before that, and `people` counts them for
    /// the caption. Nothing for a single send.
    static func receiptParts(
        _ view: SendViewWire, symbol: String, loc: Loc
    ) -> (title: String?, rows: [BreakdownRowModel], people: String?) {
        let coins = view.receipt?.coins ?? []
        // Several coins (spec 097 F): one coin — a single send, a split, or a
        // sweep whose native line the gas reserve dropped — is the title's.
        if coins.count > 1 {
            // The sweep's chain — never 0. With none (a receipt read back
            // with no send in hand), the coins' own named logos and no badge.
            let chainId = view.multiChainId ?? view.selectedToken?.chainId
            let rows = coins.map { coin in
                BreakdownRowModel(
                    lead: chainId.map { chainId in
                        TokenMarkModel.of(
                            chainId: chainId, symbol: coin.symbol, tokenAddress: coin.tokenAddress,
                            color: chainColor(chainId), named: coin.logoUrls
                        )
                    } ?? TokenMarkModel(ticker: coin.symbol, badgeColor: chainColor(0),
                                        logoURLs: coin.logoUrls, badgeHidden: true),
                    label: coin.symbol,
                    value: "\(WalletLive.tokenAmountText(coin.amount)) \(coin.symbol)"
                )
            }
            return (loc.t("componentsTx.receipt.assetsCount", vars: ["n": String(coins.count)]), rows, nil)
        }
        let frozen = view.receipt?.kind == "split" ? view.receipt?.transfers ?? [] : []
        let rows: [BreakdownRowModel]
        if !frozen.isEmpty {
            rows = frozen.map { transfer in
                BreakdownRowModel(
                    identiconSeed: transfer.to,
                    label: transfer.toName ?? AddressText.short(transfer.to),
                    value: "\(transfer.amount) \(transfer.symbol)".trimmingCharacters(in: .whitespaces)
                )
            }
        } else if view.splitMode {
            rows = view.recipients.map { draft in
                BreakdownRowModel(
                    identiconSeed: draft.address.isEmpty ? nil : draft.address,
                    label: draft.name ?? AddressText.short(draft.address),
                    value: "\(draft.amount) \(symbol)".trimmingCharacters(in: .whitespaces)
                )
            }
        } else {
            rows = []
        }
        guard !rows.isEmpty else { return (nil, [], nil) }
        let key = rows.count == 1 ? "send.recipientCount_one" : "send.recipientCount_other"
        let people = loc.t(key, vars: ["count": String(rows.count)])
        return (people, rows, people)
    }

    // MARK: - SD2F, the fee-token sheet

    /// `chainId` is the SEND's chain (its token's, or the sweep's) — the
    /// estimate's is absent while a quote is out or after one failed, and
    /// "absent" used to be chain 0: every ERC-20 asked `assets/eip155-0` and
    /// every badge `eip155-0.png`, so the sheet was letters and grey dots.
    static func feeSheet(
        _ fee: FeeViewWire, on model: FeeTokenPickModel, loc: Loc, chainId: Int? = nil
    ) -> FeeTokenPickModel {
        let chain = chainId ?? fee.fee?.chainId
        let rows = fee.options.map { option in
            FeeTokenRowModel(
                mark: chain.map { chain in
                    TokenMarkModel.of(chainId: chain, symbol: option.symbol,
                                      tokenAddress: option.contract, color: chainColor(chain))
                } ?? TokenMarkModel(ticker: option.symbol, badgeColor: chainColor(0), badgeHidden: true),
                symbol: option.symbol,
                // The app's own figures: a balance as the asset list reads it
                // (`tokenAmountText`), a fee as the confirm reads it
                // (`feeFromBase`, six places, rounded up). Full precision —
                // "0.000300194967818323", "~0.0001720278 BNB" — wrapped mid-
                // number on a phone (iPhone pass 2026-10-09).
                balanceLabel: WalletLive.tokenAmountText(fromBase(option.balance, decimals: option.decimals)),
                fee: option.amount.map { "~\(feeFromBase($0, decimals: option.decimals)) \(option.symbol)" } ?? "—",
                selected: option.selected,
                // The core's verdict, passed on: every published row is drawn,
                // and the ones that cannot pay are drawn as that.
                insufficient: option.insufficient,
                insufficientNote: loc.t("send.warnInsufficientGas", vars: ["sym": option.symbol])
            )
        }
        return FeeTokenPickModel(
            title: model.title, closeLabel: model.closeLabel, hint: model.hint,
            estimateLabel: model.estimateLabel, rows: rows
        )
    }

    /// SD2f before the fee session has said anything (no quote asked yet):
    /// the sheet's chrome and no coins. The drawing's rows were ETH, USDC
    /// and USDT at its own balances, and they stood in on a live send.
    static func feeSheetReading(on model: FeeTokenPickModel) -> FeeTokenPickModel {
        FeeTokenPickModel(
            title: model.title, closeLabel: model.closeLabel, hint: model.hint,
            estimateLabel: model.estimateLabel, rows: []
        )
    }

    // MARK: - SD2C, the payroll importer

    /// The web's `liveBatchImport`, word for word: the core parsed, priced and
    /// gated; this only says so.
    ///
    /// Every number here is the core's own string. The rate is shown as typed
    /// rather than reformatted, because a rate somebody pinned by hand and a
    /// rate the wallet fetched must read the same — and because reformatting
    /// what is in an open field steals characters as they type.
    ///
    /// `replaces` is the person's choice, made on this sheet: an import ADDS to
    /// the rows already on the form unless they asked for it to replace them.
    static func batchImport(
        _ batch: BatchViewWire, view: SendViewWire, on model: BatchImportModel, loc: Loc,
        replaces: Bool = false
    ) -> BatchImportModel {
        let symbol = view.selectedToken?.symbol ?? ""
        let count = batch.recipientCount
        // Whether there is anyone on the form for an import to meet — the
        // core's own count, read back from the room it reports.
        let formHasRows = view.splitImportRoom < BatchStore.maxRecipients
        let total: BatchTotalModel? = count > 0
            ? BatchTotalModel(
                label: "\(loc.t("send.splitTotalLabel")) · " + loc.t(
                    count == 1 ? "send.recipientCount_one" : "send.recipientCount_other",
                    vars: ["count": String(count)]
                ),
                value: "\(trim(batch.totalToken)) \(symbol)",
                detail: batch.totalFiat.map { "\($0) \(batch.fiatCode)" },
                // Adding to people already typed, what is left to give out is
                // the figure that matters; otherwise the balance itself.
                balance: formHasRows && !replaces && view.splitRemaining != nil
                    ? loc.t("send.splitRemaining", vars: [
                        "amount": "\(trim(view.splitRemaining ?? "")) \(symbol)",
                    ])
                    : loc.t("send.balanceLabel", vars: [
                        "amount": "\(trim(view.selectedToken?.balance ?? "")) \(symbol)",
                    ]),
                over: batch.overBalance
            )
            : nil
        // Said once the import can happen, beside the button that does it —
        // with the way to choose the other, because either can be what is meant.
        let merge: BatchMergeModel? = formHasRows && batch.canApply
            ? (replaces
                ? BatchMergeModel(
                    note: loc.t("send.batchReplacesRows"), action: loc.t("send.batchAddInstead")
                )
                : BatchMergeModel(
                    note: loc.t("send.batchAddsToRows"), action: loc.t("send.batchReplaceInstead")
                ))
            : nil
        let rateValue = switch batch.rateStatus {
        case .ok: "\(batch.rateInput) \(batch.fiatCode)"
        case .loading: loc.t("send.batchRateLoading")
        // Unknown, and said so — the core has already refused to apply.
        case .failed: loc.t("send.batchRateFailed")
        }
        let cta = switch count {
        case 0: loc.t("send.batchApplyEmpty")
        case 1: loc.t("send.batchApply_one", vars: ["count": "1"])
        default: loc.t("send.batchApply_other", vars: ["count": String(count)])
        }
        // One line, in the desktop's order of consequence: a file that could
        // not be read at all, then a total that cannot be paid, then a list
        // that will be trimmed, then the receipt for a template just saved.
        //
        // A picked file that silently does nothing is indistinguishable from a
        // broken picker, which is why the first of these exists at all.
        let note: (text: String, error: Bool)? =
            if batch.fileError {
                (
                    // 087: a legacy code page says how to save the file — the
                    // contacts import's sentence — not "use a CSV", which it is.
                    "\(loc.t("send.batchImportFailedTitle"))\n\(loc.t(batch.fileFailure == .unsupportedEncoding ? "contacts.importFailEncoding" : "send.batchImportFailedBody"))",
                    true
                )
            } else if batch.overBalance {
                (loc.t("send.batchOverBalance", vars: ["sym": symbol]), true)
            } else if batch.overCap {
                // The cap the sheet was opened with: the room the form has
                // left, which is sixty only while the form is empty.
                (loc.t("send.batchOverCap", vars: ["n": String(view.splitImportRoom)]), false)
            } else if batch.templateSaved {
                (loc.t("send.batchTemplateSaved"), false)
            } else {
                nil
            }
        return BatchImportModel(
            title: model.title,
            closeLabel: model.closeLabel,
            unitFiat: loc.t("send.batchUnitFiat", vars: ["code": batch.fiatCode]),
            unitToken: loc.t("send.batchUnitToken", vars: ["sym": symbol]),
            unit: batch.unit,
            pasteValue: batch.rawText,
            pastePlaceholder: model.pastePlaceholder,
            importFile: model.importFile,
            template: model.template,
            rateSection: model.rateSection,
            rateLabel: loc.t("send.batchRateLabel", vars: ["sym": symbol]),
            rateValue: rateValue,
            rateHint: loc.t("send.batchRateHint", vars: ["code": batch.fiatCode, "sym": symbol]),
            rateReset: loc.t("send.batchRateReset"),
            rateEdited: batch.rateEdited,
            // The core's own flag. In 按 xDAI 数量 the figures in the file ARE
            // the token amounts and no rate is applied, so a rate row there is
            // a control for a conversion that is not happening.
            priced: batch.priced,
            parsedLabel: loc.t("send.batchParsedCount", vars: ["n": String(count)]),
            rows: batch.preview.map { row in
                BatchRowModel(
                    ok: row.ok,
                    address: row.name ?? row.address,
                    conversion: row.tokenAmount.isEmpty
                        ? row.rawAmount
                        : "\(row.tokenAmount) \(symbol)"
                )
            },
            rejectedText: batch.rejected > 0
                ? loc.t(
                    batch.rejected == 1 ? "send.batchRejected_one" : "send.batchRejected_other",
                    vars: ["count": String(batch.rejected)]
                )
                : nil,
            note: note?.text,
            noteIsError: note?.error ?? false,
            total: total,
            merge: merge,
            cta: cta,
            // The core's ONE gate. Never a conjunction assembled here:
            // `canApply` already knows about the busy fetch, the rejected rows
            // and the balance, and a second opinion would eventually disagree.
            ctaDisabled: !batch.canApply
        )
    }

    // MARK: - SD2E, the contact picker

    /// The person's own address book, in the drawn picker.
    ///
    /// The name shown is theirs if they gave one, the resolved name if a
    /// registry knew it, and the shortened address otherwise — the same order
    /// the address book itself uses, so one person is not two names on two
    /// screens.
    static func contactSheet(
        _ book: ContactsViewWire, on model: ContactPickModel, loc: Loc
    ) -> ContactPickModel {
        // The person's own groups — the drawing's two were a picture that did
        // nothing when tapped. A tap ADDS the group's members to the form
        // (`append_split_recipients`), named by the group's id and each person
        // by their address (issue #467) — never by a place in the list.
        let swatches = ChainCatalog.chains.map { chainColor($0.chainId) }
        return ContactPickModel(
            title: model.title,
            closeLabel: model.closeLabel,
            searchPlaceholder: model.searchPlaceholder,
            groupsTitle: model.groupsTitle,
            groups: book.groups.enumerated().map { index, group in
                ContactGroupModel(
                    id: group.id,
                    name: group.name,
                    count: loc.t("contacts.groupMembers", vars: ["count": String(group.members.count)]),
                    // Two discs from the chain palette — decoration, not
                    // identity, and the web's same cycle.
                    colors: swatches.isEmpty ? [] : [
                        swatches[(2 * index) % swatches.count],
                        swatches[(2 * index + 1) % swatches.count],
                    ]
                )
            },
            contactsTitle: model.contactsTitle,
            contacts: book.contacts.map { contact in
                ContactEntryModel(
                    name: contact.name ?? contact.resolvedName ?? AddressText.short(contact.address),
                    group: book.groups.first { group in
                        group.members.contains { $0.address == contact.address }
                    }?.name,
                    address: contact.address,
                    addressDisplay: AddressText.short(contact.address),
                    identiconSeed: contact.address
                )
            }
        )
    }

    /// The picker while the book is still being read: the drawn chrome —
    /// title and search — and nobody in it. Never the drawing's
    /// people (issue #467).
    static func contactSheetReading(on model: ContactPickModel) -> ContactPickModel {
        ContactPickModel(
            title: model.title,
            closeLabel: model.closeLabel,
            searchPlaceholder: model.searchPlaceholder,
            groupsTitle: model.groupsTitle,
            groups: [],
            contactsTitle: model.contactsTitle,
            contacts: []
        )
    }

    /// A whole group as split rows, amounts blank, each member by the name
    /// the person gave them. The core mints the ids; `nil` for an empty group,
    /// which adds nobody.
    ///
    /// Only the person's OWN name: a split row's name is drawn as their word,
    /// untagged, on the page that signs (spec 097 F) — a registry or ENS name
    /// seeded here would pose as one. Unnamed, the row shows the address.
    static func groupRecipients(_ group: ContactGroupWire) -> [[String: Any]]? {
        guard !group.members.isEmpty else { return nil }
        return group.members.map { member in
            [
                "id": "",
                "address": member.address,
                "amount": "",
                "name": member.name.map { $0 as Any } ?? NSNull(),
            ]
        }
    }

    // MARK: - Formatting

    static let zeroAddress = "0x0000000000000000000000000000000000000000"

    /// The chain's badge colour, from the one place that decides it.
    static func chainColor(_ chainId: Int) -> Color {
        SettingsLive.mark(chainId: chainId, name: "").color
    }

    static func isAddress(_ value: String) -> Bool {
        value.count == 42 && value.hasPrefix("0x") && value.dropFirst(2).allSatisfy(\.isHexDigit)
    }

    /// A base-unit decimal string as a human one. `TokenReads` owns the
    /// arithmetic; a `Double` here would round somebody's money.
    /// A FEE in its coin, to read rather than to audit: six decimals, as the
    /// web shows it. Full precision — `0.000410400290875302 ETH` — crushed the
    /// row's label and told nobody anything the first three figures had not
    /// (founder's device, 2026-09-19). Rounded UP, because a fee that displays
    /// as less than it costs is the wrong way to be wrong; a fee below the sixth
    /// decimal keeps two significant figures instead of reading as zero.
    static func feeFromBase(_ base: String, decimals: Int) -> String {
        let value = NSDecimalNumber(string: base).multiplying(byPowerOf10: Int16(-decimals))
        guard value != .notANumber else { return base }
        if value == .zero { return "0" }
        var scale: Int16 = 6
        if value.compare(NSDecimalNumber(string: "0.000001")) == .orderedAscending {
            // Two significant figures: the exponent of the leading digit, plus one.
            let exponent = Int16(ceil(-log10(value.doubleValue)))
            scale = exponent + 1
        }
        let rounding = NSDecimalNumberHandler(
            roundingMode: .up, scale: scale, raiseOnExactness: false,
            raiseOnOverflow: false, raiseOnUnderflow: false, raiseOnDivideByZero: false
        )
        return trim(value.rounding(accordingToBehavior: rounding).stringValue)
    }

    static func fromBase(_ base: String, decimals: Int) -> String {
        TokenReads.scaled(decimal: base, decimals: decimals) ?? base
    }

    /// Trailing zeros off, nothing else. Never a rounding.
    static func trim(_ value: String) -> String {
        guard value.contains(".") else { return value }
        var out = value
        while out.hasSuffix("0") { out.removeLast() }
        if out.hasSuffix(".") { out.removeLast() }
        return out.isEmpty ? "0" : out
    }
}
