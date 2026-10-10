//
//  FlowsLive.swift
//  VelaWallet
//
//  The flow screens, wearing real state.
//
//  A sibling of `WalletFlowFixtures`, and — like `WalletLive` and `SettingsLive`
//  — a **partial** one. Spec 021 drew nine flows; two have machines behind them
//  in this cut:
//
//  - **T1 资产** — the same holdings the home lists, from `balance_dashboard`.
//  - **T3 添加代币** — `manage_tokens`: what was typed, what the chains
//    answered, and what is already saved.
//
//  Everything else this file does not touch stays the drawing, visibly.
//
//  ## The drawn add-token sheet and the machine do not have the same shape
//
//  The web panel searches every network and lists a card per chain; the drawing
//  has one network row, one result card and one CTA. So the card shown is the
//  **first** the core found (registry order), and the network row names the
//  chain it was found on rather than being a chooser. That is a recorded
//  deviation, not a redesign: the core's other found chains are still in its
//  view, and a drawing that lists them is what would let the shell show them.
//

import SwiftUI
import VelaCore

enum FlowsLive {

    // MARK: - T1, the assets list

    /// The person's own holdings, in the drawn list — narrowed to the chain
    /// the pill has chosen, as the history is.
    static func assets(
        _ balance: BalanceViewWire,
        currency: CurrencyViewWire?,
        selected: Int? = nil,
        on model: AssetsModel,
        loc: Loc,
        networks: WalletNetworks = .builtin
    ) -> AssetsModel {
        let all = WalletLive.assetRows(balance, display: WalletLive.Display.from(currency),
                                       networks: networks)
        let rows = visibleAssetIndices(balance, selected: selected).map { all[$0] }
        // Empty only once the core has actually looked — never while the
        // fetch is still out (FR-008) — or when the chosen chain holds nothing
        // while others do: a narrowed list with nothing in it is still an
        // answer, and a blank screen is not one.
        let settledEmpty = rows.isEmpty && !balance.balanceUnknown && !balance.holdingsLoading
        let filteredEmpty = rows.isEmpty && !balance.tokens.isEmpty
        return AssetsModel(
            header: FlowHeaderModel(
                title: model.header.title,
                backLabel: model.header.backLabel,
                action: model.header.action,
                pill: pill(selected: selected, loc: loc, fallback: model.header.pill,
                           networks: networks)
            ),
            searchPlaceholder: model.searchPlaceholder,
            rows: rows,
            addByAddress: model.addByAddress,
            // The drawn guided-empty body. T1 is drawn full, so its words come
            // from the same place T4's do rather than being absent.
            empty: settledEmpty || filteredEmpty
                ? (model.empty ?? WalletFlowFixtures.assetsEmpty(loc))
                : nil
        )
    }

    /// Which holdings the assets list shows, as indices into
    /// `balance.tokens` — the list a tapped row is looked up in, so a tap on
    /// a narrowed list names the token that was tapped.
    static func visibleAssetIndices(_ balance: BalanceViewWire, selected: Int?) -> [Int] {
        balance.tokens.indices.filter { selected == nil || balance.tokens[$0].chainId == selected }
    }

    /// The chain picker over the assets list: the chains this account HOLDS
    /// something on, with how many — the web's `liveChainRows`. A filter
    /// offering a chain with nothing on it is a dead end. The chains are the
    /// wallet's own, the person's added networks included.
    static func assetChainSheet(
        _ balance: BalanceViewWire,
        selected: Int?,
        loc: Loc,
        networks: WalletNetworks = .builtin
    ) -> ChainSheetModel {
        var counts: [Int: Int] = [:]
        for token in balance.tokens { counts[token.chainId, default: 0] += 1 }
        var rows = [ChainRowModel(
            name: loc.t("componentsUi.networkFilter.allNetworks"),
            dot: .all,
            count: balance.tokens.count,
            selected: selected == nil,
            chainId: nil
        )]
        for chain in networks.chains {
            guard let count = counts[chain.chainId] else { continue }
            rows.append(chainRow(chain, count: count, selected: selected))
        }
        return ChainSheetModel(
            title: loc.t("componentsUi.networkFilter.selectChain"), rows: rows
        )
    }

    /// One chain's row in either filter: its colour for the dot, its logo
    /// over it.
    private static func chainRow(_ chain: ChainMeta, count: Int, selected: Int?) -> ChainRowModel {
        let mark = SettingsLive.mark(chainId: chain.chainId, name: chain.displayName)
        return ChainRowModel(
            name: chain.displayName,
            dot: .color(mark.color),
            count: count,
            selected: selected == chain.chainId,
            chainId: chain.chainId,
            logoUrl: mark.logoUrl
        )
    }

    // MARK: - The network filter

    /// The chain picker over the history, and the pill that opens it.
    ///
    /// The rows are the chains this account actually has transfers on, with
    /// their counts — not the twelve built-ins, because a filter offering a
    /// chain with nothing on it is a dead end somebody has to back out of.
    /// **The filtering itself is the core's**: the shell sends
    /// `chain_filter_changed` and re-renders whatever comes back.
    static func chainSheet(
        _ feed: FeedViewWire,
        selected: Int?,
        loc: Loc,
        networks: WalletNetworks = .builtin
    ) -> ChainSheetModel {
        var counts: [Int: Int] = [:]
        for item in items(feed) { counts[item.chainId, default: 0] += 1 }

        var rows = [ChainRowModel(
            name: loc.t("componentsUi.networkFilter.allNetworks"),
            dot: .all,
            count: counts.values.reduce(0, +),
            selected: selected == nil,
            chainId: nil
        )]
        // Registry order, so the list does not reshuffle as counts change —
        // the core's, the person's own networks after the built-ins.
        for chain in networks.chains {
            guard let count = counts[chain.chainId] else { continue }
            rows.append(chainRow(chain, count: count, selected: selected))
        }
        return ChainSheetModel(
            title: loc.t("componentsUi.networkFilter.selectChain"), rows: rows
        )
    }

