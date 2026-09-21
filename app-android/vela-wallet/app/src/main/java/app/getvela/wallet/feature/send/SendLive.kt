package app.getvela.wallet.feature.send

import app.getvela.wallet.core.format.Formats
import app.getvela.wallet.feature.flows.FeeSpeedModel
import app.getvela.wallet.feature.flows.FeeSpeedOptionModel
import app.getvela.wallet.feature.send.core.FeeSpeedView
import app.getvela.wallet.feature.send.core.FeeTier
import app.getvela.wallet.feature.flows.ContactGroupModel
import androidx.compose.ui.graphics.Color
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.contacts.core.ContactsView
import app.getvela.wallet.feature.flows.AmountFieldModel
import app.getvela.wallet.feature.send.core.BatchUnit as WireBatchUnit
import app.getvela.wallet.feature.send.core.BatchView
import app.getvela.wallet.feature.send.core.BatchRateStatus
import app.getvela.wallet.feature.send.core.BatchParseReason
import app.getvela.wallet.feature.send.core.BATCH_MAX_RECIPIENTS
import app.getvela.wallet.feature.flows.BatchUnit
import app.getvela.wallet.feature.flows.BatchRowModel
import app.getvela.wallet.feature.flows.BatchImportModel
import app.getvela.wallet.feature.send.core.SweepPick
import app.getvela.wallet.feature.flows.SweepRowModel
import app.getvela.wallet.feature.flows.SendSelectionModel
import app.getvela.wallet.feature.flows.SendNoticeModel
import app.getvela.wallet.feature.flows.SendCtaModel
import app.getvela.wallet.feature.send.core.SendRecipientDraft
import app.getvela.wallet.feature.send.core.SendRowFieldState
import app.getvela.wallet.feature.flows.SendFormMode
import app.getvela.wallet.feature.flows.BreakdownRowModel
import app.getvela.wallet.feature.flows.SummaryLineModel
import app.getvela.wallet.feature.flows.FillEmptyModel
import app.getvela.wallet.feature.flows.RecipientCardModel
import app.getvela.wallet.feature.flows.RecipientActionModel
import app.getvela.wallet.feature.flows.RecipientAction
import app.getvela.wallet.feature.flows.ContactEntryModel
import app.getvela.wallet.feature.flows.ContactPickModel
import app.getvela.wallet.feature.flows.FactLead
import app.getvela.wallet.feature.flows.FactRowModel
import app.getvela.wallet.feature.flows.FeeRowModel
import app.getvela.wallet.feature.flows.FeeTokenPickModel
import app.getvela.wallet.feature.flows.FeeTokenRowModel
import app.getvela.wallet.feature.flows.FlowState
import app.getvela.wallet.feature.flows.ReceiptEtaModel
import app.getvela.wallet.feature.flows.ReceiptHashModel
import app.getvela.wallet.feature.flows.ReceiptStage
import app.getvela.wallet.feature.flows.RecipientFieldModel
import app.getvela.wallet.feature.flows.SendConfirmModel
import app.getvela.wallet.feature.flows.SendFormModel
import app.getvela.wallet.feature.flows.SendPickModel
import app.getvela.wallet.feature.flows.SendReceiptModel
import app.getvela.wallet.feature.flows.SendTokenCardModel
import app.getvela.wallet.feature.flows.TokenMarkModel
import app.getvela.wallet.feature.send.core.SendAlertKind
import app.getvela.wallet.feature.send.core.SendAmountWarning
import app.getvela.wallet.feature.send.core.SendTreasuryAsset
import app.getvela.wallet.feature.send.core.SendTxErrorKey
import app.getvela.wallet.feature.send.core.FeeAssetView
import app.getvela.wallet.feature.send.core.FeeEstimateView
import app.getvela.wallet.feature.send.core.FeeView
import app.getvela.wallet.feature.send.core.SendReceiptStatus
import app.getvela.wallet.feature.send.core.SendStage
import app.getvela.wallet.feature.send.core.SendToken
import app.getvela.wallet.feature.send.core.SendTxStatus
import app.getvela.wallet.feature.send.core.SendView
import app.getvela.wallet.feature.wallet.AssetFiatModel
import app.getvela.wallet.feature.wallet.AssetRowModel
import app.getvela.wallet.feature.wallet.WalletLive
import java.math.BigDecimal
import java.math.RoundingMode

/**
 * The send screens' display models from the core's view (spec 043 T030).
 *
 * The rule the read path set in 041 holds here with more at stake: **every
 * figure comes from the view, the fixture only lends its labels.** The amount
 * on the confirm page is `SendView.confirm_amount`, the one number the signed
 * batch is built from; the fee is the estimate the fee session settled; the
 * receipt's stage is the machine's. Nothing here validates, converts a
 * currency the core did not, or decides whether a button is enabled — the
 * `can_*` flags are the core's.
 *
 * What the fixture keeps: titles, labels, placeholders, the CTA words, the
 * filter chips (which 043 does not wire), the pill.
 */
object SendLive {

    /** The pieces every builder needs beyond the view. */
    internal class Context(
        val strings: VelaStrings,
        /** Chain id → display name, from the settings machine. */
        val chainNames: Map<Int, String>,
        /** Chain id → explorer base URL, for the receipt's link. */
        val explorers: Map<Int, String>,
        val money: WalletLive.Money,
        val fromName: String,
        val fromAddress: String,
    )

    /** Which drawn state the live view is in — the flow host renders by this. */
    private val ADDRESS = Regex("^0x[0-9a-fA-F]{40}$")

    fun flowState(view: SendView, feeSheetOpen: Boolean): FlowState = if (view.show_scanner && view.stage != SendStage.Receipt) FlowState.S1 else when (view.stage) {
        SendStage.SelectToken, SendStage.LockResolving, SendStage.LockError -> FlowState.SD1
        SendStage.EnterDetails -> when {
            view.show_batch_import -> FlowState.SD2C
            view.show_contact_picker -> FlowState.SD2E
            feeSheetOpen -> FlowState.SD2F
            else -> FlowState.SD2
        }
        SendStage.Confirm -> if (feeSheetOpen) FlowState.SD2F else FlowState.SD3
        SendStage.Receipt -> when (view.receipt?.status) {
            SendReceiptStatus.Confirmed -> FlowState.SD4C
            SendReceiptStatus.Failed -> FlowState.SD4B
            SendReceiptStatus.Submitted -> FlowState.SD4B
            null -> FlowState.SD4A
        }
    }

    /** The core's token id (`SendToken::id`): `network_address|native_symbol`. */
    fun tokenId(token: SendToken): String =
        "${token.network}_${token.token_address ?: "native"}_${token.symbol}"

    // -- SD1 ---------------------------------------------------------------------

    /** The web's stablecoin set (`services/activity.ts`), the Tether glyph folded to T. */
    private val STABLE_SYMBOLS = setOf("USDT", "USDT0", "USDC", "USDC.E", "DAI", "BUSD", "TUSD", "FDUSD", "USDE", "PYUSD", "USDP", "GUSD", "LUSD", "FRAX", "USDD")

    private fun isStable(symbol: String): Boolean = symbol.uppercase().replace("₮", "T") in STABLE_SYMBOLS

    /** Spec 048: the web's `sendTokenClass` — a native coin pays gas, a known stable is stable, the rest is other. */
    fun sendTokenClass(token: SendToken): String = when {
        token.token_address == null -> "gas"
        isStable(token.symbol) -> "stable"
        else -> "other"
    }

    /** The picker's rows after both narrowings, as indices into `view.tokens`, in the core's order (the web's `visibleSendTokens`). */
    fun visibleTokens(view: SendView, chainFilter: Int?, classFilter: String): List<Int> =
        view.tokens.indices.filter { i ->
            val token = view.tokens[i]
            (chainFilter == null || token.chain_id.toInt() == chainFilter) &&
                (classFilter == "all" || sendTokenClass(token) == classFilter)
        }

