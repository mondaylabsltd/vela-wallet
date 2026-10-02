package app.getvela.wallet.feature.flows

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.format.Formats
import kotlinx.serialization.json.jsonPrimitive
import app.getvela.wallet.core.marks.Marks
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.send.core.MtokView
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.feature.settings.core.CurrencyView
import app.getvela.wallet.feature.settings.core.NetView
import app.getvela.wallet.feature.wallet.AssetFiatModel
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.wallet.core.BalanceToken
import app.getvela.wallet.feature.wallet.core.BalanceView
import app.getvela.wallet.feature.wallet.core.FeedCounterpartyRole
import app.getvela.wallet.feature.wallet.core.FeedDapp
import app.getvela.wallet.feature.wallet.core.FeedDappChange
import app.getvela.wallet.feature.wallet.core.FeedDappContent
import app.getvela.wallet.feature.wallet.core.FeedDappOperation
import app.getvela.wallet.feature.wallet.core.FeedDirection
import app.getvela.wallet.feature.wallet.core.FeedFact
import app.getvela.wallet.feature.wallet.core.FeedItem
import app.getvela.wallet.feature.wallet.core.FeedRow
import app.getvela.wallet.feature.wallet.core.FeedView
import app.getvela.wallet.feature.wallet.core.FeedTxKind
import app.getvela.wallet.feature.wallet.core.FeedTxStatus
import app.getvela.wallet.feature.wallet.core.PaymentRequestView

/**
 * The live flow builders: the drawn flow screens, showing this device's facts.
 *
 * The sibling of [FlowFixtures], the way `WalletLive` is of `WalletFixtures`.
 *
 * **The receive screen is the one place in this app where a stale value is
 * unrecoverable.** Every other fixture that leaks shows somebody the wrong
 * information; a fixture ADDRESS on a receive QR sends their money to a
 * stranger, permanently. So the address is replaced unconditionally here — not
 * "if we have one", not "unless it is blank" — and an empty session renders no
 * address at all rather than the one that was drawn.
 */
object FlowLive {

    /**
     * The QR sheet, for this person.
     *
     * [qrValue] is the core's: in address mode it is the bare recipient, and in
     * request mode the built `ethereum:` URI. The shell does not assemble it,
     * because what a payment request SAYS is the thing a scanner acts on.
     */
    fun receiveQr(
        fallback: ReceiveQrModel,
        address: String,
        name: String,
        request: PaymentRequestView,
        explorers: Map<Int, String> = emptyMap(),
        strings: VelaStrings? = null,
    ): ReceiveQrModel = fallback.copy(
        explorerUrl = explorers[request.asset.chain_id]?.let { "${it.trimEnd('/')}/address/$address" },
        // Spec 048 (device-found): the sheet said "Ethereum" and drew ETH for every
        // network row and every token — the fixture's asset. It is the request
        // machine's asset now: the title, the mark in the code, the contract line.
        title = strings?.let { st ->
            val asset = request.asset
            if (asset.token_address != null) {
                st.t(I18nKeys.Flows.RECEIVE_QR_ASSET, mapOf("network" to asset.network_name, "symbol" to asset.symbol))
            } else {
                st.t(I18nKeys.Flows.RECEIVE_QR_NETWORK, mapOf("network" to asset.network_name))
            }
        } ?: fallback.title,
        // Issue #263: a code for the NETWORK ("receive assets on Ethereum") carries
        // the network's mark, as the web's does; the native coin's mark said "only
        // ETH". A token's own code keeps that token's mark.
        centre = if (request.asset.token_address == null) {
            WalletLive.chainMark(request.asset.chain_id, request.asset.symbol)
        } else {
            WalletLive.mark(request.asset.chain_id, request.asset.symbol, request.asset.token_address)
        },
        contract = request.asset.token_address?.let { contract ->
            ContractLineModel(
                label = strings?.t(I18nKeys.Flows.RECEIVE_TOKEN_CONTRACT) ?: (fallback.contract?.label ?: ""),
                value = shortAddress(contract),
                copyLabel = strings?.t(I18nKeys.Flows.COPY_ADDRESS) ?: (fallback.contract?.copyLabel ?: ""),
                copyValue = contract,
            )
        },
        account = fallback.account.copy(
            name = name,
            // The identicon is drawn FROM the address, so a stale seed is a
            // stale face beside a live address — the exact mismatch somebody
            // uses to check they are looking at their own wallet.
            identiconSeed = address,
            lines = addressLines(address),
        ),
        // Spec 090: the code says what the CORE says — the bare address, or
        // with "include network" on, `ethereum:<address>@<chain>` for the asset
        // this sheet is drawn from. Before the machine has an answer, the
        // address, which every wallet reads.
        code = request.qr_value.ifEmpty { address },
        network = if (request.network_switch && strings != null) {
            NetworkSwitchModel(
                label = strings.t(I18nKeys.Flows.RECEIVE_INCLUDE_NETWORK),
                isOn = request.include_network,
                hint = if (request.network_hint) strings.t(I18nKeys.Flows.RECEIVE_INCLUDE_NETWORK_HINT) else null,
            )
        } else {
            null
        },
    )

