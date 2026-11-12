package app.getvela.wallet

import androidx.compose.ui.graphics.Color
import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.i18n.I18nCatalogs
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.feature.multisig.MsProposalModel
import app.getvela.wallet.feature.multisig.MultisigWords
import app.getvela.wallet.feature.multisig.core.MsAccountsView
import app.getvela.wallet.feature.multisig.core.MsProposalsView
import app.getvela.wallet.feature.signing.BalanceDeltaRow
import app.getvela.wallet.feature.signing.SigningBlock
import app.getvela.wallet.feature.signing.SigningLive
import app.getvela.wallet.feature.signing.SigningTone
import app.getvela.wallet.feature.signing.core.ClearSigningView
import app.getvela.wallet.feature.signing.core.ClearSurface
import app.getvela.wallet.feature.signing.core.GuardView
import app.getvela.wallet.feature.signing.core.SignAccountRef
import app.getvela.wallet.feature.signing.core.SignApproveOpts
import app.getvela.wallet.feature.signing.core.SignEvent
import app.getvela.wallet.feature.signing.core.SignOperation
import app.getvela.wallet.feature.signing.core.SignShellResult
import app.getvela.wallet.feature.signing.core.SignSponsorship
import app.getvela.wallet.feature.signing.core.SignSubmitOutcome
import app.getvela.wallet.feature.signing.core.SignSurface
import app.getvela.wallet.feature.signing.core.SignView
import app.getvela.wallet.feature.signing.core.SigningController
import app.getvela.wallet.feature.signing.core.SimDeltas
import app.getvela.wallet.feature.wallet.core.TrustAssetDelta
import app.getvela.wallet.feature.wallet.core.TrustDeltaKind
import app.getvela.wallet.feature.wallet.core.TrustSimDirection
import app.getvela.wallet.feature.wallet.core.TrustSimJudgment
import java.io.File
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.SignRequestCore

/**
 * §41 (core P2-3 / P2-5): the Android shell follows the core.
 *
 * - P2-3: the simulation reads NFTs, approvals and what it cannot read — the
 *   wire carries them verbatim and the sheet draws each as a balance row.
 * - P2-5: the signing machines hold the "… anyway" gate themselves — every
 *   approve carries the review the sheet showed and whether the press was the
 *   "… anyway" one; every multisig signing press says so too.
 */
class Sec41ShellTest {
    private val repoRoot = File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle"))
    private val assets = File(repoRoot, "assets/i18n")
    private val strings: I18nRuntime = I18nRuntime(
        readOnDemand = { namespace, tag -> File(assets, I18nCatalogs.assetPath(tag, namespace).removePrefix("i18n/")).takeIf { it.isFile }?.readBytes() },
    ) { tag -> File(assets, "$tag.json").readBytes() }.apply {
        initialize("en")
        include(I18nCatalogs.MULTISIG)
    }
    private val ctx = SigningLive.Context(strings, "Gnosis", Color.Red, "XDAI", "Parallel space", ME)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    @After fun tearDown() = scope.cancel()

    // -- P2-3: the wire --------------------------------------------------------------------

    @Test
    fun `the three new judgments decode from the core's JSON and go back out verbatim`() {
        val wire = """[
            {"type":"nft","token":"$NFT","token_id":"7","standard":"erc721","direction":"in","amount":"1"},
            {"type":"nft","token":"$NFT","token_id":"42","standard":"erc1155","direction":"out","amount":"3"},
            {"type":"approval","token":"$NFT","spender":"$SPENDER","unlimited":true},
            {"type":"approval","token":"$NFT","spender":"$SPENDER","unlimited":false,"token_id":"9"},
            {"type":"unread","token":"$NFT"},
            {"type":"unread"}
        ]"""
        val list = ListSerializer(TrustSimJudgment.serializer())
        val judgments = Wire.json.decodeFromString(list, wire)
        assertEquals(
            listOf(
                TrustSimJudgment.Nft(NFT, "7", TrustDeltaKind.Erc721, TrustSimDirection.In, "1"),
                TrustSimJudgment.Nft(NFT, "42", TrustDeltaKind.Erc1155, TrustSimDirection.Out, "3"),
                TrustSimJudgment.Approval(NFT, SPENDER, unlimited = true),
                TrustSimJudgment.Approval(NFT, SPENDER, unlimited = false, token_id = "9"),
                TrustSimJudgment.Unread(NFT),
                TrustSimJudgment.Unread(null),
            ),
            judgments,
        )
        // Stored with a record and read back: the same JSON, nothing dropped or added.
        assertEquals(Wire.json.parseToJsonElement(wire), Wire.json.encodeToJsonElement(list, judgments))
    }

