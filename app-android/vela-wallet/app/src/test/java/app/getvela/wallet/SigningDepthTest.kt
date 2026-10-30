package app.getvela.wallet

import androidx.compose.ui.graphics.Color
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.StoreAccountPort
import app.getvela.wallet.feature.send.core.UserOpSpine
import app.getvela.wallet.feature.signing.AllowanceChip
import app.getvela.wallet.feature.signing.SigningAdvice
import app.getvela.wallet.feature.signing.SigningBlock
import app.getvela.wallet.feature.signing.SigningDepthBoards
import app.getvela.wallet.feature.signing.SigningFixtures
import app.getvela.wallet.feature.signing.SigningLive
import app.getvela.wallet.feature.signing.SigningScreenState
import app.getvela.wallet.feature.signing.SigningTone
import app.getvela.wallet.feature.signing.core.ClearLineSource
import app.getvela.wallet.feature.signing.core.ClearMessageLine
import app.getvela.wallet.feature.signing.core.ClearReading
import app.getvela.wallet.feature.signing.core.ClearSurface
import app.getvela.wallet.feature.signing.core.GuardChoice
import app.getvela.wallet.feature.signing.core.GuardEditorMode
import app.getvela.wallet.feature.signing.core.GuardEvent
import app.getvela.wallet.feature.signing.core.PageFailure
import app.getvela.wallet.feature.signing.core.SignErrorKind
import app.getvela.wallet.feature.signing.core.SignEvent
import app.getvela.wallet.feature.signing.core.SignExecutor
import app.getvela.wallet.feature.signing.core.SignSubmitOutcome
import app.getvela.wallet.feature.signing.core.SignView
import app.getvela.wallet.feature.signing.core.SigningController
import app.getvela.wallet.feature.signing.core.SimDeltas
import app.getvela.wallet.feature.signing.trustedsigner.SigningRoute
import kotlinx.coroutines.runBlocking
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.WalletKeyRecord
import java.io.File

/**
 * Android phase 2, P2.3 — the signing sheet's depth, each line the web
 * draws, from the core's own records: the venue everywhere (R7), the
 * message's drawn lines, a reading's provenance, the grant-all's deliberate
 * choice per leg, "Check again", the page's failure words, and the agent's
 * opinion card.
 */
class SigningDepthTest {
    private val en: VelaStrings = run {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
    }
    private val ctx = SigningLive.Context(en, "Ethereum", Color.Blue, "ETH", "Me", "0x88cCA0EeDbF2C4426110bbFc998F048689266894", descriptorHost = "data.getvela.app")

    // -- R7: where each signature goes ---------------------------------------------------

    private val address = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private val credential = "aabbcc"
    private val publicKey = "04" + "11".repeat(64)

    private fun record(venue: String, domain: String = "getvela.app"): JSONObject = JSONObject()
        .put("id", credential).put("name", "Savings").put("created_at_iso", "2026-09-30T10:00:00.000Z")
        .put("address", address).put("public_key_hex", publicKey)
        .put("keys", JSONArray().put(JSONObject().put("credential_id", credential).put("public_key_hex", publicKey).put("name", "Key 1").put("transports", "internal")))
        .put("sign_in_key", JSONObject().put("credential_id", credential).put("method", "platform").put("transports", "internal"))
        .put("signing_domain", domain).put("signing_venue", JSONObject(venue))

    private fun spine(record: JSONObject): UserOpSpine {
        val accounts = AccountStore(FakeStore())
        runBlocking { accounts.saveAccount(record) }
        return UserOpSpine(
            relay = RelayClient(FakeRelayPort(), builtinBase = { "https://builtin.test" }, retryDelayMs = 0),
            accounts = StoreAccountPort(accounts),
            signer = { error("no ceremony in this test") },
        )
    }

    private val page = """{"type":"page","url":"https://sign.getvela.app/"}"""

    @Test
    fun `every kind of signature by a page account is routed to its page by the core`() = runBlocking {
        val onPage = spine(record(page))
        val key = WalletKeyRecord(credential, publicKey)
        // The wallet's own send, a site's transaction, a batch, a message, typed data.
        for (method in listOf("", "eth_sendTransaction", "wallet_sendCalls", "personal_sign", "eth_signTypedData_v4")) {
            val route = onPage.routeFor(address, key, method)
            assertEquals(method, "https://sign.getvela.app/", route.page)
            assertFalse(method, route.unsupported)
            assertEquals(method, "https://sign.getvela.app/", onPage.handoffFor(address, method)?.page)
        }
        // A method no venue signs is refused, never signed in Vela behind the person's choice.
        val ethSign = onPage.routeFor(address, key, "eth_sign")
        assertTrue(ethSign.unsupported)
        assertNull(onPage.handoffFor(address, "eth_sign"))
    }