    /**
     * Whether this sheet may be shown at all.
     *
     * The core's gate: while it is loading, the QR stays covered so a first
     * visit never flashes a code before the warning about which networks this
     * address is safe on.
     */
    /**
     * R4 — the share card for THIS account (spec 047 US2): its identicon, name,
     * address and network.
     *
     * The mark in the code's centre is the NETWORK's, token or not (the web's
     * `chainMark`): the card says which network may pay, and one card serves
     * every asset on it. [nativeSymbol] letters the disc that stands in when
     * the logo cannot be fetched; without a [chainId] the drawn mark stays.
     */
    fun shareCard(
        fallback: ShareCardModel,
        address: String,
        name: String,
        networkName: String,
        strings: VelaStrings,
        chainId: Int? = null,
        nativeSymbol: String? = null,
        /** Spec 090: the code the screen shows (the core's `qr_value`); blank = the address. */
        code: String = "",
    ): ShareCardModel = fallback.copy(
        // Exactly what the screen's code says (spec 090).
        code = code.ifEmpty { address },
        chainLogoUrl = chainId?.let { Marks.chainLogoUrl(it) },
        networkMark = chainId?.let { WalletLive.chainMark(it, nativeSymbol ?: fallback.networkMark.ticker) } ?: fallback.networkMark,
        name = name.ifBlank { shortAddress(address) },
        lines = addressLines(address),
        identiconSeed = address,
        networkNote = strings.t(I18nKeys.Flows.SHARE_CARD_NETWORK_NOTE, mapOf("network" to networkName)),
    )

    fun receiveGateOpen(request: PaymentRequestView): Boolean =
        !request.gate_loading && request.acknowledged

    /**
     * R1's rows: the networks this device actually has, each showing THIS
     * address.
     *
     * The fixture drew eight rows with a fixture address on every one. The
     * count is the person's own now, and so is the address — the same one, on
     * every network, which is the whole point the subtitle makes.
     */
    fun receiveNetworks(
        fallback: ReceiveListModel,
        view: NetView,
        address: String,
        badge: (Long) -> androidx.compose.ui.graphics.Color,
    ): ReceiveListModel {
        if (!view.loaded) return fallback
        val shown = shortAddress(address)
        // Copy on a row means the same address every time; the fixture's own
        // labels carry the wording.
        val template = fallback.rows.firstOrNull()
        return fallback.copy(
            address = address,
            subtitle = fallback.subtitle.replaceFirst(
                Regex("\\d+"),
                view.networks.size.toString(),
            ),
            rows = view.networks.map { row ->
                NetworkRowModel(
                    name = row.display_name,
                    code = row.native_symbol,
                    badgeColor = badge(row.chain_id),
                    logoUrl = Marks.chainLogoUrl(row.chain_id.toInt()),
                    addressDisplay = shown,
                    copyLabel = template?.copyLabel.orEmpty(),
                    qrLabel = template?.qrLabel.orEmpty(),
                )
            },
        )
    }

    /** `0x1234…abcd` — a row has no space for forty-two characters. */
    internal fun shortAddress(address: String): String =
        if (address.length <= 12) address else address.take(6) + "…" + address.takeLast(4)


    // -- the read screens behind "全部" ---------------------------------------

    /**
     * A1 — the whole history, not the first few.
     *
     * The home screen's Activity section shows a handful and offers "全部".
     * That link opened a fixture: a person tapped past their own payments into
     * somebody else's. Same rows, same grouping, same core ordering as the home
     * feed — this only removes the cut-off.
     */
    fun history(
        fallback: HistoryModel,
        feed: FeedView,
        strings: VelaStrings,
        now: Long = System.currentTimeMillis(),
        chainFilter: Int? = null,
        chainNames: Map<Int, String> = emptyMap(),
    ): HistoryModel {
        // The feed machine narrows the rows itself (`chain_filter_changed`); the pill says which chain.
        val groups = WalletLive.activity(feed, strings, now, chainNames)
        return fallback.copy(
            header = fallback.header.copy(pill = pill(fallback.header.pill, chainFilter, chainNames)),
            mode = if (groups.isEmpty()) HistoryMode.Empty else HistoryMode.Rows,
            // Spec 082 RG5: the empty line is the core's key, chosen by the
            // chain filter — never the drawn state's "none on this network".
            emptyText = strings.t(feed.history_empty_key),
            groups = groups,
        )
    }