    @Test
    fun `an NFT, approval or unread delta round-trips with its id and spender, absent when none`() {
        val wire = """[
            {"kind":"erc721","token":"$NFT","delta":"-1","token_id":"7"},
            {"kind":"erc1155","token":"$NFT","delta":"3","token_id":"42"},
            {"kind":"approval","token":"$NFT","delta":"115792089237316195423570985008687907853269984665640564039457584007913129639935","spender":"$SPENDER"},
            {"kind":"unread","token":"$NFT","delta":"1"},
            {"kind":"native","delta":"-5"}
        ]"""
        val list = ListSerializer(TrustAssetDelta.serializer())
        val deltas = Wire.json.decodeFromString(list, wire)
        assertEquals(listOf(TrustDeltaKind.Erc721, TrustDeltaKind.Erc1155, TrustDeltaKind.Approval, TrustDeltaKind.Unread, TrustDeltaKind.Native), deltas.map { it.kind })
        assertEquals("7", deltas[0].token_id)
        assertEquals(SPENDER, deltas[2].spender)
        assertEquals("passed on to the trust machine verbatim", Wire.json.parseToJsonElement(wire), Wire.json.encodeToJsonElement(list, deltas))
        // None of them is a coin the fee can be paid from.
        assertEquals(1, SimDeltas.feeBalanceChanges(deltas).size)
    }

    // -- P2-3: the rows --------------------------------------------------------------------

    @Test
    fun `the sheet draws an NFT, an approval and an unread change as balance rows, in their tones`() {
        val sim = SigningController.SimOutcome.Ready(
            listOf(
                TrustSimJudgment.Nft(NFT, "7", TrustDeltaKind.Erc721, TrustSimDirection.In, "1"),
                TrustSimJudgment.Nft(NFT, "42", TrustDeltaKind.Erc1155, TrustSimDirection.Out, "3"),
                TrustSimJudgment.Nft(NFT, "8", TrustDeltaKind.Erc721, TrustSimDirection.Still, "0"),
                TrustSimJudgment.Nft(NFT, "9", TrustDeltaKind.Erc721, TrustSimDirection.Unreadable, "1"),
                TrustSimJudgment.Approval(NFT, SPENDER, unlimited = true),
                TrustSimJudgment.Approval(NFT, SPENDER, unlimited = false),
                TrustSimJudgment.Unread(NFT),
            ),
        )
        val card = SigningLive.simBlocks(sim, ctx).single() as SigningBlock.Balances
        val short = app.getvela.wallet.core.format.ShortAddress.of(SPENDER)
        assertTrue("shortened as the app shortens addresses: $short", short.contains("…"))
        assertEquals(
            listOf(
                BalanceDeltaRow("NFT #7", "+", SigningTone.Caution),
                BalanceDeltaRow("NFT #42", "−3", SigningTone.Caution),
                // `still`: no row. `unreadable`: the row, no sign.
                BalanceDeltaRow("NFT #9", "", SigningTone.Caution),
                BalanceDeltaRow(strings.t("componentsUi.signing.balanceApprovalAll", mapOf("spender" to short)), "", SigningTone.Danger),
                BalanceDeltaRow(strings.t("componentsUi.signing.balanceApproval", mapOf("spender" to short)), "", SigningTone.Danger),
                BalanceDeltaRow("This changes something Vela can’t read", "", SigningTone.Caution),
            ),
            card.rows,
        )
        assertEquals("Unlimited spending approval for $short", card.rows[3].symbol)
        // An NFT nobody vouches for carries the unverified caution under the card.
        assertEquals(strings.t("componentsUi.signing.unverifiedWarning"), card.note)
        assertEquals(SigningTone.Caution, card.noteTone)

        // A grant alone: its row, and no unverified caution.
        val grant = SigningLive.simBlocks(SigningController.SimOutcome.Ready(listOf(TrustSimJudgment.Approval(NFT, SPENDER, unlimited = false))), ctx).single() as SigningBlock.Balances
        assertEquals(1, grant.rows.size)
        assertNull(grant.note)
    }

