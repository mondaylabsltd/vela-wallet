package app.getvela.wallet.feature.signing

import androidx.compose.ui.graphics.Color
import app.getvela.wallet.core.crux.CoreScript
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.signing.core.ClearLineSource
import app.getvela.wallet.feature.signing.core.ClearReading
import app.getvela.wallet.feature.signing.core.ClearRisk
import app.getvela.wallet.feature.signing.core.ClearSignField
import app.getvela.wallet.feature.signing.core.ClearSignMethod
import app.getvela.wallet.feature.signing.core.ClearSignResult
import app.getvela.wallet.feature.signing.core.ClearFieldRole
import app.getvela.wallet.feature.signing.core.ClearSigningEvent
import app.getvela.wallet.feature.signing.core.ClearSigningView
import app.getvela.wallet.feature.signing.core.ClearSurface
import app.getvela.wallet.feature.signing.core.GuardApprovalKind
import app.getvela.wallet.feature.signing.core.GuardDetectedApproval
import app.getvela.wallet.feature.signing.core.GuardEditorView
import app.getvela.wallet.feature.signing.core.GuardLocus
import app.getvela.wallet.feature.signing.core.GuardSurface
import app.getvela.wallet.feature.signing.core.GuardView
import app.getvela.wallet.feature.signing.core.SignErrorKind
import app.getvela.wallet.feature.signing.core.SignErrorNotice
import app.getvela.wallet.feature.signing.core.SignView
import app.getvela.wallet.feature.signing.core.SigningController
import app.getvela.wallet.feature.signing.core.SimDeltas
import app.getvela.wallet.feature.wallet.WalletFixtures
import uniffi.vela_core_uniffi.ClearSigningCore

/**
 * The signing-depth boards (Android phase 2, P2.3) — every one drawn through
 * the LIVE builders over the views the machines write, so the gallery shows
 * what a real sheet shows:
 *
 * - CS71 — a message as the core draws it (`ClearMessageView.display_lines`,
 *   from the REAL clear-signing machine): a direction-changing character as
 *   its muted `⟨U+202E⟩` marker, a long run of empty lines folded into one
 *   muted line that counts them, and the hidden-characters warning (cs-10,
 *   cs-11).
 * - CS72 — a reading with its provenance (cs-1…cs-14, `ClearReading`): who
 *   described it, what the description says against Vela's decode, an
 *   output paid to a stranger, a token named only by its own contract, and
 *   the contract with who named it.
 * - CS73 — a grant-all approval (cs-17): "Allow all" or "Revoke", no amount;
 *   the confirm waits for the deliberate choice.
 * - CS74 — no node answered the simulation: "Vela couldn't check…" with
 *   "Check again" (dapp-15), and CS75 the same line while a new check runs.
 * - CS76 — the account's trusted signing page never answered (round 7): the
 *   failure said as what happened there, never "couldn't be submitted".
 * - CS77 — the AI agent's opinion in its own card (spec 103), current; CS78
 *   the same opinion once the request changed under it, greyed.
 */
internal object SigningDepthBoards {

    private fun VelaStrings.s(key: String) = t("componentsUi.signing.$key")

    private fun ctx(s: VelaStrings, chainId: Int = 1) = SigningLive.Context(
        s, "Ethereum", Color(0xFF627EEA), "ETH", WalletFixtures.NAME, WalletFixtures.ADDRESS_FULL, chainId = chainId,
        descriptorHost = "data.getvela.app",
    )

    fun build(state: SigningScreenState, s: VelaStrings, base: SigningScreenModel): SigningScreenModel = when (state) {
        SigningScreenState.CS71 -> message(s, base)
        SigningScreenState.CS72 -> reading(s, base)
        SigningScreenState.CS73 -> grantAll(s, base)
        SigningScreenState.CS74, SigningScreenState.CS75 -> recheck(s, base, checking = state == SigningScreenState.CS75)
        SigningScreenState.CS76 -> pageFailed(s, base)
        SigningScreenState.CS77, SigningScreenState.CS78 -> advice(s, base, stale = state == SigningScreenState.CS78)
        else -> base
    }

    // -- CS71: the message, as the core draws it ---------------------------------------

    /** A sign-in message with a right-to-left override in it and a screenful of nothing before its nonce. */
    const val MESSAGE = "app.example.org wants you to sign in.‮evil.example\n\n\n\n\n\n\n\n\n\nNonce: 8c1f"

    /** The clear-signing machine's view of [MESSAGE] presented as `personal_sign` — the core's own lines. */
    fun messageView(): ClearSigningView {
        val script = CoreScript(ClearSigningCore().asBridge()) { null }
        val hex = "0x" + MESSAGE.toByteArray(Charsets.UTF_8).joinToString("") { "%02x".format(it) }
        val event: ClearSigningEvent = ClearSigningEvent.MessagePresented(
            method = ClearSignMethod.PersonalSign,
            params = listOf(hex, WalletFixtures.ADDRESS_FULL),
            request_origin = "https://app.example.org",
        )
        script.dispatch(Wire.json.encodeToString(ClearSigningEvent.serializer(), event))
        return Wire.json.decodeFromString(ClearSigningView.serializer(), script.viewJson())
    }

    private fun message(s: VelaStrings, base: SigningScreenModel): SigningScreenModel {
        val view = messageView()
        return base.copy(
            blocks = SigningLive.blocks(view, null, 0, ctx(s)),
            confirmAction = s.s("signLabel"),
            fee = FeeModel.OffChain(s.s("noNetworkFee")),
        )
    }