    /** Spec 048: the 全部网络 pill reads the chosen network's name, or its drawn "all" label. */
    fun pill(drawn: FlowPillModel?, chainFilter: Int?, chainNames: Map<Int, String>): FlowPillModel? =
        drawn?.let { p -> p.copy(label = chainFilter?.let { chainNames[it] } ?: p.label) }

    /**
     * A2 — one transaction, the one that was tapped.
     *
     * [id] comes from the row. With no id, or an id the feed no longer holds,
     * the fixture is NOT shown: a detail screen about the wrong payment is
     * worse than one that admits it has nothing, because both look equally
     * authoritative and only one of them is wrong.
     */
    fun txDetail(
        fallback: TxDetailModel,
        feed: FeedView,
        id: String?,
        strings: VelaStrings,
        chainNames: Map<Int, String> = emptyMap(),
        explorers: Map<Int, String> = emptyMap(),
        /** Spec 049: the display currency; without one the fiat line is dollars, said so. */
        money: WalletLive.Money? = null,
    ): TxDetailModel? {
        val item = feed.rows
            .filterIsInstance<FeedRow.Item>()
            .map { it.item }
            .firstOrNull { it.id == id }
            ?: return null
        // Spec 093: a dApp's transaction or signature opens to the core's facts.
        item.dapp?.let { dapp -> return dappDetail(fallback, item, dapp, strings, chainNames, explorers, money) }

        val received = item.direction == FeedDirection.In
        val amount = Formats.current.plain(
            item.value?.toBigDecimalOrNull()?.stripTrailingZeros()?.toPlainString().orEmpty(),
        )
        val counterparty = item.counterparty.orEmpty()
        // Spec 082 RJ16: the core names a tx hash only when it is one — an op
        // hash is never an explorer link. 087 F05: and without one there is no
        // hash row at all — the record's own id is not a hash, and copying it
        // handed a tester `dapp-17905…-tx` as one.
        val txHash = item.tx_hash?.takeIf { it.isNotBlank() }
        // A dApp record whose payload this build could not read (spec 093:
        // the core describes every dApp row) — drawn the way it was before.
        val dapp = item.kind == FeedTxKind.DappTx || item.kind == FeedTxKind.SignMessage || item.kind == FeedTxKind.SignTypedData
        // Spec 043 phase 4 (device-found): the notification's deep link opened
        // this sheet with the fixture's title, status, counterparty, network,
        // date and hash around a live amount. Every line is the item's now.
        val facts = buildList {
            add(
                FactRowModel(
                    // Spec 082 RJ16: a call's `to` is the contract it went to,
                    // not somebody paid — the core says which.
                    label = when {
                        received -> strings.t(I18nKeys.Flows.DETAIL_FROM)
                        item.counterparty_role == FeedCounterpartyRole.Contract -> strings.t(I18nKeys.Flows.DETAIL_CONTRACT)
                        else -> strings.t(I18nKeys.Flows.DETAIL_TO)
                    },
                    value = item.alias ?: shortAddress(counterparty),
                    copyValue = counterparty,
                    lead = counterparty.takeIf { it.isNotBlank() }?.let { FactLead.Identicon(it) },
                    mono = item.alias == null,
                    copy = strings.t(I18nKeys.Flows.COPY_ADDRESS),
                ),
            )
            add(
                FactRowModel(
                    label = strings.t(I18nKeys.Flows.DETAIL_CHAIN),
                    value = chainNames[item.chain_id] ?: item.chain_id.toString(),
                    lead = FactLead.Token(WalletLive.mark(item.chain_id, item.symbol, null)),
                ),
            )
            // Spec 082 RG2: a dApp's transaction says which site asked for it.
            item.site?.takeIf { dapp && it.isNotBlank() }?.let { site ->
                add(FactRowModel(label = strings.t(I18nKeys.Flows.REQUESTED_BY), value = site))
            }
            add(FactRowModel(label = strings.t(I18nKeys.Flows.DETAIL_DATE), value = detailDate(item.timestamp, strings)))
            txHash?.let { hash ->
                add(
                    FactRowModel(
                        label = strings.t(I18nKeys.Flows.DETAIL_HASH),
                        value = if (hash.length > 16) "${hash.take(10)}…${hash.takeLast(6)}" else hash,
                        mono = true,
                        copy = strings.t(I18nKeys.Flows.COPY_ADDRESS),
                        copyValue = hash,
                    ),
                )
            }
        }
        return fallback.copy(
            explorerUrl = txHash?.let { tx -> explorers[item.chain_id]?.let { "${it.trimEnd('/')}/tx/$tx" } },
            explorerShown = txHash != null && explorers[item.chain_id] != null,
            title = if (dapp) {
                strings.t(I18nKeys.Wallet.LABEL_DAPP_TX)
            } else {
                strings.t(if (received) I18nKeys.Flows.TX_LABEL_RECEIVED else I18nKeys.Flows.TX_LABEL_SENT, mapOf("symbol" to item.symbol))
            },
            // Spec 082 RG1: where the record stands is the core's `status` —
            // the tracker alone moves it — never a guess from whether a hash
            // looks like a transaction's. A failed dApp row can say so now.
            status = statusChip(item.status, strings),
            // A dApp's call that moved no coin of ours has no amount (RG2).
            amount = if (amount.isBlank()) "" else "${if (received) "+" else "\u2212"}$amount ${item.symbol}".trim(),
            positive = received,
            // The fiat line is the core's own `usd_value`, which is 0 when
            // nothing could price it — and a confident "$0.00" on a detail
            // screen is the same lie the hero told once.
            // Spec 049: the chosen currency and preset (the web's `moneyText`), never a bare `$`.
            fiat = if (item.priced) {
                "≈ " + (money?.fiat(item.usd_value) ?: ("$" + Formats.current.fixed2(item.usd_value)))
            } else {
                ""
            },
            facts = facts,
            // The chain keeps the transaction; this is the wallet forgetting
            // it, which is why the sentence is "delete record" (spec 058).
            deleteLabel = strings.t(I18nKeys.Flows.DELETE_RECORD),
            // Spec 082 RJ18: while it may still land, deleting it is quiet —
            // and a record nothing settles (087 F04) may have been sent too.
            deleteQuiet = item.status == FeedTxStatus.Pending || item.status == FeedTxStatus.Unknown,
        )
    }