    internal fun pick(
        fallback: SendPickModel,
        view: SendView,
        ctx: Context,
        sweepPicking: Boolean = false,
        chainFilter: Int? = null,
        classFilter: String = "all",
    ): SendPickModel {
        val s = ctx.strings
        val visible = visibleTokens(view, chainFilter, classFilter)
        val rows = visible.map { i -> assetRow(view.tokens[i], ctx) }
        VelaLog.event("send.pick", "narrowed", "class" to classFilter, "chain" to chainFilter, "visible" to visible.size, "of" to view.tokens.size, "networks" to view.tokens.map { it.network }.distinct().take(4))
        // Spec 048: the chips and the pill say what narrowed the list.
        val filters = fallback.filters.map { it.copy(selected = it.id == classFilter) }
        val pill = fallback.header.pill?.let { p -> p.copy(label = chainFilter?.let { ctx.chainNames[it] } ?: p.label) }
        // Issue 209 (the web's `liveSendPick`): an empty list says WHY. The
        // core's own token list tells "holds nothing" from "a filter or the
        // search hid everything".
        val empty = s.t(if (view.tokens.isEmpty()) I18nKeys.Flows.NO_TOKENS_WITH_BALANCE else I18nKeys.Flows.NO_MATCHING_TOKENS)
        if (!sweepPicking) {
            return fallback.copy(
                header = fallback.header.copy(pill = pill),
                filters = filters,
                notice = null,
                selection = null,
                rows = rows,
                cta = SendCtaModel(s.t(I18nKeys.Flows.MULTI_SEND_TITLE), accent = false),
                empty = empty,
            )
        }
        // Spec 045 US2: the tick per row is `multi_selected_ids`, the greying
        // is `multi_chain_id` (a sweep is one chain), and the CTA counts.
        val chain = view.multi_chain_id
        val count = view.multi_selected_ids.size
        val chainName = chain?.let { ctx.chainNames[it] ?: "chain-$it" } ?: ""
        return fallback.copy(
            header = fallback.header.copy(title = s.t(I18nKeys.Flows.MULTI_SEND_TITLE), pill = pill),
            filters = filters,
            empty = empty,
            notice = chain?.let {
                SendNoticeModel(
                    mark = WalletLive.mark(it, nativeSymbol(it, ctx), null),
                    text = s.t(I18nKeys.Flows.MULTI_SEND_NOTICE, mapOf("network" to chainName)),
                )
            },
            rows = rows,
            selection = SendSelectionModel(
                selected = view.tokens.map { tokenId(it) in view.multi_selected_ids },
                dimmed = SweepPick.dimmed(view),
                selectAll = s.t(I18nKeys.Flows.SELECT_ALL_VALUABLE),
            ),
            cta = if (count > 0) {
                SendCtaModel(s.t(I18nKeys.Flows.MULTI_SEND_CONTINUE, mapOf("n" to count.toString(), "chain" to chainName)), accent = true)
            } else {
                SendCtaModel(s.t(I18nKeys.Flows.MULTI_SEND_TITLE), accent = false)
            },
        )
    }

    // -- SD2c: the batch sheet (spec 045 US3) --------------------------------------

    /** The web's `liveBatchImport`, word for word: the core parsed, priced and gated; this only says so. */
    internal fun batchImport(fallback: BatchImportModel, batch: BatchView, view: SendView, ctx: Context, replaces: Boolean = false): BatchImportModel {
        val s = ctx.strings
        val symbol = view.selected_token?.symbol ?: ""
        val count = batch.recipient_count
        // Lines READ — the ones that became rows and the ones the parser refused
        // (the web's `seen`): the count above a list is the length of that list.
        val seen = batch.preview.size + batch.errors.size
        // Someone is already on the form (the core's own count, as `merge` reads it).
        val formHasRows = view.split_import_room < BATCH_MAX_RECIPIENTS
        return fallback.copy(
            unitFiat = s.t(I18nKeys.Flows.BATCH_UNIT_FIAT, mapOf("code" to batch.fiat_code)),
            unitToken = s.t(I18nKeys.Flows.BATCH_UNIT_TOKEN, mapOf("sym" to symbol)),
            unit = if (batch.unit == WireBatchUnit.Fiat) BatchUnit.Fiat else BatchUnit.Token,
            pasteValue = batch.raw_text,
            rateLabel = s.t(I18nKeys.Flows.BATCH_RATE_LABEL, mapOf("sym" to symbol)),
            // A token-denominated sheet converts nothing (the web's `unitHint`): it
            // says so instead of explaining a rate the core ignores in that mode.
            rateHint = if (batch.unit == WireBatchUnit.Fiat) {
                s.t(I18nKeys.Flows.BATCH_RATE_HINT, mapOf("code" to batch.fiat_code, "sym" to symbol))
            } else {
                s.t(I18nKeys.Flows.BATCH_TOKEN_HINT, mapOf("sym" to symbol))
            },
            rateValue = when (batch.rate_status) {
                BatchRateStatus.Ok -> "${batch.rate_input} ${batch.fiat_code}"
                BatchRateStatus.Loading -> s.t(I18nKeys.Flows.BATCH_RATE_LOADING)
                // Unknown, and said so: the core has already refused to apply.
                BatchRateStatus.Failed -> s.t(I18nKeys.Flows.BATCH_RATE_FAILED)
            },
            rateInput = batch.rate_input,
            rateEdited = batch.rate_edited,
            rateReset = s.t(I18nKeys.Flows.BATCH_RATE_RESET),
            parsedLabel = s.t(I18nKeys.Flows.BATCH_PARSED_COUNT, mapOf("n" to seen.toString())),
            // The sheet's order (the web's `previewRows`): a refused line sits
            // between the neighbours it has in the sheet, each with its reason.
            rows = (
                batch.preview.map { row ->
                    row.line to BatchRowModel(
                        ok = row.ok,
                        address = row.name ?: row.address,
                        // The core converted it; an unconvertible row carries no
                        // token amount, and the raw figure there would read as if it had.
                        conversion = if (row.token_amount.isNotEmpty() && row.token_amount != "0") "${Formats.current.plain(row.token_amount)} $symbol" else "—",
                        note = when {
                            row.dup -> s.t(I18nKeys.Flows.BATCH_DUP)
                            !row.valid -> s.t(I18nKeys.Flows.BATCH_BAD_ADDRESS)
                            else -> null
                        },
                    )
                } + batch.errors.map { error ->
                    error.line to BatchRowModel(
                        ok = false,
                        address = error.raw,
                        conversion = "",
                        note = s.t(if (error.reason == BatchParseReason.NoAddress) I18nKeys.Flows.BATCH_BAD_ADDRESS else I18nKeys.Flows.BAD_AMOUNT),
                    )
                }
                ).sortedBy { it.first }.map { it.second },
            // `file_error` outlives a paste in the core (only the next pick clears
            // it), and an error about a file above a list that parsed is about nothing.
            fileError = if (batch.file_error && seen == 0) {
                "${s.t(I18nKeys.Flows.BATCH_IMPORT_FAILED_TITLE)}. ${s.t(I18nKeys.Flows.BATCH_IMPORT_FAILED_BODY)}"
            } else {
                null
            },
            total = if (count > 0) {
                SummaryLineModel(
                    label = "${s.t(I18nKeys.Flows.SPLIT_TOTAL)} · ${s.t(if (count == 1) I18nKeys.Flows.RECIPIENT_COUNT_ONE else I18nKeys.Flows.RECIPIENT_COUNT, mapOf("count" to count.toString()))}",
                    value = "${Formats.current.plain(batch.total_token)} $symbol".trim() +
                        (batch.total_fiat?.let { " · ${Formats.current.plain(it)} ${batch.fiat_code}" } ?: ""),
                    over = batch.over_balance,
                    // Adding to people already on the form draws from what the form
                    // has not given out yet (`split_remaining`), not the whole balance.
                    remaining = if (formHasRows && !replaces && view.split_remaining != null) {
                        s.t(I18nKeys.Flows.SPLIT_REMAINING, mapOf("amount" to "${trim(view.split_remaining)} $symbol".trim()))
                    } else {
                        s.t(I18nKeys.Flows.BALANCE_LABEL, mapOf("amount" to "${trim(view.selected_token?.balance ?: "0")} $symbol".trim()))
                    },
                )
            } else {
                null
            },
            rejectedText = if (batch.rejected > 0) {
                s.t(if (batch.rejected == 1) I18nKeys.Flows.BATCH_REJECTED_ONE else I18nKeys.Flows.BATCH_REJECTED_OTHER, mapOf("count" to batch.rejected.toString()))
            } else {
                null
            },
            note = when {
                batch.over_cap -> s.t(I18nKeys.Flows.BATCH_OVER_CAP, mapOf("n" to BATCH_MAX_RECIPIENTS.toString()))
                batch.over_balance -> s.t(I18nKeys.Flows.BATCH_OVER_BALANCE, mapOf("sym" to symbol))
                batch.template_saved -> s.t(I18nKeys.Flows.BATCH_TEMPLATE_SAVED)
                else -> null
            },
            noteWarning = batch.over_cap || batch.over_balance,
            // Said once the import can happen, beside the button that does it — and
            // with the way to choose the other (the web's `merge` line). Only when
            // there is someone on the form: the core's own count, read back from
            // the room it reports.
            merge = if (formHasRows && batch.can_apply) {
                s.t(if (replaces) I18nKeys.Flows.BATCH_REPLACES_ROWS else I18nKeys.Flows.BATCH_ADDS_TO_ROWS)
            } else {
                null
            },
            mergeAction = if (formHasRows && batch.can_apply) {
                s.t(if (replaces) I18nKeys.Flows.BATCH_ADD_INSTEAD else I18nKeys.Flows.BATCH_REPLACE_INSTEAD)
            } else {
                null
            },
            cta = when (count) {
                0 -> s.t(I18nKeys.Flows.BATCH_APPLY_EMPTY)
                1 -> s.t(I18nKeys.Flows.BATCH_APPLY_ONE, mapOf("count" to "1"))
                else -> s.t(I18nKeys.Flows.BATCH_APPLY_OTHER, mapOf("count" to count.toString()))
            },
            ctaDisabled = !batch.can_apply,
        )
    }