    // -- CS72: a reading and where it came from ---------------------------------------

    /** A swap a built-in description reads, with everything the certain decode says against it. */
    fun readingView(): ClearSigningView = ClearSigningView(
        resolved = true,
        surface = ClearSurface.ClearSign,
        result = ClearSignResult(
            intent = "Swap",
            intent_term = "intentSwap",
            contract_name = "Universal Router",
            contract_address = "0x66a9893cc07d91d95644aedd05d03f95e1dba8af",
            risk = ClearRisk.Caution,
            fields = listOf(
                ClearSignField(label = "Pay", value = "1,000 USDC", role = ClearFieldRole.SendAmount, token_address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
                ClearSignField(
                    label = "Receive (min)", value = "0.0001 WETH?", role = ClearFieldRole.ReceiveAmount,
                    token_address = "0x" + "c0".repeat(20), token_named_by_contract = true, unverified = true,
                ),
                ClearSignField(label = "Recipient", value = "0x9f1c…a2e1", role = ClearFieldRole.Recipient, address = "0x9f1c" + "0".repeat(32) + "a2e1"),
            ),
            reading = ClearReading(
                source = ClearLineSource.BuiltIn,
                descriptor_owner = "Uniswap Labs",
                contract_named_by = ClearLineSource.BuiltIn,
                contradictions = listOf("Claim"),
                goes_elsewhere = true,
                dust_minimum = true,
            ),
        ),
        confirm = app.getvela.wallet.feature.signing.core.ClearConfirm.ConfirmIntent("Swap", "intentSwap"),
    )

    private fun reading(s: VelaStrings, base: SigningScreenModel): SigningScreenModel =
        base.copy(blocks = SigningLive.blocks(readingView(), null, 0, ctx(s)))

    // -- CS73: allow all, or revoke ------------------------------------------------------

    /** `setApprovalForAll(OpenSea Conduit, true)` before the person has chosen — the guard's view of it. */
    fun grantAllGuard(): GuardView = GuardView(
        surface = GuardSurface.ApprovalEditor,
        detected = GuardDetectedApproval(
            kind = GuardApprovalKind.SetApprovalForAll,
            token_address = "0xbc4ca0eda7647a8ab7c2061c2e118a18a936f13d",
            spender = "0x1e0049783f008a0085193e00003d00cd54003c71",
            is_boolean_grant = true,
            editable = true,
            locus = GuardLocus.CalldataWord(1),
        ),
        editor = GuardEditorView(revoke_offered = true),
        confirm_allowed = false,
    )

    private fun grantAll(s: VelaStrings, base: SigningScreenModel): SigningScreenModel = base.copy(
        blocks = listOf(SigningBlock.Intent(t(s, "componentsUi.signingApprove.verbApproveAll"), SigningTone.Danger)) +
            SigningLive.guardBlocks(grantAllGuard(), s),
        confirmAction = t(s, "componentsUi.signingApprove.verbApproveAll"),
        confirmEnabled = false,
        // The line the live sheet says under a grant-all's shut confirm.
        confirmBlockLine = t(s, "componentsUi.signing.certain.chooseGrant"),
    )

    // -- CS74 / CS75: could not check — check again ---------------------------------------

    /** The core's reading of a node nobody reached: "could not check", about the network (`recheck`). */
    fun unreachable(): SigningController.SimOutcome = SimDeltas.outcome(WalletFixtures.ADDRESS_FULL, null).let { record ->
        SimDeltas.notice(record) ?: SimDeltas.couldNotCheck()
    }

    private fun recheck(s: VelaStrings, base: SigningScreenModel, checking: Boolean): SigningScreenModel {
        val ctx = ctx(s).copy(simAgain = !checking, simRechecking = checking)
        return base.copy(blocks = base.blocks + SigningLive.verdictPlace(unreachable(), SignView(), ctx))
    }

    // -- CS76: the page did not sign -----------------------------------------------------

    private fun pageFailed(s: VelaStrings, base: SigningScreenModel): SigningScreenModel {
        val failed = SignView(error = SignErrorNotice(kind = SignErrorKind.PageUnanswered))
        return base.copy(blocks = base.blocks + SigningLive.statusBlocks(failed, s))
    }

    // -- CS77 / CS78: the agent's opinion -------------------------------------------------

    /** An opinion as the core keeps one — plain text, capped, hosts only. */
    fun accepted(): SigningAdvice.Accepted = SigningAdvice.Accepted(
        review_digest = "0x" + "ab".repeat(32),
        verdict = SigningAdvice.Verdict.Caution,
        summary = "A transfer of 1,000 USDC to an address this wallet has never sent to.",
        points = listOf("The recipient has no history with this account.", "The amount is most of the USDC balance."),
        source_hosts = listOf("etherscan.io"),
    )

    private fun advice(s: VelaStrings, base: SigningScreenModel, stale: Boolean): SigningScreenModel {
        val advice = accepted()
        // How it stands is the core's: the digest it was given for, against the one on screen.
        val standing = SigningAdvice.standing(advice, if (stale) "0x" + "cd".repeat(32) else advice.review_digest, null)
        return base.copy(advice = SigningAdvice.card(advice, standing, s))
    }

    private fun t(s: VelaStrings, key: String) = s.t(key)
}