    /// The header pill: which filter is on.
    static func pill(
        selected: Int?, loc: Loc, fallback: FlowPillModel?,
        networks: WalletNetworks = .builtin
    ) -> FlowPillModel? {
        guard let selected, let chain = networks.meta(selected) else { return fallback }
        return FlowPillModel(
            dots: [SettingsLive.mark(chainId: chain.chainId, name: chain.displayName).color],
            label: chain.displayName
        )
    }

    // MARK: - A1 / A2, the history and one transaction

    /// The whole feed, in the drawn history screen.
    ///
    /// The same rows the home shows, from the same machine — the home draws
    /// the core's cut (`FeedView.home_rows`, the newest three; issue #469)
    /// and this one draws every row. Building it twice from two sources is
    /// how a person taps 全部 and sees a different history than the one they
    /// were looking at.
    static func history(
        _ feed: FeedViewWire,
        selected: Int? = nil,
        on model: HistoryModel,
        loc: Loc,
        hidden: Bool,
        networks: WalletNetworks = .builtin
    ) -> HistoryModel {
        let header = FlowHeaderModel(
            title: model.header.title,
            backLabel: model.header.backLabel,
            action: model.header.action,
            pill: pill(selected: selected, loc: loc, fallback: model.header.pill,
                       networks: networks)
        )
        let groups = WalletLive.activityGroups(feed.rows, loc: loc, hidden: hidden, networks: networks)
        return HistoryModel(
            header: header,
            mode: groups.isEmpty ? .empty : .rows,
            // The core's choice (spec 082 RG5): "no transactions yet" on
            // every network, "none on this network" under a filter — never
            // the gallery board's filter sentence on an unfiltered history.
            emptyText: loc.t(feed.historyEmptyKey),
            groups: groups
        )
    }

    /// The feed's items in render order — the list a tapped row indexes into.
    static func items(_ feed: FeedViewWire) -> [FeedItemWire] {
        feed.rows.compactMap { row in
            if case .item(let item) = row { return item }
            return nil
        }
    }

    /// One transaction, opened from a history row.
    ///
    /// Built from the **stored record** when there is one — `FeedItem` is a
    /// lossy projection and the sheet wants the hash, the counterparty and the
    /// stored USD string. A folded batch row has no single record; it falls
    /// back to the item, which is what the core hands the shell for exactly
    /// that case.
    ///
    /// `hidden`: the feed's own privacy flag (`FeedView.hidden`, PR 2) — the
    /// figure and its worth are the mask; who, where and when stay.
    ///
    /// `display`: only its `settled` is read. The worth under the figure is
    /// the STORED dollar string, never re-priced — but it is a fiat figure,
    /// and the withhold rule has no surface it skips (PR 3 notes 9/27): while
    /// the person's currency is not known it keeps its line and draws nothing.
    static func txDetail(
        _ item: FeedItemWire,
        record: FeedTxRecordWire?,
        on model: TxDetailModel,
        loc: Loc,
        hidden: Bool = false,
        display: WalletLive.Display = .usd,
        readRequest: @escaping (String) -> String? = { _ in nil },
        networks: WalletNetworks = .builtin
    ) -> TxDetailModel {
        if let dapp = item.dapp {
            return dappDetail(item, dapp: dapp, record: record, on: model, loc: loc, hidden: hidden,
                              display: display, readRequest: readRequest, networks: networks)
        }
        let incoming = item.direction == .in
        let dapp = item.kind == .dappTx
        let chain = networks.meta(item.chainId)
        let counterparty = item.counterparty ?? record?.from ?? ""

        var facts: [FactRowModel] = []
        // Spec 082 RG2: a dApp's transaction says who asked for it.
        if dapp, let site = item.site, !site.isEmpty {
            facts.append(FactRowModel(
                label: loc.t("componentsUi.signing.siweOrigin"),
                value: site
            ))
        }
        if !counterparty.isEmpty {
            facts.append(FactRowModel(
                // The core says who the counterparty is (spec 082 RJ16): the
                // contract a dApp's call went to is "Interacting with", never
                // "To" — G52 named a token contract as the recipient.
                label: item.counterpartyRole == .contract
                    ? loc.t("componentsUi.signing.interactingLabel")
                    : loc.t(incoming ? "componentsTx.detail.from" : "componentsTx.detail.to"),
                value: item.alias ?? AddressText.short(counterparty),
                lead: .identicon(counterparty),
                mono: item.alias == nil,
                copy: loc.t("componentsUi.identiconViewer.copyAddress"),
                // The ADDRESS, even when the row shows a name: a name is not
                // something anybody can paste into a send.
                copyValue: counterparty
            ))
        }
        if let chain {
            facts.append(networkFact(chain, loc: loc))
        }
        // The drawn sheet has a 代币合约 row and this build cannot fill it: the
        // stored record carries a symbol and decimals, not the contract it came
        // from — every client's `LocalTransaction` is shaped that way. The row
        // is omitted rather than filled with a plausible address, because
        // "which contract?" is exactly the question it exists to answer.
        facts.append(FactRowModel(
            label: loc.t("componentsTx.detail.labelDate"),
            value: timestamp(item.timestamp, loc: loc)
        ))
        // The core's own tx hash (spec 082 RJ16): `nil` for an op the chain
        // has not shown, and for a record whose stored "hash" is the op's —
        // never read back from the record, which is how an op hash became an
        // explorer link.
        let hash = item.txHash ?? ""
        if !hash.isEmpty {
            facts.append(FactRowModel(
                label: loc.t("componentsTx.detail.labelHash"),
                value: AddressText.short(hash),
                mono: true,
                copy: loc.t("componentsUi.identiconViewer.copyAddress"),
                copyValue: hash
            ))
        }

        let parts = batchParts(item, loc: loc, hidden: hidden, networks: networks)
        return TxDetailModel(
            title: dapp
                ? loc.t("history.txLabelDappTx")
                : loc.t(incoming ? "history.txLabelReceived" : "history.txLabelSent",
                        vars: ["symbol": item.symbol]),
            // The row's own lifecycle, from the core (spec 082 RG1) — never a
            // record lookup that defaulted a missing one to "succeeded".
            status: status(item.status, loc: loc),
            closeLabel: model.closeLabel,
            // Hidden, the figure is the core's masked amount — the mask and
            // the coin, "•••• USDC" (`maskedAmount(unit:)`, PR 3 notes 3/11).
            amount: dapp && item.value == nil
                ? ""
                : (hidden && item.figureMaskable
                    ? maskedAmount(unit: item.symbol)
                    : (incoming ? "+" : "\u{2212}") + WalletLive.compactAmount(item.value, batch: item.batch)
                        + (item.symbol.isEmpty ? "" : " \(item.symbol)")),
            // The STORED figure, not a recomputed one: it is what this wallet
            // recorded the transfer was worth when it happened, and re-pricing
            // it today would quietly restate history. None at all when the
            // core knows no price (spec 097 N7: unknown is not "$0.00").
            fiat: Self.storedFiat(item.priced ? record?.usd : nil, hidden: hidden, display: display),
            positive: incoming,
            facts: facts,
            viewOnExplorer: hash.isEmpty ? nil : model.viewOnExplorer,
            // The LOCAL record. The chain keeps the transaction; this is the
            // wallet forgetting it, which is why the sentence is "delete
            // record" and not "delete transaction". Placed on the detail, as
            // the web places it — a swipe on a feed row is a gesture nobody
            // drew and a destructive one to discover by accident.
            deleteLabel: loc.t("history.deleteRecord"),
            // RJ18: quiet while it may still land — and a record nothing
            // settles (087 F04) may have been sent too.
            deleteQuiet: item.status == .pending || item.status == .unknown,
            breakdownTitle: parts.title,
            breakdown: parts.rows
        )
    }