    /**
     * Spec 093 — a dApp interaction, opened: the row's header (title, and the
     * money moved or the allowance granted), the core's facts in the core's
     * order, and a collapsed "Technical details". A transaction keeps its
     * status chip and its explorer link; a signature has neither — it was
     * given off-chain and nothing settles it, which the note says instead.
     * Which lines, and in what order, are the core's (`FeedDapp.facts`,
     * `.technical`); this labels and formats them.
     */
    private fun dappDetail(
        fallback: TxDetailModel,
        item: FeedItem,
        dapp: FeedDapp,
        strings: VelaStrings,
        chainNames: Map<Int, String>,
        explorers: Map<Int, String>,
        money: WalletLive.Money?,
    ): TxDetailModel {
        val txHash = item.tx_hash?.takeIf { it.isNotBlank() }
        val allowance = dapp.allowance?.takeIf { item.value == null }
        val back = dapp.received?.let { change -> listOf(WalletLive.changeFigure(change), change.symbol).filter { it.isNotBlank() }.joinToString(" ") }
        // Spec 097 N5: nothing left and something came back (a borrow) — what
        // came back is the figure.
        val leadsWithBack = allowance == null && item.value == null && back != null
        val amount = when {
            allowance != null -> listOf(WalletLive.allowanceFigure(allowance, strings), allowance.symbol).filter { it.isNotBlank() }.joinToString(" ")
            leadsWithBack -> back.orEmpty()
            item.value == null -> ""
            else -> {
                val digits = Formats.current.plain(item.value.toBigDecimalOrNull()?.stripTrailingZeros()?.toPlainString() ?: item.value)
                // 083 F1: the simulation's figure says so.
                "${if (dapp.estimated) "≈ " else ""}−$digits ${item.symbol}".trim()
            }
        }
        return fallback.copy(
            title = WalletLive.dappTitle(dapp, strings),
            status = if (dapp.off_chain) null else statusChip(item.status, strings),
            // A signature says it was off-chain; a failed operation says why
            // (spec 097 N4), under its chip.
            note = when {
                dapp.off_chain -> strings.t(I18nKeys.Flows.OFF_CHAIN_NOTE)
                else -> dapp.failure?.let { failureText(it, strings) }
            },
            amount = amount,
            amountDanger = allowance?.unlimited == true,
            positive = leadsWithBack && dapp.received?.direction == FeedDirection.In,
            received = if (leadsWithBack) null else back,
            // Spec 097 N7: only a price the core knows — unknown is not "$0.00".
            fiat = if (item.priced && !leadsWithBack) "≈ " + (money?.fiat(item.usd_value) ?: ("$" + Formats.current.fixed2(item.usd_value))) else "",
            facts = dapp.facts.mapNotNull { dappFact(it, item, dapp, strings, chainNames) },
            technical = TxTechnicalModel(
                title = strings.t(I18nKeys.Flows.TECHNICAL),
                lines = dapp.technical.mapNotNull { technicalLine(it, item, strings) },
            ).takeIf { it.lines.isNotEmpty() },
            explorerUrl = txHash?.let { tx -> explorers[item.chain_id]?.let { "${it.trimEnd('/')}/tx/$tx" } },
            explorerShown = txHash != null && explorers[item.chain_id] != null,
            deleteLabel = strings.t(I18nKeys.Flows.DELETE_RECORD),
            deleteQuiet = item.status == FeedTxStatus.Pending || item.status == FeedTxStatus.Unknown,
        )
    }