    @Test
    fun `the corpus says the chosen node, not your own`() {
        assertEquals("Checked by your chosen node only", strings.t("componentsUi.signing.simOwnNode"))
    }

    // -- P2-5: the approve ------------------------------------------------------------------

    @Test
    fun `an approve hands back the views the gate read, verbatim, with the press and the pref`() {
        val clearJson = """{"surface":"clear_sign","resolving":false,"sign_anyway":true,"a_field_this_shell_does_not_know":1}"""
        val guardJson = """{"surface":"none","carrier_unread":true}"""
        val simJson = """{"kind":"deltas","deltas":[],"source":"corroborated"}"""
        val review = SigningController.reviewOf(clearJson, guardJson, simJson, refuseBlind = true, ClearSigningView(), GuardView())
        assertEquals(Wire.json.parseToJsonElement(clearJson), review.clear)
        assertEquals(Wire.json.parseToJsonElement(guardJson), review.guard)
        assertEquals(Wire.json.parseToJsonElement(simJson), review.sim)
        assertTrue(review.refuse_blind)
        // No raw view held: the decoded one, re-encoded; no verdict: none.
        val bare = SigningController.reviewOf(null, null, null, refuseBlind = false, ClearSigningView(surface = ClearSurface.ClearSign), GuardView())
        assertEquals("clear_sign", bare.clear.jsonObject["surface"]!!.jsonPrimitive.content)
        assertEquals(false, bare.guard.jsonObject["carrier_unread"]!!.jsonPrimitive.boolean)
        assertNull(bare.sim)

        // On the wire: `review` and `anyway_confirmed`, the names the core reads.
        val opts = SignApproveOpts(review = review, anyway_confirmed = true)
        val event = Wire.json.encodeToJsonElement(SignEvent.serializer(), SignEvent.ApproveTapped(opts)).jsonObject
        val sent = event["opts"]!!.jsonObject
        assertEquals(true, sent["anyway_confirmed"]!!.jsonPrimitive.boolean)
        assertEquals(true, sent["review"]!!.jsonObject["refuse_blind"]!!.jsonPrimitive.boolean)
        assertEquals(Wire.json.parseToJsonElement(clearJson), sent["review"]!!.jsonObject["clear"])
    }

    @Test
    fun `the core signs an "anyway" request only with the review and the "anyway" press, never with Refuse blind signing`() = runBlocking<Unit> {
        // No review: nothing is signed.
        assertFalse("signed with no review", signs(SignApproveOpts()))
        // A plain review, a plain press: signed.
        assertTrue("a plain review on a plain press signs", signs(SignApproveOpts(review = plainReview())))
        // The reading could not be made ("… anyway"): a plain press signs nothing…
        val anyway = plainReview(signAnyway = true)
        assertFalse("an \"anyway\" posture signed on a plain press", signs(SignApproveOpts(review = anyway)))
        // …the "… anyway" press does…
        assertTrue("the \"anyway\" press signs", signs(SignApproveOpts(review = anyway, anyway_confirmed = true)))
        // …and with Refuse blind signing on, nothing does.
        assertFalse(
            "signed \"anyway\" with Refuse blind signing on",
            signs(SignApproveOpts(review = anyway.copy(refuse_blind = true), anyway_confirmed = true)),
        )
    }

    // -- P2-5: the multisig presses -------------------------------------------------------------