    /** The tokens a sweep moves, in the pick's order. */
    private fun pickedTokens(view: SendView): List<SendToken> =
        view.tokens.filter { tokenId(it) in view.multi_selected_ids }

    /**
     * The amount a sweep moves for one token: the core's reserved spec when it
     * has computed one (net of the gas the fee coin pays), else the balance the
     * spec will become. Both are the core's HUMAN decimal strings.
     */
    private fun sweepAmount(view: SendView, token: SendToken): String =
        view.multi_specs.firstOrNull { it.token_address == token.token_address }?.amount ?: token.balance

    /** SD2d — the sweep's form: the picked rows with the core's amounts, one recipient for all. */
    internal fun sweepForm(fallback: SendFormModel, view: SendView, fee: FeeView, ctx: Context, speed: SpeedInputs? = null): SendFormModel {
        val s = ctx.strings
        val picked = pickedTokens(view)
        val chainId = view.multi_chain_id ?: view.selected_token?.chain_id ?: 1
        return fallback.copy(
            header = fallback.header.copy(title = s.t(I18nKeys.Flows.MULTI_SEND_TITLE)),
            mode = SendFormMode.Sweep,
            token = null,
            sweepSummary = s.t(I18nKeys.Flows.MULTI_SEND_SUMMARY, mapOf("n" to picked.size.toString(), "chain" to (ctx.chainNames[chainId] ?: "chain-$chainId"))),
            sweepRows = picked.map { row ->
                SweepRowModel(
                    mark = WalletLive.mark(row.chain_id.toInt(), row.symbol, row.token_address, row.logo_urls),
                    symbol = row.symbol,
                    balanceLabel = s.t(I18nKeys.Flows.BALANCE_LABEL, mapOf("amount" to trim(row.balance))),
                    amount = trim(sweepAmount(view, row)),
                    max = s.t(I18nKeys.Flows.MAX),
                )
            },
            amount = null,
            addRecipient = null,
            recipients = emptyList(),
            recipientActions = emptyList(),
            summary = null,
            recipient = recipientModel(view, ctx).copy(note = s.t(I18nKeys.Flows.MULTI_SEND_SAME_RECIPIENT)),
            fee = feeRow(fallback.fee, view.fee ?: fee.fee, view.estimating_gas || view.fee_busy || fee.busy, view, fee, ctx, speed),
            speed = speed?.let { speedModel(it, view, ctx) },
            cta = s.t(I18nKeys.Flows.CONTINUE),
            ctaEnabled = view.can_continue,
            warning = formWarning(view, ctx),
        )
    }

    private fun assetRow(token: SendToken, ctx: Context): AssetRowModel = AssetRowModel(
        id = tokenId(token),
        ticker = token.symbol,
        chain = ctx.chainNames[token.chain_id] ?: token.network,
        badgeColor = WalletLive.badge(token.chain_id.toLong()),
        logoUrls = WalletLive.mark(token.chain_id.toInt(), token.symbol, token.token_address, token.logo_urls).logoUrls,
        badgeLogoUrl = WalletLive.mark(token.chain_id.toInt(), token.symbol, token.token_address, token.logo_urls).badgeLogoUrl,
        badgeHidden = WalletLive.mark(token.chain_id.toInt(), token.symbol, token.token_address, token.logo_urls).badgeHidden,
        balance = "${trim(token.balance)} ${token.symbol}",
        fiat = token.price_usd?.let { price ->
            AssetFiatModel.Value(ctx.money.symbol + fixed2(ctx.money.convert(amount(token.balance) * price)))
        } ?: AssetFiatModel.NoPrice("—"),
        masked = false,
    )

    // -- SD2 ---------------------------------------------------------------------

    internal fun form(fallback: SendFormModel, view: SendView, fee: FeeView, ctx: Context, speed: SpeedInputs? = null): SendFormModel {
        if (view.multi_select_mode) return sweepForm(fallback, view, fee, ctx, speed)
        val s = ctx.strings
        val token = view.selected_token
        val symbol = token?.symbol ?: ""
        val chain = token?.let { ctx.chainNames[it.chain_id] ?: it.network } ?: ""
        val fiatLine = token?.price_usd?.let { price ->
            val typed = view.token_amount.toBigDecimalOrNull() ?: BigDecimal.ZERO
            "≈ ${ctx.money.symbol}${fixed2(ctx.money.convert(typed.toDouble() * price))}"
        } ?: ""
        return fallback.copy(
            header = fallback.header.copy(title = s.t(I18nKeys.Flows.SEND_TITLE, mapOf("symbol" to symbol))),
            token = token?.let {
                SendTokenCardModel(
                    mark = WalletLive.mark(it.chain_id.toInt(), it.symbol, it.token_address, it.logo_urls),
                    symbol = it.symbol,
                    detail = "$chain · ${s.t(I18nKeys.Flows.BALANCE_LABEL, mapOf("amount" to trim(it.balance)))}",
                    max = s.t(I18nKeys.Flows.MAX),
                )
            },
            // Spec 045 US1: the door into a split, and — once through it —
            // the core's rows. Split mode is the core's flag; these are its
            // drafts, edited in place and sent back as the whole list.
            mode = if (view.split_mode) SendFormMode.Split else SendFormMode.Single,
            addRecipient = if (view.split_mode) null else s.t(I18nKeys.Flows.ADD_RECIPIENT),
            // Only a real address earns an identicon (the founder's anti-poisoning rule).
            amount = if (view.split_mode) null else amountModel(view, symbol, fiatLine, ctx),
            recipient = if (view.split_mode) null else recipientModel(view, ctx),
            recipients = if (view.split_mode) view.recipients.mapIndexed { index, draft -> splitRow(draft, index, symbol, ctx, view) } else emptyList(),
            recipientActions = if (view.split_mode) {
                listOf(
                    RecipientActionModel(RecipientAction.Add, s.t(I18nKeys.Flows.ADD_RECIPIENT)),
                    RecipientActionModel(RecipientAction.Contacts, s.t(I18nKeys.Flows.FROM_CONTACTS)),
                    RecipientActionModel(RecipientAction.Import, s.t(I18nKeys.Flows.BATCH_IMPORT)),
                )
            } else {
                emptyList()
            },
            summary = if (view.split_mode) splitSummary(view, symbol, ctx) else null,
            fee = feeRow(fallback.fee, view.fee ?: fee.fee, view.estimating_gas || view.fee_busy || fee.busy, view, fee, ctx, speed),
            speed = speed?.let { speedModel(it, view, ctx) },
            ctaEnabled = view.can_continue,
            warning = formWarning(view, ctx),
            hint = splitHint(view, ctx),
            fillEmpty = splitFillEmpty(view, symbol, ctx),
        )
    }