    /// What a folded batch row folded (spec 038 #D2): a split's recipients
    /// by name and avatar with each one's share, a sweep's coins by their
    /// marks — nothing for a single transfer. The detail used to state a
    /// split's total and nobody it went to.
    ///
    /// Every part is a figure on a masked surface (the core's transfer
    /// detail): `hidden`, each reads the core's masked amount with its coin
    /// — "•••• USDC" — never the share (PR 3 note 15: the web drew who got
    /// how much of a split under a masked hero).
    static func batchParts(
        _ item: FeedItemWire, loc: Loc, hidden: Bool, networks: WalletNetworks = .builtin
    ) -> (title: String?, rows: [BreakdownRowModel]) {
        guard let batch = item.batch, !batch.transfers.isEmpty else { return (nil, []) }
        let masked = hidden && item.figureMaskable
        func value(_ transfer: FeedBatchTransferWire) -> String {
            masked
                ? maskedAmount(unit: transfer.symbol)
                : "\(WalletLive.compactAmount(transfer.value)) \(transfer.symbol)"
                    .trimmingCharacters(in: .whitespaces)
        }
        switch batch.kind {
        case .split:
            let rows = batch.transfers.map { transfer in
                BreakdownRowModel(
                    identiconSeed: transfer.to,
                    label: transfer.toName ?? AddressText.short(transfer.to),
                    value: value(transfer),
                    mono: transfer.toName == nil
                )
            }
            return (loc.t("send.recipientCount", count: rows.count), rows)
        case .multiSelect:
            let color = SettingsLive.chainColor(item.chainId)
            let native = networks.meta(item.chainId)?.nativeSymbol
            let rows = batch.transfers.map { transfer in
                BreakdownRowModel(
                    // A stored line has no contract address: the network's
                    // own coin is told by its ticker, any other coin is a
                    // contract the mark rule cannot place ("" — no logo
                    // guessed for it), as the desktop's detail draws it.
                    lead: TokenMarkModel.of(
                        chainId: item.chainId, symbol: transfer.symbol,
                        tokenAddress: transfer.symbol.caseInsensitiveCompare(native ?? "") == .orderedSame ? nil : "",
                        color: color, named: transfer.logoUrls ?? []
                    ),
                    label: transfer.symbol,
                    value: value(transfer)
                )
            }
            return (loc.t("componentsTx.receipt.assetsCount", vars: ["n": String(rows.count)]), rows)
        }
    }