    @Test
    fun `every multisig signing press says whether it was the "anyway" one and the pref`() {
        val plain = proposal(anyway = false, refuseBlind = false)
        assertFalse(plain.signAnyway)
        for (event in listOf(plain.approveEvent(ALICE), plain.approveOutsideEvent, plain.executeEvent(ALICE, oneTap = false), plain.executeEvent(ALICE, oneTap = true))) {
            assertEquals("$event", false, event["anyway_confirmed"]!!.jsonPrimitive.boolean)
            assertEquals("$event", false, event["refuse_blind"]!!.jsonPrimitive.boolean)
        }
        val anyway = proposal(anyway = true, refuseBlind = false)
        assertTrue("the door says \"… anyway\"", anyway.signAnyway)
        for (event in listOf(anyway.approveEvent(ALICE), anyway.approveOutsideEvent, anyway.executeEvent(ALICE, oneTap = false), anyway.executeEvent(ALICE, oneTap = true))) {
            assertEquals("$event", true, event["anyway_confirmed"]!!.jsonPrimitive.boolean)
        }
        assertEquals("approve", anyway.approveEvent(ALICE)["type"]!!.jsonPrimitive.content)
        assertEquals("approve_outside", anyway.approveOutsideEvent["type"]!!.jsonPrimitive.content)
        // With Refuse blind signing on the pref travels with the press (the core refuses it).
        val refused = proposal(anyway = true, refuseBlind = true)
        assertEquals(true, refused.approveEvent(ALICE)["refuse_blind"]!!.jsonPrimitive.boolean)

        // The draft's presses (`confirm_draft`, `propose_execute`) carry the same two fields.
        val review = File(repoRoot, "app-android/vela-wallet/app/src/main/java/app/getvela/wallet/feature/multisig/MultisigReview.kt").readText()
        assertTrue(review.contains("ev(\"propose_execute\", \"as_member\" to by, \"anyway_confirmed\" to pressedAnyway, \"refuse_blind\" to refuseBlind)"))
        assertTrue(review.contains("hub.confirmDraft(by, anywayConfirmed = pressedAnyway, refuseBlind = refuseBlind)"))
        val hub = File(repoRoot, "app-android/vela-wallet/app/src/main/java/app/getvela/wallet/feature/multisig/core/MultisigHub.kt").readText()
        assertTrue(hub.contains("ev(\"confirm_draft\", \"as_member\" to asMember, \"anyway_confirmed\" to anywayConfirmed, \"refuse_blind\" to refuseBlind)"))
    }

    @Test
    fun `the multisig review says an NFT, a grant and an unread change in the sheet's words, never as a coin figure`() {
        fun said(json: String) = app.getvela.wallet.feature.multisig.deltaText(Wire.json.parseToJsonElement(json), 100, words, emptyMap())
        assertEquals("NFT #7 +", said("""{"kind":"erc721","token":"$NFT","delta":"1","token_id":"7"}"""))
        assertEquals("NFT #42 −3", said("""{"kind":"erc1155","token":"$NFT","delta":"-3","token_id":"42"}"""))
        val short = MultisigWords.short(SPENDER)
        assertEquals(
            "Unlimited spending approval for $short",
            said("""{"kind":"approval","token":"$NFT","delta":"115792089237316195423570985008687907853269984665640564039457584007913129639935","spender":"$SPENDER"}"""),
        )
        assertEquals("Spending approval for $short", said("""{"kind":"approval","token":"$NFT","delta":"1000","spender":"$SPENDER"}"""))
        assertEquals("This changes something Vela can’t read", said("""{"kind":"unread","token":"$NFT","delta":"1"}"""))
        // A coin is said as it always was.
        assertEquals("−1.5 USDC", said("""{"kind":"native","delta":"-1500000000000000000"}"""))
    }

    // -- helpers -------------------------------------------------------------------------------

    private val words = MultisigWords(
        strings = strings,
        accounts = MsAccountsView(),
        localNames = emptyMap(),
        networkName = { if (it == 5042002) "Arc Testnet" else null },
        nativeSymbol = { "USDC" },
    )

    private fun proposal(anyway: Boolean, refuseBlind: Boolean): MsProposalModel {
        val review = """{"sign":{"allowed":true,"anyway":$anyway},"concerns":[],"recipients":[],"simulation":{"kind":"deltas","deltas":[]},"state":{"verdict":{"kind":"unchanged"}}}"""
        val view = MsProposalsView.of(
            Wire.json.parseToJsonElement(
                """
                {"followed":"$SAFE","standings":{},"reviews":{"5042002:$HASH":$review},"review_states":{"5042002:$HASH":{"kind":"ready"}},
                 "approve_execute":{"5042002:$HASH":["$ALICE"]},
                 "items":[{"chainId":5042002,"safeTxHash":"$HASH","nonce":"0","proposer":"$BOB","approvals":[{"owner":"$BOB"}],"approved":1,"threshold":2,
                  "status":"pending","canApprove":["$ALICE"],"canExecute":["$ALICE"],"cancelledBy":null,"sameNonce":[],"executing":false,"createdMs":1,
                  "unsent":false,"renameUndeployed":false,"review":{"action":{"kind":"call","to":"$BOB","value":"0","data":"0x"},"sign":{"allowed":true,"anyway":$anyway}}}],
                 "chains":[{"chainId":5042002,"members":{"deployed":false,"owners":["$ALICE","$BOB"],"threshold":2,"nonce":"0"},"items":[],"mailbox":true}]}
                """.trimIndent(),
            ),
        )
        return MsProposalModel(view.items.single(), view, words, null, refuseBlind = refuseBlind)
    }

