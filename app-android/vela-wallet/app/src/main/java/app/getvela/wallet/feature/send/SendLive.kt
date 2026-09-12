package app.getvela.wallet.feature.send

import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.contacts.core.ContactsView
import app.getvela.wallet.feature.flows.AmountFieldModel
import app.getvela.wallet.feature.send.core.SweepPick
import app.getvela.wallet.feature.flows.SweepRowModel
import app.getvela.wallet.feature.flows.SendSelectionModel
import app.getvela.wallet.feature.flows.SendNoticeModel
import app.getvela.wallet.feature.flows.SendCtaModel
import app.getvela.wallet.feature.send.core.SendRecipientDraft
import app.getvela.wallet.feature.flows.SendFormMode
import app.getvela.wallet.feature.flows.BreakdownRowModel
import app.getvela.wallet.feature.flows.SummaryLineModel
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

    fun flowState(view: SendView, feeSheetOpen: Boolean): FlowState = when (view.stage) {
        SendStage.SelectToken, SendStage.LockResolving, SendStage.LockError -> FlowState.SD1
        SendStage.EnterDetails -> when {
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

    internal fun pick(fallback: SendPickModel, view: SendView, ctx: Context, sweepPicking: Boolean = false): SendPickModel {
        val s = ctx.strings
        val rows = view.tokens.map { token -> assetRow(token, ctx) }
        if (!sweepPicking) {
            return fallback.copy(
                // Filters stay drawn but inert; the door reads "send several".
                notice = null,
                selection = null,
                rows = rows,
                cta = SendCtaModel(s.t(I18nKeys.Flows.MULTI_SEND_TITLE), accent = false),
            )
        }
        // Spec 045 US2: the tick per row is `multi_selected_ids`, the greying
        // is `multi_chain_id` (a sweep is one chain), and the CTA counts.
        val chain = view.multi_chain_id
        val count = view.multi_selected_ids.size
        val chainName = chain?.let { ctx.chainNames[it] ?: "chain-$it" } ?: ""
        return fallback.copy(
            header = fallback.header.copy(title = s.t(I18nKeys.Flows.MULTI_SEND_TITLE)),
            notice = chain?.let {
                SendNoticeModel(
                    mark = TokenMarkModel(nativeSymbol(it, ctx), WalletLive.badge(it.toLong())),
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
    internal fun sweepForm(fallback: SendFormModel, view: SendView, fee: FeeView, ctx: Context): SendFormModel {
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
                    mark = TokenMarkModel(row.symbol, WalletLive.badge(row.chain_id.toLong())),
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
            fee = feeRow(fallback.fee, view.fee, view.estimating_gas || view.fee_busy || fee.busy, ctx),
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
        balance = "${trim(token.balance)} ${token.symbol}",
        fiat = token.price_usd?.let { price ->
            AssetFiatModel.Value(ctx.money.symbol + fixed2(ctx.money.convert(amount(token.balance) * price)))
        } ?: AssetFiatModel.NoPrice("—"),
        masked = false,
    )

    // -- SD2 ---------------------------------------------------------------------

    internal fun form(fallback: SendFormModel, view: SendView, fee: FeeView, ctx: Context): SendFormModel {
        if (view.multi_select_mode) return sweepForm(fallback, view, fee, ctx)
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
                    mark = TokenMarkModel(it.symbol, WalletLive.badge(it.chain_id.toLong())),
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
            recipients = if (view.split_mode) view.recipients.mapIndexed { index, draft -> splitRow(draft, index, symbol, ctx) } else emptyList(),
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
            fee = feeRow(fallback.fee, view.fee, view.estimating_gas || view.fee_busy || fee.busy, ctx),
            ctaEnabled = view.can_continue,
            warning = formWarning(view, ctx),
        )
    }

    private fun amountModel(view: SendView, symbol: String, fiatLine: String, ctx: Context) = AmountFieldModel(
        value = view.amount.ifEmpty { "0" },
        fiat = if (view.amount_fiat_code != null) "${view.token_amount} $symbol" else fiatLine,
        denomLabel = view.amount_fiat_code ?: ctx.money.code,
        raw = view.amount,
    )

    private fun recipientModel(view: SendView, ctx: Context) = RecipientFieldModel(
        label = ctx.strings.t(I18nKeys.Flows.RECIPIENT_LABEL),
        lines = addressLines(view.recipient),
        identiconSeed = view.recipient.takeIf { ADDRESS.matches(it) } ?: "0x0000000000000000000000000000000000000000",
        pickLabel = ctx.strings.t(I18nKeys.Flows.RECIPIENT_PICK_ARIA),
        scanLabel = null,
        note = view.recipient_identity?.name,
        raw = view.recipient,
    )

    /** One of the split's rows as the card draws it: the core's draft, editable in place. */
    internal fun splitRow(draft: SendRecipientDraft, index: Int, symbol: String, ctx: Context): RecipientCardModel {
        val s = ctx.strings
        return RecipientCardModel(
            ordinal = s.t(I18nKeys.Flows.RECIPIENT_N, mapOf("n" to (index + 1).toString())),
            name = draft.name ?: if (ADDRESS.matches(draft.address)) shortAddress(draft.address) else "",
            identiconSeed = draft.address.takeIf { ADDRESS.matches(it) } ?: "0x0000000000000000000000000000000000000000",
            amount = "${draft.amount} $symbol".trim(),
            removeLabel = s.t(I18nKeys.Flows.REMOVE_RECIPIENT),
            id = draft.id,
            address = draft.address,
            amountValue = draft.amount,
            addressPlaceholder = s.t(I18nKeys.Flows.RECIPIENT_LABEL),
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
            value = "${view.confirm_amount} $symbol".trim() + fiat,
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
        return view.amount_warning?.let { warningText(it, s) }
    }

    /** One sentence per `SendAmountWarning`, the web's keys. */
    fun warningText(warning: SendAmountWarning, s: VelaStrings): String = when (warning) {
        is SendAmountWarning.NotEnoughToken -> s.t(I18nKeys.Flows.ALERT_INSUFFICIENT_BODY)
        is SendAmountWarning.InsufficientForGas -> s.t(I18nKeys.Flows.WARN_INSUFFICIENT_FOR_GAS, mapOf("sym" to (warning.symbol ?: "")))
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

    private fun feeRow(fallback: FeeRowModel, estimate: FeeEstimateView?, busy: Boolean, ctx: Context): FeeRowModel {
        val (text, mark) = feeText(estimate, ctx)
        return fallback.copy(
            mark = mark ?: fallback.mark,
            value = when {
                estimate != null -> text
                busy -> ctx.strings.t(I18nKeys.Flows.FEE_ESTIMATING)
                else -> "—"
            },
        )
    }

    /** "0.0021 XDAI" from the estimate: the fee asset's own units, never re-priced here. */
    private fun feeText(estimate: FeeEstimateView?, ctx: Context): Pair<String, TokenMarkModel?> {
        if (estimate == null) return "—" to null
        val colour = WalletLive.badge(estimate.chain_id.toLong())
        return when (val asset = estimate.fee_asset) {
            is FeeAssetView.Native -> {
                val symbol = nativeSymbol(estimate.chain_id, ctx)
                "${fromBase(estimate.total_wei, 18)} $symbol" to TokenMarkModel(symbol, colour)
            }
            is FeeAssetView.Erc20 -> {
                val symbol = asset.symbol ?: "TOKEN"
                "${fromBase(asset.amount, asset.decimals)} $symbol" to TokenMarkModel(symbol, colour)
            }
        }
    }

    // -- SD2F --------------------------------------------------------------------

    internal fun feeSheet(fallback: FeeTokenPickModel, fee: FeeView, ctx: Context): FeeTokenPickModel = fallback.copy(
        rows = fee.options.map { option ->
            FeeTokenRowModel(
                mark = TokenMarkModel(option.symbol, WalletLive.badge((fee.fee?.chain_id ?: 0).toLong())),
                symbol = option.symbol,
                balanceLabel = ctx.strings.t(I18nKeys.Flows.BALANCE_LABEL, mapOf("amount" to trim(fromBase(option.balance, option.decimals)))),
                fee = option.amount?.let { "~${trim(fromBase(it, option.decimals))} ${option.symbol}" } ?: "—",
                selected = option.selected,
            )
        },
    )

    // -- SD2E --------------------------------------------------------------------

    fun contactSheet(fallback: ContactPickModel, book: ContactsView): ContactPickModel = fallback.copy(
        groups = emptyList(),
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

    internal fun confirm(fallback: SendConfirmModel, view: SendView, ctx: Context): SendConfirmModel {
        val s = ctx.strings
        val token = view.selected_token
        val symbol = token?.symbol ?: ""
        val chain = token?.let { ctx.chainNames[it.chain_id] ?: it.network } ?: ""
        val fiat = token?.price_usd?.let { price ->
            val amount = view.confirm_amount.toBigDecimalOrNull() ?: BigDecimal.ZERO
            "≈ ${ctx.money.symbol}${fixed2(ctx.money.convert(amount.toDouble() * price))}"
        } ?: ""
        val (feeLine, _) = feeText(view.fee, ctx)
        val recipientName = view.recipient_identity?.name
        val split = view.split_mode && view.recipients.isNotEmpty()
        return fallback.copy(
            amount = "${view.confirm_amount} $symbol",
            subline = view.confirm_amount_issue?.let { s.t(I18nKeys.Flows.CANNOT_CONVERT, mapOf("code" to it.code, "symbol" to it.symbol)) } ?: fiat,
            facts = listOf(
                FactRowModel(label = s.t(I18nKeys.Flows.FROM_LABEL), value = ctx.fromName.ifBlank { shortAddress(ctx.fromAddress) }, lead = FactLead.Identicon(ctx.fromAddress)),
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
                        lead = FactLead.Identicon(view.recipient),
                        mono = recipientName == null,
                    )
                },
                FactRowModel(
                    label = s.t(I18nKeys.Flows.DETAIL_CHAIN),
                    value = chain,
                    lead = token?.let { FactLead.Token(TokenMarkModel(it.symbol, WalletLive.badge(it.chain_id.toLong()))) },
                ),
                FactRowModel(
                    label = s.t(I18nKeys.Flows.EST_FEE),
                    value = if (view.fee != null) "~$feeLine" else s.t(I18nKeys.Flows.FEE_ESTIMATING),
                ),
            ),
            breakdown = if (split) {
                view.recipients.map { draft ->
                    BreakdownRowModel(
                        identiconSeed = draft.address.takeIf { ADDRESS.matches(it) },
                        label = draft.name ?: shortAddress(draft.address),
                        value = "${draft.amount} $symbol".trim(),
                    )
                }
            } else {
                emptyList()
            },
            ctaEnabled = view.can_confirm && !view.sending && view.treasury_bootstrap == null && view.tx_error == null,
            notice = confirmNotice(view, ctx),
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
            return "${s.t(I18nKeys.Flows.TREASURY_TITLE)} · ${s.t(I18nKeys.Flows.TREASURY_LEAD)} $hint"
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
                title = s.t(I18nKeys.Flows.TX_CONFIRMED_TITLE, mapOf("amount" to (if (receipt.transfers.size > 1 && view.confirm_amount.isNotEmpty()) view.confirm_amount else receipt.amount), "symbol" to symbol)),
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
                hash = hash?.let { ReceiptHashModel(label = s.t(I18nKeys.Flows.TX_HASH), value = shortHash(it), copyLabel = s.t(I18nKeys.Flows.COPY_ADDRESS)) },
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
                hash = hash?.let { ReceiptHashModel(label = s.t(I18nKeys.Flows.TX_HASH), value = shortHash(it), copyLabel = s.t(I18nKeys.Flows.COPY_ADDRESS)) },
                viewOnExplorer = explorer?.takeIf { hash != null }?.let { s.t(I18nKeys.Flows.VIEW_ON_EXPLORER) },
                cta = s.t(I18nKeys.Flows.DONE),
                ctaAccent = true,
            )
            else -> fallback.copy(
                header = header,
                stage = ReceiptStage.Submitted,
                title = s.t(I18nKeys.Flows.TX_SUBMITTED_TITLE),
                captions = listOfNotNull(
                    if (receipt.hold_reason != null) s.t(I18nKeys.Flows.TX_HELD_FEES) else s.t(I18nKeys.Flows.TX_WAITING_CONFIRM),
                    receipt.typical_inclusion_s?.let {
                        s.t(I18nKeys.Flows.TX_TYPICAL_TIME, mapOf("chainName" to chain, "estSecs" to it.toString()))
                    },
                ),
                hash = null,
                viewOnExplorer = null,
                cta = s.t(I18nKeys.Flows.TX_CLOSE_BACKGROUND),
                ctaAccent = false,
            )
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
        return value.movePointLeft(decimals).stripTrailingZeros().toPlainString()
    }

    private fun trim(human: String): String {
        val parsed = human.toBigDecimalOrNull() ?: return human
        return parsed.setScale(6, RoundingMode.DOWN).stripTrailingZeros().toPlainString()
    }

    private fun amount(human: String): Double = human.toDoubleOrNull() ?: 0.0

    private fun fixed2(value: Double): String = BigDecimal(value).setScale(2, RoundingMode.DOWN).toPlainString()

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
    )
}