    /**
     * "Use 0.5 ETH for the empty rows" (the web's `fillEmpty`): offered while
     * the core flags a row's amount as empty and another row has a figure the
     * core accepts. The figure is the first such row's, exactly as typed —
     * nothing is computed; the label shows every digit it carries.
     */
    internal fun splitFillEmpty(view: SendView, symbol: String, ctx: Context): FillEmptyModel? {
        if (!view.split_mode) return null
        if (view.split_row_issues.none { it.amount == SendRowFieldState.Empty }) return null
        val flagged = view.split_row_issues.associate { it.id to it.amount }
        val source = view.recipients.firstOrNull { row ->
            row.amount.isNotBlank() && (flagged[row.id] ?: SendRowFieldState.Ok) == SendRowFieldState.Ok
        } ?: return null
        return FillEmptyModel(
            label = ctx.strings.t(I18nKeys.Flows.SPLIT_FILL_EMPTY, mapOf("amount" to "${exact(source.amount)} $symbol".trim())),
            amount = source.amount,
        )
    }

    /** The web's `exactAmount`: the person's decimal mark and every digit, trailing zeros dropped. */
    private fun exact(amount: String): String {
        if (!amount.contains('.')) return amount
        val whole = amount.substringBefore('.')
        val cut = amount.substringAfter('.').trimEnd('0')
        return if (cut.isEmpty()) whole else Formats.current.plain("$whole.$cut")
    }

    /**
     * Why a split's Continue is dark, when a row is why: the FIRST unfinished
     * recipient and what it still needs (the web's `splitHint`). The core's
     * `split_row_issues` IS the gate's reason, so nothing here re-derives the
     * address or amount rule. Silent while the pre-check is out — the button
     * is busy then, not refused.
     */
    internal fun splitHint(view: SendView, ctx: Context): String? {
        if (!view.split_mode || view.estimating_gas) return null
        val first = view.split_row_issues.firstOrNull() ?: return null
        val key = if (first.address == SendRowFieldState.Ok) I18nKeys.Flows.SPLIT_NEEDS_AMOUNT else I18nKeys.Flows.SPLIT_NEEDS_ADDRESS
        return ctx.strings.t(key, mapOf("n" to first.ordinal.toString()))
    }

    private fun amountModel(view: SendView, symbol: String, fiatLine: String, ctx: Context): AmountFieldModel {
        val (prefix, suffix) = unitAdornment(view.amount_fiat_code, symbol)
        return AmountFieldModel(
            value = view.amount.ifEmpty { "0" },
            fiat = if (view.amount_fiat_code != null) "${Formats.current.plain(view.token_amount)} $symbol" else fiatLine,
            // The unit being TYPED: the figure's own currency, or the token.
            denomLabel = view.amount_fiat_code ?: symbol,
            raw = view.amount,
            // Issue 231: the unit on the figure, keyed on the FIGURE's own
            // code (`amount_fiat_code`), never the display currency — which
            // may already have moved on under digits typed in another.
            unitPrefix = prefix,
            unitSuffix = suffix,
            // Issue 197: ⇄ exists only where the core offers it, and is live
            // only where pressing it would change something.
            denomShown = view.denom_toggle_shown,
            denomEnabled = view.denom_toggle_enabled,
        )
    }

    /**
     * The web's `unitAdornment`: `code == null` means the figure is in token
     * units and the symbol follows it ("0.00075 BNB"); a currency with a sign
     * leads it ("$4.00"); one without follows as its code ("4.00 PLN").
     * Nothing defaults to "$" — an unknown unit is no adornment at all.
     */
    internal fun unitAdornment(code: String?, tokenSymbol: String): Pair<String?, String?> {
        if (code == null) return null to tokenSymbol.ifEmpty { null }
        // The same ISO-4217 table `WalletLive.Money` writes the "≈" line with,
        // so the figure and the line beneath it agree on what a currency looks like.
        val sign = runCatching { java.util.Currency.getInstance(code).getSymbol(java.util.Locale.US) }
            .getOrNull()?.takeIf { it != code }
        return if (sign == null) null to code else sign to null
    }

    private fun recipientModel(view: SendView, ctx: Context) = RecipientFieldModel(
        label = ctx.strings.t(I18nKeys.Flows.RECIPIENT_LABEL),
        lines = addressLines(view.recipient),
        identiconSeed = view.recipient.takeIf { ADDRESS.matches(it) } ?: "0x0000000000000000000000000000000000000000",
        name = view.recipient_identity?.name,
        pickLabel = ctx.strings.t(I18nKeys.Flows.RECIPIENT_PICK_ARIA),
        scanLabel = null,
        note = recipientNote(view, ctx),
        raw = view.recipient,
    )

    /**
     * The trust line the core resolved (the web's `recipientNote`): "name ·
     * source" when it knows one, else the first-time tell — the one that
     * matters for a poisoned look-alike.
     */
    internal fun recipientNote(view: SendView, ctx: Context): String? {
        val identity = view.recipient_identity
        val name = identity?.name?.takeIf { it.isNotEmpty() }
        if (name != null) return identity.source?.takeIf { it.isNotEmpty() }?.let { "$name · $it" } ?: name
        if (view.recipient_risk?.first_time == true) return ctx.strings.t(I18nKeys.Flows.FIRST_TIME_SEND)
        return null
    }

    /**
     * One of the split's rows as the card draws it: the core's draft, editable
     * in place, with the core's word on it. Only a field with something IN it
     * can be wrong (an empty one is unfinished — its placeholder already says
     * what it wants), and a row that repeats an earlier payee says WHICH row it
     * repeats (issue 203) — the core matched them; this only picks the words.
     */
    internal fun splitRow(draft: SendRecipientDraft, index: Int, symbol: String, ctx: Context, view: SendView? = null): RecipientCardModel {
        val s = ctx.strings
        val issue = view?.split_row_issues?.firstOrNull { it.id == draft.id }
        val repeat = view?.split_duplicates?.firstOrNull { it.id == draft.id }
        return RecipientCardModel(
            ordinal = s.t(I18nKeys.Flows.RECIPIENT_N, mapOf("n" to (index + 1).toString())),
            name = draft.name ?: if (ADDRESS.matches(draft.address)) shortAddress(draft.address) else "",
            identiconSeed = draft.address.takeIf { ADDRESS.matches(it) } ?: "0x0000000000000000000000000000000000000000",
            amount = "${Formats.current.plain(draft.amount)} $symbol".trim(),
            removeLabel = s.t(I18nKeys.Flows.REMOVE_RECIPIENT),
            id = draft.id,
            address = draft.address,
            amountValue = draft.amount,
            addressPlaceholder = s.t(I18nKeys.Flows.RECIPIENT_LABEL),
            addressNote = if (issue?.address == SendRowFieldState.Invalid) s.t(I18nKeys.Flows.BATCH_BAD_ADDRESS) else null,
            duplicateNote = repeat?.let { s.t(I18nKeys.Flows.RECIPIENT_DUPLICATE, mapOf("n" to it.first_ordinal.toString())) },
            amountNote = if (issue?.amount == SendRowFieldState.Invalid) s.t(I18nKeys.Flows.BAD_AMOUNT) else null,
        )
    }

    /** The split's total above the fee: the core's SUM of the rows (`confirm_amount`), never the single field. */
    internal fun splitSummary(view: SendView, symbol: String, ctx: Context): SummaryLineModel {
        val s = ctx.strings
        val total = view.confirm_amount.toBigDecimalOrNull() ?: BigDecimal.ZERO
        val fiat = view.selected_token?.price_usd?.let { price ->
            " · ≈ ${ctx.money.symbol}${fixed2(ctx.money.convert(total.toDouble() * price))}"
        } ?: ""
        return SummaryLineModel(
            label = "${s.t(I18nKeys.Flows.SPLIT_TOTAL)} · ${s.t(I18nKeys.Flows.RECIPIENT_COUNT, mapOf("count" to view.recipients.size.toString()))}",
            value = "${Formats.current.plain(view.confirm_amount)} $symbol".trim() + fiat,
            // The core's live verdict that the rows outrun the balance — the
            // same predicate Continue refuses on, shown while typing — and how
            // much is still left to give out when they do not.
            over = view.split_over_balance,
            remaining = view.split_remaining?.let { s.t(I18nKeys.Flows.SPLIT_REMAINING, mapOf("amount" to "${trim(it)} $symbol".trim())) },
        )
    }