    /** Why a dApp operation failed, in the words its request ended with (spec 097 N4). */
    private fun failureText(failure: app.getvela.wallet.feature.send.core.TrackFailure, strings: VelaStrings): String = when (failure) {
        app.getvela.wallet.feature.send.core.TrackFailure.Reverted -> strings.t(I18nKeys.Flows.TX_FAILED_HINT)
        app.getvela.wallet.feature.send.core.TrackFailure.Refused -> strings.t(I18nKeys.Flows.SIGN_REFUSED)
        app.getvela.wallet.feature.send.core.TrackFailure.NotSent -> strings.t(I18nKeys.Flows.TX_ERROR_GENERIC)
    }

    /** Where a record stands, as its chip says it (spec 082 RG1; 087 F04: unknown is pending that nothing will settle — not failed). */
    private fun statusChip(status: FeedTxStatus, strings: VelaStrings): StatusChipModel = when (status) {
        FeedTxStatus.Pending -> StatusChipModel(strings.t(I18nKeys.Flows.STATUS_PENDING), StatusTone.Warning)
        FeedTxStatus.Failed -> StatusChipModel(strings.t(I18nKeys.Flows.STATUS_FAILED_DETAIL), StatusTone.Error)
        FeedTxStatus.Confirmed -> StatusChipModel(strings.t(I18nKeys.Flows.STATUS_CONFIRMED), StatusTone.Success)
        FeedTxStatus.Unknown -> StatusChipModel(strings.t(I18nKeys.Flows.STATUS_UNKNOWN), StatusTone.Info)
    }

    /** One of the core's detail facts, labelled and formatted. */
    private fun dappFact(fact: FeedFact, item: FeedItem, dapp: FeedDapp, strings: VelaStrings, chainNames: Map<Int, String>): FactRowModel? = when (fact) {
        is FeedFact.Site -> FactRowModel(label = strings.t(I18nKeys.Flows.DETAIL_APP), value = fact.site)
        is FeedFact.Network -> {
            val name = chainNames[fact.chain_id] ?: fact.chain_id.toString()
            FactRowModel(
                label = strings.t(I18nKeys.Flows.DETAIL_CHAIN),
                value = name,
                lead = FactLead.Token(WalletLive.chainMark(fact.chain_id, name)),
            )
        }
        // 083 F3 review: the noun "Contract" — the call's target is not somebody it paid.
        is FeedFact.Contract -> party(strings.t(I18nKeys.Flows.DAPP_CONTRACT), fact.address, fact.name, strings)
        is FeedFact.Recipient -> party(strings.t(I18nKeys.Flows.DETAIL_TO), fact.address, fact.name, strings)
        is FeedFact.Spender -> party(strings.t(I18nKeys.Flows.DETAIL_SPENDER), fact.address, fact.name, strings)
        is FeedFact.SpendingCap -> FactRowModel(
            label = strings.t(I18nKeys.Flows.SPENDING_CAP),
            value = listOf(WalletLive.allowanceFigure(fact.allowance, strings), fact.allowance.symbol).filter { it.isNotBlank() }.joinToString(" "),
            danger = fact.allowance.unlimited,
        )
        is FeedFact.Expires -> FactRowModel(
            label = strings.t(I18nKeys.Flows.EXPIRES),
            value = fact.at?.let { detailDate(it, strings) } ?: strings.t(I18nKeys.Flows.NO_EXPIRY),
        )
        is FeedFact.BalanceChanges -> dapp.changes.map(::changeLine).takeIf { it.isNotEmpty() }?.let { lines ->
            FactRowModel(label = strings.t(I18nKeys.Flows.BALANCE_CHANGES), value = lines.first().resolve(strings), lines = lines.drop(1).map { it.resolve(strings) })
        }
        is FeedFact.Date -> FactRowModel(label = strings.t(I18nKeys.Flows.DETAIL_DATE), value = detailDate(fact.timestamp, strings))
        // A technical line in the facts would be the core's mistake; drawn
        // where the core put it all the same, never dropped.
        else -> technicalFact(fact, strings)
    }