    @Test
    fun `an in-Vela account signs every kind in Vela, and an unreachable one is blocked`() = runBlocking {
        val inVela = spine(record("""{"type":"in_vela"}"""))
        val key = WalletKeyRecord(credential, publicKey)
        for (method in listOf("", "eth_sendTransaction", "wallet_sendCalls", "personal_sign", "eth_signTypedData_v4", "eth_sign")) {
            val route = inVela.routeFor(address, key, method)
            assertNull(method, route.page)
            assertFalse(method, route.unsupported)
            assertNull(method, route.blocked)
        }
        // R1: keys on a custom domain cannot be reached from Vela's own sheet.
        val custom = record("""{"type":"in_vela"}""", domain = "sign.example.com")
        val blocked = SigningRoute.of(custom.toString(), "personal_sign")
        assertTrue("$blocked", blocked is SigningRoute.Blocked)
        assertNotNull(spine(custom).routeFor(address, key, "personal_sign").blocked)
        assertEquals(SigningRoute.Page("https://sign.getvela.app/"), SigningRoute.of(record(page).toString(), "wallet_sendCalls"))
        assertNull(SigningRoute.of("not a record", "personal_sign"))
    }

    @Test
    fun `the page's no is PageDeclined and its failure is the core's PageFailure`() {
        assertEquals(SignSubmitOutcome.PageDeclined, SignExecutor.outcomeOf(UserOpSpine.Failure.PageDeclined))
        for (failure in PageFailure.entries) {
            assertEquals(SignSubmitOutcome.PageFailed(failure), SignExecutor.outcomeOf(UserOpSpine.Failure.PageFailed(failure)))
        }
        // The wire the core reads.
        assertEquals("""{"type":"page_declined"}""", Wire.json.encodeToString(SignSubmitOutcome.serializer(), SignSubmitOutcome.PageDeclined))
    }

    @Test
    fun `a page failure is said as what happened there, never as a generic error`() {
        assertEquals("componentsUi.signing.trustedSignerUnanswered", SigningLive.pageFailureKey(SignErrorKind.PageUnanswered))
        assertEquals("componentsUi.signing.trustedSignerMismatch", SigningLive.pageFailureKey(SignErrorKind.PageMismatch))
        assertEquals("componentsUi.signing.trustedSignerRefused", SigningLive.pageFailureKey(SignErrorKind.PageRefused))
        assertEquals("componentsUi.signing.integrity.couldNotCheck", SigningLive.pageFailureKey(SignErrorKind.PageNotOpened))
        assertNull(SigningLive.pageFailureKey(SignErrorKind.SubmitFailed))
        // The sheet's own line (CS76): the core's words, not "couldn't submit".
        val board = SigningFixtures.build(SigningScreenState.CS76, en)
        val said = board.blocks.filterIsInstance<SigningBlock.Warning>().map { it.text }
        assertTrue("$said", en.t("componentsUi.signing.trustedSignerUnanswered") in said)
        assertFalse("$said", en.t("send.txErrorGeneric") in said)
        // And the site hears the core's developer words for it.
        for (kind in listOf("page_unanswered", "page_mismatch", "page_refused", "page_not_opened")) {
            assertTrue(kind, uniffi.vela_core_uniffi.signErrorWords(kind, null)?.isNotBlank() == true)
        }
    }

    // -- landing_closed -----------------------------------------------------------------

    @Test
    fun `landing_closed is the core's event, by the operation's hash`() {
        val json = Wire.json.encodeToString(SignEvent.serializer(), SignEvent.LandingClosed("0xab"))
        assertEquals("""{"type":"landing_closed","user_op_hash":"0xab"}""", json)
    }

    // -- the guard's grant, per leg ------------------------------------------------------

    @Test
    fun `a grant-all chip is the deliberate grant, on its own leg`() {
        assertEquals(GuardEvent.GrantDeliberatelyChosen, SigningController.chipEvent("grant", null))
        assertEquals(GuardEvent.LegGrantDeliberatelyChosen(2), SigningController.chipEvent("grant", 2))
        assertEquals(GuardEvent.PresetSelected(GuardEditorMode.Revoke), SigningController.chipEvent("revoke", null))
        assertEquals(GuardEvent.LegPresetSelected(1, GuardEditorMode.Custom), SigningController.chipEvent("custom", 1))
        assertNull(SigningController.chipEvent("nonsense", null))
        assertEquals(
            """{"type":"leg_grant_deliberately_chosen","index":2}""",
            Wire.json.encodeToString(GuardEvent.serializer(), GuardEvent.LegGrantDeliberatelyChosen(2)),
        )
    }