    /** The core's live refusal on the form — the amount warning, or the same-asset fee ceiling. */
    internal fun formWarning(view: SendView, ctx: Context): String? {
        val s = ctx.strings
        view.same_asset_fee_issue?.let { issue ->
            // The core hands base-unit decimal strings ("the shell formats");
            // device-found: the first cut printed 5000000000000000000 XDAI.
            val decimals = view.selected_token?.decimals ?: 18
            fun human(base: String) = fromBase(base, decimals)
            return s.t(
                I18nKeys.Flows.SAME_FEE_BODY,
                mapOf("amount" to human(issue.transfer_amount), "fee" to human(issue.fee_amount), "total" to human(issue.total), "symbol" to issue.symbol, "balance" to human(issue.balance)),
            ) + " " + s.t(I18nKeys.Flows.SAME_FEE_MAX, mapOf("amount" to human(issue.max_transfer_amount), "symbol" to issue.symbol))
        }
        // A split has its own live verdict, `split_over_balance`. It does not
        // take `amount_warning`: that one judges the single form's figure,
        // which a split leaves behind — a number no longer on the screen.
        if (view.split_mode) return if (view.split_over_balance) s.t(I18nKeys.Flows.ALERT_INSUFFICIENT_BODY) else null
        view.amount_warning?.let { return warningText(it, s) }
        // Last, ⇄'s own refusal (issue 197; `Some` exactly when the row is
        // shown and dimmed): a dimmed toggle with no sentence is a refusal
        // nobody can act on. Single mode only — the others have no ⇄.
        if (view.multi_select_mode) return null
        return view.denom_toggle_reason?.let {
            s.t(I18nKeys.Flows.DENOM_TOGGLE_NO_RATE, mapOf("code" to it.code, "symbol" to it.symbol))
        }
    }

    /** One sentence per `SendAmountWarning`, the web's keys. */
    fun warningText(warning: SendAmountWarning, s: VelaStrings): String = when (warning) {
        is SendAmountWarning.NotEnoughToken -> s.t(I18nKeys.Flows.ALERT_INSUFFICIENT_BODY)
        is SendAmountWarning.InsufficientForGas -> s.t(I18nKeys.Flows.WARN_INSUFFICIENT_FOR_GAS, mapOf("sym" to (warning.symbol ?: "")))
        is SendAmountWarning.InsufficientGas -> s.t(I18nKeys.Flows.WARN_INSUFFICIENT_GAS, mapOf("sym" to (warning.symbol ?: "")))
        is SendAmountWarning.NeedGas -> s.t(I18nKeys.Flows.WARN_NEED_GAS, mapOf("sym" to (warning.symbol ?: "")))
        is SendAmountWarning.CannotConvert -> s.t(I18nKeys.Flows.CANNOT_CONVERT, mapOf("code" to warning.code, "symbol" to warning.symbol))
    }

    /** Title and body for every `SendAlertKind` — the core's refusal, in the core's words. */
    fun alertText(kind: SendAlertKind, s: VelaStrings): Pair<String, String> = when (kind) {
        SendAlertKind.InvalidAddress -> s.t(I18nKeys.Flows.ALERT_INVALID_ADDRESS_TITLE) to s.t(I18nKeys.Flows.ALERT_INVALID_ADDRESS_BODY)
        SendAlertKind.InvalidAmount -> s.t(I18nKeys.Flows.ALERT_INVALID_AMOUNT_TITLE) to s.t(I18nKeys.Flows.ALERT_INVALID_AMOUNT_BODY)
        is SendAlertKind.InsufficientBalance ->
            s.t(I18nKeys.Flows.ALERT_INSUFFICIENT_TITLE) to (kind.warning?.let { warningText(it, s) } ?: s.t(I18nKeys.Flows.ALERT_INSUFFICIENT_BODY))
        SendAlertKind.SplitOverBalance -> s.t(I18nKeys.Flows.ALERT_INSUFFICIENT_TITLE) to s.t(I18nKeys.Flows.ALERT_INSUFFICIENT_BODY)
        SendAlertKind.LoadTokensFailed -> s.t(I18nKeys.Flows.ALERT_LOAD_TOKENS) to ""
        is SendAlertKind.EstimateFailed -> s.t(I18nKeys.Flows.ALERT_ESTIMATE_TITLE) to s.t(I18nKeys.Flows.ALERT_ESTIMATE_BODY)
        SendAlertKind.AccountUnavailable -> s.t(I18nKeys.Flows.ALERT_ESTIMATE_TITLE) to s.t(I18nKeys.Flows.ALERT_ACCOUNT_UNAVAILABLE_BODY)
    }

    private fun feeRow(
        fallback: FeeRowModel,
        inHand: FeeEstimateView?,
        busy: Boolean,
        view: SendView,
        fee: FeeView?,
        ctx: Context,
        speed: SpeedInputs? = null,
    ): FeeRowModel {
        // NEVER ANOTHER TIER'S FIGURE WEARING THIS TIER'S NAME (issue 681):
        // on the path where a pick re-measures, the estimate in hand still
        // belongs to the speed just left — the send machine keeps it across a
        // tier change — and "measuring" is the honest thing to say.
        val ofAnotherTier = inHand != null && speed != null && offered(inHand.tier) != speed.view.tier
        val estimate = inHand.takeIf { !ofAnotherTier }
        val (text, mark) = feeText(estimate, view, fee, ctx)
        val s = ctx.strings
        return fallback.copy(
            mark = mark ?: fallback.mark,
            // A figure in hand stays on screen while a re-quote is out (spec
            // 028); only a figure of ANOTHER speed gives way to "measuring".
            value = when {
                ofAnotherTier -> s.t(I18nKeys.Flows.FEE_ESTIMATING)
                estimate != null -> text
                busy -> s.t(I18nKeys.Flows.FEE_ESTIMATING)
                else -> "—"
            },
            refreshLabel = speed?.let { s.t(I18nKeys.Flows.FEE_REFRESH) },
            // A measurement is out — whoever started it — the same fact the
            // "measuring" text reads, so the row is never settled and busy at once.
            refreshing = busy,
            // `FeeView.stale` had no consumer on Android: the 30 s TTL ran out
            // and nothing said so. Not while a fresh measurement is out, and
            // not over a row with no figure of its own on it.
            staleNote = if (speed != null && fee?.stale == true && !busy && estimate != null) s.t(I18nKeys.Flows.FEE_STALE) else null,
        )
    }

    /**
     * The speed control's inputs (spec 069): the `fee_speed` core's view, and
     * the fee session pricing each tier — whose fee-coin options format that
     * option's fee, as the fee row formats its own.
     */
    class SpeedInputs(val view: FeeSpeedView, val feeViewOf: (FeeTier) -> FeeView?)

    /** A tier as one this build offers: the dead `rapid` reads as the factory `fast`. */
    internal fun offered(tier: FeeTier): FeeTier = if (tier == FeeTier.Rapid) FeeTier.Fast else tier

    private fun tierName(tier: FeeTier, s: VelaStrings): String = s.t(
        when (offered(tier)) {
            FeeTier.Standard -> I18nKeys.Flows.GAS_TIER_STANDARD
            FeeTier.Slow -> I18nKeys.Flows.GAS_TIER_SLOW
            else -> I18nKeys.Flows.GAS_TIER_FAST
        },
    )

    private fun tierHint(tier: FeeTier, s: VelaStrings): String = s.t(
        when (offered(tier)) {
            FeeTier.Standard -> I18nKeys.Flows.GAS_TIER_HINT_STANDARD
            FeeTier.Slow -> I18nKeys.Flows.GAS_TIER_HINT_SLOW
            else -> I18nKeys.Flows.GAS_TIER_HINT_FAST
        },
    )

    /**
     * The folded speed control (spec 068), drawn from the `fee_speed` core's
     * view (spec 069). Every figure is that tier's OWN settled quote, echoed
     * by the core; only the words and the fee line are made here.
     */
    internal fun speedModel(speed: SpeedInputs, view: SendView, ctx: Context): FeeSpeedModel =
        speedModel(speed, ctx.strings) { quote, fee -> feeText(quote, view, fee, ctx).first }