    /** One of the core's technical lines; the stored request stays unread until the section opens. */
    private fun technicalLine(fact: FeedFact, item: FeedItem, strings: VelaStrings): TxTechnicalLine? = when (fact) {
        is FeedFact.Content -> TxTechnicalLine.Content(
            label = strings.t(
                when (fact.content) {
                    FeedDappContent.CallData -> I18nKeys.Flows.CONTENT_CALL_DATA
                    FeedDappContent.TypedData -> I18nKeys.Flows.CONTENT_TYPED_DATA
                    FeedDappContent.Message -> I18nKeys.Flows.CONTENT_MESSAGE
                },
            ),
            recordId = item.id,
            missing = strings.t(I18nKeys.Flows.CONTENT_MISSING),
            // The core's own word for it, as the wire spells it.
            content = Wire.json.encodeToJsonElement(FeedDappContent.serializer(), fact.content).jsonPrimitive.content,
        )
        else -> technicalFact(fact, strings)?.let(TxTechnicalLine::Fact)
    }

    /**
     * Spec 093: a stored request as Technical details shows it — the core's
     * one rule (`dapp_request_display`): typed data as its pretty-printed
     * document, a message as its text (or its hex), call data as its
     * params. `null` (the record kept nothing, or a word this build does not
     * know) is `connect.detail.contentMissing`.
     */
    fun requestDisplay(content: String, storedRequest: String?): String? =
        storedRequest?.takeIf { it.isNotBlank() }
            ?.let { uniffi.vela_core_uniffi.dappRequestDisplay(content, it) }
            ?.takeIf { it.isNotBlank() }

    private fun technicalFact(fact: FeedFact, strings: VelaStrings): FactRowModel? = when (fact) {
        is FeedFact.Operation -> FactRowModel(
            label = strings.t(I18nKeys.Flows.OPERATION),
            value = when (val operation = fact.operation) {
                FeedDappOperation.ContractInteraction -> strings.t(I18nKeys.Flows.OP_CONTRACT)
                is FeedDappOperation.Batch -> strings.t(I18nKeys.Flows.OP_BATCH, mapOf("count" to operation.calls.toString()))
                FeedDappOperation.Signature -> strings.t(I18nKeys.Flows.OP_SIGNATURE)
                FeedDappOperation.TypedDataSignature -> strings.t(I18nKeys.Flows.OP_TYPED_DATA)
            },
        )
        is FeedFact.PrimaryType -> FactRowModel(label = strings.t(I18nKeys.Flows.TYPE), value = fact.name, mono = true)
        is FeedFact.Hash -> hashFact(strings.t(I18nKeys.Flows.DETAIL_HASH), fact.tx_hash, strings)
        is FeedFact.UserOpHash -> hashFact(strings.t(I18nKeys.Flows.USER_OP_HASH), fact.hash, strings)
        else -> null
    }

    /** A contract, a recipient or a spender: its name, else the short address — copyable either way. */
    private fun party(label: String, address: String, name: String?, strings: VelaStrings) = FactRowModel(
        label = label,
        value = name ?: shortAddress(address),
        lead = FactLead.Identicon(address),
        mono = name == null,
        copy = strings.t(I18nKeys.Flows.COPY_ADDRESS),
        copyValue = address,
    )

    private fun hashFact(label: String, hash: String, strings: VelaStrings) = FactRowModel(
        label = label,
        value = if (hash.length > 16) "${hash.take(10)}…${hash.takeLast(6)}" else hash,
        mono = true,
        copy = strings.t(I18nKeys.Flows.COPY_ADDRESS),
        copyValue = hash,
    )

    /** A balance-change line: the figure and the coin, or "Unverified token" with its direction only (083 F1). */
    private fun changeLine(change: FeedDappChange): ChangeLine = ChangeLine(
        figure = WalletLive.changeFigure(change),
        symbol = change.symbol.takeIf { change.verified && it.isNotBlank() },
        unverified = !change.verified,
    )

    private data class ChangeLine(val figure: String, val symbol: String?, val unverified: Boolean) {
        fun resolve(strings: VelaStrings): String = when {
            unverified -> "$figure ${strings.t(I18nKeys.Flows.UNVERIFIED_TOKEN)}"
            // A native coin the chain table does not name: no guessed ticker.
            symbol == null -> "$figure —"
            else -> "$figure $symbol"
        }
    }

