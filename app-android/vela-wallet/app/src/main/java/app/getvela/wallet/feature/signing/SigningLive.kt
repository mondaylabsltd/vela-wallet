package app.getvela.wallet.feature.signing

import app.getvela.wallet.feature.wallet.core.TrustSimJudgment
import androidx.compose.ui.graphics.Color
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.browser.ExploreLive
import app.getvela.wallet.feature.send.SendLive
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.send.core.FeeEstimateView
import app.getvela.wallet.feature.send.core.FeeView
import app.getvela.wallet.feature.signing.core.ClearBatchCall
import app.getvela.wallet.feature.signing.core.ClearBatchView
import app.getvela.wallet.feature.signing.core.ClearConfirm
import app.getvela.wallet.feature.signing.core.ClearDangerClass
import app.getvela.wallet.feature.signing.core.ClearMessageView
import app.getvela.wallet.feature.signing.core.ClearNativeValue
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
import app.getvela.wallet.feature.signing.core.ConfirmState
import app.getvela.wallet.feature.send.core.FeeTier
import kotlinx.serialization.json.jsonPrimitive
import app.getvela.wallet.feature.signing.core.SignMethodKind
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.feature.flows.FlowHeaderModel
import app.getvela.wallet.feature.flows.ReceiptEtaModel
import app.getvela.wallet.feature.flows.ReceiptStage
import app.getvela.wallet.feature.flows.SendReceiptModel
import app.getvela.wallet.feature.signing.core.SignView
import app.getvela.wallet.feature.signing.core.SigningController
import app.getvela.wallet.feature.signing.core.SignPhase
import app.getvela.wallet.feature.signing.core.SignEndingState
import app.getvela.wallet.feature.send.core.TrackOutcome

