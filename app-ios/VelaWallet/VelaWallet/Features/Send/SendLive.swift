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
        _ view: SendViewWire, on model: SendPickModel, picking: Bool = false, loc: Loc
    ) -> SendPickModel {
        SendPickModel(
            header: model.header,
            searchPlaceholder: model.searchPlaceholder,
            filters: model.filters,
            // Once a chain is pinned, say which — the dimmed rows are the
            // consequence and this is the reason. The corpus has no sentence
            // for "locked to", so the summary line's own words carry it:
            // "N 个代币 · Gnosis" is exactly what has been chosen.
            notice: picking && view.multiChainId != nil
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
                : nil,
            rows: view.tokens.map(assetRow),
            selection: picking ? SendSelectionModel(
                selected: view.tokens.map { view.multiSelectedIds.contains($0.id) },
                // A row on a chain the pick has left behind is drawn dimmed and
                // is not tappable: a tappable row is an invitation the wallet
                // will not honour.
                dimmed: view.tokens.map { SweepPick.dimmed(view: view, chainId: $0.chainId) },
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

        live.token = token.map { held in
            SendTokenCardModel(
                mark: TokenMarkModel(ticker: held.symbol, badgeColor: chainColor(held.chainId)),
                symbol: held.symbol,
                detail: "\(chain) · \(trim(held.balance))",
                max: model.token?.max
            )
        } ?? model.token

        // The field shows what was typed; the line under it shows the OTHER
        // unit. When the figure is already fiat the other unit is the token's,
        // which is `token_amount` — the very number the signed batch is built
        // from, so the two can never disagree.
        live.amount = model.amount.map { drawn in
            AmountFieldModel(
                value: view.amount.isEmpty ? "0" : view.amount,
                fiat: view.amountFiatCode != nil
                    ? "\(trim(view.tokenAmount)) \(symbol)"
                    : fiatLine(view, token: token, display: display),
                denomLabel: view.amountFiatCode ?? display.code
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
                    mark: TokenMarkModel(
                        ticker: token.symbol, badgeColor: chainColor(token.chainId)
                    ),
                    symbol: token.symbol,
                    balanceLabel: "\(trim(token.balance)) \(token.symbol)",
                    // **No spec, no figure.** Falling back to the balance would
                    // print a number the operation does not carry — the whole
                    // reason these rows read `multi_specs` at all. An empty
                    // amount is "not worked out yet", which is true.
                    amount: spec.map { "\(trim($0.amount)) \(token.symbol)" } ?? "",
                    max: model.sweepRows.first?.max ?? ""
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
        live.recipients = view.splitMode ? view.recipients.enumerated().map { index, row in
            RecipientCardModel(
                ordinal: loc.t("send.recipientN", vars: ["n": String(index + 1)]),
                name: row.name ?? (row.address.isEmpty
                    ? loc.t("send.recipientPlaceholder")
                    : AddressText.short(row.address)),
                identiconSeed: isAddress(row.address) ? row.address : "",
                amount: "\(trim(row.amount)) \(symbol)",
                removeLabel: loc.t("send.removeRecipient"),
                rowId: row.id,
                problem: rowProblem(row, in: view, loc: loc)
            )
        } : live.recipients

        // A split's sweep-style summary: how many people, and the sum.
        live.summary = view.splitMode
            ? SummaryLineModel(
                label: loc.t("send.recipientCount", vars: [
                    "count": String(view.recipients.count),
                ]),
                value: "\(trim(view.confirmAmount)) \(symbol)"
            )
            : live.summary

        return SendFormModel(
            // The title names the token being SENT. The fixture's said USDT,
            // which on a wallet holding xDAI is a sentence about somebody
            // else's money.
            header: FlowHeaderModel(
                title: loc.t("send.sendTitle", vars: ["symbol": symbol]),
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
            fee: feeRow(model.fee, view: view, fee: fee, loc: loc),
            cta: live.cta
        )
    }

    /// What is wrong with **this** row, in the core's vocabulary.
    ///
    /// A list of six rows with one sentence underneath makes somebody count
    /// rows to find the bad one. The core does not publish a per-row verdict
    /// for the split, so the two facts it does publish are read here: an
    /// address that is not one, and a row that repeats an earlier address.
    /// Anything subtler stays the form's single warning.
    static func rowProblem(
        _ row: SendRecipientDraftWire, in view: SendViewWire, loc: Loc
    ) -> String? {
        if !row.address.isEmpty, !isAddress(row.address) {
            return loc.t("send.batchBadAddress")
        }
        let earlier = view.recipients.prefix { $0.id != row.id }
        if !row.address.isEmpty,
           earlier.contains(where: { $0.address.caseInsensitiveCompare(row.address) == .orderedSame }) {
            return loc.t("send.batchDup")
        }
        return nil
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
        return "≈ \(display.glyph)\(String(format: "%.2f", converted))"
    }

    private static func feeRow(
        _ fallback: FeeRowModel, view: SendViewWire, fee: FeeViewWire?, loc: Loc
    ) -> FeeRowModel {
        let busy = view.estimatingGas || view.feeBusy || (fee?.busy ?? false)
        let text = view.fee.map { feeText($0) }
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
                TokenMarkModel(
                    ticker: feeTicker(estimate),
                    badgeColor: chainColor(estimate.chainId)
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
            return "\(fromBase(estimate.totalWei, decimals: 18)) \(symbol)"
        case .erc20(_, let decimals, let amount, let symbol):
            return "\(fromBase(amount, decimals: decimals)) \(symbol ?? "TOKEN")"
        }
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
        return view.amountWarning.map { warningText($0, loc: loc) }
    }

    /// One sentence per warning, in the core's own keys.
    static func warningText(_ warning: SendAmountWarningWire, loc: Loc) -> String {
        switch warning {
        case .notEnoughToken:
            return loc.t("send.alertInsufficientBalanceBody")
        case .insufficientForGas(let symbol):
            return loc.t("send.warnInsufficientForGas", vars: ["sym": symbol ?? ""])
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
        loc: Loc
    ) -> SendConfirmModel {
        let live = model
        let token = view.selectedToken
        let symbol = token?.symbol ?? ""
        let chain = token.map { ChainCatalog.meta($0.chainId)?.displayName ?? $0.network } ?? ""

        let recipientName = view.recipientIdentity?.name
        let facts = [
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
                value: view.fee.map { "~\(feeText($0))" } ?? (model.facts.count > 3 ? model.facts[3].value : "—")
            ),
        ]
        return SendConfirmModel(
            header: live.header,
            // `confirm_amount` and nothing else. It is the figure the money
            // gates measured and the batch is built from — a shell that
            // re-derived it would put a number on the signing page nothing
            // else in the flow had agreed to.
            amount: "\(trim(view.confirmAmount)) \(symbol)",
            subline: view.confirmAmountIssue.map { issue in
                loc.t("send.warnCannotConvert", vars: ["code": issue.code, "symbol": issue.symbol])
            } ?? fiatLine(view, token: token, display: display),
            facts: facts,
            breakdown: live.breakdown,
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
            cta: live.cta
        )
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
            return loc.t("componentsUi.treasuryBootstrap.title") + " · "
                + loc.t("componentsUi.treasuryBootstrap.lead") + " "
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
        switch stage {
        case .submitting:
            title = loc.t("send.txSubmitting")
            captions = model.captions
        case .submitted:
            title = loc.t("send.txSubmittedTitle")
            captions = [loc.t("send.txWaitingConfirm")]
            if let seconds = view.receipt?.typicalInclusionS {
                captions.append(loc.t("send.txTypicalTime", vars: [
                    "chainName": chain, "estSecs": String(seconds),
                ]))
            }
        case .confirmed:
            title = loc.t("send.txConfirmedTitle", vars: [
                "amount": trim(view.receipt?.amount ?? view.confirmAmount),
                "symbol": symbol,
            ])
            let to = view.recipientIdentity?.name ?? AddressText.short(view.recipient)
            captions = ["\(to) · \(chain)"]
        case .failed:
            title = loc.t("componentsTx.receipt.statusFailed")
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
            cta: stage == .confirmed || stage == .failed
                ? loc.t("componentsTx.receipt.done")
                : loc.t("send.txCloseBackground"),
            ctaAccent: stage == .confirmed || stage == .failed
        )
    }

    // MARK: - SD2F, the fee-token sheet

    static func feeSheet(_ fee: FeeViewWire, on model: FeeTokenPickModel, loc: Loc) -> FeeTokenPickModel {
        let rows = fee.options.map { option in
            FeeTokenRowModel(
                mark: TokenMarkModel(
                    ticker: option.symbol,
                    badgeColor: chainColor(fee.fee?.chainId ?? 0)
                ),
                symbol: option.symbol,
                balanceLabel: trim(fromBase(option.balance, decimals: option.decimals)),
                fee: option.amount.map { "~\(trim(fromBase($0, decimals: option.decimals))) \(option.symbol)" } ?? "—",
                selected: option.selected
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
    static func batchImport(
        _ batch: BatchViewWire, view: SendViewWire, on model: BatchImportModel, loc: Loc
    ) -> BatchImportModel {
        let symbol = view.selectedToken?.symbol ?? ""
        let count = batch.recipientCount
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
                (loc.t("send.batchOverCap", vars: ["n": String(BatchStore.maxRecipients)]), false)
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
        ContactPickModel(
            title: model.title,
            closeLabel: model.closeLabel,
            searchPlaceholder: model.searchPlaceholder,
            scanRow: model.scanRow,
            groupsTitle: model.groupsTitle,
            groups: model.groups,
            contactsTitle: model.contactsTitle,
            contacts: book.contacts.map { contact in
                ContactEntryModel(
                    name: contact.name ?? contact.resolvedName ?? AddressText.short(contact.address),
                    group: nil,
                    addressDisplay: AddressText.short(contact.address),
                    identiconSeed: contact.address
                )
            }
        )
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