    @Test
    fun `a grant-all is allow all or revoke, never an amount, and the confirm says to choose`() {
        val guard = SigningDepthBoards.grantAllGuard()
        val card = SigningLive.guardBlocks(guard, en).filterIsInstance<SigningBlock.Allowance>().single()
        assertEquals(listOf("grant", "revoke"), card.chips.map { it.id })
        assertEquals(en.t("componentsUi.signing.certain.chipGrantAll"), card.chips.first().label)
        assertTrue(card.chips.all { it.state == AllowanceChip.ChipState.Idle })
        assertEquals("—", card.value)
        assertEquals(en.t("componentsUi.signing.labelApproved"), card.label)
        val granted = guard.copy(editor = guard.editor!!.copy(mode = GuardEditorMode.Grant, choice = GuardChoice.Grant))
        val chosen = SigningLive.guardBlocks(granted, en).filterIsInstance<SigningBlock.Allowance>().single()
        assertEquals(en.t("componentsUi.signing.certain.chipGrantAll"), chosen.value)
        assertEquals(SigningTone.Danger, chosen.valueTone)
        assertEquals(AllowanceChip.ChipState.Selected, chosen.chips.first { it.id == "grant" }.state)
        assertEquals(en.t("componentsUi.signing.certain.chooseGrant"), SigningFixtures.build(SigningScreenState.CS73, en).confirmBlockLine)
    }

    // -- the message, as the core draws it ------------------------------------------------

    @Test
    fun `display_lines are decoded and drawn - a hidden character muted, empty lines folded`() {
        val view = SigningDepthBoards.messageView()
        assertEquals(ClearSurface.MessageSign, view.surface)
        val message = view.message!!
        assertTrue(message.has_invisible_chars)
        val lines = message.display_lines
        assertTrue("the core drew the lines: $lines", lines.isNotEmpty())
        val markers = lines.filterIsInstance<ClearMessageLine.Text>().flatMap { it.runs }.filter { it.marker }
        assertTrue("$markers", markers.any { it.text.contains("202E") })
        val folded = lines.filterIsInstance<ClearMessageLine.Blank>().single()
        assertTrue("ten empty lines, folded into one that counts them", folded.count >= 3)

        val code = SigningLive.messageCode(message, en)!!
        val runs = code.runs!!
        assertTrue(runs.flatten().any { it.muted && it.text.contains("202E") })
        assertTrue(runs.any { line -> line.size == 1 && line[0].muted && line[0].text.contains(folded.count.toString()) })
        assertEquals(lines.size, code.lines.size)

        val blocks = SigningLive.blocks(view, null, 0, ctx)
        assertTrue(blocks.any { it is SigningBlock.Code && it.runs != null })
        assertTrue(blocks.any { it is SigningBlock.Warning && it.tone == SigningTone.Danger && it.text == en.t("componentsUi.signing.hiddenChars") })
    }

    // -- a reading and where it came from -----------------------------------------------

    @Test
    fun `a reading says who described it and what Vela's decode says against it`() {
        val blocks = SigningLive.blocks(SigningDepthBoards.readingView(), null, 0, ctx)
        val sources = blocks.filterIsInstance<SigningBlock.Source>().map { it.text }
        val warnings = blocks.filterIsInstance<SigningBlock.Warning>()
        assertTrue("$sources", sources.any { it.contains("Uniswap Labs") && it.contains("ERC-7730") })
        assertTrue("$warnings", warnings.any { it.tone == SigningTone.Danger && it.text.contains("Claim") })
        assertTrue(warnings.any { it.tone == SigningTone.Danger && it.text == en.t("componentsUi.signing.certain.elsewhere") })
        assertTrue(warnings.any { it.tone == SigningTone.Caution && it.text == en.t("componentsUi.signing.certain.dustMinimum") })
        assertTrue("$sources", sources.any { it.contains("WETH?") && it.contains("own contract") })
        assertTrue("$sources", sources.any { it.contains("Universal Router") })
        // The provenance line sits right under the intent.
        assertTrue(blocks[0] is SigningBlock.Intent)
        assertTrue(blocks[1] is SigningBlock.Source)
    }