    /** A `personal_sign` through the REAL `sign_request` machine, approved with [opts]: whether it went to the signer. */
    private suspend fun signs(opts: SignApproveOpts): Boolean {
        val signed = CompletableDeferred<Unit>()
        fun answer(op: SignOperation): SignShellResult = when (op) {
            is SignOperation.SendResponse -> SignShellResult.Responded
            is SignOperation.CheckBundlerFunding -> SignShellResult.PreCheck(null)
            is SignOperation.AttemptSponsorship -> SignShellResult.Sponsorship(SignSponsorship.Denied(null))
            is SignOperation.SignAndSubmit -> {
                signed.complete(Unit)
                SignShellResult.Submit(SignSubmitOutcome.Succeeded("0x" + "5a".repeat(65)), System.currentTimeMillis().toDouble())
            }
            is SignOperation.PersistRecord -> SignShellResult.RecordPersisted
            is SignOperation.UpdateRecord -> SignShellResult.RecordUpdated
            is SignOperation.ClearToPost -> SignShellResult.Responded
            is SignOperation.DeleteRecord -> SignShellResult.RecordUpdated
            is SignOperation.SwitchActiveAccount -> SignShellResult.AccountSwitched
            is SignOperation.SimVerdictTimer -> SignShellResult.SimVerdictTimerFired(op.id, op.round)
            is SignOperation.ReadSafeOwner -> SignShellResult.SafeOwnerRead(op.id, op.chain_id, op.safe, app.getvela.wallet.feature.signing.core.SafeOwnerAnswer.Unreachable("stub"))
        }
        val sign = CoreHost(
            bridge = SignRequestCore().asBridge(),
            scope = scope,
            initial = SignView(),
            serializer = SignView.serializer(),
            perform = JsonShell.perform(SignOperation.serializer(), SignShellResult.serializer(), ::answer),
            escapedFailure = JsonShell.escapedFailure(SignOperation.serializer(), SignShellResult.serializer(), fallback = SignShellResult.Responded, answer = ::answer),
            onFault = { error -> throw AssertionError("sign fault: $error", error) },
        )
        sign.dispatch(SignEvent.NetworksChanged(listOf(1)), SignEvent.serializer())
        sign.dispatch(SignEvent.AccountsChanged(listOf(SignAccountRef(ME, "cred-1")), 0), SignEvent.serializer())
        sign.dispatch(
            SignEvent.RequestArrived(
                id = "req-1", method = "personal_sign", params_json = """["0x48656c6c6f","$ME"]""", origin = "https://app.example", transport_id = "tab-1",
                dedicated_transport = true, per_request_chain = 1, granted_address = ME, now_ms = System.currentTimeMillis().toDouble(),
            ),
            SignEvent.serializer(),
        )
        withTimeout(10_000) { sign.view.first { it.surface == SignSurface.Sheet && !it.reconcile_pending } }
        sign.dispatch(SignEvent.ApproveTapped(opts), SignEvent.serializer())
        return withTimeoutOrNull(3_000) { signed.await() } != null
    }

    private companion object {
        const val ME = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        const val NFT = "0xb47e3cd837ddf8e4c57f05d70ab865de6e193bbb"
        const val SPENDER = "0x1e0049783f008a0085193e00003d00cd54003c71"
        const val ALICE = "0xD400866e00B055B20752a826CD5C89b811de130b"
        const val BOB = "0x031d7D57c99CAF891e1C250554691Fd12D84772b"
        const val SAFE = "0x7f3cE90000000000000000000000000000A0de87"
        val HASH = "0x" + "cd".repeat(32)
    }
}
