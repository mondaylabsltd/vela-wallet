package app.getvela.wallet.feature.send

import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.contacts.core.ContactsView
import app.getvela.wallet.feature.flows.AmountFieldModel
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

    internal fun pick(fallback: SendPickModel, view: SendView, ctx: Context): SendPickModel = fallback.copy(
        // Filters and the multi-send door stay drawn but inert in 043; the
        // notice and the selection belong to multi-select (045).
        notice = null,
        selection = null,
        rows = view.tokens.map { token -> assetRow(token, ctx) },
    )

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
            amount = AmountFieldModel(
                value = view.amount.ifEmpty { "0" },
                fiat = if (view.amount_fiat_code != null) "${view.token_amount} $symbol" else fiatLine,
                denomLabel = view.amount_fiat_code ?: ctx.money.code,
                raw = view.amount,
            ),
            recipient = RecipientFieldModel(
                label = s.t(I18nKeys.Flows.RECIPIENT_LABEL),
                lines = addressLines(view.recipient),
                identiconSeed = view.recipient.ifBlank { "0x0000000000000000000000000000000000000000" },
                pickLabel = s.t(I18nKeys.Flows.RECIPIENT_PICK_ARIA),
                scanLabel = null,
                note = view.recipient_identity?.name,
                raw = view.recipient,
            ),
            // Split is 045: the door stays shut on this base.
            addRecipient = null,
            recipients = emptyList(),
            recipientActions = emptyList(),
            summary = null,
            fee = feeRow(fallback.fee, view.fee, view.estimating_gas || view.fee_busy || fee.busy, ctx),
            ctaEnabled = view.can_continue,
        )
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
        return fallback.copy(
            amount = "${view.confirm_amount} $symbol",
            subline = view.confirm_amount_issue?.let { s.t(I18nKeys.Flows.CANNOT_CONVERT, mapOf("code" to it.code, "symbol" to it.symbol)) } ?: fiat,
            facts = listOf(
                FactRowModel(label = s.t(I18nKeys.Flows.FROM_LABEL), value = ctx.fromName.ifBlank { shortAddress(ctx.fromAddress) }, lead = FactLead.Identicon(ctx.fromAddress)),
                FactRowModel(
                    label = s.t(I18nKeys.Flows.TO_LABEL),
                    value = recipientName?.let { "$it · ${shortAddress(view.recipient)}" } ?: shortAddress(view.recipient),
                    lead = FactLead.Identicon(view.recipient),
                    mono = recipientName == null,
                ),
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
            breakdown = emptyList(),
            ctaEnabled = view.can_confirm && !view.sending,
        )
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
                cta = s.t(I18nKeys.Flows.TX_CLOSE_BACKGROUND),
                ctaAccent = false,
            )
            receipt.status == SendReceiptStatus.Confirmed -> fallback.copy(
                header = header,
                stage = ReceiptStage.Confirmed,
                title = s.t(I18nKeys.Flows.TX_CONFIRMED_TITLE, mapOf("amount" to receipt.amount, "symbol" to symbol)),
                captions = listOf(
                    "${s.t(I18nKeys.Flows.TO_NAME, mapOf("name" to (receipt.transfers.firstOrNull()?.to_name ?: shortAddress(view.recipient))))} · $chain",
                ),
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
