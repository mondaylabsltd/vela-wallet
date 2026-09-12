package app.getvela.wallet.feature.signing

import app.getvela.wallet.feature.wallet.core.TrustSimJudgment
import androidx.compose.ui.graphics.Color
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.browser.ExploreLive
import app.getvela.wallet.feature.send.SendLive
import app.getvela.wallet.feature.send.core.FeeView
import app.getvela.wallet.feature.signing.core.ClearConfirm
import app.getvela.wallet.feature.signing.core.ClearDangerClass
import app.getvela.wallet.feature.signing.core.ClearMessageView
import app.getvela.wallet.feature.signing.core.ClearRisk
import app.getvela.wallet.feature.signing.core.ClearSignField
import app.getvela.wallet.feature.signing.core.ClearSignResult
import app.getvela.wallet.feature.signing.core.ClearSignType
import app.getvela.wallet.feature.signing.core.ClearSigningView
import app.getvela.wallet.feature.signing.core.ClearSiweBinding
import app.getvela.wallet.feature.signing.core.ClearSurface
import app.getvela.wallet.feature.signing.core.GuardAmountError
import app.getvela.wallet.feature.signing.core.GuardEditorMode
import app.getvela.wallet.feature.signing.core.GuardEditorView
import app.getvela.wallet.feature.signing.core.GuardIncreaseTotalView
import app.getvela.wallet.feature.signing.core.GuardSurface
import app.getvela.wallet.feature.signing.core.GuardTokenMetaView
import app.getvela.wallet.feature.signing.core.GuardView
import app.getvela.wallet.feature.signing.core.IncomingRequest
import app.getvela.wallet.feature.signing.core.SignErrorKind
import app.getvela.wallet.feature.signing.core.SignView
import app.getvela.wallet.feature.signing.core.SigningController

/**
 * The signing sheet from the four machines' views (spec 044 T033; the
 * desktop's `signing/live.rs`). The drawn model keeps only its labels; the
 * dApp is the HOST (guessing a pretty name from a domain is exactly the
 * counterfeit route), the blocks are the core's reading, the fee is the
 * fee policy's, the slide opens only when all three machines say so.
 */
object SigningLive {
    data class Context(
        val strings: VelaStrings,
        val chainName: String,
        val chainDot: Color,
        val nativeSymbol: String,
        val walletName: String,
        val walletAddress: String,
    )

    private fun VelaStrings.s(key: String) = t("componentsUi.signing.$key")
    private fun VelaStrings.s(key: String, vars: Map<String, String>) = t("componentsUi.signing.$key", vars)
    private fun VelaStrings.a(key: String) = t("componentsUi.signingApprove.$key")
    private fun VelaStrings.a(key: String, vars: Map<String, String>) = t("componentsUi.signingApprove.$key", vars)

    fun model(fallback: SigningScreenModel, request: IncomingRequest, sign: SignView, clear: ClearSigningView, guard: GuardView, fee: FeeView, ctx: Context, sim: SigningController.SimOutcome? = null): SigningScreenModel {
        val s = ctx.strings
        val host = request.origin.substringAfter("://").substringBefore('/').ifBlank { request.origin }
        val facts = SigningController.firstCall(request.paramsJson)
        val dataBytes = facts?.second?.removePrefix("0x")?.length?.div(2) ?: 0
        val blocks = statusBlocks(sign, s) + blocks(clear, facts?.first, facts?.third, dataBytes, ctx) + simBlocks(sim, ctx) + guardBlocks(guard, s)
        return fallback.copy(
            dappName = host,
            dappHost = host,
            dappLetter = ExploreLive.letterOf(host),
            dappTint = ExploreLive.tintOf(host),
            networkName = ctx.chainName,
            networkDot = ctx.chainDot,
            blocks = blocks,
            tech = fallback.tech.copy(
                title = fallback.tech.title,
                summary = clear.result?.contract_name,
                functionLabel = clear.result?.let { s.s("techFunction") },
                signature = clear.result?.intent,
                params = emptyList(),
                identities = emptyList(),
                simResult = null,
                rawLabel = if (dataBytes > 0) s.s("techRawData") else null,
                rawHex = facts?.second?.takeIf { dataBytes > 0 },
            ),
            techOpen = false,
            fee = feeModel(clear, fee, ctx),
            signerLabel = s.s("signingAccount"),
            signerName = ctx.walletName,
            signerSeed = ctx.walletAddress,
            confirmHint = s.s("slideToConfirm"),
            confirmAction = confirmLabel(clear, s),
            confirmEnabled = confirmEnabled(sign, guard, fee, clear),
            panelTitle = s.s("signatureRequest"),
        )
    }