    /**
     * T3 — the add-token sheet's ERC-20 half, from the `manage_tokens` view
     * (spec 043 T046; the web's `liveAddToken`). The drawn model keeps only
     * the labels; the field, the card, the chip and the button are the
     * core's state.
     */
    fun addToken(fallback: AddTokenModel, view: MtokView, strings: VelaStrings): AddTokenModel {
        val first = view.found.firstOrNull()
        val typed = view.input_address.isNotBlank()
        val result: AddTokenResult = when {
            view.detecting -> AddTokenResult.Searching(strings.t(I18nKeys.Flows.ADD_SEARCHING))
            first != null -> AddTokenResult.Token(
                mark = WalletLive.mark(first.chain_id, first.symbol, view.input_address.takeIf { view.address_valid }),
                name = first.name,
                detail = "${first.symbol} · ${strings.t(I18nKeys.Flows.ADD_LABEL_DECIMALS)} ${first.decimals} · ${first.network_name}",
                chip = if (first.added) StatusChipModel(strings.t(I18nKeys.Flows.ADD_TOKEN_ADDED), StatusTone.Success) else null,
            )
            // Before `not_found`: the probe FINDS this contract — it is a real,
            // well-formed ERC-20 view of the network's own coin — so "not found"
            // would be the wrong words for a refusal that has a reason.
            view.native_alias -> AddTokenResult.NotFound(
                "${strings.t(I18nKeys.Flows.ADD_NATIVE_ALIAS_TITLE)} — " +
                    strings.t(I18nKeys.Flows.ADD_NATIVE_ALIAS_MESSAGE),
            )
            view.not_found -> AddTokenResult.NotFound(
                "${strings.t(I18nKeys.Flows.ADD_NOT_FOUND_TITLE)} — ${strings.t(I18nKeys.Flows.ADD_NOT_FOUND_MESSAGE)}",
            )
            else -> AddTokenResult.None
        }
        return fallback.copy(
            tab = AddTokenTab.Erc20,
            // The lookup runs on every network at once; there is no network to pick.
            network = null,
            fieldLabel = strings.t(I18nKeys.Flows.ADD_TOKEN_ADDRESS),
            fieldValue = view.input_address,
            fieldError = when {
                view.save_error -> strings.t(I18nKeys.Flows.ADD_ERROR_SAVE)
                typed && !view.address_valid -> strings.t(I18nKeys.Flows.ADD_INVALID_ADDRESS)
                else -> null
            },
            result = result,
            cta = strings.t(I18nKeys.Flows.ADD_TO_WALLET),
            ctaDisabled = first == null || first.added || view.saving,
        )
    }

    /** "今天 11:20" or a dated line, from the item's epoch seconds, on this device's clock. */
    private fun detailDate(timestampSeconds: Double, strings: VelaStrings): String {
        val millis = (timestampSeconds * 1000).toLong()
        val calendar = java.util.Calendar.getInstance().apply { timeInMillis = millis }
        val today = java.util.Calendar.getInstance()
        val sameDay = calendar.get(java.util.Calendar.YEAR) == today.get(java.util.Calendar.YEAR) &&
            calendar.get(java.util.Calendar.DAY_OF_YEAR) == today.get(java.util.Calendar.DAY_OF_YEAR)
        // The person's date and time presets (spec 047 D2), not a fixed US shape.
        val time = Formats.current.time(millis)
        return if (sameDay) "${strings.t(I18nKeys.Flows.DAY_TODAY)} $time" else "${Formats.current.date(millis)} $time"
    }

    /**
     * T1 — every holding.
     *
     * The same rows the home screen shows, minus its cut-off. `chainNames`
     * comes from the network machine for the same reason it does there: a
     * row's second line is the chain, and `BalanceToken.name` is the token's.
     */
    fun assets(
        fallback: AssetsModel,
        view: BalanceView,
        chainNames: Map<Int, String>,
        currency: CurrencyView,
        chainFilter: Int? = null,
        emptyCopy: AssetsEmptyModel? = fallback.empty,
    ): AssetsModel {
        // Spec 048: narrowed to the chosen network (the row id starts with its chain id).
        val rows = WalletLive.assetRows(view, chainNames, currency)
            .filter { chainFilter == null || it.id.startsWith("$chainFilter:") }
        // The web's rule (`liveAssets`): empty once the core has actually
        // looked — never while the holdings are still loading — or when the
        // chosen network holds nothing while others do (issue #266: that list
        // used to be blank, with nothing saying why).
        val settledEmpty = rows.isEmpty() && !view.balance_unknown && !view.holdings_loading
        val filteredEmpty = rows.isEmpty() && view.tokens.isNotEmpty()
        return fallback.copy(
            header = fallback.header.copy(pill = pill(fallback.header.pill, chainFilter, chainNames)),
            rows = rows,
            // The guided-empty body replaces the list; it must not sit under
            // one. A wallet that holds something is not an empty wallet.
            empty = if (settledEmpty || filteredEmpty) emptyCopy else null,
        )
    }

