package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.i18n.I18nCatalogs
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.feature.browser.ExploreLive
import app.getvela.wallet.feature.multisig.MsProposalModel
import app.getvela.wallet.feature.multisig.MultisigWords
import app.getvela.wallet.feature.multisig.amountText
import app.getvela.wallet.feature.multisig.core.MsAccountsView
import app.getvela.wallet.feature.multisig.core.MsProposalsView
import app.getvela.wallet.feature.multisig.core.MsRules
import app.getvela.wallet.feature.multisig.core.arr
import app.getvela.wallet.feature.multisig.core.get
import app.getvela.wallet.feature.multisig.core.kind
import app.getvela.wallet.feature.multisig.core.str
import app.getvela.wallet.feature.multisig.readingTitle
import app.getvela.wallet.feature.multisig.shownConcerns
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.wallet.core.BalanceView
import java.io.File
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Round 10's Android shell fixes, read through the builders the screens draw
 * from: the core's readings and views in (the reading vectors through the
 * link door, the machines' views as JSON), what the app says out.
 */
class R10ShellFixesTest {

    private val repoRoot = File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set"))
    private val assets = File(repoRoot, "assets/i18n")

    private val strings: I18nRuntime = I18nRuntime(
        readOnDemand = { namespace, tag -> File(assets, I18nCatalogs.assetPath(tag, namespace).removePrefix("i18n/")).takeIf { it.isFile }?.readBytes() },
    ) { tag -> File(assets, "$tag.json").readBytes() }.apply {
        initialize("en")
        include(I18nCatalogs.MULTISIG)
    }

    private val alice = "0xD400866e00B055B20752a826CD5C89b811de130b"
    private val bob = "0x031d7D57c99CAF891e1C250554691Fd12D84772b"
    private val carol = "0x58cd0ce6A27099220543b31710d7860d75Ba1d3d"
    private val safe = "0x726726d55Aa06DB856d4E6a9e65e8773829BC302"
    private val hash = "0x" + "ab".repeat(32)

    private fun json(text: String): JsonElement = Wire.json.parseToJsonElement(text)

    private val words = MultisigWords(
        strings = strings,
        accounts = MsAccountsView(),
        localNames = mapOf(alice.lowercase() to "Alice"),
        networkName = { if (it == 5042002) "Arc Testnet" else null },
        nativeSymbol = { "USDC" },
    )

    private val cases: List<JsonElement> by lazy {
        Wire.json.parseToJsonElement(File(repoRoot, "rust/crates/vela-core/tests/vectors/multisig/reading.json").readText())["cases"].arr
    }

    /** A vector's review as the core reads it, through the link door (never the vector's own expectation). */
    private fun reviewOf(name: String, extra: Map<String, JsonElement> = emptyMap()): JsonObject {
        val case = cases.first { it["name"].str.orEmpty().startsWith(name) }
        val got = MsRules.call("review.reading", (case["args"] as JsonObject).toMap())!!
        return JsonObject(mapOf("reading" to got["reading"]!!, "concerns" to (got["concerns"] ?: JsonArray(emptyList()))) + extra)
    }

    // -- r10-8: the sheet's title is the core reading's --------------------------------

    @Test
    fun the_sheet_is_titled_by_the_core_reading_as_web_and_desktop_title_it() {
        val r = { key: String -> strings.t("home.multisig.review.reading.$key") }
        assertEquals("${r("approve")} · USDC", readingTitle(reviewOf("approve(spender, 5 USDC)"), 5042002, words))
        assertEquals(r("batchOne"), readingTitle(reviewOf("multiSend called four deep"), 5042002, words))
        assertEquals(r("batchOne"), readingTitle(reviewOf("a batch whose leg calls multiSend"), 5042002, words))
        assertEquals(
            strings.t("home.multisig.review.reading.batchMany", mapOf("total" to "2")),
            readingTitle(reviewOf("a batch: a transfer and an unlimited approve"), 5042002, words),
        )
        assertEquals("${r("send")} 0.01 USDC", readingTitle(reviewOf("Arc's USDC ERC-20 transfer"), 5042002, words))
        assertNull("a member change says its own title", readingTitle(reviewOf("a member change"), 5042002, words))
        assertFalse(readingTitle(reviewOf("approve(spender, 5 USDC)"), 5042002, words)!!.contains("Contract call"))
    }

    @Test
    fun the_proposal_model_heads_with_the_reading_else_its_action() {
        val review = reviewOf("approve(spender, 5 USDC)")
        val m = MsProposalModel(proposals(reviews = mapOf("5042002:$hash" to review)).items.single(), proposals(reviews = mapOf("5042002:$hash" to review)), words, null)
        assertEquals("${strings.t("home.multisig.review.reading.approve")} · USDC", m.title)
        val plain = proposals()
        assertEquals(strings.t("home.multisig.action.call"), MsProposalModel(plain.items.single(), plain, words, null).title)
    }