    /// A dApp interaction, opened from its row (spec 093) — every line the
    /// core's, in its order; this only labels and formats them.
    ///
    /// The header is the row's: its title and its figure, or the allowance
    /// it granted. A transaction keeps its status chip and explorer link; a
    /// signature has no chip — nothing settles it — and says it was off-chain
    /// instead. "Technical details" is collapsed, and the request the record
    /// kept is read from the store (`readRequest`, by record id) only when it
    /// is opened.
    ///
    /// `hidden` (PR 2): every figure in the detail masks — the header's, what
    /// came back, the balance changes, a capped allowance — and the unlimited
    /// allowance stays said: it is a risk to see, not an amount.
    static func dappDetail(
        _ item: FeedItemWire,
        dapp: FeedDappWire,
        record: FeedTxRecordWire?,
        on model: TxDetailModel,
        loc: Loc,
        hidden: Bool = false,
        display: WalletLive.Display = .usd,
        readRequest: @escaping (String) -> String?,
        networks: WalletNetworks = .builtin
    ) -> TxDetailModel {
        // A figure and its unit, as one text — hidden, the core's masked
        // amount (`maskedAmount(unit:)`, PR 3 notes 3/11): the mask keeps
        // its coin, "•••• USDC".
        func figure(_ amount: String, _ unit: String, masked: Bool) -> String {
            masked ? maskedAmount(unit: unit) : [amount, unit].filter { !$0.isEmpty }.joined(separator: " ")
        }
        let money = WalletLive.dappFigure(item, dapp: dapp)
        let allowance = money == nil
            ? dapp.allowance.flatMap { WalletLive.allowanceText($0, loc: loc) }
            : nil
        let backText = dapp.received.map { WalletLive.changeText($0, hidden: hidden) }
        // Spec 097 N5: nothing left and something came back (a borrow) —
        // what came back is the figure.
        let leadsWithBack = money == nil && allowance == nil && backText != nil
        let amount = money.map { figure($0, item.symbol, masked: hidden) }
            // 无限额 is a risk to see, not an amount: never masked.
            ?? allowance.map { figure($0.amount, $0.unit, masked: hidden && !$0.unlimited) }
            ?? (leadsWithBack ? backText ?? "" : "")
        let hash = item.txHash ?? ""
        let technical = dapp.technical.compactMap {
            technicalLine($0, id: item.id, loc: loc, readRequest: readRequest)
        }
        return TxDetailModel(
            title: WalletLive.dappTitle(dapp, loc: loc),
            status: dapp.offChain ? nil : status(item.status, loc: loc),
            closeLabel: model.closeLabel,
            amount: amount,
            // The STORED figure, as for any transfer — never re-priced; none
            // when the core knows no price (spec 097 N7).
            fiat: Self.storedFiat(money == nil || !item.priced ? nil : record?.usd, hidden: hidden,
                                  display: display),
            positive: leadsWithBack && dapp.received?.direction == .in,
            facts: dapp.facts.compactMap {
                fact($0, item: item, dapp: dapp, loc: loc, hidden: hidden, networks: networks)
            },
            viewOnExplorer: hash.isEmpty ? nil : model.viewOnExplorer,
            deleteLabel: loc.t("history.deleteRecord"),
            deleteQuiet: !dapp.offChain && (item.status == .pending || item.status == .unknown),
            // A signature says it was off-chain; a failed operation says why
            // (spec 097 N4), under its chip.
            note: dapp.offChain
                ? loc.t("connect.detail.offChainNote")
                : dapp.failure.flatMap { failureText($0, loc: loc) },
            amountDanger: money == nil && allowance?.unlimited == true,
            received: leadsWithBack ? nil : backText,
            technical: technical.isEmpty ? nil : TxTechnicalModel(
                title: loc.t("componentsUi.signing.advancedToggle"), lines: technical
            )
        )
    }

    /// Why a dApp operation failed, in the words its request ended with
    /// (spec 097 N4). `nil` for a reason this build has never heard of.
    static func failureText(_ failure: String, loc: Loc) -> String? {
        switch failure {
        case "reverted": return loc.t("componentsTx.receipt.failedHint")
        case "refused": return loc.t("componentsUi.signing.refused")
        case "not_sent": return loc.t("send.txErrorGeneric")
        default: return nil
        }
    }

    /// One of the core's detail facts, labelled (spec 093). `nil` for a
    /// fact this build has never heard of.
    private static func fact(
        _ fact: FeedFactWire, item: FeedItemWire, dapp: FeedDappWire, loc: Loc,
        hidden: Bool = false, networks: WalletNetworks
    ) -> FactRowModel? {
        switch fact {
        case .site(let site):
            return FactRowModel(label: loc.t("connect.detail.labelApp"), value: site)
        case .network(let chainId):
            return chainFact(chainId, loc: loc, networks: networks)
        case .contract(let address, let name):
            // A noun — the record is of something done (083 F3 review).
            return partyFact(loc.t("tokenDetail.labelContract"), address, name, loc: loc)
        case .recipient(let address, let name):
            return partyFact(loc.t("componentsTx.detail.to"), address, name, loc: loc)
        case .spender(let address, let name):
            return partyFact(loc.t("componentsUi.signing.labelSpender"), address, name, loc: loc)
        case .spendingCap(let allowance):
            guard let cap = WalletLive.allowanceText(allowance, loc: loc) else { return nil }
            return FactRowModel(
                label: loc.t("componentsUi.signingApprove.spendingCap"),
                // Hidden, a capped allowance is the core's masked amount with
                // its coin; the unlimited one stays said.
                value: hidden && !cap.unlimited
                    ? maskedAmount(unit: cap.unit)
                    : [cap.amount, cap.unit].filter { !$0.isEmpty }.joined(separator: " "),
                danger: cap.unlimited
            )
        case .expires(let at):
            return FactRowModel(
                label: loc.t("componentsUi.signingApprove.expiresLabel"),
                value: at.map { Formats.dateTime(Date(timeIntervalSince1970: $0)) }
                    ?? loc.t("componentsUi.signingApprove.noExpiry")
            )
        case .balanceChanges:
            let lines = dapp.changes.map { change in
                BalanceDeltaRow(
                    symbol: change.verified ? change.symbol : loc.t("componentsUi.signing.balanceUnverifiedToken"),
                    delta: WalletLive.changeFigure(change, hidden: hidden),
                    tone: !change.verified ? .caution : (change.direction == .in ? .success : .neutral)
                )
            }
            guard !lines.isEmpty else { return nil }
            return FactRowModel(label: loc.t("componentsUi.signing.balanceChangesTitle"), value: "", lines: lines)
        case .date(let seconds):
            return FactRowModel(label: loc.t("componentsTx.detail.labelDate"), value: timestamp(seconds, loc: loc))
        default:
            // Technical lines live under their own disclosure.
            return nil
        }
    }