    @Test
    fun `a name from the public database is no headline, and a Vela reading says so`() {
        val view = SigningDepthBoards.readingView()
        val result = view.result!!.copy(reading = ClearReading(source = ClearLineSource.SelectorDb, selector_name = "claimRewards(address)"))
        val blocks = SigningLive.blocks(view.copy(result = result), null, 0, ctx)
        assertEquals(en.t("componentsUi.signing.intentContractCall"), (blocks[0] as SigningBlock.Intent).text)
        val caution = blocks[1] as SigningBlock.Warning
        assertEquals(SigningTone.Caution, caution.tone)
        assertTrue(caution.text.contains("claimRewards(address)"))

        val own = SigningLive.blocks(view.copy(result = view.result!!.copy(reading = ClearReading())), null, 0, ctx)
        assertEquals(en.t("componentsUi.signing.certain.readByVela"), (own[1] as SigningBlock.Source).text)
    }

    // -- check again ----------------------------------------------------------------------

    @Test
    fun `a node nobody reached offers check again, and the line stays while it runs`() {
        val unreachable = SigningDepthBoards.unreachable() as SigningController.SimOutcome.Notice
        assertTrue("no node answered: about the network", unreachable.recheck)
        assertFalse("moves nobody could read are not the network's", SimDeltas.couldNotCheck().recheck)
        assertFalse(SimDeltas.reverted("nope")!!.recheck)

        fun warning(place: List<SigningBlock>) = ((place.single() as SigningBlock.Held).shown as SigningBlock.Warning)
        val offered = warning(SigningLive.verdictPlace(unreachable, SignView(), ctx.copy(simAgain = true)))
        assertEquals(SigningBlock.Recheck(en.t("componentsUi.signing.simRecheck"), checking = false), offered.recheck)
        val running = warning(SigningLive.verdictPlace(unreachable, SignView(), ctx.copy(simRechecking = true)))
        assertEquals(SigningBlock.Recheck(en.t("componentsUi.signing.simRechecking"), checking = true), running.recheck)
        // Not while the core still holds the confirm, or once nothing is on offer.
        assertNull(warning(SigningLive.verdictPlace(unreachable, SignView(), ctx)).recheck)
        val checking = SigningLive.verdictPlace(unreachable, SignView(sim_checking = true), ctx.copy(simAgain = true))
        assertNull("the core's skeleton, not a stale line", (checking.single() as SigningBlock.Held).shown)
        // The boards.
        assertEquals(false, warning(SigningFixtures.build(SigningScreenState.CS74, en).blocks.filterIsInstance<SigningBlock.Held>()).recheck?.checking)
        assertEquals(true, warning(SigningFixtures.build(SigningScreenState.CS75, en).blocks.filterIsInstance<SigningBlock.Held>()).recheck?.checking)
    }

    // -- the agent's opinion ---------------------------------------------------------------

    @Test
    fun `the agent's opinion stands as the core says, and the card is greyed once it is not current`() {
        val advice = SigningDepthBoards.accepted()
        val current = SigningAdvice.standing(advice, advice.review_digest, null)
        assertEquals(SigningAdvice.Standing.Current, current.standing)
        val earlier = SigningAdvice.standing(advice, "0x" + "cd".repeat(32), null)
        assertEquals(SigningAdvice.Standing.EarlierVersion, earlier.standing)

        val card = SigningAdvice.card(advice, current, en)
        assertFalse(card.stale)
        assertEquals(en.t("componentsUi.signing.agentAdvice.caution"), card.verdictWord)
        assertEquals(en.t("componentsUi.signing.agentAdvice.title"), card.title)
        assertEquals(listOf("etherscan.io"), card.hosts)
        val stale = SigningAdvice.card(advice, earlier, en)
        assertTrue(stale.stale)
        assertEquals(en.t("componentsUi.signing.agentAdvice.stale"), stale.staleLine)

        // The boards draw it beside the sheet, never in Vela's own lines.
        val board = SigningFixtures.build(SigningScreenState.CS77, en)
        assertEquals(false, board.advice?.stale)
        assertEquals(true, SigningFixtures.build(SigningScreenState.CS78, en).advice?.stale)
        assertEquals(SigningFixtures.build(SigningScreenState.CS1, en).blocks, board.blocks)
    }

    @Test
    fun `an opinion from an agent the wallet does not let in is refused by the core`() {
        val args = JSONObject().put("review_digest", "0x" + "ab".repeat(32)).put("verdict", "caution").put("summary", "hm").toString()
        val answer = SigningAdvice.accept("{}", "0x" + "ab".repeat(32), null, args)
        assertTrue("$answer", answer is SigningAdvice.Answer.Refused)
    }
}