    // -- r10-6: a withheld simulation draws nothing, nor a revert in its place -----------

    @Test
    fun a_refusal_withholds_the_simulation_and_its_revert_and_says_its_reason_once() {
        val deep = reviewOf("multiSend called four deep")
        val withRevert = JsonObject(
            deep + mapOf(
                "concerns" to JsonArray(deep["concerns"].arr + json("""{"kind":"reverts"}""")),
                "simulation" to json("""{"kind":"withheld","deltas":[]}"""),
                "refused" to json("""{"why":"tooDeep","key":"home.multisig.review.refusedWhy.tooDeep"}"""),
            ),
        )
        val shown = shownConcerns(withRevert).map { it.kind }
        assertFalse("no 'expected to fail' under 'Vela won't sign this': $shown", "reverts" in shown)
        // r10 follow-up (core §10.2): the review leaves the deep batch's own
        // concern out at the source (`multisig_review`); the sheet no longer
        // filters it, it draws what the review says. (This review is built
        // from the READING's concerns, which still carry it.)
        assertTrue("the sheet draws what it is handed: $shown", "unreadableBatch" in shown)
        assertTrue("what else it says stays: $shown", "nestedBatch" in shown)
        // An open (not refused) review keeps its revert.
        val open = JsonObject(mapOf("concerns" to json("""[{"kind":"reverts"}]"""), "simulation" to json("""{"kind":"deltas","deltas":[]}""")))
        assertEquals(listOf("reverts"), shownConcerns(open).map { it.kind })
    }

    // -- r10 S4: amounts are grouped --------------------------------------------------

    @Test
    fun a_big_amount_is_grouped_and_keeps_every_place() {
        assertEquals("1,000,000 USDC", amountText(json("""{"raw":"1000000000000","decimals":6,"plain":"1000000","unlimited":false}"""), "USDC", words))
        assertEquals("1,234.000001 USDC", amountText(json("""{"raw":"1234000001","decimals":6,"plain":"1234.000001","unlimited":false}"""), "USDC", words))
        assertEquals("0.01 USDC", amountText(json("""{"raw":"10000","decimals":6,"plain":"0.01","unlimited":false}"""), "USDC", words))
    }

    // -- r10-7: an executed one is said as it ran ----------------------------------------

    private fun proposals(
        status: String = "pending",
        approvers: List<String> = listOf(bob),
        frozen: String? = null,
        timeline: String? = null,
        reviews: Map<String, JsonElement> = emptyMap(),
    ): MsProposalsView = MsProposalsView.of(
        json(
            """
            {"followed":"$safe","standings":{},
             ${frozen?.let { "\"frozen\":{\"5042002:$hash\":$it}," }.orEmpty()}
             ${timeline?.let { "\"timelines\":{\"5042002:$hash\":$it}," }.orEmpty()}
             "reviews":${JsonObject(reviews)},
             "items":[{"chainId":5042002,"safeTxHash":"$hash","nonce":"4",
              "proposer":"$bob","approvals":[${approvers.joinToString(",") { "{\"owner\":\"$it\"}" }}],"approved":${approvers.size},"threshold":2,"status":"$status",
              "canApprove":[],"canExecute":[],"cancelledBy":null,"sameNonce":[],
              "executing":false,"createdMs":1,"unsent":false,"renameUndeployed":false,
              "review":{"action":{"kind":"call","to":"$carol","value":"0","data":"0x1234"}}}],
             "chains":[{"chainId":5042002,"members":{"deployed":true,"owners":["$alice","$bob","$carol"],"threshold":2,"nonce":"5"},"items":[],"mailbox":true}]}
            """.trimIndent(),
        ),
    )

    private fun model(view: MsProposalsView) = MsProposalModel(view.items.single(), view, words, null)

    @Test
    fun executed_by_its_own_one_press_says_executed_and_who_sent_it() {
        val view = proposals(
            status = "done",
            frozen = """{"executor":"$alice","threshold":2,"approvers":["$bob","$alice"],"executorApproved":true,"source":"chain","atMs":1}""",
            timeline = """[{"kind":"created","who":"$bob"},{"kind":"approved","who":"$bob"},{"kind":"approved","who":"$alice","by_executing":true},{"kind":"executed","who":"$alice","tx_hash":"0x01"}]""",
        )
        val m = model(view)
        assertEquals("never '1/2' beside an execution that counted 2", strings.t("home.multisig.proposal.executed"), m.approvalsCount)
        assertEquals(listOf(bob, alice), m.listedMembers)
        assertEquals(strings.t("home.multisig.proposal.executed"), m.memberState(alice).text)
        assertEquals(strings.t("home.multisig.proposal.approved"), m.memberState(bob).text)
        assertTrue(m.memberState(alice).done)
        assertEquals(
            "${strings.t("home.multisig.proposal.executedBy", mapOf("name" to "Alice"))} · " +
                strings.t("home.multisig.proposal.countedThen", mapOf("approved" to "2", "threshold" to "2")),
            m.executedFacts,
        )
        assertTrue(m.timeline.mapNotNull(m::timelineLine).any { it == strings.t("home.multisig.timeline.executedBy", mapOf("name" to "Alice")) })
    }