    /**
     * T2 — one holding, the one that was tapped.
     *
     * Same rule as the transaction detail: no id, or an id nothing matches,
     * shows nothing rather than a fixture token. The transactions listed under
     * it are the ones on THIS token's chain and symbol.
     */
    fun tokenDetail(
        fallback: TokenDetailModel,
        view: BalanceView,
        feed: FeedView,
        id: String?,
        chainNames: Map<Int, String>,
        currency: CurrencyView,
        strings: VelaStrings,
        now: Long = System.currentTimeMillis(),
        explorers: Map<Int, String> = emptyMap(),
    ): TokenDetailModel? {
        val token = view.tokens
            .firstOrNull { WalletLive.holdingId(it.chain_id, it.token_address) == id }
            ?: return null

        val row = WalletLive.assetRows(
            BalanceView(tokens = listOf(token)),
            chainNames,
            currency,
        ).single()

        val theirs = FeedView(
            rows = feed.rows.filter { entry ->
                entry !is FeedRow.Item ||
                    (entry.item.chain_id == token.chain_id && entry.item.symbol == token.symbol)
            },
        )
        return fallback.copy(
            explorerUrl = explorers[token.chain_id]?.let { base -> token.token_address?.let { "${base.trimEnd('/')}/token/$it" } ?: "${base.trimEnd('/')}/address/${row.id.substringAfter(':')}" },
            mark = TokenMarkModel(token.symbol, row.badgeColor, row.logoUrls, row.badgeLogoUrl, row.badgeHidden),
            symbol = token.symbol,
            chain = row.chain,
            balance = row.balance,
            fiat = when (val fiat = row.fiat) {
                is AssetFiatModel.Value -> fiat.text
                // "—", not "$0.00": an unpriced holding has no fiat figure, and
                // a zero here would say it is worthless.
                is AssetFiatModel.NoPrice -> fiat.text
                else -> ""
            },
            // Issue #269: the facts are this token's. They were the fixture's —
            // USDT's price, its Ethereum contract, 6 decimals — under every
            // token's header, POL on Polygon included. The web's
            // `liveTokenDetail` facts, row for row.
            facts = tokenFacts(token, chainNames, currency, strings),
            rows = WalletLive.activity(theirs, strings, now, chainNames).flatMap { it.rows },
        )
    }

    private fun tokenFacts(
        token: BalanceToken,
        chainNames: Map<Int, String>,
        currency: CurrencyView,
        strings: VelaStrings,
    ): List<FactRowModel> {
        val money = WalletLive.Money.of(currency)
        val contract = token.token_address
        return listOf(
            FactRowModel(
                label = strings.t(I18nKeys.Flows.TOKEN_PRICE),
                // Unpriced is said, never a confident "$0.00".
                value = token.price_usd?.let { price ->
                    strings.t(I18nKeys.Flows.TOKEN_PRICE_VALUE, mapOf("symbol" to token.symbol, "value" to money.fiat(price)))
                } ?: strings.t(I18nKeys.Wallet.NO_PRICE),
            ),
            FactRowModel(
                label = strings.t(I18nKeys.Flows.TOKEN_CONTRACT),
                value = contract?.let(::shortAddress) ?: strings.t(I18nKeys.Flows.ADD_NATIVE_TOKEN),
                mono = contract != null,
                copy = contract?.let { strings.t(I18nKeys.Flows.COPY_ADDRESS) },
                copyValue = contract,
            ),
            FactRowModel(label = strings.t(I18nKeys.Flows.TOKEN_DECIMALS), value = token.decimals.toString()),
            FactRowModel(label = strings.t(I18nKeys.Flows.ADD_LABEL_NETWORK), value = chainNames[token.chain_id] ?: token.name),
        )
    }

    /** The address as the card draws it: two lines, split halfway. */
    internal fun addressLines(address: String): Pair<String, String> {
        if (address.isEmpty()) return "" to ""
        val half = (address.length + 1) / 2
        return address.take(half) to address.drop(half)
    }
}