    /**
     * The guard's verdict on an approval (the desktop's `guard_editor`): the
     * spending cap with the chips the core offers, the custom amount when
     * chosen, the notes, the resulting total for an increase; an off-chain
     * permit that cannot be capped; a batch's legs.
     */
    fun guardBlocks(guard: GuardView, s: VelaStrings): List<SigningBlock> = when (guard.surface) {
        GuardSurface.None -> emptyList()
        GuardSurface.PermitSign -> buildList {
            guard.detected?.let { add(SigningBlock.Party(s.a("spenderLabel"), ExploreLive.shortAddress(it.spender), it.spender)) }
            add(SigningBlock.Warning(SigningTone.Danger, s.a("permitCantCap")))
        }
        GuardSurface.ApprovalEditor -> buildList {
            guard.editor?.let { editor -> add(allowanceBlock(editor, guard.meta, guard.increase_total, guard.decimals_unverified, guard.expired, s)) }
            guard.detected?.let { add(SigningBlock.Party(s.a("spenderLabel"), ExploreLive.shortAddress(it.spender), it.spender)) }
            if (guard.detected?.is_unbounded == true && guard.editor?.choice == null) add(SigningBlock.Warning(SigningTone.Danger, s.s("unlimitedWarning")))
        }
        GuardSurface.Batch -> buildList {
            guard.batch?.legs?.forEachIndexed { index, leg ->
                leg.editor?.let { editor -> add(allowanceBlock(editor, leg.meta, null, false, false, s, prefix = "#${index + 1} ")) }
                leg.approval?.let { add(SigningBlock.Party(s.a("spenderLabel"), ExploreLive.shortAddress(it.spender), it.spender)) }
            }
            if (guard.batch?.any_uncapped == true) add(SigningBlock.Warning(SigningTone.Danger, s.s("unlimitedWarning")))
        }
    }

    private fun allowanceBlock(
        editor: GuardEditorView,
        meta: GuardTokenMetaView,
        increase: GuardIncreaseTotalView?,
        decimalsUnverified: Boolean,
        expired: Boolean,
        s: VelaStrings,
        prefix: String = "",
    ): SigningBlock.Allowance {
        fun chip(id: String, label: String, mode: GuardEditorMode, offered: Boolean) = AllowanceChip(
            id = id, label = label,
            state = when {
                !offered -> AllowanceChip.ChipState.Disabled
                editor.mode == mode -> AllowanceChip.ChipState.Selected
                else -> AllowanceChip.ChipState.Idle
            },
        )
        val chips = listOf(
            chip("requested", s.a("requested"), GuardEditorMode.Requested, editor.requested_finite),
            chip("balance", s.a("balanceCap"), GuardEditorMode.Balance, editor.has_balance_cap),
            chip("custom", s.a("custom"), GuardEditorMode.Custom, true),
            chip("revoke", s.a("revoke"), GuardEditorMode.Revoke, true),
        )
        val value = editor.display_amount_raw?.toBigIntegerOrNull()?.let { units ->
            "${SendLive.fromBase(units.toString(), meta.decimals)} ${meta.symbol}".trim()
        } ?: s.a("unlimitedValue")
        val notes = buildList {
            if (!editor.requested_finite) add(s.a("unlimitedDisabled") + "\n" + s.a("choosePrompt"))
            if (decimalsUnverified) add(s.a("decimalsUnverified"))
            if (expired) add(s.a("expired"))
        }
        return SigningBlock.Allowance(
            label = prefix + s.a("spendingCap"),
            value = value,
            valueTone = if (editor.choice != null) SigningTone.Neutral else SigningTone.Danger,
            chips = chips,
            note = notes.takeIf { it.isNotEmpty() }?.joinToString("\n"),
            resultingTotal = increase?.let { total ->
                SigningRow(s.a("resultingTotal"), total.total ?: s.a("resultingTotalUnknown", mapOf("amount" to total.increment)))
            },
            custom = if (editor.mode == GuardEditorMode.Custom) {
                AllowanceInput(
                    value = editor.custom_text, symbol = meta.symbol, placeholder = "0",
                    error = when (editor.error) {
                        GuardAmountError.InvalidAmount -> s.a("invalidAmount")
                        GuardAmountError.UnlimitedDisabled -> s.a("unlimitedDisabled")
                        null -> null
                    },
                )
            } else {
                null
            },
        )
    }

