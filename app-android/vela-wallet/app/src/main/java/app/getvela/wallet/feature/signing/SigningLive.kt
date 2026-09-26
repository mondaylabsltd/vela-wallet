package app.getvela.wallet.feature.signing

import app.getvela.wallet.feature.wallet.core.TrustSimJudgment
import androidx.compose.ui.graphics.Color
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.browser.ExploreLive
import app.getvela.wallet.feature.send.SendLive
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.send.core.FeeEstimateView
import app.getvela.wallet.feature.send.core.FeeView
import app.getvela.wallet.feature.signing.core.ClearConfirm
import app.getvela.wallet.feature.signing.core.ClearDangerClass
import app.getvela.wallet.feature.signing.core.ClearMessageView
import app.getvela.wallet.feature.signing.core.ClearProvenance
import app.getvela.wallet.feature.signing.core.ClearRisk
import app.getvela.wallet.feature.signing.core.ClearSignField
import app.getvela.wallet.feature.signing.core.ClearSignResult
import app.getvela.wallet.feature.signing.core.ClearSignType
import app.getvela.wallet.feature.signing.core.ClearSigningView
import app.getvela.wallet.feature.signing.core.ClearSiweBinding
import app.getvela.wallet.feature.signing.core.ClearSurface
import app.getvela.wallet.feature.signing.core.GuardAmountError
import app.getvela.wallet.feature.signing.core.GuardChoice
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
        /** The display currency the fee's "≈" half is written in (issue 201). */
        val money: WalletLive.Money = WalletLive.Money.dollars(),
        /** The page's host, for the SIWE verdict's words (spec 046). */
        val origin: String? = null,
        /** The request's chain, for its logo. */
        val chainId: Int = 0,
        /** Whether the fee row's coin list is open (issue #262). */
        val feeOpen: Boolean = false,
        /** Spec 071: the Trusted Signer's page is open for this request. */
        val trustedSignerWaiting: Boolean = false,
        /** Spec 071: why the last Trusted Signer attempt did not sign. */
        val trustedSignerNotice: String? = null,
    )

    /** The transport of a request the WALLET made of itself (`VelaWalletApplication`). */
    const val WALLET_TRANSPORT = "wallet"

    /**
     * Where a site's icon conventionally lives, best first. Https only: never
     * over plain http, where anybody on the path could answer with somebody
     * else's brand.
     */
    fun siteIconUrls(origin: String): List<String> {
        if (!origin.startsWith("https://")) return emptyList()
        val base = "https://" + origin.removePrefix("https://").substringBefore('/')
        return if (base.length <= "https://".length) emptyList() else listOf("$base/apple-touch-icon.png", "$base/favicon.ico")
    }

    /** Spec 071: the waiting card, while the Trusted Signer's page is open. */
    fun trustedSignerWait(ctx: Context): TrustedSignerWaitModel? {
        if (!ctx.trustedSignerWaiting) return null
        val s = ctx.strings
        return TrustedSignerWaitModel(
            title = s.s("trustedSignerWaiting"),
            hint = s.s("trustedSignerWaitingHint"),
            reopen = s.s("trustedSignerReopen"),
            cancel = s.t("common.cancel"),
        )
    }

    private fun VelaStrings.s(key: String) = t("componentsUi.signing.$key")
    private fun VelaStrings.s(key: String, vars: Map<String, String>) = t("componentsUi.signing.$key", vars)
    private fun VelaStrings.a(key: String) = t("componentsUi.signingApprove.$key")
    private fun VelaStrings.a(key: String, vars: Map<String, String>) = t("componentsUi.signingApprove.$key", vars)

    /** `registry_backup::REGISTRY` — the one contract the wallet's own backup request calls. */
    private const val PASSKEY_REGISTRY = "0x94fd1a891eb6c5f340622baf2f3a0cb70a941ea9"

    /**
     * The wallet's own key backup, in the person's language. The core's built-in
     * results are English, like the descriptors beside them ("the words stay in
     * the shell"); this one is OURS, and a Chinese sheet whose three most
     * important lines were English read as half-finished (founder, 2026-09-19).
     * Matched on the request being first-party AND the verified registry
     * address — never on the English words.
     */
    fun localizedOwnBackup(clear: ClearSigningView, own: Boolean, strings: VelaStrings): ClearSigningView {
        val result = clear.result ?: return clear
        if (!own || !result.verified || result.contract_address?.equals(PASSKEY_REGISTRY, ignoreCase = true) != true) return clear
        val labels = listOf("settingsModals.backup.registeredAs", "contacts.addressLabel", "settingsModals.backup.publicKeys").map(strings::t)
        return clear.copy(
            result = result.copy(
                intent = strings.t("settingsModals.backup.intent"),
                fields = result.fields.mapIndexed { index, field -> field.copy(label = labels.getOrElse(index) { field.label }) },
            ),
        )
    }

    /**
     * The cap the person chose, where the decode still says "Unlimited".
     *
     * The clear-signing result describes the REQUEST, and an unlimited
     * approve decodes as "Unlimited" in the danger tone. Once the guard holds a
     * finite choice for it (a cap, or revoke), that line would describe bytes
     * that are no longer the ones being signed: the approval's amount field
     * reads the cap instead and stops being a warning, and if it was the only
     * warning, the risk falls to what an approve is anyway — caution
     * (`clear_signing::assess_risk`). The same rule in every shell.
     */
    fun cappedApproval(clear: ClearSigningView, guard: GuardView): ClearSigningView {
        val result = clear.result ?: return clear
        val cap = capText(guard) ?: return clear
        val fields = result.fields.map { field ->
            if (field.warning && field.format == "tokenAmount") field.copy(value = cap, warning = false) else field
        }
        val risk = if (result.risk == ClearRisk.Danger && fields.none { it.warning }) ClearRisk.Caution else result.risk
        return clear.copy(result = result.copy(fields = fields, risk = risk))
    }

    /** A drawn chip's id, in the guard's vocabulary — the single card's and every leg's. */
    fun chipMode(id: String): GuardEditorMode? = when (id) {
        "requested" -> GuardEditorMode.Requested
        "balance" -> GuardEditorMode.Balance
        "custom" -> GuardEditorMode.Custom
        "revoke" -> GuardEditorMode.Revoke
        else -> null
    }

    /**
     * The guard's finite choice on an unlimited request, as the cap row prints
     * it — the single approval's, or a batch's FIRST leg's: a bundle decodes
     * from its first leg, so that is the line the decode's "Unlimited" sits on.
     */
    private fun capText(guard: GuardView): String? {
        val leg = guard.batch?.legs?.firstOrNull()
        val (detected, editor, meta) = when (guard.surface) {
            GuardSurface.ApprovalEditor -> Triple(guard.detected, guard.editor, guard.meta)
            GuardSurface.Batch -> Triple(leg?.approval, leg?.editor, leg?.meta ?: return null)
            else -> return null
        }
        if (detected?.is_unbounded != true || editor == null) return null
        if (editor.choice !is GuardChoice.Amount && editor.choice != GuardChoice.Revoke) return null
        val raw = editor.display_amount_raw?.toBigIntegerOrNull() ?: return null
        return "${SendLive.fromBase(raw.toString(), meta.decimals)} ${meta.symbol}".trim()
    }

    fun model(
        fallback: SigningScreenModel,
        request: IncomingRequest,
        sign: SignView,
        rawClear: ClearSigningView,
        guard: GuardView,
        fee: FeeView,
        ctx: Context,
        sim: SigningController.SimOutcome? = null,
        /** The sheet's speed control (spec 069); `null` draws the fee alone. */
        speed: SendLive.SpeedInputs? = null,
    ): SigningScreenModel {
        val s = ctx.strings
        val clear = cappedApproval(localizedOwnBackup(rawClear, request.transportId == WALLET_TRANSPORT, s), guard)
        val host = request.origin.substringAfter("://").substringBefore('/').ifBlank { request.origin }
        val facts = SigningController.firstCall(request.paramsJson)
        val dataBytes = facts?.second?.removePrefix("0x")?.length?.div(2) ?: 0
        // Spec 081: a refused request gets the refusal and nothing else. The
        // decoded body, the simulation and the guard all describe a
        // transaction that will never be signed, and reading them invites the
        // question "so why can't I?" — which the sentence above already answers.
        val refused = sign.blocked != null
        val blocks =
            if (refused) statusBlocks(sign, s)
            else statusBlocks(sign, s) + blocks(clear, facts?.first, facts?.third, dataBytes, ctx) +
                simBlocks(sim, ctx) + guardBlocks(guard, s)
        // The wallet's own request (the key backup) is not a site: its own mark
        // and name, and no host — "getvela.app" under a letter read as a stranger.
        val own = request.transportId == WALLET_TRANSPORT
        return fallback.copy(
            dappName = if (own) "Vela Wallet" else host,
            dappHost = if (own) "" else host,
            dappLetter = ExploreLive.letterOf(host),
            dappTint = ExploreLive.tintOf(host),
            dappOwn = own,
            dappIconUrls = if (own) emptyList() else siteIconUrls(request.origin),
            networkName = ctx.chainName,
            networkDot = ctx.chainDot,
            networkLogoUrl = app.getvela.wallet.core.marks.Marks.chainLogoUrl(ctx.chainId),
            trustedSignerWait = trustedSignerWait(ctx),
            trustedSignerNotice = ctx.trustedSignerNotice,
            blocks = blocks,
            tech = fallback.tech.copy(
                title = fallback.tech.title,
                summary = if (refused) null else clear.result?.contract_name,
                functionLabel = if (refused) null else clear.result?.let { s.s("techFunction") },
                signature = if (refused) null else clear.result?.intent,
                params = emptyList(),
                identities = emptyList(),
                simResult = null,
                rawLabel = if (!refused && dataBytes > 0) s.s("techRawData") else null,
                rawHex = if (refused) null else facts?.second?.takeIf { dataBytes > 0 },
            ),
            techOpen = false,
            // No fee and no confirm control under a refusal: a fee for a
            // transaction nobody will send is a number about nothing, and a
            // dead "Slide to confirm" reads as an option somebody merely
            // failed to use.
            fee = if (refused) null else feeModel(clear, fee, ctx, speed),
            signerLabel = s.s("signingAccount"),
            signerName = ctx.walletName,
            signerSeed = ctx.walletAddress,
            confirmHint = if (refused) null else s.s("slideToConfirm"),
            confirmAction = if (refused) null else confirmLabel(clear, s),
            confirmEnabled = !refused && confirmEnabled(sign, guard, fee, clear, speed),
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
            // Kept as the site asked (2026-09-26) — allowed, never unsaid.
            if (guard.editor?.choice == GuardChoice.Unlimited) add(SigningBlock.Warning(SigningTone.Danger, s.s("unlimitedWarning")))
        }
        GuardSurface.Batch -> buildList {
            guard.batch?.legs?.forEachIndexed { index, leg ->
                leg.editor?.let { editor -> add(allowanceBlock(editor, leg.meta, null, false, false, s, prefix = "#${index + 1} ", leg = index)) }
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
        leg: Int? = null,
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
            // An unlimited request opens HERE — the site's own bytes, kept
            // (Permit2 bundles revert when the wallet re-encodes the approve).
            chip("requested", s.a("requested"), GuardEditorMode.Requested, editor.requested_finite || editor.requested_unlimited),
            chip("balance", s.a("balanceCap"), GuardEditorMode.Balance, editor.has_balance_cap),
            chip("custom", s.a("custom"), GuardEditorMode.Custom, true),
            // Not on increaseAllowance: "revoke" would sign an increase of 0.
            chip("revoke", s.a("revoke"), GuardEditorMode.Revoke, editor.revoke_offered),
        )
        val value = editor.display_amount_raw?.toBigIntegerOrNull()?.let { units ->
            "${SendLive.fromBase(units.toString(), meta.decimals)} ${meta.symbol}".trim()
        } ?: s.a("unlimitedValue")
        val notes = buildList {
            if (decimalsUnverified) add(s.a("decimalsUnverified"))
            if (expired) add(s.a("expired"))
        }
        return SigningBlock.Allowance(
            leg = leg,
            label = prefix + s.a("spendingCap"),
            value = value,
            // Only a chosen, finite cap reads as settled; unlimited kept as
            // asked reads as the danger it is.
            valueTone = when (editor.choice) {
                null, GuardChoice.Unlimited -> SigningTone.Danger
                else -> SigningTone.Neutral
            },
            chips = chips,
            note = notes.takeIf { it.isNotEmpty() }?.joinToString("\n"),
            resultingTotal = increase?.let { total ->
                SigningRow(s.a("resultingTotal"), total.total ?: s.a("resultingTotalUnknown", mapOf("amount" to total.increment)))
            },
            custom = if (editor.mode == GuardEditorMode.Custom) {
                AllowanceInput(
                    value = editor.custom_text, symbol = meta.symbol, placeholder = "0",
                    error = when (editor.error) {
                        // A typed "cap" of 10^60 is no cap — an amount the field
                        // cannot take. Keeping the site's unlimited ask is the
                        // Requested chip, so "unlimited is disabled" would be false.
                        GuardAmountError.InvalidAmount, GuardAmountError.UnlimitedDisabled -> s.a("invalidAmount")
                        null -> null
                    },
                )
            } else {
                null
            },
        )
    }

    /**
     * The slide opens only when the request, the guard and the fee all say it
     * may — and the reading is in. The fee's say includes its SPEED: between a
     * tap and that speed's own figure landing, the core's `confirm_fee_ready`
     * is still true on the speed just left, and the slide must not sign it.
     */
    fun confirmEnabled(sign: SignView, guard: GuardView, fee: FeeView, clear: ClearSigningView, speed: SendLive.SpeedInputs? = null): Boolean {
        val feeReady = offChain(clear) || (fee.confirm_fee_ready && !ofAnotherTier(fee, speed))
        return sign.confirm_gate_open && guard.confirm_allowed && feeReady && !sign.is_signing && !sign.is_submitting
    }

    private fun offChain(clear: ClearSigningView): Boolean =
        clear.result?.sign_type == ClearSignType.Signature || clear.surface == ClearSurface.MessageSign ||
            clear.surface == ClearSurface.EthSign || clear.surface == ClearSurface.BlindTypedData

    /** NEVER ANOTHER TIER'S FIGURE WEARING THIS TIER'S NAME (issue 681). */
    private fun ofAnotherTier(fee: FeeView, speed: SendLive.SpeedInputs?): Boolean {
        val estimate = fee.fee ?: return false
        return speed != null && SendLive.offered(estimate.tier) != SendLive.offered(speed.view.tier)
    }

    fun statusBlocks(sign: SignView, s: VelaStrings): List<SigningBlock> = buildList {
        // Spec 081: the core refused this request outright — it would have
        // changed who controls the account. Nothing else on the sheet matters,
        // and `confirm_gate_open` is already false, so say it and stop.
        sign.blocked?.let { blocked ->
            add(SigningBlock.Intent(s.s("selfCallBlockedTitle"), SigningTone.Danger))
            val text = when {
                blocked.function == "SafeTx" -> s.s("selfCallBlockedSafeTx")
                blocked.leg_index != null -> s.s(
                    "selfCallBlockedLegBody",
                    mapOf("index" to blocked.leg_index.toString(), "function" to blocked.function),
                )
                else -> s.s("selfCallBlockedBody", mapOf("function" to blocked.function))
            }
            add(SigningBlock.Warning(SigningTone.Danger, text))
            return@buildList
        }
        // Founder, 2026-09-22: this surface means THE RELAY HAS NO GAS ON THIS
        // CHAIN. The `componentsUi.funding.*` line it used to show —
        // "your transactions run on a small fee reserve… later transactions
        // top it back up" — describes the retired per-wallet deposit, and was
        // false about whose money this is. The second line is the one the
        // surface never had: it is non-refundable and it goes to the bundler
        // operator, not to Vela.
        sign.funding?.let { _ ->
            add(SigningBlock.Warning(SigningTone.Caution, s.t("componentsUi.treasuryBootstrap.lead")))
            add(SigningBlock.Warning(SigningTone.Caution, s.t("componentsUi.treasuryBootstrap.disclaimer")))
        }
        sign.error?.let { error ->
            val text = when (error.kind) {
                // UnlimitedApproval falls to the plain sentence: since
                // 2026-09-26 it means the approval screen did not show the
                // unlimited approval — a wallet fault, not "unlimited is disabled".
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
        return blocksBySurface(clear, to, dataBytes, s, ctx.origin)
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

    private fun blocksBySurface(clear: ClearSigningView, to: String?, dataBytes: Int, s: VelaStrings, origin: String? = null): List<SigningBlock> = when (clear.surface) {
        ClearSurface.None -> emptyList()
        ClearSurface.Loading -> listOf(SigningBlock.Sentence(s.s("loading"), SigningTone.Neutral))
        ClearSurface.ClearSign -> clear.result?.let { resultBlocks(it, s) }.orEmpty()
        ClearSurface.EthSign, ClearSurface.MessageSign -> clear.message?.let { messageBlocks(it, s, origin) }.orEmpty()
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
        // Spec 081 FR-008: the descriptor service answered over plain HTTP,
        // from a base URL the person can edit, and nothing signed the answer.
        // The other sources say nothing here: built in and pinned are what
        // "verified" means, a token-standard shape is the standard doing its
        // job, the 4-byte database has its line above, and a deployment
        // claims nothing to doubt.
        if (result.provenance == ClearProvenance.Fetched) {
            add(SigningBlock.Warning(SigningTone.Caution, s.s("descriptorFetchedWarning")))
        }
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

    private fun messageBlocks(message: ClearMessageView, s: VelaStrings, origin: String? = null): List<SigningBlock> = buildList {
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
            val domain = siwe.domain_host ?: siwe.domain
            when (message.binding) {
                ClearSiweBinding.Ok -> add(SigningBlock.Positive(s.s("siweOk", mapOf("domain" to domain))))
                ClearSiweBinding.Mismatch -> add(SigningBlock.Warning(SigningTone.Danger, s.s("siweMismatch", mapOf("domain" to domain, "origin" to (origin ?: "")))))
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

    fun feeModel(clear: ClearSigningView, fee: FeeView, ctx: Context, speed: SendLive.SpeedInputs? = null): FeeModel {
        if (offChain(clear)) return FeeModel.OffChain(ctx.strings.s("noNetworkFee"))
        // For the moment between a speed being picked and its own figure
        // landing, the fee in hand is the previous speed's: "estimating".
        val estimate = fee.fee.takeIf { !ofAnotherTier(fee, speed) }
        val value = when {
            // The send screens' own line (issue 201): the coin that is ACTUALLY
            // paying — an in-band ERC-20 fee is its own amount under its own
            // ticker, never the native figure — and what it costs in money.
            // One formatter, because two surfaces pricing one operation must
            // not give two answers.
            estimate != null -> "~" + feeLine(estimate, fee, ctx)
            fee.failed != null -> ctx.strings.t("componentsUi.gas.estimateFailed")
            else -> ctx.strings.t("componentsUi.gas.estimating")
        }
        val choosable = fee.options.size > 1
        val selected = fee.options.firstOrNull { it.selected }
        // Issue #262: the core shut the gate because the coin that pays is not
        // there — the send form's own sentence (#211), about the same shortfall.
        val short = estimate != null && !fee.busy && fee.failed == null && !fee.confirm_fee_ready && selected?.insufficient == true
        val options = if (ctx.feeOpen && choosable) {
            fee.options.map { option ->
                FeeTokenOption(
                    id = option.contract ?: NATIVE_FEE_ID,
                    mark = TokenMark(option.symbol.take(1).uppercase(), ctx.chainDot),
                    name = option.symbol,
                    balance = "${ctx.strings.t("componentsUi.gas.rowBalance")} ${SendLive.fromBase(option.balance, option.decimals)}",
                    fee = option.amount?.let { "~${SendLive.feeFromBase(it, option.decimals)} ${option.symbol}" } ?: "—",
                    selected = option.selected,
                    disabled = option.insufficient,
                )
            }
        } else {
            emptyList()
        }
        return FeeModel.OnChain(
            label = ctx.strings.t("componentsUi.gas.networkFee"),
            value = value,
            selectorTitle = if (options.isEmpty()) null else ctx.strings.s("feeTokenTitle"),
            options = options,
            tappable = fee.failed != null || choosable,
            warning = if (short) ctx.strings.t("send.warnInsufficientGas", mapOf("sym" to selected!!.symbol)) else null,
            // Each option in the words its row would use, minus the "~".
            speed = speed?.let { inputs ->
                SendLive.speedModel(inputs, ctx.strings) { quote, view -> feeLine(quote, view ?: fee, ctx) }
            },
        )
    }

    private fun feeLine(estimate: FeeEstimateView, fee: FeeView, ctx: Context): String {
        val parts = SendLive.feeParts(estimate, ctx.nativeSymbol)
        return SendLive.feeLine(parts, SendLive.feePriceUsd(parts.contract, fee), ctx.money)
    }

    /** The fee list's id for the chain's own coin (the web's `'native'`). */
    const val NATIVE_FEE_ID = "native"

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