    /// A record's stored worth as the detail's line: "≈ $163.25" — the mask
    /// while hidden, nothing where the record kept none, and (PR 3 notes
    /// 9/27) the line's own height with nothing on it while the display
    /// currency is not known yet: no fiat figure is drawn before it commits,
    /// on this surface either.
    static func storedFiat(_ usd: String?, hidden: Bool, display: WalletLive.Display) -> String {
        guard let usd, !usd.isEmpty else { return "" }
        if hidden { return WalletFixtures.mask }
        return display.settled ? "≈ \(usd)" : WalletLive.Display.withheldLine
    }

    /// One of the core's technical lines, labelled (spec 093).
    private static func technicalLine(
        _ fact: FeedFactWire, id: String, loc: Loc, readRequest: @escaping (String) -> String?
    ) -> TxTechnicalLine? {
        switch fact {
        case .operation(let operation):
            let value: String
            switch operation {
            case .contractInteraction: value = loc.t("componentsTx.detail.opContractInteraction")
            case .batch(let calls): value = loc.t("componentsUi.signing.batchSubtitle", vars: ["count": String(calls)])
            case .signature: value = loc.t("componentsTx.detail.opSignature")
            case .typedDataSignature: value = loc.t("componentsTx.detail.opTypedDataSignature")
            case .unknown: return nil
            }
            return .fact(FactRowModel(label: loc.t("componentsTx.detail.labelOperation"), value: value))
        case .content(let content):
            let label: String
            switch content {
            case .callData: label = loc.t("connect.detail.contentCallData")
            case .typedData: label = loc.t("connect.detail.contentTypedData")
            case .message: label = loc.t("connect.detail.contentMessage")
            case .unknown: return nil
            }
            // Read when the section opens, and shown as the core words it
            // (`dappRequestDisplay`): typed data pretty-printed, a message as
            // its text or hex, call data pretty-printed. Nothing kept → nil.
            return .content(TxContentModel(
                label: label, missing: loc.t("connect.detail.contentMissing"),
                read: {
                    readRequest(id).flatMap {
                        dappRequestDisplay(content: content.rawValue, storedRequest: $0)
                    }
                }
            ))
        case .primaryType(let name):
            return .fact(FactRowModel(label: loc.t("componentsUi.signing.typeLabel"), value: name, mono: true))
        case .hash(let txHash):
            return .fact(hashFact(loc.t("componentsTx.detail.labelHash"), txHash, loc: loc))
        case .userOpHash(let hash):
            return .fact(hashFact(loc.t("componentsTx.receipt.userOpHash"), hash, loc: loc))
        default:
            return nil
        }
    }

    /// The network, with its mark — as the transfer detail draws it.
    private static func chainFact(_ chainId: Int, loc: Loc, networks: WalletNetworks) -> FactRowModel {
        guard let chain = networks.meta(chainId) else {
            return FactRowModel(label: loc.t("componentsTx.detail.labelChain"), value: String(chainId))
        }
        return networkFact(chain, loc: loc)
    }

    /// A transaction's network: its name, and the NETWORK's own mark (the
    /// kind rule) — never its coin's. ETH sent on Base is "Base" beside
    /// Base's logo.
    private static func networkFact(_ chain: ChainMeta, loc: Loc) -> FactRowModel {
        FactRowModel(
            label: loc.t("componentsTx.detail.labelChain"),
            value: chain.displayName,
            lead: .token(TokenMarkModel.chain(
                chainId: chain.chainId,
                symbol: chain.nativeSymbol,
                color: SettingsLive.mark(chainId: chain.chainId, name: chain.displayName).color
            ))
        )
    }

    /// A contract, a recipient or a spender: its name, else its short
    /// address — copying the ADDRESS either way.
    private static func partyFact(_ label: String, _ address: String, _ name: String?, loc: Loc) -> FactRowModel {
        FactRowModel(
            label: label,
            value: name ?? AddressText.short(address),
            lead: .identicon(address),
            mono: name == nil,
            copy: loc.t("componentsUi.identiconViewer.copyAddress"),
            copyValue: address
        )
    }

    /// A hash, short and mono, copying the whole of it.
    private static func hashFact(_ label: String, _ hash: String, loc: Loc) -> FactRowModel {
        FactRowModel(
            label: label,
            value: AddressText.short(hash),
            mono: true,
            copy: loc.t("componentsUi.identiconViewer.copyAddress"),
            copyValue: hash
        )
    }

    private static func status(_ status: FeedTxStatusWire, loc: Loc) -> StatusChipModel {
        switch status {
        case .confirmed:
            StatusChipModel(text: loc.t("componentsTx.detail.statusSucceeded"), tone: .success)
        case .pending:
            StatusChipModel(text: loc.t("componentsTx.detail.statusPending"), tone: .warning)
        case .failed:
            StatusChipModel(text: loc.t("componentsTx.detail.statusFailed"), tone: .error)
        case .unknown:
            // 087 F04: pending, and nothing will settle it — not failed.
            StatusChipModel(text: loc.t("componentsUi.signing.intentUnknown"), tone: .info)
        }
    }

