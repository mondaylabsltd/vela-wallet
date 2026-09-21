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
            case "submitted", "failed": return .sd4b
            default: return .sd4a
            }
        }
    }

    // MARK: - SD1, the token picker

    static func pick(
        _ view: SendViewWire, on model: SendPickModel, picking: Bool = false,
        classFilter: String = "all", loc: Loc
    ) -> SendPickModel {
        // Which class of token is on screen. The CORE lists every holding —
        // which subset a person is looking at is a render decision, like the
        // sweep's picking flag, and the chips were drawn in 021 with nothing
        // behind them.
        let shown = view.tokens.filter { matches(classFilter, token: $0) }
        return SendPickModel(
            header: model.header,
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
            notice: lockNotice(view, loc: loc) ?? (picking && view.multiChainId != nil
                ? SendNoticeModel(
                    mark: model.notice?.mark
                        ?? TokenMarkModel(
                            ticker: "",
                            badgeColor: chainColor(view.multiChainId ?? 0)
                        ),
                    text: loc.t("send.multiSendSummary", vars: [
                        "n": String(view.multiSelectedIds.count),
                        "chain": view.multiChainId
                            .flatMap { ChainCatalog.meta($0)?.displayName } ?? "",
                    ])
                )
                : nil),
            rows: shown.map(assetRow),
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
                            .flatMap { ChainCatalog.meta($0)?.displayName } ?? "",
                    ]),
                    // Nothing ticked is nothing to send.
                    accent: !view.multiSelectedIds.isEmpty
                )
                : model.cta
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
    static func lockNotice(_ view: SendViewWire, loc: Loc) -> SendNoticeModel? {
        switch view.lockError {
        case .network(let chainId):
            return SendNoticeModel(
                mark: TokenMarkModel(ticker: "", badgeColor: chainColor(chainId)),
                text: "\(loc.t("send.lock.netTitle")) · "
                    + loc.t("send.lock.netBody", vars: ["chainId": String(chainId)])
            )
        case .token:
            return SendNoticeModel(
                mark: TokenMarkModel(ticker: "", badgeColor: chainColor(0)),
                text: "\(loc.t("send.lock.tokenTitle")) · \(loc.t("send.lock.tokenBody"))"
            )
        case nil:
            // An add-network attempt that FAILED still owes a sentence — the
            // person asked for something and it did not happen.
            switch view.addNetworkMsg {
            case .netNotFound:
                return SendNoticeModel(
                    mark: TokenMarkModel(ticker: "", badgeColor: chainColor(0)),
                    text: loc.t("send.lock.netNotFound")
                )
            case .netNotCompatible:
                return SendNoticeModel(
                    mark: TokenMarkModel(ticker: "", badgeColor: chainColor(0)),
                    text: loc.t("send.lock.netNotCompatible")
                )
            case .netAddError:
                return SendNoticeModel(
                    mark: TokenMarkModel(ticker: "", badgeColor: chainColor(0)),
                    text: loc.t("send.lock.netAddError")
                )
            case nil:
                return nil
            }
        }
    }

    private static func assetRow(_ token: SendTokenWire) -> AssetRowModel {
        AssetRowModel(
            ticker: token.symbol,
            chain: ChainCatalog.meta(token.chainId)?.displayName ?? token.network,
            badgeColor: chainColor(token.chainId),
            balance: "\(trim(token.balance)) \(token.symbol)",
            // The picker's job is to choose an asset, so the row states the
            // holding. A priced row's fiat line is the home's business.
            fiat: token.priceUsd == nil ? .noPrice("") : .value(""),
            masked: false
        )
    }

    // MARK: - SD2, the form

    static func form(
        _ view: SendViewWire,
        fee: FeeViewWire?,
        display: WalletLive.Display,
        on model: SendFormModel,
        loc: Loc
    ) -> SendFormModel {
        var live = model
        let token = view.selectedToken
        let symbol = token?.symbol ?? ""
        let chain = token.map { ChainCatalog.meta($0.chainId)?.displayName ?? $0.network } ?? ""

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
                mark: TokenMarkModel.of(
                    chainId: held.chainId, symbol: held.symbol,
                    tokenAddress: held.tokenAddress, color: chainColor(held.chainId)
                ),
                symbol: held.symbol,
                detail: "\(chain) · \(trim(held.balance))",
                max: model.token?.max
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
                fiat: view.amountFiatCode != nil
                    ? "\(trim(view.tokenAmount)) \(symbol)"
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
                note: view.recipientIdentity?.name ?? drawn.note
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
                    mark: TokenMarkModel.of(
                        chainId: token.chainId, symbol: token.symbol,
                        tokenAddress: token.tokenAddress, color: chainColor(token.chainId)
                    ),
                    symbol: token.symbol,
                    balanceLabel: "\(trim(token.balance)) \(token.symbol)",
                    // **No spec, no figure.** Falling back to the balance would
                    // print a number the operation does not carry — the whole
                    // reason these rows read `multi_specs` at all. An empty
                    // amount is "not worked out yet", which is true.
                    amount: spec.map { "\(trim($0.amount)) \(token.symbol)" } ?? "",
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
                    .flatMap { ChainCatalog.meta($0)?.displayName } ?? "",
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
            fee: feeRow(model.fee, view: view, fee: fee, display: display, loc: loc),
            cta: live.cta,
            // While the pre-check is out the button is busy, not unfinished.
            hint: view.splitMode && !view.estimatingGas ? splitHint(issues, loc: loc) : nil
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
        display: WalletLive.Display, loc: Loc
    ) -> FeeRowModel {
        let busy = view.estimatingGas || view.feeBusy || (fee?.busy ?? false)
        let text = view.fee.map { feeLine($0, view: view, fee: fee, display: display) }
        // With no estimate, the row says so — it does NOT fall back to the
        // drawing's value. The fixture's "0.0021 ETH · ≈$0.55" appeared on a
        // Gnosis send on the founder's iPhone while the quote was still in
        // flight: a number in the fee slot is a promise about what this costs,
        // and the drawing's number is a promise about somebody else's send.
        let waiting = busy ? loc.t("send.estimatingFee") : "—"
        return FeeRowModel(
            label: fallback.label,
            // The fee is paid on THIS chain. The drawing's mark was ETH, which
            // is the wrong badge on every network but one.
            mark: view.fee.map { estimate in
                TokenMarkModel.of(
                    chainId: estimate.chainId, symbol: feeTicker(estimate),
                    color: chainColor(estimate.chainId)
                )
            } ?? fallback.mark,
            value: text ?? waiting,
            openLabel: fallback.openLabel
        )
    }

    /// "0.0021 XDAI" from the estimate — the fee asset's own units, **never
    /// re-priced here**.
    ///
    /// `total_wei` is the native figure even when the fee is paid in an ERC-20;
    /// the token figure is the asset's own `amount` in its own decimals.
    /// Reading the first where the second belongs prints a six-decimal
    /// stablecoin fee as an eighteen-decimal number.
    static func feeText(_ estimate: FeeEstimateWire) -> String {
        switch estimate.feeAsset {
        case .native:
            let symbol = ChainCatalog.meta(estimate.chainId)?.nativeSymbol ?? ""
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
        display: WalletLive.Display
    ) -> String {
        let coin = feeText(estimate)
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

    /// The symbol the fee is charged in.
    private static func feeTicker(_ estimate: FeeEstimateWire) -> String {
        switch estimate.feeAsset {
        case .native: return ChainCatalog.meta(estimate.chainId)?.nativeSymbol ?? ""
        case .erc20(_, _, _, let symbol): return symbol ?? "TOKEN"
        }
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
            let body = loc.t("send.sameFeeTokenBody", vars: [
                "amount": fromBase(issue.transferAmount, decimals: decimals),
                "fee": fromBase(issue.feeAmount, decimals: decimals),
                "total": fromBase(issue.total, decimals: decimals),
                "symbol": issue.symbol,
                "balance": fromBase(issue.balance, decimals: decimals),
            ])
            let most = loc.t("send.sameFeeTokenMax", vars: [
                "amount": fromBase(issue.maxTransferAmount, decimals: decimals),
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
        fee: FeeViewWire? = nil
    ) -> SendConfirmModel {
        let live = model
        let token = view.selectedToken
        let symbol = token?.symbol ?? ""
        let chain = token.map { ChainCatalog.meta($0.chainId)?.displayName ?? $0.network } ?? ""

        let recipientName = view.recipientIdentity?.name
        var facts = [
            FactRowModel(
                label: model.facts.first?.label ?? "",
                value: from.name ?? AddressText.short(from.address)
            ),
            FactRowModel(
                label: model.facts.count > 1 ? model.facts[1].label : "",
                value: recipientName.map { "\($0) · \(AddressText.short(view.recipient))" }
                    ?? AddressText.short(view.recipient)
            ),
            FactRowModel(
                label: model.facts.count > 2 ? model.facts[2].label : "",
                value: chain
            ),
            FactRowModel(
                label: model.facts.count > 3 ? model.facts[3].label : "",
                // With no estimate the row says so, as the form's row does —
                // never the drawing's "~0.0021 ETH · ≈$0.55", a promise about
                // somebody else's send (the web's `feeText`: `send.fee ?? fee.fee`).
                value: (view.fee ?? fee?.fee).map { "~\(feeLine($0, view: view, fee: fee, display: display))" }
                    ?? (view.estimatingGas || view.feeBusy || (fee?.busy ?? false)
                        ? loc.t("send.estimatingFee") : "—")
            ),
        ]

        // **The parts, from the core's own rows — never the drawing's.** The
        // drawn SD3b/SD3c carry "Alice 50 USDT" and a three-coin sweep, and a
        // confirm page is the one screen that must not show a payee nobody
        // typed: it is what the person reads before they sign.
        var amount = "\(trim(view.confirmAmount)) \(symbol)"
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
            subline = loc.t("send.confirmTotalLine", vars: [
                "fiat": money(sweep.totalUsd, display: display),
                "network": view.multiChainId
                    .flatMap { ChainCatalog.meta($0)?.displayName } ?? chain,
            ])
        } else if view.splitMode, !view.recipients.isEmpty {
            // A split: every payee by name and face, and how many there are.
            // The single "To" row has no one address to name, so it goes —
            // the count and the list below say who instead (web 038 #D2).
            breakdown = splitBreakdown(view)
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
            subline: subline,
            facts: facts,
            breakdown: breakdown,
            notice: confirmNotice(view, loc: loc),
            // The treasury pause has TWO exits (spec 054 US4): the core's
            // retry, and 暂不 — which keeps the facts on screen rather than
            // throwing the attempt away. A submit the relay refused has one,
            // and a passkey prompt that is up has only cancel, which is the
            // core's own checkpoint.
            noticeAction: {
                if view.treasuryBootstrap != nil {
                    loc.t("componentsUi.treasuryBootstrap.retryBtn")
                } else if view.txError != nil {
                    loc.t("send.txRetryBtn")
                } else if view.txStatus == "signing" {
                    loc.t("componentsUi.funding.cancel")
                } else {
                    nil
                }
            }(),
            noticeSecondary: view.treasuryBootstrap != nil
                ? loc.t("componentsUi.funding.cancel")
                : nil,
            repeatNote: confirmRepeatNote(view, loc: loc),
            cta: live.cta
        )
    }

    /// A split's payees, one row each, from the core's drafts.
    ///
    /// A name never stands in for the address on the page that signs: a named
    /// row reads "Alice · 0x12…34f0". Only a real address earns a face.
    static func splitBreakdown(_ view: SendViewWire) -> [BreakdownRowModel] {
        let symbol = view.selectedToken?.symbol ?? ""
        return view.recipients.map { row in
            let short = AddressText.short(row.address)
            return BreakdownRowModel(
                identiconSeed: isAddress(row.address) ? row.address : "",
                label: row.name.map { short.isEmpty ? $0 : "\($0) · \(short)" } ?? short,
                value: "\(trim(row.amount)) \(symbol)"
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
            let value = "\(trim(amount)) \(token.symbol)"
            return BreakdownRowModel(
                lead: TokenMarkModel.of(
                    chainId: token.chainId, symbol: token.symbol,
                    tokenAddress: token.tokenAddress, color: chainColor(token.chainId)
                ),
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
        if let status = view.treasuryBootstrap {
            let decimals = status.asset == "path_usd" ? 6 : 18
            let symbol = status.asset == "path_usd"
                ? "pathUSD"
                : (ChainCatalog.meta(status.chainId)?.nativeSymbol ?? "")
            let short = shortfall(floor: status.floor, balance: status.balance)
            // Two situations, one symptom: on a network Vela ships the
            // operator owns that relayer and telling them is the fix; on one
            // the person added, there may be nobody else who can hold gas
            // there at all (spec 060).
            let lead = status.operatorServed
                ? loc.t("componentsUi.treasuryBootstrap.operatorLead")
                : loc.t("componentsUi.treasuryBootstrap.customLead")
            return loc.t("componentsUi.treasuryBootstrap.title") + " · "
                + lead + " "
                + loc.t("componentsUi.treasuryBootstrap.amountHint", vars: [
                    "amount": fromBase(short, decimals: decimals), "symbol": symbol,
                ])
        }
        switch view.txError {
        case "bundler_fund": return loc.t("send.txErrorBundlerFund")
        case "generic": return loc.t("send.txErrorGeneric")
        default: return view.txStatus == "signing" ? loc.t("send.txPreparingBiometric") : nil
        }
    }

    /// Title and body for every alert the core raises, in the core's words.
    static func alertText(_ kind: [String: Any], loc: Loc) -> (title: String, body: String) {
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
            return (loc.t("send.alertEstimateFailedTitle"), loc.t("send.alertEstimateFailedBody"))
        case "account_unavailable":
            return (loc.t("send.alertEstimateFailedTitle"), loc.t("send.alertAccountUnavailableBody"))
        default:
            return (loc.t("send.alertEstimateFailedTitle"), "")
        }
    }

    // MARK: - SD4, the receipt

    /// Which drawn state the receipt is in. The core's status, never a guess
    /// from whether a hash happens to be present.
    static func receiptStage(_ view: SendViewWire) -> ReceiptStage {
        switch view.receipt?.status {
        case "confirmed": return .confirmed
        case "failed": return .failed
        case "submitted": return .submitted
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
        loc: Loc
    ) -> SendReceiptModel {
        let token = view.selectedToken
        let symbol = token?.symbol ?? ""
        let chain = token.map { ChainCatalog.meta($0.chainId)?.displayName ?? $0.network } ?? ""
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
        case .submitted:
            title = loc.t("send.txSubmittedTitle")
            // Held for fees: the payment is not stuck and not lost — it goes
            // out by itself when fees settle, and saying so is the difference
            // between waiting and sending it twice.
            if view.receipt?.holdReason != nil {
                captions = [loc.t("send.txHeldFees")]
                break
            }
            captions = [loc.t("send.txWaitingConfirm")]
            if let seconds = view.receipt?.typicalInclusionS {
                let typical = loc.t("send.txTypicalTime", vars: [
                    "chainName": chain, "estSecs": String(seconds),
                ])
                // When the core says WHEN it was handed over, the screen counts
                // (web `live-send.ts`, #199); the typical line leads the clock.
                if let at = view.receipt?.submittedAtMs {
                    eta = ReceiptEtaModel(
                        submittedAtMs: at, typicalS: seconds, typicalLine: typical,
                        // Filled with its own placeholder: the screen fills the number.
                        remainingTemplate: loc.t("send.txRemaining", vars: ["remaining": "{{remaining}}"]),
                        elapsedTemplate: loc.t("send.txElapsed", vars: ["elapsed": "{{elapsed}}"]),
                        slowLine: loc.t("send.txSlowConfirm")
                    )
                } else {
                    captions.append(typical)
                }
            }
        case .confirmed:
            title = loc.t("send.txConfirmedTitle", vars: [
                "amount": trim(view.receipt?.amount ?? view.confirmAmount),
                "symbol": symbol,
            ])
            // A split names its count here and its people below; "To " with
            // nobody after it was what the single-recipient line read as.
            let to = parts.title ?? view.recipientIdentity?.name ?? AddressText.short(view.recipient)
            captions = ["\(to) · \(chain)"]
        case .failed:
            title = loc.t("componentsTx.receipt.statusFailed")
            // A fee-rejected send is not a failure of the transfer: the fee
            // rose above what was approved and NOTHING was sent, which is a
            // different sentence and a different next step. `hold_reason` was
            // on this client's wire and read by no Swift at all until 058.
            if view.receipt?.holdReason != nil {
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
                title: loc.t("send.sendTitle", vars: ["symbol": symbol]),
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
                if stage == .confirmed || stage == .failed {
                    return loc.t("componentsTx.receipt.done")
                }
                // The ceremony's only honest button.
                return view.txStatus == "signing"
                    ? loc.t("componentsUi.funding.cancel")
                    : loc.t("send.txCloseBackground")
            }(),
            ctaAccent: stage == .confirmed || stage == .failed,
            ctaCancels: stage == .submitting && view.txStatus == "signing",
            eta: eta,
            breakdownTitle: parts.title,
            breakdown: parts.rows
        )
    }

    /// A split's parts on the receipt as on the confirm (web `receiptParts`,
    /// #261): from the receipt's own frozen transfers once the core has them,
    /// from the drafts before that. Nothing for a single send or a sweep — the
    /// sweep's parts are assets, and its one recipient is already the caption.
    static func receiptParts(
        _ view: SendViewWire, symbol: String, loc: Loc
    ) -> (title: String?, rows: [BreakdownRowModel]) {
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
        guard !rows.isEmpty else { return (nil, []) }
        let key = rows.count == 1 ? "send.recipientCount_one" : "send.recipientCount_other"
        return (loc.t(key, vars: ["count": String(rows.count)]), rows)
    }

    // MARK: - SD2F, the fee-token sheet

    static func feeSheet(_ fee: FeeViewWire, on model: FeeTokenPickModel, loc: Loc) -> FeeTokenPickModel {
        let rows = fee.options.map { option in
            FeeTokenRowModel(
                mark: TokenMarkModel.of(
                    chainId: fee.fee?.chainId ?? 0, symbol: option.symbol,
                    tokenAddress: option.contract, color: chainColor(fee.fee?.chainId ?? 0)
                ),
                symbol: option.symbol,
                balanceLabel: trim(fromBase(option.balance, decimals: option.decimals)),
                fee: option.amount.map { "~\(trim(fromBase($0, decimals: option.decimals))) \(option.symbol)" } ?? "—",
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
                    "\(loc.t("send.batchImportFailedTitle"))\n\(loc.t("send.batchImportFailedBody"))",
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
        // (`append_split_recipients`), so the order here is the order the
        // shell indexes into: `book.groups`.
        let swatches = ChainCatalog.chains.map { chainColor($0.chainId) }
        return ContactPickModel(
            title: model.title,
            closeLabel: model.closeLabel,
            searchPlaceholder: model.searchPlaceholder,
            scanRow: model.scanRow,
            groupsTitle: model.groupsTitle,
            groups: book.groups.enumerated().map { index, group in
                ContactGroupModel(
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
                    addressDisplay: AddressText.short(contact.address),
                    identiconSeed: contact.address
                )
            }
        )
    }

    /// A whole group as split rows, amounts blank, each member by the name the
    /// book shows for them. The core mints the ids; `nil` for an empty group,
    /// which adds nobody.
    static func groupRecipients(_ group: ContactGroupWire) -> [[String: Any]]? {
        guard !group.members.isEmpty else { return nil }
        return group.members.map { member in
            [
                "id": "",
                "address": member.address,
                "amount": "",
                "name": (member.name ?? member.resolvedName).map { $0 as Any } ?? NSNull(),
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

    /// `floor - balance`, floored at zero, in base units and string arithmetic.
    private static func shortfall(floor: String, balance: String) -> String {
        guard let a = Double(floor), let b = Double(balance), a > b else { return "0" }
        return String(format: "%.0f", a - b)
    }
}