/**
 * The signing sheet from the four machines' views (spec 044 T033; the
 * desktop's `signing/live.rs`). The drawn model keeps only its labels; the
 * dApp is the HOST (guessing a pretty name from a domain is exactly the
 * counterfeit route), the blocks are the core's reading, the fee is the
 * fee policy's, the confirm opens only when all three machines say so.
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
        /** Spec 079: back from the page with no answer, and its address does not answer. */
        val trustedSignerUnreachable: Boolean = false,
        /** Spec 102: a key ceremony's own title (a corpus key), `null` for a signature. */
        val trustedSignerTitle: String? = null,
        /** Spec 102: a key ceremony's key row (the core's `KeyLabel`), `null` for a signature. */
        val trustedSignerKey: app.getvela.wallet.feature.signing.trustedsigner.SigningPlan.KeyLabel? = null,
        /** Spec 071: why the last Trusted Signer attempt did not sign. */
        val trustedSignerNotice: String? = null,
        /** Spec 079: the chain's explorer base, for the landed receipt's link. */
        val explorerUrl: String? = null,
        /** Spec 079: the tracker's entry for the submitted operation — its clock (spec 099: `relay_sent_at_ms`) and its outcome. */
        val track: app.getvela.wallet.feature.send.core.TrackEntryView? = null,
        /** Spec 079: the chain's usual inclusion time (the core's table), for the receipt's ring. */
        val typicalS: Int? = null,
        /**
         * Spec 102 D4: this account reviews and signs on a page — the sheet is
         * the hand-off card (where, with which key, the page's integrity line)
         * and its Open goes there. `null`: in Vela.
         */
        val handoff: Handoff? = null,
        /** The number preset's wire name the signed deltas are written in (spec 082 RJ15). */
        val numberPreset: String = app.getvela.wallet.core.format.Formats.current.resolvedNumber().wire,
        /** The screen's clock, ms since the epoch: the core's landing pace is read at it (spec 099 R6). */
        val nowMs: () -> Double = { System.currentTimeMillis().toDouble() },
    )

    /**
     * Spec 102 D4: the page an account signs on, the key it confirms with (the
     * plan's `KeyLabel`; `null` when it names none), and that page's line now.
     */
    data class Handoff(
        val page: String,
        val key: app.getvela.wallet.feature.signing.trustedsigner.SigningPlan.KeyLabel?,
        val line: uniffi.vela_core_uniffi.SignerIntegrityLine,
    )

    /**
     * A key row in the person's words — the core's `KeyLabel`: the label its
     * `label_key` names ("Confirm with", "New key on"), the value the key's
     * name or its place. `null` when there is no value to draw.
     */
    fun keyRow(
        key: app.getvela.wallet.feature.signing.trustedsigner.SigningPlan.KeyLabel?,
        strings: VelaStrings,
    ): KeyRowModel? {
        key ?: return null
        val value = key.value(strings::t)
        return if (value.isBlank()) null else KeyRowModel(label = key.label(strings::t), value = value)
    }

    /**
     * The hand-off card's words (D4) — shared by the dApp sheet and a send's
     * own card. [fee] is the card's fee + speed row (`handoffFeeRow`), passed
     * only by a screen that shows no fee of its own; `null` — no row. A
     * self-hosted page whose check asks to be trusted carries the answer
     * (`settings.signing.pageTrust`) under its line.
     */
    fun handoffModel(handoff: Handoff, strings: VelaStrings, fee: HandoffFeeModel? = null): HandoffModel {
        val integrity = app.getvela.wallet.feature.settings.components.integrityModel(handoff.line, strings)
        return HandoffModel(
            title = strings.s("handoffTitle"),
            key = keyRow(handoff.key, strings),
            page = handoff.page.substringAfter("://").trimEnd('/'),
            integrity = integrity,
            open = strings.s("openSigner"),
            fee = fee,
            trust = strings.t("settings.signing.pageTrust").takeIf { integrity.asksToTrust },
        )
    }

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
        // Spec 079: the page never opened — say so, and the button is a retry.
        if (ctx.trustedSignerUnreachable) {
            return TrustedSignerWaitModel(
                title = s.s("signerDown"),
                hint = "",
                reopen = s.t("connect.browser.retry"),
                cancel = s.t("common.cancel"),
            )
        }
        return TrustedSignerWaitModel(
            title = s.s("trustedSignerWaiting"),
            // Spec 102: a ceremony says what it is doing there — create, sign
            // in, confirm (`trustedSignerCeremonyTitleKey`) — where a
            // signature says "check the request on the page and sign it
            // there": a ceremony has no request to check. Every shell's line.
            hint = ctx.trustedSignerTitle?.let(s::t) ?: s.s("trustedSignerWaitingHint"),
            reopen = s.s("trustedSignerReopen"),
            cancel = s.t("common.cancel"),
            // A ceremony names the key it makes or uses, as the page does.
            key = keyRow(ctx.trustedSignerKey, s),
        )
    }

    private fun VelaStrings.s(key: String) = t("componentsUi.signing.$key")
    private fun VelaStrings.s(key: String, vars: Map<String, String>) = t("componentsUi.signing.$key", vars)
    private fun VelaStrings.a(key: String) = t("componentsUi.signingApprove.$key")
    private fun VelaStrings.a(key: String, vars: Map<String, String>) = t("componentsUi.signingApprove.$key", vars)

    /**
     * The core's words in the reader's language. A clear-signing result is
     * English — a descriptor's intent and labels, the "Unlimited" a threshold
     * prints — and the core names the ones it recognises (`intent_term`,
     * `label_term`, `value_term`: the key leaf under `componentsUi.signing`).
     * Each named word is swapped for this locale's; anything unnamed stays as
     * the descriptor wrote it. Same rule in every shell; runs before
     * [cappedApproval]. The confirm is left alone: [confirmLabel] switches on
     * its English intent and falls back to the term.
     *
     * The wallet's own key backup is no exception: the core names its intent
     * and every row (Network, Address, Public keys), so it is translated here
     * like any other reading — never relabelled by position, which put the
     * wrong word on a row the moment the core added one.
     */
    fun localizedTerms(clear: ClearSigningView, strings: VelaStrings): ClearSigningView {
        fun word(term: String?, text: String): String {
            if (term == null) return text
            val key = "componentsUi.signing.$term"
            val translated = strings.t(key)
            return if (translated.isBlank() || translated == key) text else translated
        }
        fun localize(result: ClearSignResult) = result.copy(
            intent = word(result.intent_term, result.intent),
            fields = result.fields.map { field ->
                field.copy(label = word(field.label_term, field.label), value = word(field.value_term, field.value))
            },
        )
        return clear.copy(
            result = clear.result?.let(::localize),
            // 089 S1: every call of a batch, in the words a lone call gets.
            batch = clear.batch?.let { batch -> batch.copy(calls = batch.calls.map { it.copy(result = it.result?.let(::localize)) }) },
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
        fun capped(result: ClearSignResult, cap: String): ClearSignResult {
            val fields = result.fields.map { field ->
                if (field.warning && field.format == "tokenAmount") field.copy(value = cap, warning = false) else field
            }
            val risk = if (result.risk == ClearRisk.Danger && fields.none { it.warning }) ClearRisk.Caution else result.risk
            return result.copy(fields = fields, risk = risk)
        }
        // 089 S1: each call's "Unlimited" is replaced by ITS OWN leg's cap —
        // the guard's legs are the calls, in order.
        clear.batch?.let { batch ->
            val calls = batch.calls.mapIndexed { index, call ->
                val result = call.result ?: return@mapIndexed call
                val cap = capText(guard, index) ?: return@mapIndexed call
                val shown = capped(result, cap)
                // Capped, the call is what its decode now says — unless it burns.
                val risk = if (call.risk == ClearRisk.Danger && shown.risk != ClearRisk.Danger && !shown.to_own_token) shown.risk else call.risk
                call.copy(result = shown, risk = risk)
            }
            return clear.copy(batch = batch.copy(calls = calls, risk = calls.maxOfOrNull { it.risk } ?: batch.risk))
        }
        val result = clear.result ?: return clear
        val cap = capText(guard, 0) ?: return clear
        return clear.copy(result = capped(result, cap))
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
     * it — the single approval's, or batch leg [legIndex]'s: each call of a
     * batch carries its own decode, so each "Unlimited" takes its own leg's cap.
     */
    private fun capText(guard: GuardView, legIndex: Int): String? {
        val leg = guard.batch?.legs?.getOrNull(legIndex)
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
        /**
         * Spec 099 R7: the core's one gate over the four views as the sheet
         * received them ([confirmState] over their JSON). A caller holding only
         * the decoded views gets the core's answer over those, re-encoded.
         */
        confirm: ConfirmState = confirmState(sign, guard, rawClear, fee, speed),
    ): SigningScreenModel {
        val s = ctx.strings
        val clear = cappedApproval(localizedTerms(rawClear, s), guard)
        val host = request.origin.substringAfter("://").substringBefore('/').ifBlank { request.origin }
        val facts = SigningController.firstCall(request.paramsJson, request.method)
        val dataBytes = facts?.second?.removePrefix("0x")?.length?.div(2) ?: 0
        // 089 S1: a batch's technical details are the whole batch, never call 1's calldata.
        val wholeBatch = clear.surface == ClearSurface.Batch
        // Spec 081: a refused request gets the refusal and nothing else. The
        // decoded body, the simulation and the guard all describe a
        // transaction that will never be signed, and reading them invites the
        // question "so why can't I?" — which the sentence above already answers.
        val refused = sign.blocked != null
        // The wallet's own request (the key backup) is not a site, and the core
        // says so (`first_party`, set in the one place the backup is raised) —
        // never this sheet, from bytes or an origin any page could send.
        val own = sign.request?.first_party == true
        val sims = simBlocks(sim, ctx)
        // Issue #314: on the wallet's own request a simulation that moves
        // nothing only confirms what the wallet itself wrote — a technical
        // fact, folded with the others, not a bordered card weighing as much as
        // the outcome. Anything else it has to say (a revert, a node that could
        // not check, a balance that would move) stays on the sheet.
        val quietSim = (sims.singleOrNull() as? SigningBlock.Balances)?.takeIf { own && it.rows.isEmpty() }
        // Spec 102 D4: a page venue's sheet does not repeat the preview — the
        // page is the authority. What stays is what only Vela can decide
        // before it hands off: an approval's amount (the guard), and the fee.
        val handoff = ctx.handoff?.takeIf { !refused }
        val drawn =
            if (refused) statusBlocks(sign, s)
            else if (handoff != null) statusBlocks(sign, s, ctx.trustedSignerWaiting) + guardBlocks(guard, s)
            else statusBlocks(sign, s, ctx.trustedSignerWaiting) + blocks(clear, facts?.first, dataBytes, ctx) +
                (if (quietSim != null) emptyList() else sims) + guardBlocks(guard, s)
        val hidePreview = refused || handoff != null
        // The wallet's own request leads with what it does, as the header's
        // title beside the ✕ — so that intent is not said a second time under it.
        val lead = if (own) drawn.indexOfFirst { it is SigningBlock.Intent } else -1
        val headline = (drawn.getOrNull(lead) as? SigningBlock.Intent)?.text
        val blocks = if (lead >= 0) drawn.filterIndexed { index, _ -> index != lead } else drawn
        // Spec 082 RE7: the name and whether the host is said again are the
        // core's (`browserSiteLabel`); a request carries no page title, so a
        // site is named by its host, once.
        val label = if (own) null else uniffi.vela_core_uniffi.browserSiteLabel("", host)
        return fallback.copy(
            dappName = label?.name.orEmpty(),
            dappHost = label?.hostLine.orEmpty(),
            dappLetter = ExploreLive.letterOf(host),
            dappTint = ExploreLive.tintOf(host),
            dappOwn = own,
            headline = headline,
            dappIconUrls = if (own) emptyList() else siteIconUrls(request.origin),
            networkName = ctx.chainName,
            networkDot = ctx.chainDot,
            networkLogoUrl = app.getvela.wallet.core.marks.Marks.chainLogoUrl(ctx.chainId),
            trustedSignerWait = trustedSignerWait(ctx),
            trustedSignerNotice = ctx.trustedSignerNotice,
            handoff = handoff?.let { handoffModel(it, s) },
            blocks = blocks,
            tech = fallback.tech.copy(
                title = fallback.tech.title,
                // "· Vela passkey registry" names a contract to the person who
                // asked for nothing but their own backup: not on their own request.
                summary = if (hidePreview || own) null else clear.result?.contract_name,
                functionLabel = if (hidePreview) null else clear.result?.let { s.s("techFunction") },
                signature = if (hidePreview) null else clear.result?.intent,
                params = emptyList(),
                identities = emptyList(),
                simResult = quietSim?.takeIf { !hidePreview }?.let { SigningRow(s.s("simResultLabel"), it.note ?: s.s("simResultNoChange")) },
                rawLabel = if (!hidePreview && (dataBytes > 0 || wholeBatch)) s.s("techRawData") else null,
                rawHex = when {
                    hidePreview -> null
                    wholeBatch -> request.paramsJson
                    else -> facts?.second?.takeIf { dataBytes > 0 }
                },
            ),
            techOpen = false,
            // No fee and no confirm control under a refusal: a fee for a
            // transaction nobody will send is a number about nothing, and a
            // dead confirm reads as an option somebody merely failed to use.
            fee = if (refused) null else feeModel(clear, fee, ctx, speed),
            signerLabel = s.s("signingAccount"),
            signerName = ctx.walletName,
            signerSeed = ctx.walletAddress,
            confirmAction = if (refused) null else confirmLabel(clear, s),
            // D4: Open is enabled only when the page's check says it opens.
            confirmEnabled = !refused && confirm.enabled && (handoff == null || handoff.line.opens),
            // Spec 099 R7: a shut confirm says why, in the core's line for the
            // part that is shut — none under a refusal, which has no confirm.
            confirmBlockLine = confirm.key?.takeIf { !refused && !confirm.enabled }?.let { s.t(it) },
            panelTitle = s.s("signatureRequest"),
            closeLabel = s.t(I18nKeys.Flow.CLOSE),
            receipt = if (refused) null else receipt(sign, blocks, ctx),
            requestKey = request.id,
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
            // An unlimited permit is said like any unlimited approval (spec 094 S8).
            if (guard.unlimited_warning) add(SigningBlock.Warning(SigningTone.Danger, s.s("unlimitedWarning")))
            add(SigningBlock.Warning(SigningTone.Danger, s.a("permitCantCap")))
        }
        GuardSurface.ApprovalEditor -> buildList {
            guard.editor?.let { editor -> add(allowanceBlock(editor, guard.meta, guard.increase_total, guard.decimals_unverified, guard.expired, s)) }
            guard.detected?.let { add(SigningBlock.Party(s.a("spenderLabel"), ExploreLive.shortAddress(it.spender), it.spender)) }
            // Kept as the site asked (2026-09-26) — allowed, never unsaid; the core decides when (094 S8).
            if (guard.unlimited_warning) add(SigningBlock.Warning(SigningTone.Danger, s.s("unlimitedWarning")))
        }
        GuardSurface.Batch -> buildList {
            guard.batch?.legs?.forEachIndexed { index, leg ->
                leg.editor?.let { editor -> add(allowanceBlock(editor, leg.meta, null, false, false, s, prefix = "#${index + 1} ", leg = index)) }
                leg.approval?.let { add(SigningBlock.Party(s.a("spenderLabel"), ExploreLive.shortAddress(it.spender), it.spender)) }
            }
            if (guard.unlimited_warning) add(SigningBlock.Warning(SigningTone.Danger, s.s("unlimitedWarning")))
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
     * May the confirm arm, and if not why (spec 099 R7) — the core's one gate,
     * `sign_confirm::confirm_state`, over the sign, guard, clear-signing and
     * fee views exactly as the sheet received them (JSON; [feeJson] `null`
     * with no fee session) and the speed in force. It holds every rule this
     * sheet used to AND itself: the request's own gate, nothing in flight,
     * the reading in (096 F7), the approval chosen, a message has no fee to
     * wait for, another speed's figure is not this speed's (issue 681), the
     * fee priced and its coin not short. A view that does not read keeps the
     * confirm shut — never a guess.
     */
    fun confirmState(signJson: String?, guardJson: String?, clearJson: String?, feeJson: String?, speedTier: FeeTier?): ConfirmState {
        if (signJson == null || guardJson == null || clearJson == null) return ConfirmState()
        val tier = speedTier?.let { app.getvela.wallet.core.crux.Wire.json.encodeToJsonElement(FeeTier.serializer(), it).jsonPrimitive.content }
        val json = uniffi.vela_core_uniffi.signConfirmState(signJson, guardJson, clearJson, feeJson, tier) ?: return ConfirmState()
        return runCatching { app.getvela.wallet.core.crux.Wire.json.decodeFromString(ConfirmState.serializer(), json) }
            .getOrDefault(ConfirmState())
    }

    /**
     * [confirmState] over decoded views, re-encoded — for a caller that holds
     * no raw JSON (the gallery, a test). The live sheet passes the views as
     * the machines wrote them: a mirror that dropped a field the core needs
     * would keep the confirm shut, never open it.
     */
    fun confirmState(sign: SignView, guard: GuardView, clear: ClearSigningView, fee: FeeView, speed: SendLive.SpeedInputs?): ConfirmState {
        val wire = app.getvela.wallet.core.crux.Wire.json
        return confirmState(
            wire.encodeToString(SignView.serializer(), sign),
            wire.encodeToString(GuardView.serializer(), guard),
            wire.encodeToString(ClearSigningView.serializer(), clear),
            wire.encodeToString(FeeView.serializer(), fee),
            speed?.view?.tier,
        )
    }

    /**
     * The fee ROW's words only — a message says "no network fee" (the core's
     * `sign_confirm::off_chain`, which no export carries yet). Whether the
     * confirm arms is [confirmState]'s, never this.
     */
    private fun offChain(clear: ClearSigningView): Boolean =
        clear.result?.sign_type == ClearSignType.Signature || clear.surface == ClearSurface.MessageSign ||
            clear.surface == ClearSurface.EthSign || clear.surface == ClearSurface.BlindTypedData

    /**
     * The fee ROW's figure only: NEVER ANOTHER TIER'S FIGURE WEARING THIS
     * TIER'S NAME (issue 681) — the row says "estimating" meanwhile. The gate's
     * own copy of this rule is the core's ([confirmState]).
     */
    private fun ofAnotherTier(fee: FeeView, speed: SendLive.SpeedInputs?): Boolean {
        val estimate = fee.fee ?: return false
        return speed != null && SendLive.offered(estimate.tier) != SendLive.offered(speed.view.tier)
    }

    /**
     * Spec 079: after the approval the sheet stops being a form — the owner saw
     * nothing change after the fingerprint ("可信签名器签完后，回到签名提示框，
     * 似乎没有任何提示"). This is the send receipt's own model and words, so a
     * dApp transaction lands exactly as a send does: signing → submitting →
     * submitted with the chain's clock → (the core closes the sheet, and the
     * aftercare sheet shows the tick). `null` while the request is still a
     * request.
     */
    fun receipt(sign: SignView, blocks: List<SigningBlock>, ctx: Context): SendReceiptModel? {
        val s = ctx.strings
        val summary = summaryOf(blocks)
        val header = FlowHeaderModel(title = "", backLabel = "")
        val closeBackground = s.t(I18nKeys.Flows.TX_CLOSE_BACKGROUND)
        // A message never goes to the network: it is signing, then signed —
        // never "submitting" (device-found on the Xiaomi, spec 079).
        val onChain = sign.request?.kind.let { it == null || it == SignMethodKind.Transaction || it == SignMethodKind.Batch }
        if (!onChain && sign.phase != SignPhase.Idle) {
            return SendReceiptModel(
                header = header,
                stage = ReceiptStage.Submitting,
                title = s.s("signing"),
                captions = listOfNotNull(summary),
                cta = s.t(I18nKeys.Flow.CLOSE),
                ctaAccent = false,
            )
        }
        return when {
            // A refusal after the approval (the submission failed): the core's
            // reason, the sheet's own sentence for it.
            // PR 2 polish: the relay turned the submit back because the
            // account's previous transaction on this network still holds the
            // nonce. Nothing was sent and nothing went wrong — "Not sent
            // yet", calmly (a still clock, never the failure's red), over
            // the core's sentence, with Try again beside Done.
            sign.failure_not_sent && sign.error != null && sign.error.kind != SignErrorKind.UserRejected ->
                SendReceiptModel(
                    header = header,
                    stage = ReceiptStage.NotSent,
                    title = s.t(I18nKeys.Flows.NOT_SENT_TITLE),
                    captions = listOfNotNull(summary, failureWords(sign, s)),
                    cta = s.t(I18nKeys.Flows.DONE),
                    ctaAccent = !sign.failure_retryable,
                    retry = if (sign.failure_retryable) s.t(I18nKeys.Flows.TX_RETRY) else null,
                )
            sign.error != null && sign.error.kind != SignErrorKind.UserRejected && (sign.pending_op_hash != null || sign.error.kind in SUBMIT_FAILURES) ->
                SendReceiptModel(
                    header = header,
                    stage = ReceiptStage.Failed,
                    title = s.t(I18nKeys.Flows.STATUS_FAILED),
                    // Spec 082 RJ3: a relay refusal is "refused, nothing was
                    // sent" — never "try again": it would be refused again.
                    captions = listOfNotNull(summary, failureWords(sign, s)),
                    cta = s.t(I18nKeys.Flows.DONE),
                    // Spec 096 F8: the core holds the page's answer until this
                    // closes; a failure that sent nothing may be tried again.
                    ctaAccent = !sign.failure_retryable,
                    retry = if (sign.failure_retryable) s.t(I18nKeys.Flows.TX_RETRY) else null,
                )
            // Spec 082 RA10: the relay's reply was lost. "Submitting…", it may
            // have been sent, Vela keeps checking — the op hash, and a close
            // that keeps it running. Never "failed", never a Retry: a second
            // attempt could pay twice. Once the relay shows it holds the op,
            // the ordinary words below take over.
            sign.pending_op_hash != null && maybeSentNow(sign, ctx) -> SendReceiptModel(
                header = header,
                stage = ReceiptStage.Submitting,
                title = s.t(I18nKeys.Flows.TX_SUBMITTING),
                captions = listOfNotNull(summary, s.t(I18nKeys.Flows.SIGN_MAYBE_SENT)),
                hash = opHashRow(sign.pending_op_hash, s),
                cta = closeBackground,
                ctaAccent = false,
            )
            sign.pending_op_hash != null -> {
                val track = ctx.track?.takeIf { it.user_op_hash.equals(sign.pending_op_hash, ignoreCase = true) }
                val still = track?.outcome == app.getvela.wallet.feature.send.core.TrackOutcome.StillConfirming
                // Spec 099 R6: the core's one countdown, from the relay's send.
                val clock = landingClock(track, ctx).takeIf { !still }
                SendReceiptModel(
                    header = header,
                    stage = ReceiptStage.Submitted,
                    title = s.t(I18nKeys.Flows.TX_SUBMITTED_TITLE),
                    captions = listOfNotNull(
                        summary,
                        when {
                            still -> s.s("stillConfirming")
                            clock?.waiting == true -> s.t(I18nKeys.Flows.TX_RELAY_SENDING)
                            else -> s.t(I18nKeys.Flows.TX_WAITING_CONFIRM)
                        },
                    ),
                    cta = closeBackground,
                    ctaAccent = false,
                    eta = clock?.eta,
                )
            }
            // Spec 082 RA9: the words are the core's phase — never "waiting
            // for your signature" through the pre-check and the relay's
            // estimate, which is what shell flags used to say.
            sign.phase == SignPhase.Submitting -> SendReceiptModel(
                header = header,
                stage = ReceiptStage.Submitting,
                title = s.t(I18nKeys.Flows.TX_SUBMITTING),
                captions = listOfNotNull(summary, s.t(I18nKeys.Flows.TX_BACKGROUND_HINT)),
                cta = closeBackground,
                ctaAccent = false,
            )
            // The passkey is up (or the trusted signer's page is — its own
            // waiting card wins over this, see the sheet).
            sign.phase == SignPhase.AwaitingSignature -> SendReceiptModel(
                header = header,
                stage = ReceiptStage.Submitting,
                title = s.t(I18nKeys.Flows.TX_SIGNING),
                captions = listOfNotNull(summary),
                cta = s.t(I18nKeys.Flow.CLOSE),
                ctaAccent = false,
            )
            sign.phase == SignPhase.Preparing -> SendReceiptModel(
                header = header,
                stage = ReceiptStage.Submitting,
                title = s.t(I18nKeys.Flows.TX_PREPARING),
                captions = listOfNotNull(summary),
                cta = s.t(I18nKeys.Flow.CLOSE),
                ctaAccent = false,
            )
            else -> null
        }
    }

    /** The error kinds that end a request after its approval: the receipt's failure, not the form's. */
    private val SUBMIT_FAILURES = setOf(
        SignErrorKind.SubmitFailed,
        // Spec 099 R8: the passkey failed — nothing was signed; a prompt that
        // failed may be tried again (the core's `failure_retryable`).
        SignErrorKind.SignerUnavailable,
        SignErrorKind.SignerNotDiscoverable,
        SignErrorKind.SignerFailed,
        // Spec 102: the venue cannot be used here — nothing was signed, and
        // the core says why (`venue_block`); not retryable.
        SignErrorKind.VenueBlocked,
    )

    /** The landing's clock for one operation: [waiting] — the relay has not sent it; [eta] — the count, when there is one. */
    private class LandingClock(val waiting: Boolean, val eta: ReceiptEtaModel?)

    /**
     * Spec 099 R6: the core's `landing_pace` for the tracker's entry, counted
     * from when the relay sent the operation (`relay_sent_at_ms`) against the
     * chain's usual time. `waiting` until the relay has sent it — its words,
     * no chain clock, the ring roams; a clock once it has and the chain has a
     * usual time.
     */
    private fun landingClock(track: app.getvela.wallet.feature.send.core.TrackEntryView?, ctx: Context): LandingClock {
        val s = ctx.strings
        val sentAt = track?.relay_sent_at_ms
        val pace = app.getvela.wallet.feature.send.core.Landing.pace(sentAt, ctx.typicalS, ctx.nowMs())
        val counting = pace.line != app.getvela.wallet.feature.send.core.LandingLine.Waiting &&
            pace.line != app.getvela.wallet.feature.send.core.LandingLine.None
        val eta = if (counting && sentAt != null && ctx.typicalS != null) {
            ReceiptEtaModel(
                sentAtMs = sentAt,
                typicalS = ctx.typicalS,
                typicalLine = s.t(I18nKeys.Flows.TX_TYPICAL_TIME, mapOf("chainName" to ctx.chainName, "estSecs" to ctx.typicalS.toString())),
                remainingTemplate = s.t(I18nKeys.Flows.TX_REMAINING),
                elapsedTemplate = s.t(I18nKeys.Flows.TX_ELAPSED),
                slowLine = s.t(I18nKeys.Flows.TX_SLOW_CONFIRM),
            )
        } else {
            null
        }
        return LandingClock(waiting = pace.line == app.getvela.wallet.feature.send.core.LandingLine.Waiting, eta = eta)
    }

    /**
     * The lost reply is still unanswered: the machine says the pending op's
     * reply was lost, and the tracker has not seen the relay hold it yet (no
     * entry yet reads the same — the tracker has not taken it).
     */
    private fun maybeSentNow(sign: SignView, ctx: Context): Boolean {
        val track = ctx.track?.takeIf { it.user_op_hash.equals(sign.pending_op_hash, ignoreCase = true) }
        return track?.outcome == TrackOutcome.MaybeSent || (track == null && sign.pending_op_maybe_sent)
    }

    /** The operation's hash, short, copyable — what a person can quote while it may have been sent. */
    private fun opHashRow(hash: String, s: VelaStrings) = app.getvela.wallet.feature.flows.ReceiptHashModel(
        label = s.t(I18nKeys.Flows.TX_HASH),
        value = "${hash.take(10)}…${hash.takeLast(8)}",
        copyLabel = s.t(I18nKeys.Flows.COPY_ADDRESS),
        copyValue = hash,
    )

    /**
     * Spec 079: the ending of a request whose sheet the core has closed — the
     * same receipt, with the tracker's word for an operation still on its way
     * (never "failed" on time alone: a timeout is not a failure).
     *
     * Spec 082 RA8/RA10: what the ending stands for is the core's
     * (`sign_ending_state` over the tracker's entry). Confirmed only when the
     * tracker saw it land; a revert is "failed" with its hint and the
     * explorer; "not sent" is the plain failure; a lost reply still unanswered
     * reads "may have been sent", with no Retry.
     */
    fun aftercareReceipt(
        aftercare: SigningAftercare,
        summary: String?,
        ctx: Context,
    ): SendReceiptModel = aftercareReceipt(aftercare.state(ctx.track), summary, ctx)

    fun aftercareReceipt(
        state: SignEndingState,
        summary: String?,
        ctx: Context,
    ): SendReceiptModel {
        val s = ctx.strings
        val header = FlowHeaderModel(title = "", backLabel = "")
        fun hashRow(txHash: String?) = txHash?.takeIf { it.isNotBlank() }?.let {
            app.getvela.wallet.feature.flows.ReceiptHashModel(
                label = s.t(I18nKeys.Flows.TX_HASH),
                value = "${it.take(10)}…${it.takeLast(8)}",
                copyLabel = s.t(I18nKeys.Flows.COPY_ADDRESS),
                copyValue = it,
            )
        }
        fun explorer(txHash: String?) =
            ctx.explorerUrl?.takeIf { !txHash.isNullOrBlank() && it.isNotBlank() }?.let { s.t(I18nKeys.Flows.VIEW_ON_EXPLORER) }
        return when (state) {
            SignEndingState.Signed -> SendReceiptModel(
                header = header,
                stage = ReceiptStage.Confirmed,
                // "已签名！" — `signHandoff.signed` reads "已发送" in zh, which a
                // message that went nowhere is not.
                title = s.t("clearSigning.alertSignedTitle"),
                captions = listOfNotNull(summary),
                cta = s.t(I18nKeys.Flows.DONE),
                ctaAccent = true,
            )
            is SignEndingState.Confirmed -> SendReceiptModel(
                header = header,
                stage = ReceiptStage.Confirmed,
                title = s.t("componentsTx.receipt.statusConfirmed"),
                captions = listOfNotNull(summary, ctx.chainName.takeIf { it.isNotBlank() }),
                hash = hashRow(state.tx_hash),
                viewOnExplorer = explorer(state.tx_hash),
                cta = s.t(I18nKeys.Flows.DONE),
                ctaAccent = true,
            )
            // It landed and reverted: gas was spent, nothing else happened.
            is SignEndingState.Reverted -> SendReceiptModel(
                header = header,
                stage = ReceiptStage.Failed,
                title = s.t(I18nKeys.Flows.STATUS_FAILED),
                captions = listOfNotNull(summary, s.t(I18nKeys.Flows.TX_FAILED_HINT)),
                hash = hashRow(state.tx_hash),
                viewOnExplorer = explorer(state.tx_hash),
                cta = s.t(I18nKeys.Flows.DONE),
                ctaAccent = true,
            )
            // Spec 082 RJ3: the relay refused it — nothing was sent, and the
            // same request would be refused again: no Retry words.
            // A refusal is told by its reason: the tracker entry's
            // `refusal_key` (the fee words only for a fee refusal, "another
            // went first" for a used nonce), else the plain "refused".
            SignEndingState.Refused -> SendReceiptModel(
                header = header,
                stage = ReceiptStage.Failed,
                title = s.t(I18nKeys.Flows.STATUS_FAILED),
                captions = listOfNotNull(summary, refusedWords(ctx.track, s)),
                cta = s.t(I18nKeys.Flows.DONE),
                ctaAccent = true,
            )
            // The relay never had it: nothing was sent, and "try again" is true.
            SignEndingState.NotSent -> SendReceiptModel(
                header = header,
                stage = ReceiptStage.Failed,
                title = s.t(I18nKeys.Flows.STATUS_FAILED),
                captions = listOfNotNull(summary, s.t(I18nKeys.Flows.TX_ERROR_GENERIC)),
                cta = s.t(I18nKeys.Flows.DONE),
                ctaAccent = true,
            )
            is SignEndingState.Following -> when (state.outcome) {
                TrackOutcome.MaybeSent -> SendReceiptModel(
                    header = header,
                    stage = ReceiptStage.Submitting,
                    title = s.t(I18nKeys.Flows.TX_SUBMITTING),
                    captions = listOfNotNull(summary, s.t(I18nKeys.Flows.SIGN_MAYBE_SENT)),
                    hash = opHashRow(state.user_op_hash, s).takeIf { state.user_op_hash.isNotBlank() },
                    cta = s.t(I18nKeys.Flows.TX_CLOSE_BACKGROUND),
                    ctaAccent = false,
                )
                // The ring: it is on its way inside the chain's usual window —
                // counted (spec 099 R6) from when the relay sent it.
                TrackOutcome.Landing -> {
                    val clock = landingClock(ctx.track?.takeIf { it.user_op_hash.equals(state.user_op_hash, ignoreCase = true) }, ctx)
                    SendReceiptModel(
                        header = header,
                        stage = ReceiptStage.Submitted,
                        title = s.t(I18nKeys.Flows.TX_SUBMITTED_TITLE),
                        captions = listOfNotNull(
                            summary,
                            when {
                                state.fee_held -> s.t(I18nKeys.Flows.TX_HELD_FEES)
                                // The relay holds it while it tops up its gas (098 follow-up).
                                state.relay_funding -> s.t(I18nKeys.Flows.TX_RELAY_FUNDING)
                                clock.waiting -> s.t(I18nKeys.Flows.TX_RELAY_SENDING)
                                else -> s.t(I18nKeys.Flows.TX_WAITING_CONFIRM)
                            },
                        ),
                        cta = s.t(I18nKeys.Flows.TX_CLOSE_BACKGROUND),
                        ctaAccent = false,
                        // Nothing is on the network while the relay funds itself: no clock.
                        eta = clock.eta.takeIf { !state.relay_funding },
                    )
                }
                TrackOutcome.StillConfirming, TrackOutcome.Unknown, TrackOutcome.Final -> SendReceiptModel(
                    header = header,
                    stage = ReceiptStage.Submitted,
                    title = s.t(I18nKeys.Flows.TX_SUBMITTED_TITLE),
                    captions = listOfNotNull(
                        summary,
                        when {
                            state.fee_held -> s.t(I18nKeys.Flows.TX_HELD_FEES)
                            state.outcome == TrackOutcome.Unknown -> s.s("unknownOutcome")
                            state.relay_funding -> s.t(I18nKeys.Flows.TX_RELAY_FUNDING)
                            else -> s.s("stillConfirming")
                        },
                    ),
                    cta = s.t(I18nKeys.Flows.TX_CLOSE_BACKGROUND),
                    ctaAccent = false,
                )
            }
        }
    }

    /**
     * The request in one line, from the blocks the sheet already drew: what it
     * is ("发送", "授权") and its figure — so the receipt still says WHAT is
     * landing once the form has gone.
     */
    fun summaryOf(blocks: List<SigningBlock>): String? {
        val intent = blocks.firstNotNullOfOrNull { (it as? SigningBlock.Intent)?.text?.takeIf(String::isNotBlank) }
        val figure = blocks.firstNotNullOfOrNull { block ->
            when (block) {
                is SigningBlock.Amount -> "${block.line.sign}${block.line.value} ${block.line.symbol}".trim()
                is SigningBlock.Swap -> "${block.pay.value} ${block.pay.symbol} → ${block.receive.value} ${block.receive.symbol}"
                else -> null
            }
        }
        return listOfNotNull(intent, figure).joinToString(" · ").ifBlank { null }
    }

    /** [signerPageOpen]: the Trusted Signer's waiting card speaks for the signature (spec 079 — "签名中…" above "签名页没能打开" contradicted it). */
    fun statusBlocks(sign: SignView, s: VelaStrings, signerPageOpen: Boolean = false): List<SigningBlock> = buildList {
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
                else -> failureWords(sign, s)
            }
            when {
                text.isEmpty() -> Unit
                // "Not sent yet" (PR 2 polish) is no failure: its sentence is
                // said in the sheet's neutral voice, never as a warning.
                sign.failure_not_sent -> add(SigningBlock.Sentence(text, SigningTone.Neutral))
                else -> add(SigningBlock.Warning(SigningTone.Danger, text))
            }
        }
        when {
            sign.pending_op_hash != null -> add(SigningBlock.Positive(s.s("submitted")))
            signerPageOpen -> Unit
            // Spec 082 RA9: the core's phase — the pre-check and the relay's
            // estimate are "preparing", never "signing".
            sign.phase == SignPhase.Preparing -> add(SigningBlock.Sentence(s.t(I18nKeys.Flows.TX_PREPARING), SigningTone.Neutral))
            sign.phase != SignPhase.Idle -> add(SigningBlock.Sentence(s.s("signing"), SigningTone.Neutral))
        }
    }

    /**
     * A refusal's sentence, by its reason: the tracker entry's `refusal_key`
     * when it has one, else the plain `componentsUi.signing.refused`.
     */
    private fun refusedWords(track: app.getvela.wallet.feature.send.core.TrackEntryView?, s: VelaStrings): String =
        track?.refusal_key?.takeIf { it.isNotBlank() }?.let { s.t(it) } ?: s.t(I18nKeys.Flows.SIGN_REFUSED)

    /**
     * The failure's sentence. A refusal is told by its reason — the core's
     * `failure_refusal_key` (PR 2 note 9), the one field for a refusal at
     * submit (another operation of the account holds the nonce, with Try
     * again) and one the tracker reported after it (the entry's `refusal`,
     * worded as its `refusal_key` is). Without one, a refusal (spec 082 RJ3)
     * is `componentsUi.signing.refused` — nothing was sent, and no Retry
     * words; anything else is the plain "not submitted, try again".
     */
    private fun failureWords(sign: SignView, s: VelaStrings): String = when {
        sign.failure_refusal_key != null -> s.t(sign.failure_refusal_key)
        sign.failure_refused -> s.t(I18nKeys.Flows.SIGN_REFUSED)
        // Spec 099 R8: the passkey failed — the signer is named, and how
        // (the core's kind, from the app's own passkey classifier).
        sign.error?.kind == SignErrorKind.SignerUnavailable -> s.t(I18nKeys.BrowserStatus.REASON_SIGNER_UNAVAILABLE)
        sign.error?.kind == SignErrorKind.SignerNotDiscoverable -> s.t(I18nKeys.BrowserStatus.REASON_SIGNER_NOT_DISCOVERABLE)
        sign.error?.kind == SignErrorKind.SignerFailed -> s.t(I18nKeys.BrowserStatus.REASON_SIGNER_FAILED)
        // Spec 102: why this account cannot sign here — the core's line
        // (`venueBlockLine`), in the person's language.
        sign.error?.kind == SignErrorKind.VenueBlocked ->
            sign.error.venue_block?.words { key, vars -> s.t(key, vars) }?.ifBlank { null } ?: s.t("send.txErrorGeneric")
        else -> s.t("send.txErrorGeneric")
    }

    fun blocks(clear: ClearSigningView, to: String?, dataBytes: Int, ctx: Context): List<SigningBlock> =
        blocksBySurface(clear, to, dataBytes, ctx.strings, ctx)

    /**
     * Spec 082 RC1–RC5: a dApp's plain value transfer, as the core read it —
     * the verdict (empty calldata, a readable value) and the amount are the
     * core's; the coin's symbol is the fee row's. A zero send is "Send · 0"
     * with no minus. This app used to decide it here from the calldata.
     */
    private fun plainSendBlocks(plain: app.getvela.wallet.feature.signing.core.ClearPlainSend, ctx: Context): List<SigningBlock> {
        val s = ctx.strings
        return listOf(
            SigningBlock.Intent(s.s("intentSend"), SigningTone.Neutral),
            SigningBlock.Amount(AmountLine(sign = if (plain.no_value) "" else "−", value = plain.amount, symbol = ctx.nativeSymbol), card = true),
            SigningBlock.Party(s.s("recipientLabel"), ExploreLive.shortAddress(plain.to), plain.to),
        )
    }

    private fun blocksBySurface(clear: ClearSigningView, to: String?, dataBytes: Int, s: VelaStrings, ctx: Context): List<SigningBlock> = when (clear.surface) {
        ClearSurface.None -> emptyList()
        ClearSurface.Loading -> listOf(SigningBlock.Sentence(s.s("loading"), SigningTone.Neutral))
        ClearSurface.ClearSign -> clear.result?.let { resultBlocks(it, clear.native_value, ctx) }.orEmpty()
        ClearSurface.EthSign, ClearSurface.MessageSign -> clear.message?.let { messageBlocks(it, s, ctx.origin) }.orEmpty()
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
            // Spec 096 F4: nobody could read the call; the coin it sends is known.
            clear.native_value?.let { add(SigningBlock.Rows(listOf(coinRow(it, ctx)))) }
            to?.let { add(SigningBlock.Party(s.s("interactingLabel"), s.s("unverifiedLabel"), it, PartyBadge(s.s("unverifiedLabel"), SigningTone.Caution))) }
        }
        ClearSurface.PlainSend -> clear.plain_send?.let { plainSendBlocks(it, ctx) }.orEmpty()
        // 089 S1: every call of a batch, never call 1 alone.
        ClearSurface.Batch -> clear.batch?.let { batchBlocks(it, ctx) }.orEmpty()
    }

    /**
     * 089 S1: a batch as the sheet draws it — "Batch", how many transactions
     * are signed together, then EVERY call as its own card (the drawn CS26),
     * the coin the whole batch moves, and every flag any call raised, said
     * once. The headline is never call 1's: `[1 wei → A, 1 xDAI → B]` read
     * "Send 0.000…1 xDAI" and signed both. The guard's per-call cap cards and
     * its unlimited sentence follow ([guardBlocks]).
     */
    fun batchBlocks(batch: ClearBatchView, ctx: Context): List<SigningBlock> = buildList {
        val s = ctx.strings
        add(SigningBlock.Intent(s.s("batchIntent"), toneOf(batch.risk)))
        add(SigningBlock.Sentence(s.s("batchSubtitle", mapOf("count" to batch.calls.size.toString())), SigningTone.Accent))
        batch.calls.forEach { add(batchCallCard(it, ctx)) }
        val total = batch.total_amount
        if (total != null && batch.total_value_wei != "0") {
            add(SigningBlock.Rows(listOf(SigningRow(s.t(I18nKeys.Flows.SPLIT_TOTAL), "−$total ${ctx.nativeSymbol}"))))
        }
        val results = batch.calls.mapNotNull { it.result }
        if (results.any { it.to_own_token }) add(SigningBlock.Warning(SigningTone.Danger, s.s("tokenToContractWarning")))
        if (results.any { it.best_effort }) add(SigningBlock.Warning(SigningTone.Caution, s.s("bestEffortWarning")))
        if (results.any { it.partial }) add(SigningBlock.Warning(SigningTone.Caution, s.s("partialWarning")))
        if (results.any { it.provenance == ClearProvenance.Fetched }) add(SigningBlock.Warning(SigningTone.Caution, s.s("descriptorFetchedWarning")))
        if (results.any { it.terms_off_chain }) add(SigningBlock.Warning(SigningTone.Caution, s.s("warnOrderTerms")))
        if (results.any { r -> r.fields.any { it.unverified } }) add(SigningBlock.Warning(SigningTone.Caution, s.s("unverifiedWarning")))
        if (results.any { r -> r.fields.any { it.expired } }) add(SigningBlock.Warning(SigningTone.Caution, s.a("expired")))
    }

    /**
     * 089 S1: one call of a batch, as its own card — the words its call would
     * get alone. A decoded call is its intent and its fields, then the coin it
     * moves and whom it calls; a plain send is "Send", the exact amount and the
     * recipient; a call nobody could read says so in its title, with whom it
     * calls and what coin it moves. Never omitted.
     */
    private fun batchCallCard(call: ClearBatchCall, ctx: Context): SigningBlock.Card {
        val s = ctx.strings
        fun step(action: String) = s.s("batchStep", mapOf("index" to call.index.toString(), "action" to action))
        val tone = toneOf(call.risk)
        val result = call.result
        val plain = call.plain_send
        // What the call moves of the chain's own coin, and whom it calls:
        // inside a batch nothing else on the sheet says it for this call.
        val coin = call.amount?.takeIf { call.value_wei != "0" }?.let { listOf(SigningRow(s.s("labelAmount"), "−$it ${ctx.nativeSymbol}")) }.orEmpty()
        // A contract the wallet knows on this chain is named (096 F5); any
        // other is its full address.
        val target = call.to?.let { to ->
            val name = call.to_name
            listOf(if (name != null) SigningRow(s.s("interactingLabel"), name) else SigningRow(s.s("interactingLabel"), to, mono = true))
        }.orEmpty()
        return when {
            call.surface == ClearSurface.ClearSign && result != null ->
                SigningBlock.Card(step(result.intent), result.fields.filter { !it.detail }.map { rowOf(it, s) } + coin + target, tone)
            call.surface == ClearSurface.PlainSend && plain != null -> SigningBlock.Card(
                step(s.s("intentSend")),
                listOf(
                    SigningRow(s.s("labelAmount"), "${if (plain.no_value) "" else "−"}${plain.amount} ${ctx.nativeSymbol}"),
                    SigningRow(s.s("recipientLabel"), plain.to, mono = true),
                ),
                tone,
            )
            else -> SigningBlock.Card(step(s.s("blindDecodeWarning", mapOf("bytes" to call.data_bytes.toString()))), target + coin, tone)
        }
    }

    private fun toneOf(risk: ClearRisk): SigningTone = when (risk) {
        ClearRisk.Safe -> SigningTone.Success
        ClearRisk.Normal -> SigningTone.Neutral
        ClearRisk.Caution -> SigningTone.Caution
        ClearRisk.Danger -> SigningTone.Danger
    }

    private fun resultBlocks(result: ClearSignResult, native: ClearNativeValue?, ctx: Context): List<SigningBlock> = buildList {
        val s = ctx.strings
        add(SigningBlock.Intent(result.intent, toneOf(result.risk)))
        addAll(warnings(result, s))
        // Spec 096 F4: the coin the call sends leads the rows, in a batch
        // call's own words, when the reading does not say it itself.
        val coin = native?.let { listOf(coinRow(it, ctx)) }.orEmpty()
        val rows = coin + result.fields.filter { !it.detail }.map { rowOf(it, s) }
        if (rows.isNotEmpty()) add(SigningBlock.Rows(rows))
    }

    /** "Amount −0.003 BNB" — the core's `native_value`, as a batch call's coin row reads, in the fee row's coin (RC5). */
    private fun coinRow(native: ClearNativeValue, ctx: Context): SigningRow =
        SigningRow(ctx.strings.s("labelAmount"), "−${native.amount} ${ctx.nativeSymbol}")

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
        // Spec 096 F5: a CoW pre-signature signs an order whose amounts are
        // hashed into its id — not on this sheet, and said so.
        if (result.terms_off_chain) add(SigningBlock.Warning(SigningTone.Caution, s.s("warnOrderTerms")))
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
        // An address reads as monospace; a contract the core names
        // ("PancakeSwap Permit2", 096 F5) is a name, in the text face.
        mono = field.address != null && field.value.startsWith("0x"),
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
            // Spec 082 RG6: the core's line in the core's tone — a revert is a
            // danger (with its sanitised reason), a node that could not check
            // is a caution. Never "nothing changes" for either.
            is SigningController.SimOutcome.Notice -> listOf(
                SigningBlock.Warning(
                    toneOf(sim.risk),
                    sim.reason?.let { reason -> s.t(sim.key, mapOf("reason" to reason)) } ?: s.t(sim.key),
                ),
            )
            is SigningController.SimOutcome.Ready -> {
                if (sim.judgments.isEmpty()) {
                    return listOf(SigningBlock.Balances(s.s("balanceChangesTitle"), emptyList(), s.s("simResultNoChange")))
                }
                var unverified = false
                // A delta the core writes as nothing (a zero) is not drawn
                // (spec 082 RJ15, RC4/RC6).
                val rows = sim.judgments.mapNotNull { judgment ->
                    when (judgment) {
                        is TrustSimJudgment.Native -> deltaRow(ctx.nativeSymbol, judgment.delta, 18, ctx.numberPreset)
                        is TrustSimJudgment.Erc20Trusted -> deltaRow(judgment.symbol, judgment.delta, judgment.decimals, ctx.numberPreset)
                        is TrustSimJudgment.Erc20Unverified -> signedRaw(judgment.delta, ctx.numberPreset)?.let { raw ->
                            unverified = true
                            BalanceDeltaRow(s.s("balanceUnverifiedToken"), raw, SigningTone.Caution)
                        }
                    }
                }
                // Every move was a zero: nothing of theirs moves.
                if (rows.isEmpty()) {
                    return listOf(SigningBlock.Balances(s.s("balanceChangesTitle"), emptyList(), s.s("simResultNoChange")))
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

    /**
     * One signed balance change, written by the core (`formatSignedTokenAmount`,
     * spec 082 RJ15): the token ladder, a dust amount written exactly — never
     * `−0` — and U+2212 for a minus. `null` for a zero: not drawn.
     */
    private fun deltaRow(symbol: String, delta: String, decimals: Int, preset: String): BalanceDeltaRow? {
        val text = uniffi.vela_core_uniffi.formatSignedTokenAmount(delta, decimals.coerceAtLeast(0).toUInt(), preset) ?: return null
        val negative = text.startsWith("\u2212")
        return BalanceDeltaRow(symbol, text, if (negative) SigningTone.Neutral else SigningTone.Success)
    }

    /**
     * An unverified token's change in its raw units — its decimals are not
     * known, so no decimal point is guessed. The core writes the sign and
     * drops a zero; the units stay whole (decimals 0).
     */
    private fun signedRaw(delta: String, preset: String): String? =
        uniffi.vela_core_uniffi.formatSignedTokenAmount(delta, 0u, preset)

    fun feeModel(clear: ClearSigningView, fee: FeeView, ctx: Context, speed: SendLive.SpeedInputs? = null): FeeModel {
        if (offChain(clear)) return FeeModel.OffChain(ctx.strings.s("noNetworkFee"))
        // For the moment between a speed being picked and its own figure
        // landing, the fee in hand is the previous speed's: "estimating".
        val estimate = fee.fee.takeIf { !ofAnotherTier(fee, speed) }
        // PR 2 note 1: the failure, said once for the row and the footer —
        // through the re-ask that follows it too, so the row never flips to
        // "Estimating…" and back while the core retries by itself.
        val failure = fee.failure
        val value = when {
            // The figure says what a tap does (PR 2 polish): "Tap to retry"
            // only when a tap is the one way, "Pay with another coin" when it
            // opens the coins, else the dash — never a tap asked for while
            // the core retries.
            failure != null -> app.getvela.wallet.feature.send.core.FeeFailureRow.figure(failure, ctx.strings)
            // The send screens' own line (issue 201): the coin that is ACTUALLY
            // paying — an in-band ERC-20 fee is its own amount under its own
            // ticker, never the native figure — and what it costs in money.
            // One formatter, because two surfaces pricing one operation must
            // not give two answers.
            estimate != null -> "~" + feeLine(estimate, fee, ctx)
            else -> ctx.strings.t("componentsUi.gas.estimating")
        }
        val choosable = fee.options.size > 1
        val selected = fee.options.firstOrNull { it.selected }
        // Issue #262: the core shut the gate because the coin that pays is not
        // there — the send form's own sentence (#211), about the same shortfall.
        val short = estimate != null && !fee.busy && failure == null && !fee.confirm_fee_ready && selected?.insufficient == true
        // Issue #408: and not one coin on offer can pay — the core's verdict,
        // said as that rather than naming the coin in force ("Insufficient ETH"
        // over a wallet whose USDT was short too).
        val noCoinPays = estimate != null && fee.no_coin_pays
        val options = if (ctx.feeOpen && choosable) {
            fee.options.map { option ->
                FeeTokenOption(
                    id = option.contract ?: NATIVE_FEE_ID,
                    // The request's chain (never an estimate's, which can be absent).
                    mark = WalletLive.mark(ctx.chainId, option.symbol, option.contract),
                    name = option.symbol,
                    balance = "${ctx.strings.t("componentsUi.gas.rowBalance")} ${SendLive.fromBase(option.balance, option.decimals)}",
                    fee = option.amount?.let { "~${SendLive.feeFromBase(it, option.decimals)} ${option.symbol}" } ?: "—",
                    selected = option.selected,
                    disabled = option.insufficient,
                    // Issue #408: a refused coin says why — the core's numbers.
                    reason = option.short?.takeIf { option.insufficient }?.let { gap ->
                        ctx.strings.t(I18nKeys.Flows.FEE_ROW_SHORT, mapOf("need" to gap.need, "have" to gap.have))
                    },
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
            // A failed fee's row does what its figure says — a retry at once
            // (also while the core retries), or the coins — and is no control
            // when the core says a tap does nothing.
            tappable = if (failure != null) app.getvela.wallet.feature.send.core.FeeFailureRow.isControl(failure) else choosable,
            warning = when {
                noCoinPays -> ctx.strings.t(I18nKeys.Flows.FEE_NO_COIN_PAYS)
                short -> ctx.strings.t("send.warnInsufficientGas", mapOf("sym" to selected!!.symbol))
                // Spec 096 F2: the person chose a coin the transaction itself
                // spends (the PancakeSwap USDC swap, fee in USDC); the core
                // flags it, said under the fee while that coin pays.
                !fee.busy && failure == null && selected?.spent_by_operation == true ->
                    ctx.strings.t("componentsUi.gas.feeCoinSpent", mapOf("sym" to selected.symbol))
                // Spec 079: why there is no fee, in the core's words (spec 082
                // RJ13, PR 2 note 1): the relay's failure, the chain's node
                // (rate-limited, or out of reach, named), a fault inside the
                // app — or no reason line at all. Kept through the re-ask, the
                // measuring sign turning beside it.
                else -> failure?.reason_key?.let { key -> ctx.strings.t(key, mapOf("chain" to ctx.chainName)) }
            },
            refreshLabel = ctx.strings.t(I18nKeys.Flows.FEE_REFRESH),
            // A coin switched and being measured again with its own fee leg
            // (`provisional`): the switched figure stays, with the measuring
            // sign, and the confirm waits (the core's gate) until it lands.
            refreshing = fee.busy || fee.provisional,
            // The core's FeeMeasuring, as the gate reads it.
            measuring = fee.busy || fee.provisional || ofAnotherTier(fee, speed),
            // The core knows the first figure will land as "no coin can pay":
            // its line's room is held from now (iPhone pass 2026-10-09).
            reserve = ctx.strings.t(I18nKeys.Flows.FEE_NO_COIN_PAYS).takeIf { fee.nothing_to_pay_from },
            // Only where a tap opens the coin list: never over a failure a tap
            // retries, nor one a tap cannot help.
            chevron = choosable && (failure == null || failure.tap == app.getvela.wallet.feature.send.core.FeeFailureTap.ChooseCoin),
            chevronRoom = choosable,
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

    /** The confirm's words, the action alone: the core's intent id, in the corpus's words (the desktop's `confirm_label`). */
    fun confirmLabel(clear: ClearSigningView, s: VelaStrings): String = when (val confirm = clear.confirm) {
        ClearConfirm.Sign -> s.s("signLabel")
        ClearConfirm.Confirm -> s.s("confirmLabel")
        is ClearConfirm.ConfirmIntent -> when (confirm.intent) {
            "send" -> s.s("confirmSend")
            "swap" -> s.s("confirmSwap")
            "deposit" -> s.s("confirmDeposit")
            "withdraw" -> s.s("confirmWithdraw")
            // The core's word for the rest ("Approve" → 授权), else the neutral verb.
            else -> confirm.intent_term?.let { s.s(it) } ?: s.s("confirmLabel")
        }
    }
}