    /** The slide opens only when the request, the guard and the fee all say it may — and the reading is in. */
    fun confirmEnabled(sign: SignView, guard: GuardView, fee: FeeView, clear: ClearSigningView): Boolean {
        val offChain = clear.result?.sign_type == ClearSignType.Signature || clear.surface == ClearSurface.MessageSign || clear.surface == ClearSurface.EthSign || clear.surface == ClearSurface.BlindTypedData
        return sign.confirm_gate_open && guard.confirm_allowed && (offChain || fee.confirm_fee_ready) && !sign.is_signing && !sign.is_submitting
    }

    fun statusBlocks(sign: SignView, s: VelaStrings): List<SigningBlock> = buildList {
        sign.funding?.let { funding ->
            add(SigningBlock.Warning(SigningTone.Caution, s.t("componentsUi.funding.lead", mapOf("symbol" to funding.data.native_symbol))))
        }
        sign.error?.let { error ->
            val text = when (error.kind) {
                SignErrorKind.UnlimitedApproval -> s.a("unlimitedDisabled")
                SignErrorKind.UnsupportedChain -> s.t("send.lock.netNotFound")
                SignErrorKind.UserRejected, SignErrorKind.WalletSwitchedChains -> ""
                else -> s.t("send.txErrorGeneric")
            }
            if (text.isNotEmpty()) add(SigningBlock.Warning(SigningTone.Danger, text))
        }
        when {
            sign.pending_op_hash != null -> add(SigningBlock.Positive(s.s("submitted")))
            sign.is_signing || sign.is_submitting -> add(SigningBlock.Sentence(s.s("signing"), SigningTone.Neutral))
        }
    }

    fun blocks(clear: ClearSigningView, to: String?, valueHex: String?, dataBytes: Int, ctx: Context): List<SigningBlock> {
        val s = ctx.strings
        // A plain native transfer — no calldata — is the one transaction the
        // core resolves without a descriptor (`ReqKind::TxPlain`: resolved,
        // no result). The web shows its native transfer UI for it; drawing the
        // blind "unable to decode (0 bytes)" card here read a dust send as a
        // contract interaction (device-found).
        if (clear.resolved && clear.result == null && clear.message == null && clear.blind_typed == null && dataBytes == 0 && to != null) {
            return plainTransferBlocks(to, valueHex, ctx)
        }
        return blocksBySurface(clear, to, dataBytes, s)
    }

    private fun plainTransferBlocks(to: String, valueHex: String?, ctx: Context): List<SigningBlock> {
        val s = ctx.strings
        val wei = valueHex?.removePrefix("0x")?.ifEmpty { "0" }?.toBigIntegerOrNull(16) ?: java.math.BigInteger.ZERO
        return listOf(
            SigningBlock.Intent(s.s("intentSend"), SigningTone.Neutral),
            SigningBlock.Amount(AmountLine(sign = "−", value = SendLive.fromBase(wei.toString(), 18), symbol = ctx.nativeSymbol), card = true),
            SigningBlock.Party(s.s("recipientLabel"), ExploreLive.shortAddress(to), to),
        )
    }