    /// Today's transfers show a time; older ones show their date. The core
    /// emits epoch seconds and leaves both to the shell, which is where the
    /// device's clock and locale live.
    private static func timestamp(_ seconds: Double, loc: Loc) -> String {
        // The PERSON's presets (spec 056), not the device's locale: a wallet
        // where the same transaction reads two ways on two of their machines is
        // the thing the presets exist to prevent. "今天" survives — a relative
        // day is copy, and the corpus owns it.
        let date = Date(timeIntervalSince1970: seconds)
        if Calendar.current.isDateInToday(date) {
            return "\(loc.t("componentsUi.dayGroup.today")) \(Formats.time(date))"
        }
        return Formats.dateTime(date)
    }

    // MARK: - T2, one token

    /// One holding, opened from an assets row — with the transfers of that
    /// token, from the same feed the history shows.
    ///
    /// `hidden` (PR 2): the balance, its worth and the transfers' figures are
    /// the mask; the token's own facts (its price, contract, network) stay.
    static func tokenDetail(
        _ token: BalanceTokenWire,
        feed: FeedViewWire?,
        display: WalletLive.Display,
        on model: TokenDetailModel,
        loc: Loc,
        hidden: Bool = false,
        networks: WalletNetworks = .builtin
    ) -> TokenDetailModel {
        let chain = networks.meta(token.chainId)?.displayName ?? ""
        var facts: [FactRowModel] = []
        if let price = token.priceUsd {
            facts.append(FactRowModel(
                label: loc.t("tokenDetail.labelPrice"),
                // The price is a fiat figure (PR 3 notes 9/27 — the token
                // page was one of the surfaces the rule missed): while the
                // person's currency is not known the row keeps its place and
                // its label, and the value lands beside it.
                value: display.fiat(price).map { figure in
                    loc.t("tokenDetail.priceValue", vars: ["symbol": token.symbol, "value": figure])
                } ?? ""
            ))
        }
        if let contract = token.tokenAddress, !contract.isEmpty {
            facts.append(FactRowModel(
                label: loc.t("tokenDetail.labelContract"),
                value: AddressText.short(contract),
                mono: true,
                copy: loc.t("componentsUi.identiconViewer.copyAddress"),
                copyValue: contract
            ))
        }
        facts.append(FactRowModel(label: loc.t("tokenDetail.labelDecimals"),
                                  value: String(token.decimals)))
        facts.append(FactRowModel(label: loc.t("addToken.labelNetwork"), value: chain))

        // This token's own transfers. Matched on symbol AND chain, because the
        // same ticker on two networks is two different assets.
        let rows = (feed.map(items) ?? [])
            .filter { $0.chainId == token.chainId && $0.symbol == token.symbol }
            .map { WalletLive.activityRow($0, loc: loc, hidden: hidden, networks: networks) }

        return TokenDetailModel(
            mark: TokenMarkModel.of(
                chainId: token.chainId,
                symbol: token.symbol,
                tokenAddress: token.tokenAddress,
                color: SettingsLive.mark(chainId: token.chainId, name: chain).color
            ),
            symbol: token.symbol,
            chain: chain,
            closeLabel: model.closeLabel,
            // Hidden: the core's masked amount, which keeps the coin.
            balance: hidden
                ? maskedAmount(unit: token.symbol)
                : "\(WalletLive.compactAmount(token.balance)) \(token.symbol)",
            // The holding's worth: masked while hidden, and — withheld —
            // the line's own height with nothing on it, so Receive and Send
            // under it do not move when the figure lands. An unpriced coin
            // has no worth line at all, as before.
            fiat: token.priceUsd.map { price in
                hidden
                    ? WalletFixtures.mask
                    : display.fiatLine((Double(token.balance) ?? 0) * price)
            } ?? "",
            receive: model.receive,
            send: model.send,
            facts: facts,
            transactionsTitle: model.transactionsTitle,
            rows: rows,
            viewOnExplorer: model.viewOnExplorer
        )
    }

    // MARK: - R1 / R2, the receive screens

    /// The network list, with the person's OWN address on every row.
    ///
    /// This is the screen where a fixture is not merely embarrassing: money
    /// sent to the drawn address is money gone. Until this landed, 收款 showed
    /// `WalletFixtures.identity` — somebody else's address entirely — beside a
    /// code drawn from a demo pattern.
    ///
    /// Every network the wallet has — the core's list, the person's own
    /// networks included (a row's index is its place in `networks.chains`,
    /// which is the list the tap is looked up in).
    static func receiveList(
        _ address: String,
        on model: ReceiveListModel,
        loc: Loc,
        networks wallet: WalletNetworks = .builtin
    ) -> ReceiveListModel {
        guard !address.isEmpty else { return model }
        let networks = wallet.chains
        var live = model
        live.address = address
        live.subtitle = loc.t("receive.networksLine", vars: ["count": String(networks.count)])
        live.rows = networks.map { chain in
            NetworkRowModel(
                name: chain.displayName,
                code: chain.nativeSymbol,
                badgeColor: SettingsLive.mark(chainId: chain.chainId, name: chain.displayName).color,
                // One address, every network — which is what the subtitle above
                // promises and what a 4337 Safe at a deterministic address
                // actually delivers.
                addressDisplay: AddressText.short(address),
                copyLabel: model.rows.first?.copyLabel ?? "",
                qrLabel: model.rows.first?.qrLabel ?? "",
                // The network's own logo (058); the coloured disc is the
                // fallback, so a phone with no network still names each chain.
                logoURLs: [Marks.chainLogoURL(chain.chainId)].compactMap { $0 }
            )
        }
        return live
    }