    @Test
    fun executed_before_its_facts_are_read_waits_on_nobody() {
        val m = model(proposals(status = "done", timeline = """[{"kind":"executed","tx_hash":"0x01"}]"""))
        assertEquals(strings.t("home.multisig.proposal.executed"), m.approvalsCount)
        assertEquals(listOf(bob), m.listedMembers)
        assertEquals(strings.t("home.multisig.proposal.approvalsSigned"), m.executedFacts)
        // Open: counted against today's threshold, the rest waiting.
        val open = model(proposals())
        assertEquals("1/2", open.approvalsCount)
        assertEquals(strings.t("home.multisig.proposal.waiting"), open.memberState(alice).text)
        assertNull(open.executedFacts)
    }

    // -- r10 S4: Approve alone is said why ---------------------------------------------

    @Test
    fun an_approve_that_would_complete_it_says_why_it_cannot_also_execute() {
        fun view(look: String, oneTap: Boolean = false) = MsProposalsView.of(
            json(
                """
                {"followed":"$safe","standings":{},
                 "previews":{"5042002:$hash:${alice.lowercase()}":$look},
                 "approve_execute":{"5042002:$hash":${if (oneTap) "[\"$alice\"]" else "[]"}},
                 "items":[{"chainId":5042002,"safeTxHash":"$hash","nonce":"4","proposer":"$bob","approvals":[{"owner":"$bob"}],"approved":1,"threshold":2,
                  "status":"pending","canApprove":["$alice"],"canExecute":[],"cancelledBy":null,"sameNonce":[],"executing":false,"createdMs":1,
                  "unsent":false,"renameUndeployed":false,"review":{"action":{"kind":"call","to":"$carol","value":"0","data":"0x1234"}}}],
                 "chains":[]}
                """.trimIndent(),
            ),
        )
        val short = """{"kind":"priced","fee":{"amount":"420000000000000000"},"balance":"190000000000000000","canPay":false}"""
        val said = model(view(short)).approveOnlyWhy(alice)
        assertTrue("said, with who cannot pay: $said", said != null && said.first.contains("Alice") && said.first.contains("0.42"))
        assertNull("offered as one press: its own fee line says it", model(view(short, oneTap = true)).approveOnlyWhy(alice))
        assertNull("nothing stops it", model(view("""{"kind":"priced","fee":{"amount":"1"},"balance":"5","canPay":true}""")).approveOnlyWhy(alice))
    }

    // -- r10-9: a multisig's connect sheet says what is true of it -----------------------

    @Test
    fun a_multisig_connect_sheet_never_says_ask_you_to_sign() {
        val note = strings.t("connect.browser.multisigConnectNote")
        val body = strings.t("connect.browser.body")
        val fallback = app.getvela.wallet.feature.explore.ExploreFixtures
            .buildState(app.getvela.wallet.feature.explore.ExploreScreenState.E2, strings).connection
        val multisig = ExploreLive.consent(fallback, dbrConsent(safe), strings, ExploreLive.Identity(accountName = "Team", accountAddress = safe, multisig = true))
        assertEquals(note, multisig.explainer)
        assertFalse(multisig.explainer == body)
        val plain = ExploreLive.consent(fallback, dbrConsent(alice), strings, ExploreLive.Identity(accountName = "Alice", accountAddress = alice))
        assertEquals(body, plain.explainer)
    }

    // -- r10-1 (shell): never "Updated now" beside "Checking…" -----------------------------

    @Test
    fun the_refresh_control_is_not_updated_while_the_first_read_is_out() {
        // Round 16 leftover 6: the stamp is the core's one answer (`updated_at_ms`),
        // which it withholds while the first read is out.
        val checking = WalletLive.refresh(BalanceView(last_refreshed_at_ms = 1_000.0, checking_key = "wallet.checking", updated_at_ms = null), strings, now = 1_000)
        assertNull(checking.updated)
        val read = WalletLive.refresh(BalanceView(last_refreshed_at_ms = 1_000.0, updated_at_ms = 1_000.0), strings, now = 1_000)
        assertTrue(read.updated != null)
    }

    private fun dbrConsent(address: String) =
        app.getvela.wallet.feature.browser.core.DbrConsentView(tab = "t1", origin = "https://app.example", address = address, chain_id = 5042002)

    private fun JsonObject.toMap(): Map<String, Any?> = mapValues { (_, v) -> toAny(v) }

    private fun toAny(v: JsonElement): Any? = when (v) {
        is JsonObject -> v.toMap()
        is JsonArray -> v.map(::toAny)
        is JsonPrimitive -> if (v.isString) v.content else v.content.toLongOrNull() ?: v.content.toDoubleOrNull() ?: v.content.toBooleanStrictOrNull()
        else -> null
    }
}