    private fun blocksBySurface(clear: ClearSigningView, to: String?, dataBytes: Int, s: VelaStrings): List<SigningBlock> = when (clear.surface) {
        ClearSurface.None -> emptyList()
        ClearSurface.Loading -> listOf(SigningBlock.Sentence(s.s("loading"), SigningTone.Neutral))
        ClearSurface.ClearSign -> clear.result?.let { resultBlocks(it, s) }.orEmpty()
        ClearSurface.EthSign, ClearSurface.MessageSign -> clear.message?.let { messageBlocks(it, s) }.orEmpty()
        ClearSurface.BlindTypedData -> clear.blind_typed?.let { typed ->
            buildList {
                add(SigningBlock.Intent(typed.primary_type ?: s.s("signTypedData"), SigningTone.Caution))
                add(SigningBlock.Warning(SigningTone.Caution, s.s("blindTypedWarning")))
                if (typed.has_domain) add(SigningBlock.Party(s.s("signingFor"), typed.domain_name ?: s.s("unverifiedLabel"), typed.verifying_contract))
                if (typed.fields.isNotEmpty()) add(SigningBlock.Rows(typed.fields.map { SigningRow(it.key, it.value, SigningTone.Neutral, mono = true) }))
            }
        }.orEmpty()
        ClearSurface.BlindTransaction -> buildList {
            add(SigningBlock.Intent(s.s("intentContractCall"), SigningTone.Caution))
            add(SigningBlock.Warning(SigningTone.Caution, s.s("blindDecodeWarning", mapOf("bytes" to dataBytes.toString()))))
            to?.let { add(SigningBlock.Party(s.s("interactingLabel"), s.s("unverifiedLabel"), it, PartyBadge(s.s("unverifiedLabel"), SigningTone.Caution))) }
        }
    }

    private fun toneOf(risk: ClearRisk): SigningTone = when (risk) {
        ClearRisk.Safe -> SigningTone.Success
        ClearRisk.Normal -> SigningTone.Neutral
        ClearRisk.Caution -> SigningTone.Caution
        ClearRisk.Danger -> SigningTone.Danger
    }

    private fun resultBlocks(result: ClearSignResult, s: VelaStrings): List<SigningBlock> = buildList {
        add(SigningBlock.Intent(result.intent, toneOf(result.risk)))
        addAll(warnings(result, s))
        val rows = result.fields.filter { !it.detail }.map { rowOf(it, s) }
        if (rows.isNotEmpty()) add(SigningBlock.Rows(rows))
    }

    private fun warnings(result: ClearSignResult, s: VelaStrings): List<SigningBlock> = buildList {
        if (result.to_own_token) add(SigningBlock.Warning(SigningTone.Danger, s.s("tokenToContractWarning")))
        if (result.best_effort) add(SigningBlock.Warning(SigningTone.Caution, s.s("bestEffortWarning")))
        if (result.partial) add(SigningBlock.Warning(SigningTone.Caution, s.s("partialWarning")))
        if (result.fields.any { it.unverified }) add(SigningBlock.Warning(SigningTone.Caution, s.s("unverifiedWarning")))
        if (result.fields.any { it.expired }) add(SigningBlock.Warning(SigningTone.Caution, s.a("expired")))
    }

    private fun rowOf(field: ClearSignField, s: VelaStrings): SigningRow = SigningRow(
        label = field.label,
        value = field.value,
        valueTone = when {
            field.warning -> SigningTone.Danger
            field.unverified || field.expired -> SigningTone.Caution
            else -> SigningTone.Neutral
        },
        mono = field.address != null,
    )

    private fun messageBlocks(message: ClearMessageView, s: VelaStrings): List<SigningBlock> = buildList {
        val signingIn = message.siwe != null
        val danger = message.danger_class == ClearDangerClass.EthSign || message.danger_class == ClearDangerClass.SiwePhish
        add(SigningBlock.Intent(if (signingIn) s.s("signInIntent") else s.s("signMessage"), if (danger) SigningTone.Danger else SigningTone.Neutral))
        if (message.danger_class == ClearDangerClass.EthSign) add(SigningBlock.Sentence(s.s("ethSignBody"), SigningTone.Danger))
        message.decoded_text?.takeIf { it.isNotEmpty() }?.let { add(SigningBlock.Sentence(it, SigningTone.Neutral)) }
        message.binary_preview?.let { add(SigningBlock.Code(listOf(it))) }
        if (message.non_printable) add(SigningBlock.Warning(SigningTone.Caution, s.s("hexMessageWarning")))
        message.siwe?.let { siwe ->
            val rows = buildList {
                add(SigningRow(s.s("siweDomain"), siwe.domain_host ?: siwe.domain))
                siwe.statement?.let { add(SigningRow(s.s("siweStatement"), it)) }
                siwe.uri?.let { add(SigningRow(s.s("siweOrigin"), it)) }
            }
            add(SigningBlock.Rows(rows))
            when (message.binding) {
                ClearSiweBinding.Ok -> add(SigningBlock.Positive(s.s("siweOk")))
                ClearSiweBinding.Mismatch -> add(SigningBlock.Warning(SigningTone.Danger, s.s("siweMismatch")))
                else -> Unit
            }
        }
        if (message.danger_class == ClearDangerClass.EthSign) add(SigningBlock.Warning(SigningTone.Danger, s.s("ethSignWarning")))
    }