    /**
     * The same control for any fee surface — the dApp signing sheet draws it
     * too (spec 069). [optionFee] writes one tier's quote the way that surface
     * writes its own fee row, so an option never reads differently from the
     * row it would become.
     */
    internal fun speedModel(
        speed: SpeedInputs,
        strings: VelaStrings,
        optionFee: (FeeEstimateView, FeeView?) -> String,
    ): FeeSpeedModel {
        val s = strings
        val core = speed.view
        return FeeSpeedModel(
            label = s.t(I18nKeys.Flows.FEE_SPEED_LABEL),
            // THEIR default (or their pick for this send), never a hardcoded one.
            value = tierName(core.tier, s),
            open = core.open,
            onceNote = s.t(I18nKeys.Flows.FEE_SPEED_ONCE),
            freeNote = if (core.free_note) s.t(I18nKeys.Flows.FEE_SPEED_FREE) else null,
            singleNote = if (core.single) s.t(I18nKeys.Flows.FEE_SPEED_SINGLE) else null,
            gasPriceLabel = s.t(I18nKeys.Flows.GAS_PRICE_LABEL),
            gasPriceLine = core.gas_price_line,
            options = core.options.map { option ->
                val quote = option.fee
                FeeSpeedOptionModel(
                    id = option.tier.name.lowercase(),
                    label = tierName(option.tier, s),
                    detail = tierHint(option.tier, s),
                    // "measuring" while this tier's own quote is out, "—" when
                    // there is none to be had.
                    value = when {
                        quote != null -> optionFee(quote, speed.feeViewOf(option.tier))
                        option.measuring -> "…"
                        else -> "—"
                    },
                    gasPrice = option.gas_price,
                    selected = option.selected,
                )
            },
        )
    }

    /**
     * Below half a cent the coin amount is the honest primary and the fiat
     * half is left off (`03-domain-components.md` §3.1): a real fee rounded to
     * "$0.00" reads as free, which is a worse answer than no figure at all.
     */
    private const val FEE_FIAT_MIN_USD = 0.005

    /**
     * The unit price, in USD, of the coin a quote is denominated in.
     *
     * The relay's published row first — it priced the quote, so its number is
     * the one the estimate converted through — then the balances the form
     * already carries, which is where the amount's own "≈" line gets its
     * price. `null` when neither knows the coin: a fee row that invents a
     * price is worse than one that shows only the coin.
     */
    private fun feeUnitPriceUsd(contract: String?, chainId: Int, view: SendView, fee: FeeView?): Double? {
        fun same(other: String?) = if (contract == null) other == null else other?.equals(contract, ignoreCase = true) == true
        val published = fee?.let { feePriceUsd(contract, it) }
        if (published != null) return published
        val held = (listOfNotNull(view.selected_token) + view.tokens)
            .firstOrNull { it.chain_id == chainId && same(it.token_address) }
        return held?.price_usd?.takeIf { it > 0.0 }
    }

    /**
     * "0.0021 XDAI · ≈$0.55" from the estimate: the fee asset's own units,
     * **never re-priced here**, plus what that costs (issue 201).
     *
     * The amount is the quote's; only the PRICE is looked up. The fee was the
     * one figure on the send screen with no money beside it, so a person who
     * does not track the coin's price could not tell what a transfer cost —
     * the drawn row has read "0.0021 ETH · ≈$0.55" since it was drawn.
     */
    private fun feeText(estimate: FeeEstimateView?, view: SendView, fee: FeeView?, ctx: Context): Pair<String, TokenMarkModel?> {
        if (estimate == null) return "—" to null
        val parts = feeParts(estimate, nativeSymbol(estimate.chain_id, ctx))
        val price = feeUnitPriceUsd(parts.contract, estimate.chain_id, view, fee)
        return feeLine(parts, price, ctx.money) to parts.mark
    }

    /**
     * One fee line for every surface that prices the same operation — the send
     * screens and the dApp signing sheet. Two formatters would be two answers
     * about what a transaction costs.
     */
    internal fun feeLine(parts: FeeParts, priceUsd: Double?, money: WalletLive.Money): String {
        val usd = if (parts.units == null || priceUsd == null) null else parts.units * priceUsd
        return if (usd != null && usd >= FEE_FIAT_MIN_USD) "${parts.coin} · ≈${money.fiat(usd)}" else parts.coin
    }

    /**
     * The estimate split into what a row needs: the coin amount as words, its
     * mark, the units that a price multiplies, and which coin to price.
     *
     * `total_wei` is the NATIVE figure even when an ERC-20 pays; reading it
     * where the token's own `amount` belongs prints a six-decimal stablecoin
     * fee as an eighteen-decimal number, under the wrong ticker.
     */
    internal fun feeParts(estimate: FeeEstimateView, nativeSymbol: String): FeeParts = when (val asset = estimate.fee_asset) {
        is FeeAssetView.Native -> FeeParts(
            coin = "${feeFromBase(estimate.total_wei, 18)} $nativeSymbol",
            mark = WalletLive.mark(estimate.chain_id, nativeSymbol, null),
            units = estimate.total_wei.toBigDecimalOrNull()?.movePointLeft(18)?.toDouble(),
            contract = null,
        )
        is FeeAssetView.Erc20 -> {
            val symbol = asset.symbol ?: "TOKEN"
            FeeParts(
                coin = "${feeFromBase(asset.amount, asset.decimals)} $symbol",
                mark = WalletLive.mark(estimate.chain_id, symbol, asset.token),
                units = asset.amount.toBigDecimalOrNull()?.movePointLeft(asset.decimals)?.toDouble(),
                contract = asset.token,
            )
        }
    }

    /** The parts of a fee line: what it reads, its mark, and what prices it. */
    internal data class FeeParts(val coin: String, val mark: TokenMarkModel?, val units: Double?, val contract: String?)

    /** The price of the coin ONE published quote row is charged in — the signing sheet's lookup, which has no form behind it. */
    internal fun feePriceUsd(contract: String?, fee: FeeView): Double? {
        fun same(other: String?) = if (contract == null) other == null else other?.equals(contract, ignoreCase = true) == true
        return fee.options.firstOrNull { same(it.contract) }?.usd_price?.toDoubleOrNull()?.takeIf { it > 0.0 }
    }

    // -- SD2F --------------------------------------------------------------------

    internal fun feeSheet(fallback: FeeTokenPickModel, fee: FeeView, ctx: Context): FeeTokenPickModel = fallback.copy(
        rows = fee.options.map { option ->
            FeeTokenRowModel(
                mark = WalletLive.mark((fee.fee?.chain_id ?: 0).toInt(), option.symbol, option.contract),
                symbol = option.symbol,
                balanceLabel = ctx.strings.t(I18nKeys.Flows.BALANCE_LABEL, mapOf("amount" to trim(fromBase(option.balance, option.decimals)))),
                fee = option.amount?.let { "~${trim(fromBase(it, option.decimals))} ${option.symbol}" } ?: "—",
                selected = option.selected,
                // Issue 211: the core refuses to select a coin that cannot pay;
                // a row that looks like the others and silently does nothing is
                // how gas ends up "paid" in a coin the account does not hold.
                insufficient = option.insufficient,
                insufficientNote = ctx.strings.t(I18nKeys.Flows.WARN_INSUFFICIENT_GAS, mapOf("sym" to option.symbol)),
            )
        },
    )

    // -- SD2E --------------------------------------------------------------------

    fun contactSheet(fallback: ContactPickModel, book: ContactsView): ContactPickModel = fallback.copy(
        // Spec 048: the book's groups, each a door to the whole group as split rows.
        groups = book.groups.map { g ->
            ContactGroupModel(
                name = g.name,
                count = g.members.size.toString(),
                colors = fallback.groups.firstOrNull()?.colors ?: (Color.Gray to Color.White),
            )
        },
        contacts = book.contacts.map { contact ->
            ContactEntryModel(
                name = contact.name ?: contact.resolved_name ?: shortAddress(contact.address),
                group = null,
                addressDisplay = shortAddress(contact.address),
                identiconSeed = contact.address,
            )
        },
    )

    // -- SD3 ---------------------------------------------------------------------