    /// The code, and the account card above it.
    ///
    /// The QR encodes the **bare address**. That is the whole answer in address
    /// mode; the amount-carrying EIP-681 request is `payment_request`'s, and
    /// its mode toggle and amount field are drawn nowhere on EITHER phone —
    /// Android's `receiveMode` and `receiveAmount` have no callers either —
    /// so that half stays recorded rather than invented here.
    ///
    /// What 058 does add is the **asset**: `payment_request` knows which coin
    /// the code was asked for, and R3 has always been drawn for it.
    /// What a receive code says (spec 090): the core's `qrValue` — the bare
    /// address, or with "include network" on `ethereum:<address>@<chain>` —
    /// once the core's answer is about THIS code's network and token. Until
    /// then (no session yet, a pick in flight) the bare address, which every
    /// wallet reads. `current` says which, so the switch is drawn only beside
    /// a code it describes.
    static func receiveCode(
        _ address: String,
        chainId: Int?,
        tokenAddress: String?,
        pay: PaymentRequestViewWire?
    ) -> (value: String, current: Bool) {
        guard let pay, !pay.qrValue.isEmpty, let chainId,
              pay.asset.chainId == chainId, pay.asset.tokenAddress == tokenAddress
        else { return (address, false) }
        return (pay.qrValue, true)
    }

    static func receiveQr(
        _ address: String,
        name: String,
        chain: ChainMeta?,
        asset: PaymentRequestAssetWire? = nil,
        pay: PaymentRequestViewWire? = nil,
        on model: ReceiveQrModel,
        loc: Loc
    ) -> ReceiveQrModel {
        guard !address.isEmpty else { return model }
        var live = model
        // The chain the person actually tapped. Without it the sheet keeps the
        // fixture's first network and tells somebody who picked Gnosis to
        // receive Ethereum assets — the address is the same on both, but the
        // sentence would be a lie and the mark would back it up.
        if let chain {
            live.title = loc.t("receive.qrTitleNetwork", vars: ["network": chain.displayName])
            live.centre = TokenMarkModel.chain(
                chainId: chain.chainId,
                symbol: chain.nativeSymbol,
                color: SettingsLive.mark(chainId: chain.chainId, name: chain.displayName).color
            )
        }
        // A token's own code: the sentence names the coin, the mark in the
        // middle of the code is that coin, and the contract it means is
        // printed above — which is the question "which USDC?" that a symbol
        // alone cannot answer.
        if let asset, let contract = asset.tokenAddress, !contract.isEmpty {
            let network = ChainCatalog.meta(asset.chainId)?.displayName ?? asset.networkName
            live.title = loc.t("receive.qrTitleAsset", vars: [
                "symbol": asset.symbol, "network": network,
            ])
            live.centre = TokenMarkModel.of(
                chainId: asset.chainId,
                symbol: asset.symbol,
                tokenAddress: contract,
                color: SettingsLive.mark(chainId: asset.chainId, name: network).color
            )
            live.contract = ContractLineModel(
                label: loc.t("receive.tokenContract"),
                value: AddressText.short(contract),
                copyLabel: model.contract?.copyLabel
                    ?? loc.t("componentsUi.identiconViewer.copyAddress"),
                copyValue: contract
            )
        } else {
            // A network code has no contract. The fixture's row is R3's and
            // leaving it up would print a stranger's contract over a code for
            // the chain's own coin.
            live.contract = nil
        }
        live.account = AddressCardModel(
            name: name.isEmpty ? model.account.name : name,
            identiconSeed: address,
            lines: AddressText.lines(address),
            copyLabel: model.account.copyLabel
        )
        // `nil` when the address cannot be encoded — the card then draws the
        // demo pattern, which is why `QrCode` never falls back to it silently:
        // the caller decides, and here an unencodable address is a bug worth
        // seeing rather than a picture worth showing.
        let code = receiveCode(
            address,
            chainId: asset?.chainId ?? chain?.chainId,
            tokenAddress: asset?.tokenAddress,
            pay: pay
        )
        live.modules = QrCode.modules(code.value)
        // The switch and its hint are the core's (spec 090), drawn only beside
        // a code the core's answer is about.
        if code.current, let pay, pay.networkSwitch {
            live.network = NetworkSwitchModel(
                label: loc.t("receive.includeNetwork"),
                isOn: pay.includeNetwork,
                hint: pay.networkHint ? loc.t("receive.includeNetworkHint") : nil
            )
        } else {
            live.network = nil
        }
        return live
    }

    /// The card 保存图片 produces: the person's own address, a code that
    /// encodes it with the network's logo in its middle, and beside the
    /// address the identicon derived from it.
    ///
    /// This one leaves the app. A card built from the fixture identity is
    /// somebody else's address in a stranger's chat, which is the receive
    /// screen's danger with a longer half-life — the image outlives the
    /// session that made it.
    static func shareCard(
        _ address: String,
        name: String,
        chain: ChainMeta?,
        pay: PaymentRequestViewWire? = nil,
        on model: ShareCardModel,
        loc: Loc
    ) -> ShareCardModel {
        guard !address.isEmpty else { return model }
        var live = model
        live.name = name.isEmpty ? model.name : name
        live.lines = AddressText.lines(address)
        live.identiconSeed = address
        // Level H, not the screen's M: the network's logo sits on the code.
        // And exactly what the screen's code says (spec 090) — the core's
        // value for the asset the card is about.
        live.modules = QrCode.shareModules(receiveCode(
            address,
            chainId: chain?.chainId,
            tokenAddress: pay?.asset.tokenAddress,
            pay: pay
        ).value)
        if let chain {
            live.networkNote = loc.t("receive.shareCardNetworkNote",
                                     vars: ["network": chain.displayName])
            // The NETWORK's own logo (not its coin's — rule 2 is about coins),
            // the one the receive screen puts in the middle of its code.
            live.networkMark = .chain(
                chainId: chain.chainId,
                symbol: chain.nativeSymbol,
                color: SettingsLive.mark(chainId: chain.chainId,
                                         name: chain.displayName).color
            )
        }
        return live
    }