    /**
     * Spec 046 US1 — the one block a site cannot author: the simulated balance
     * changes as the trust machine judged them. Sent amounts render whenever
     * the token's symbol resolved; a received unverified token says so and
     * carries no confident figure (the asymmetric rule, spec 017 ⑥).
     */
    fun simBlocks(sim: SigningController.SimOutcome?, ctx: Context): List<SigningBlock> {
        val s = ctx.strings
        return when (sim) {
            null -> emptyList()
            SigningController.SimOutcome.Unavailable -> listOf(SigningBlock.Warning(SigningTone.Caution, s.s("simUnavailableWarning")))
            is SigningController.SimOutcome.Ready -> {
                if (sim.judgments.isEmpty()) {
                    return listOf(SigningBlock.Balances(s.s("balanceChangesTitle"), emptyList(), s.s("simResultNoChange")))
                }
                var unverified = false
                val rows = sim.judgments.map { judgment ->
                    when (judgment) {
                        is TrustSimJudgment.Native -> deltaRow(ctx.nativeSymbol, judgment.delta, 18)
                        is TrustSimJudgment.Erc20Trusted -> deltaRow(judgment.symbol, judgment.delta, judgment.decimals)
                        is TrustSimJudgment.Erc20Unverified -> {
                            unverified = true
                            BalanceDeltaRow(s.s("balanceUnverifiedToken"), signedRaw(judgment.delta), SigningTone.Caution)
                        }
                    }
                }
                listOf(
                    SigningBlock.Balances(
                        title = s.s("balanceChangesTitle"),
                        rows = rows,
                        note = if (unverified) s.s("unverifiedWarning") else null,
                        noteTone = if (unverified) SigningTone.Caution else SigningTone.Neutral,
                    ),
                )
            }
        }
    }

    private fun deltaRow(symbol: String, delta: String, decimals: Int): BalanceDeltaRow {
        val negative = delta.startsWith("-")
        val magnitude = SendLive.fromBase(delta.removePrefix("-"), decimals)
        return BalanceDeltaRow(symbol, (if (negative) "−" else "+") + magnitude, if (negative) SigningTone.Neutral else SigningTone.Success)
    }

    private fun signedRaw(delta: String): String = if (delta.startsWith("-")) "−" + delta.drop(1) else "+$delta"

    fun feeModel(clear: ClearSigningView, fee: FeeView, ctx: Context): FeeModel {
        val offChain = clear.result?.sign_type == ClearSignType.Signature || clear.surface == ClearSurface.MessageSign || clear.surface == ClearSurface.EthSign || clear.surface == ClearSurface.BlindTypedData
        if (offChain) return FeeModel.OffChain(ctx.strings.s("noNetworkFee"))
        val estimate = fee.fee
        val value = when {
            estimate != null -> "~${SendLive.fromBase(estimate.total_wei, 18)} ${ctx.nativeSymbol}"
            fee.failed != null -> ctx.strings.t("componentsUi.gas.estimateFailed")
            else -> ctx.strings.t("componentsUi.gas.estimating")
        }
        return FeeModel.OnChain(label = ctx.strings.t("componentsUi.gas.networkFee"), value = value)
    }

    /** The slide's verb: the core's intent id, in the corpus's words (the desktop's `confirm_label`). */
    fun confirmLabel(clear: ClearSigningView, s: VelaStrings): String = when (val confirm = clear.confirm) {
        ClearConfirm.Sign -> s.s("signLabel")
        ClearConfirm.Confirm -> s.s("confirmLabel")
        is ClearConfirm.ConfirmIntent -> when (confirm.intent) {
            "send" -> s.s("confirmSend")
            "swap" -> s.s("confirmSwap")
            "deposit" -> s.s("confirmDeposit")
            "withdraw" -> s.s("confirmWithdraw")
            else -> s.s("confirmLabel")
        }
    }
}