    internal fun confirm(fallback: SendConfirmModel, view: SendView, ctx: Context, fee: FeeView? = null, speed: FeeSpeedView? = null): SendConfirmModel {
        val s = ctx.strings
        val token = view.selected_token
        val symbol = token?.symbol ?: ""
        val chain = token?.let { ctx.chainNames[it.chain_id] ?: it.network } ?: ""
        val fiat = token?.price_usd?.let { price ->
            val amount = view.confirm_amount.toBigDecimalOrNull() ?: BigDecimal.ZERO
            "≈ ${ctx.money.symbol}${fixed2(ctx.money.convert(amount.toDouble() * price))}"
        } ?: ""
        val (feeLine, _) = feeText(view.fee, view, fee, ctx)
        val recipientName = view.recipient_identity?.name
        val split = view.split_mode && view.recipients.isNotEmpty()
        return fallback.copy(
            // A sweep moves several coins; one mark would name the wrong one.
            mark = if (view.multi_select_mode) null else token?.let { WalletLive.mark(it.chain_id, it.symbol, it.token_address, it.logo_urls) },
            amount = "${Formats.current.plain(view.confirm_amount)} $symbol",
            subline = view.confirm_amount_issue?.let { s.t(I18nKeys.Flows.CANNOT_CONVERT, mapOf("code" to it.code, "symbol" to it.symbol)) } ?: fiat,
            facts = listOf(
                FactRowModel(label = s.t(I18nKeys.Flows.FROM_LABEL), value = ctx.fromName.ifBlank { shortAddress(ctx.fromAddress) }, lead = FactLead.Identicon(ctx.fromAddress, ctx.fromName.ifBlank { null })),
                // SD3b (spec 038 #D2): a split names its count here and every
                // one of its people below, so what is signed can be read in full.
                if (split) {
                    FactRowModel(
                        label = s.t(I18nKeys.Flows.TO_LABEL),
                        value = s.t(I18nKeys.Flows.RECIPIENT_COUNT, mapOf("count" to view.recipients.size.toString())),
                    )
                } else {
                    FactRowModel(
                        label = s.t(I18nKeys.Flows.TO_LABEL),
                        value = recipientName?.let { "$it · ${shortAddress(view.recipient)}" } ?: shortAddress(view.recipient),
                        lead = FactLead.Identicon(view.recipient, recipientName),
                        mono = recipientName == null,
                    )
                },
                FactRowModel(
                    label = s.t(I18nKeys.Flows.DETAIL_CHAIN),
                    value = chain,
                    lead = token?.let { FactLead.Token(WalletLive.mark(it.chain_id.toInt(), it.symbol, it.token_address, it.logo_urls)) },
                ),
                FactRowModel(
                    label = s.t(I18nKeys.Flows.EST_FEE),
                    value = if (view.fee != null) "~$feeLine" else s.t(I18nKeys.Flows.FEE_ESTIMATING),
                ),
            ) + listOfNotNull(
                // The speed, but only when it was CHOSEN for this send, or taken
                // because it was free (spec 068 / issue 686): the last screen
                // before a signature says so, and a free upgrade says why. A
                // send at the stored default adds no row.
                speed?.takeIf { it.picked || it.free }?.let { chosen ->
                    FactRowModel(
                        label = s.t(I18nKeys.Flows.FEE_SPEED_LABEL),
                        value = tierName(chosen.tier, ctx.strings),
                        note = if (chosen.picked) null else s.t(I18nKeys.Flows.FEE_SPEED_FREE),
                    )
                },
            ),
            breakdown = if (split) {
                view.recipients.map { draft ->
                    BreakdownRowModel(
                        identiconSeed = draft.address.takeIf { ADDRESS.matches(it) },
                        label = draft.name ?: shortAddress(draft.address),
                        value = "${Formats.current.plain(draft.amount)} $symbol".trim(),
                    )
                }
            } else {
                emptyList()
            },
            ctaEnabled = view.can_confirm && !view.sending && view.treasury_bootstrap == null && view.tx_error == null,
            notice = confirmNotice(view, ctx),
            // The treasury pause has two exits (spec 045 US4): the core's retry,
            // and "not now" — DismissTreasurySheet, the facts kept.
            noticeSecondary = if (view.treasury_bootstrap != null) s.t(I18nKeys.Flows.FUNDING_CANCEL) else null,
            noticeAction = when {
                view.treasury_bootstrap != null -> s.t(I18nKeys.Flows.TREASURY_RETRY)
                view.tx_error != null -> s.t(I18nKeys.Flows.TX_RETRY)
                // The stage stays Confirm while the passkey prompt is up; the
                // one honest button under it is Cancel — the core's checkpoint.
                view.tx_status == SendTxStatus.Signing -> s.t(I18nKeys.Flows.CANCEL)
                else -> null
            },
        )
    }

    /** What stopped the confirm page: the relay's treasury, or a submit the relay refused. */
    internal fun confirmNotice(view: SendView, ctx: Context): String? {
        val s = ctx.strings
        view.treasury_bootstrap?.let { status ->
            val decimals = if (status.asset == SendTreasuryAsset.PathUsd) 6 else 18
            val symbol = if (status.asset == SendTreasuryAsset.PathUsd) "pathUSD" else nativeSymbol(status.chain_id, ctx)
            val short = (status.floor.toBigDecimalOrNull() ?: BigDecimal.ZERO) - (status.balance.toBigDecimalOrNull() ?: BigDecimal.ZERO)
            val hint = s.t(I18nKeys.Flows.TREASURY_AMOUNT_HINT, mapOf("amount" to fromBase(short.max(BigDecimal.ZERO).toPlainString(), decimals), "symbol" to symbol))
            // Two situations, one symptom: on a network Vela ships the operator owns that
            // relayer and telling them is the fix; on one the person added, there may be
            // nobody else who can hold gas there at all (spec 060).
            val lead = if (status.operator_served) {
                s.t(I18nKeys.Flows.TREASURY_OPERATOR_LEAD)
            } else {
                s.t(I18nKeys.Flows.TREASURY_CUSTOM_LEAD)
            }
            return "${s.t(I18nKeys.Flows.TREASURY_TITLE)} · $lead $hint"
        }
        return when (view.tx_error) {
            SendTxErrorKey.BundlerFund -> s.t(I18nKeys.Flows.TX_ERROR_BUNDLER_FUND)
            SendTxErrorKey.Generic -> s.t(I18nKeys.Flows.TX_ERROR_GENERIC)
            null -> if (view.tx_status == SendTxStatus.Signing) s.t(I18nKeys.Flows.TX_PREPARING_BIOMETRIC) else null
        }
    }

    // -- SD4 ---------------------------------------------------------------------

    /** Which fixture the receipt borrows its labels from. */
    fun receiptStage(view: SendView): ReceiptStage = when (view.receipt?.status) {
        SendReceiptStatus.Confirmed -> ReceiptStage.Confirmed
        SendReceiptStatus.Failed -> ReceiptStage.Failed
        SendReceiptStatus.Submitted -> ReceiptStage.Submitted
        null -> ReceiptStage.Submitting
    }