    // MARK: - T3, the add-token sheet

    /// The sheet, driven by `manage_tokens`.
    static func addToken(
        _ view: MtokViewWire,
        on model: AddTokenModel,
        loc: Loc,
        networks: WalletNetworks = .builtin
    ) -> AddTokenModel {
        let found = view.found.first
        var live = model
        live.network = found.map { card in
            AddTokenNetworkModel(
                // The NETWORK the token was found on, in the network's own
                // mark (the kind rule) — it drew the TOKEN's ticker, as letters.
                mark: TokenMarkModel.chain(
                    chainId: card.chainId,
                    symbol: networks.meta(card.chainId)?.nativeSymbol ?? card.networkName,
                    color: SettingsLive.mark(chainId: card.chainId, name: card.networkName).color
                ),
                name: card.networkName,
                pickLabel: model.network?.pickLabel ?? ""
            )
        }
        // NOT `?? model.network`. The lookup runs on every network at once, so
        // before a token is found there is no network to name — and falling
        // back to the drawn model put the FIXTURE's chain under a real
        // person's search, with a chevron beside it. Android says the same
        // thing by setting it to null outright (`FlowLive.kt:303`).
        live.fieldValue = view.inputAddress
        // An error only once there is something to be wrong about: an empty
        // field is not an invalid address, it is an empty field.
        live.fieldError = view.inputAddress.isEmpty || view.addressValid
            ? nil : loc.t("addToken.invalidAddress")
        live.result = result(view, loc: loc)
        live.ctaDisabled = found == nil || found?.added == true || view.saving
        return live
    }

    /// What the result slot says.
    ///
    /// **A found card outranks the spinner**, and the order of these two lines
    /// is the whole of it. The core probes every chain in parallel and
    /// publishes a card the moment one answers, but it keeps `detecting` true
    /// until the LAST chain settles — so a `detecting`-first reading shows
    /// 正在搜索所有网络… over an answer the wallet already has.
    ///
    /// On a Mac that costs a few seconds and nobody notices. On the founder's
    /// iPhone the straggler pushed the whole sweep past **sixty seconds**
    /// (measured 2026-09-14: 16.9 s on the simulator, >60 s on the device, and
    /// the accessibility tree showed the field filled, the network row reading
    /// Ethereum and the status line still spinning). A person types a contract
    /// address, the wallet finds it in two seconds, and the screen tells them
    /// it is still looking for a minute. That reads as broken.
    ///
    /// `detecting` is still what draws the spinner when there is nothing found
    /// yet, and `not_found` is still only true once every chain has answered —
    /// so this cannot say "not found" early, which would be the dangerous
    /// direction.
    private static func result(_ view: MtokViewWire, loc: Loc) -> AddTokenResult {
        if view.found.isEmpty, view.detecting {
            return .searching(loc.t("addToken.searchingNetworks"))
        }
        // Checked before `notFound`: the probe FINDS this contract — it is a
        // real, well-formed ERC-20 view of the network's own coin — so "not
        // found" would be the wrong words for a refusal that has a reason.
        if view.nativeAlias {
            return .notFound(
                "\(loc.t("addToken.nativeAliasTitle")) — \(loc.t("addToken.nativeAliasMessage"))")
        }
        if view.notFound {
            return .notFound("\(loc.t("addToken.notFoundTitle")) — \(loc.t("addToken.notFoundMessage"))")
        }
        guard let card = view.found.first else { return .none }
        return .token(
            // The token's own logo, by the contract the person typed — the
            // web's and Android's card. A card exists only for a contract
            // the probe found, so the address is never the native coin's
            // nil; one the rule cannot place gets letters and its badge.
            mark: TokenMarkModel.of(
                chainId: card.chainId,
                symbol: card.symbol,
                tokenAddress: view.addressValid
                    ? view.inputAddress.trimmingCharacters(in: .whitespaces) : "",
                color: SettingsLive.mark(chainId: card.chainId, name: card.networkName).color
            ),
            name: card.name,
            detail: "\(card.symbol) · \(loc.t("tokenDetail.labelDecimals")) \(card.decimals)"
                + " · \(card.networkName)",
            // The core recomputes "added" against the CURRENT input, so this
            // chip cannot linger from an address somebody has since edited.
            chip: card.added
                ? StatusChipModel(text: loc.t("addToken.tokenAdded"), tone: .success)
                : nil
        )
    }

    /// The save error the core raises when the write itself failed.
    ///
    /// Drawn nowhere in T3 — the mock has no alert — so it rides in the result
    /// card's place rather than being swallowed: a person who taps 添加到钱包
    /// and sees nothing change has been told nothing.
    static func saveErrorText(_ view: MtokViewWire, loc: Loc) -> String? {
        view.saveError
            ? "\(loc.t("addToken.errorTitle")) — \(loc.t("addToken.errorSaveToken"))"
            : nil
    }
}