    internal fun receipt(fallback: SendReceiptModel, view: SendView, ctx: Context): SendReceiptModel {
        val s = ctx.strings
        val token = view.selected_token
        val symbol = token?.symbol ?: ""
        val chain = token?.let { ctx.chainNames[it.chain_id] ?: it.network } ?: ""
        val header = fallback.header.copy(title = s.t(I18nKeys.Flows.SEND_TITLE, mapOf("symbol" to symbol)))
        val receipt = view.receipt
        val hash = view.tx_hash?.takeIf { it.isNotBlank() }
        val explorer = token?.let { ctx.explorers[it.chain_id] }?.takeIf { it.isNotBlank() }
        return when {
            receipt == null -> fallback.copy(
                header = header,
                stage = ReceiptStage.Submitting,
                title = when (view.tx_status) {
                    SendTxStatus.Signing -> s.t(I18nKeys.Flows.TX_SIGNING)
                    SendTxStatus.Submitting -> s.t(I18nKeys.Flows.TX_SUBMITTING)
                    else -> s.t(I18nKeys.Flows.TX_PREPARING)
                },
                captions = listOf(s.t(I18nKeys.Flows.TX_PREPARING_BIOMETRIC), s.t(I18nKeys.Flows.TX_BACKGROUND_HINT)),
                hash = null,
                viewOnExplorer = null,
                // While the ceremony is up the one honest button is Cancel —
                // the core's checkpoint; a "keep running" here would leave a
                // prompt nobody can answer.
                cta = if (view.tx_status == SendTxStatus.Signing) s.t(I18nKeys.Flows.CANCEL) else s.t(I18nKeys.Flows.TX_CLOSE_BACKGROUND),
                ctaAccent = false,
            )
            receipt.status == SendReceiptStatus.Confirmed -> fallback.copy(
                header = header,
                stage = ReceiptStage.Confirmed,
                // A split's title carries the core's SUM (`confirm_amount`), not
                // the single-send scalar the receipt view keeps for one person.
                title = s.t(I18nKeys.Flows.TX_CONFIRMED_TITLE, mapOf("amount" to Formats.current.plain(if (receipt.transfers.size > 1 && view.confirm_amount.isNotEmpty()) view.confirm_amount else receipt.amount), "symbol" to symbol)),
                // A split's parts on the receipt as on the confirm (spec 038
                // #D2): the count, then every person with their amount.
                captions = if (receipt.transfers.size > 1) {
                    listOf(
                        "${s.t(I18nKeys.Flows.RECIPIENT_COUNT, mapOf("count" to receipt.transfers.size.toString()))} · $chain",
                    ) + receipt.transfers.map { part -> "${part.to_name ?: shortAddress(part.to)} · ${part.amount} ${part.symbol}".trim() }
                } else {
                    listOf(
                        "${s.t(I18nKeys.Flows.TO_NAME, mapOf("name" to (receipt.transfers.firstOrNull()?.to_name ?: shortAddress(view.recipient))))} · $chain",
                    )
                },
                hash = hash?.let { ReceiptHashModel(label = s.t(I18nKeys.Flows.TX_HASH), value = shortHash(it), copyLabel = s.t(I18nKeys.Flows.COPY_ADDRESS), copyValue = it) },
                viewOnExplorer = explorer?.let { s.t(I18nKeys.Flows.VIEW_ON_EXPLORER) },
                cta = s.t(I18nKeys.Flows.DONE),
                ctaAccent = true,
            )
            receipt.status == SendReceiptStatus.Failed -> fallback.copy(
                header = header,
                stage = ReceiptStage.Failed,
                title = s.t(I18nKeys.Flows.STATUS_FAILED),
                captions = listOf(
                    when (receipt.hold_reason) {
                        null -> s.t(I18nKeys.Flows.TX_FAILED_HINT)
                        else -> s.t(I18nKeys.Flows.TX_REJECTED_FEES)
                    },
                ),
                hash = hash?.let { ReceiptHashModel(label = s.t(I18nKeys.Flows.TX_HASH), value = shortHash(it), copyLabel = s.t(I18nKeys.Flows.COPY_ADDRESS), copyValue = it) },
                viewOnExplorer = explorer?.takeIf { hash != null }?.let { s.t(I18nKeys.Flows.VIEW_ON_EXPLORER) },
                cta = s.t(I18nKeys.Flows.DONE),
                ctaAccent = true,
            )
            else -> {
                val typicalLine = receipt.typical_inclusion_s?.let {
                    s.t(I18nKeys.Flows.TX_TYPICAL_TIME, mapOf("chainName" to chain, "estSecs" to it.toString()))
                }
                // Issue 199 (web cf2a9e17): with the relay's clock and the chain's
                // usual time the screen counts the wait down and fills its ring.
                // Without the clock the typical time is still worth saying, once.
                val eta = if (receipt.submitted_at_ms != null && receipt.typical_inclusion_s != null && typicalLine != null) {
                    ReceiptEtaModel(
                        submittedAtMs = receipt.submitted_at_ms,
                        typicalS = receipt.typical_inclusion_s,
                        typicalLine = typicalLine,
                        remainingTemplate = s.t(I18nKeys.Flows.TX_REMAINING),
                        elapsedTemplate = s.t(I18nKeys.Flows.TX_ELAPSED),
                        slowLine = s.t(I18nKeys.Flows.TX_SLOW_CONFIRM),
                    )
                } else {
                    null
                }
                fallback.copy(
                    header = header,
                    stage = ReceiptStage.Submitted,
                    title = s.t(I18nKeys.Flows.TX_SUBMITTED_TITLE),
                    captions = listOfNotNull(
                        if (receipt.hold_reason != null) s.t(I18nKeys.Flows.TX_HELD_FEES) else s.t(I18nKeys.Flows.TX_WAITING_CONFIRM),
                        typicalLine.takeIf { eta == null },
                    ),
                    hash = null,
                    viewOnExplorer = null,
                    cta = s.t(I18nKeys.Flows.TX_CLOSE_BACKGROUND),
                    ctaAccent = false,
                    eta = eta,
                )
            }
        }
    }

    /** The explorer link for the receipt's hash, or null. */
    internal fun explorerUrl(view: SendView, ctx: Context): String? {
        val base = view.selected_token?.let { ctx.explorers[it.chain_id] }?.trimEnd('/') ?: return null
        val hash = view.tx_hash?.takeIf { it.isNotBlank() } ?: return null
        return "$base/tx/$hash"
    }

    // -- helpers ----------------------------------------------------------------------

    private fun nativeSymbol(chainId: Int, ctx: Context): String =
        ctx.chainNames[chainId]?.let { NATIVE_BY_NAME[it] } ?: "ETH"

    /** A base-unit decimal string as a human decimal, trailing zeros dropped. */
    fun fromBase(units: String, decimals: Int): String {
        val value = units.toBigDecimalOrNull() ?: return units
        // Display only (every caller draws it): the decimal mark is the preset's (spec 049).
        return Formats.current.plain(value.movePointLeft(decimals).stripTrailingZeros().toPlainString())
    }

    /**
     * A FEE in its coin, to read rather than to audit: six decimals, as the web
     * shows it. It used to be printed at full precision —
     * `~0,000410400290875302 ETH` — which crushed the row's label and told
     * nobody anything the first three figures had not (founder's device,
     * 2026-09-19). Rounded UP, because a fee that displays as less than it costs
     * is the wrong way to be wrong; a fee below the sixth decimal keeps two
     * significant figures instead of reading as zero.
     */
    fun feeFromBase(units: String, decimals: Int): String {
        val value = units.toBigDecimalOrNull()?.movePointLeft(decimals) ?: return units
        if (value.signum() == 0) return Formats.current.plain("0")
        val shown = if (value < java.math.BigDecimal("0.000001")) {
            value.round(java.math.MathContext(2, RoundingMode.UP))
        } else {
            value.setScale(6, RoundingMode.UP)
        }
        return Formats.current.plain(shown.stripTrailingZeros().toPlainString())
    }

    private fun trim(human: String): String {
        val parsed = human.toBigDecimalOrNull() ?: return human
        return Formats.current.plain(parsed.setScale(6, RoundingMode.DOWN).stripTrailingZeros().toPlainString())
    }

    private fun amount(human: String): Double = human.toDoubleOrNull() ?: 0.0

    private fun fixed2(value: Double): String = Formats.current.fixed2(value)

    fun shortAddress(address: String): String =
        if (address.length > 12) "${address.take(6)}…${address.takeLast(4)}" else address

    private fun shortHash(hash: String): String =
        if (hash.length > 16) "${hash.take(10)}…${hash.takeLast(6)}" else hash

    /** The two lines the recipient card prints, as the receive screen splits an address. */
    fun addressLines(address: String): Pair<String, String> {
        if (address.length != 42) return address to ""
        return address.substring(0, 21) to address.substring(21)
    }

    /**
     * The native symbol by chain NAME — the settings rows carry the symbol
     * (`native_symbol`) but this builder is handed names; until the context
     * carries symbols too, the built-in chains resolve here and anything else
     * says ETH. Recorded in results as a known thinness.
     */
    private val NATIVE_BY_NAME = mapOf(
        "Ethereum" to "ETH", "Arbitrum" to "ETH", "Optimism" to "ETH", "Base" to "ETH", "Unichain" to "ETH",
        "BNB Chain" to "BNB", "Polygon" to "POL", "Gnosis" to "xDAI", "Avalanche" to "AVAX",
        // Arc's gas coin IS USDC; defaulting it to ETH would name the wrong
        // asset on the send sheet (spec 060).
        "Arc" to "USDC",
        "X Layer" to "OKB", "Stable" to "USDT0", "Soneium" to "ETH", "MegaETH" to "ETH",
        "Robinhood Chain" to "ETH", "Mantle" to "MNT", "Kaia" to "KAIA", "Celo" to "CELO",
        "Ink" to "ETH", "Plume" to "PLUME", "XRPL EVM" to "XRP",
    )
}
