package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.i18n.I18nCatalogs
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.feature.multisig.MsProposalModel
import app.getvela.wallet.feature.multisig.MultisigWords
import app.getvela.wallet.feature.multisig.core.MsAccountsView
import app.getvela.wallet.feature.multisig.core.MsProposalsView
import app.getvela.wallet.feature.multisig.core.MsRules
import app.getvela.wallet.feature.multisig.core.MsSignDoor
import app.getvela.wallet.feature.multisig.core.MultisigHub
import app.getvela.wallet.feature.multisig.core.ev
import app.getvela.wallet.feature.multisig.core.get
import app.getvela.wallet.feature.multisig.core.kind
import app.getvela.wallet.feature.multisig.core.str
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import app.getvela.wallet.feature.multisig.draftDoor
import app.getvela.wallet.feature.multisig.draftReviewAgain
import app.getvela.wallet.feature.multisig.reviewFailedReason
import app.getvela.wallet.feature.multisig.reviewUncheckedLine
import java.io.File
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * §12 (docs/multisig-machines.md): every review has a state — `loading`,
 * `ready`, `failed {reason_key}` — and every button that signs, proposes,
 * approves or executes is the core's sign door over it (`review.door`
 * through the link door): enabled only when the review is read and its rule
 * allows it. The shared vectors replayed through this shell's link-door
 * call, and the sheets' builders read from the machines' views.
 */
class MultisigReviewStateTest {

    private val repoRoot = File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set"))
    private val vectors = File(repoRoot, "rust/crates/vela-core/tests/vectors/multisig/review-state.json")
    private val assets = File(repoRoot, "assets/i18n")

    private val strings: I18nRuntime = I18nRuntime(
        readOnDemand = { namespace, tag -> File(assets, I18nCatalogs.assetPath(tag, namespace).removePrefix("i18n/")).takeIf { it.isFile }?.readBytes() },
    ) { tag -> File(assets, "$tag.json").readBytes() }.apply {
        initialize("en")
        include(I18nCatalogs.MULTISIG)
    }

    private val words = MultisigWords(
        strings = strings,
        accounts = MsAccountsView(),
        localNames = emptyMap(),
        networkName = { if (it == 100) "Gnosis" else null },
        nativeSymbol = { if (it == 100) "XDAI" else null },
    )

    private val alice = "0xD400866e00B055B20752a826CD5C89b811de130b"
    private val bob = "0x031d7D57c99CAF891e1C250554691Fd12D84772b"
    private val safe = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private val hash = "0x" + "cd".repeat(32)

    private fun json(text: String): JsonElement = Wire.json.parseToJsonElement(text)

    private val allowed = """{"sign":{"allowed":true,"anyway":false},"concerns":[],"recipients":[],"simulation":{"kind":"deltas","deltas":[]},"state":{"verdict":{"kind":"unchanged"}}}"""
    private val anyway = """{"sign":{"allowed":true,"anyway":true},"concerns":[],"recipients":[],"simulation":{"kind":"deltas","deltas":[]},"state":{"verdict":{"kind":"unchanged"}}}"""
    private val refused = """{"sign":{"allowed":false,"anyway":false},"concerns":[],"recipients":[],"simulation":{"kind":"deltas","deltas":[]},"state":{"verdict":{"kind":"unchanged"}}}"""
    private val loading = """{"kind":"loading"}"""
    private val ready = """{"kind":"ready"}"""
    private fun failed(reason: String) = """{"kind":"failed","reason_key":"home.multisig.review.state.$reason"}"""

    private val item = """{"chainId":100,"safeTxHash":"$hash","nonce":"4","proposer":"$alice","approvals":[{"owner":"$alice"}],"approved":1,"threshold":2,
        "status":"pending","canApprove":["$bob"],"canExecute":[],"sameNonce":[],"executing":false,"createdMs":1,"unsent":false,"renameUndeployed":false,
        "tx":{"to":"$bob","value":"1","data":"0x","operation":0,"nonce":"4"}}"""

    /** The proposals machine's view with a draft whose review is [review] in [state] (`null`: the field left out). */
    private fun draftView(review: String?, state: String?): MsProposalsView {
        val r = review?.let { ""","review":$it""" } ?: ""","review":null"""
        val s = state?.let { ""","reviewState":$it""" }.orEmpty()
        return MsProposalsView.of(json("""{"followed":"$safe","draft":{"item":$item$r$s,"membersHere":["$bob"],"defaultMember":"$bob","executeNow":false}}"""))
    }

    /** The view with one proposal whose review is [review] in [state] (`null`: no entry — not asked yet). */
    private fun proposalView(review: String?, state: String?): MsProposalsView {
        val r = review?.let { """"reviews":{"100:$hash":$it},""" }.orEmpty()
        val s = state?.let { """"review_states":{"100:$hash":$it},""" }.orEmpty()
        return MsProposalsView.of(json("""{"followed":"$safe",$r$s"items":[$item],"chains":[{"chainId":100,"members":{"deployed":true,"owners":["$alice","$bob"],"threshold":2,"nonce":"4"},"items":[],"mailbox":true}]}"""))
    }

    private fun model(view: MsProposalsView) = MsProposalModel(view.items.single(), view, words, null)

    // -- the shared vectors, through this shell's link door ---------------------------

    @Test
    fun the_door_vectors_hold_through_the_link_door() {
        val cases = json(vectors.readText())["door"] as JsonArray
        assertTrue(cases.size >= 10)
        for (case in cases) {
            val name = case["name"].toString()
            val args = case.jsonObject["args"]!!.jsonObject
            val expect = case.jsonObject["expect"]!!.jsonObject
            // The raw answer is the vector's, field for field.
            assertEquals(name, expect, MsRules.call("review.door", args))
            // And the typed door the sheets read says the same.
            val door = MsRules.reviewDoor(args["state"]?.takeUnless { it is JsonNull }, args["review"]?.takeUnless { it is JsonNull })
            assertEquals(name, MsSignDoor.of(expect), door)
        }
    }

    @Test
    fun the_door_words_are_in_the_multisig_corpus() {
        val keys = (json(vectors.readText())["keys"] as JsonArray).map { it.toString().trim('"') }
        for (key in keys) assertNotEquals("$key is said, never its key", key, strings.t(key))
        assertEquals("Reading the transaction…", words.t("review.state.loading"))
        assertEquals("Try again", words.t("review.state.tryAgain"))
    }

    // -- the draft (send, member change, rename, cancellation, a site's request) -------

    @Test
    fun a_drafts_propose_is_shut_while_its_review_is_read() {
        // A review from before, still being read again: shut, the placeholder.
        val reading = draftDoor(draftView(allowed, loading).draft!!)
        assertTrue(reading.loading)
        assertFalse(reading.enabled)
        // A core that has not said a state yet: loading, never "allowed".
        val unsaid = draftDoor(draftView(allowed, null).draft!!)
        assertTrue(unsaid.loading)
        assertFalse(unsaid.enabled)
        // No review at all, no state: shut.
        assertFalse(draftDoor(draftView(null, null).draft!!).enabled)
    }

    @Test
    fun a_drafts_failed_review_blocks_and_offers_try_again() {
        val door = draftDoor(draftView(null, failed("timedOut")).draft!!)
        assertTrue(door.failed)
        assertFalse("nothing is proposed on a review not read", door.enabled)
        assertTrue(door.retry)
        assertEquals(strings.t("home.multisig.review.state.timedOut"), reviewFailedReason(door, words))
        assertEquals(strings.t("home.multisig.review.state.unreadable"), reviewFailedReason(draftDoor(draftView(null, failed("unreadable")).draft!!), words))
        // "Try again" is the machine's own event: the draft's review, read again.
        assertEquals(JsonObject(mapOf("type" to json("\"review_again\""))), draftReviewAgain())
    }

    @Test
    fun a_drafts_propose_opens_after_try_again_reads_it() {
        // failed → Try again (the core says loading at once) → read → open.
        assertFalse(draftDoor(draftView(null, failed("timedOut")).draft!!).enabled)
        assertFalse(draftDoor(draftView(null, loading).draft!!).enabled)
        val open = draftDoor(draftView(allowed, ready).draft!!)
        assertTrue(open.showsReview)
        assertTrue(open.enabled)
        assertFalse(open.anyway)
        // Read and "anyway": open only through the "… anyway" step.
        val through = draftDoor(draftView(anyway, ready).draft!!)
        assertTrue(through.enabled && through.anyway)
        // Read and refused: shown, shut, no override.
        val no = draftDoor(draftView(refused, ready).draft!!)
        assertTrue(no.showsReview)
        assertFalse(no.enabled || no.anyway)
    }

    @Test
    fun a_draft_review_without_a_sign_rule_is_never_allowed() {
        // The old shell read a missing rule as allowed (`?: true`): never again.
        assertFalse(draftDoor(draftView("{}", ready).draft!!).enabled)
        assertFalse(draftDoor(draftView("""{"sign":{}}""", ready).draft!!).enabled)
    }

    // -- a proposal (approve, approve and execute, execute) ----------------------------

    @Test
    fun a_proposals_approve_and_execute_are_shut_while_its_review_is_read() {
        // Not asked yet (no entry), with a review kept from before: shut.
        val unsaid = model(proposalView(allowed, null))
        assertTrue(unsaid.door.loading)
        assertFalse(unsaid.canSign)
        val reading = model(proposalView(allowed, loading))
        assertTrue(reading.door.loading)
        assertFalse(reading.canSign)
        assertFalse(reading.reviewReady)
    }

    @Test
    fun a_proposals_failed_review_blocks_and_its_try_again_names_it() {
        val m = model(proposalView("null", failed("gone")))
        assertTrue(m.door.failed && m.door.retry)
        assertFalse(m.canSign)
        assertFalse(m.signAnyway)
        assertEquals(strings.t("home.multisig.review.state.gone"), reviewFailedReason(m.door, words))
        val again = m.reviewAgain
        assertEquals("review_again", again["type"].toString().trim('"'))
        assertEquals("100", again["chain_id"].toString())
        assertEquals("\"$hash\"", again["safe_tx_hash"].toString())
    }

    @Test
    fun a_proposals_approve_opens_after_try_again_reads_it() {
        assertFalse(model(proposalView("null", failed("timedOut"))).canSign)
        assertFalse(model(proposalView("null", loading)).canSign)
        val open = model(proposalView(allowed, ready))
        assertTrue(open.reviewReady && open.canSign)
        assertFalse(open.signAnyway)
        val through = model(proposalView(anyway, ready))
        assertTrue(through.canSign && through.signAnyway)
        assertFalse(model(proposalView(refused, ready)).canSign)
        assertFalse("no sign rule is never allowed", model(proposalView("{}", ready)).canSign)
    }

    // -- D1: neither safety check read (real-device pass 2) -------------------------

    /** The core's review when the state guard and the simulation both gave up. */
    private val unchecked = """{"sign":{"allowed":true,"anyway":true},"concerns":[{"kind":"stateUnread","reason":"unreachable"},{"kind":"unsimulated"},{"kind":"unchecked"}],"recipients":[],"simulation":{"kind":"unreachable","deltas":[]},"state":{"verdict":{"kind":"unavailable","reason":"unreachable"}}}"""
    private val guardUnread = """{"sign":{"allowed":true,"anyway":false},"concerns":[{"kind":"stateUnread","reason":"unreachable"}],"recipients":[],"simulation":{"kind":"deltas","deltas":[]},"state":{"verdict":{"kind":"unavailable","reason":"unreachable"}}}"""

    @Test
    fun a_review_neither_check_read_is_drawn_in_danger_with_try_again_and_signs_only_anyway() {
        // A proposal: Approve, Approve and execute, Execute all "… anyway".
        val m = model(proposalView(unchecked, ready))
        assertTrue(m.door.showsReview && m.door.unchecked && m.door.retry)
        assertTrue(m.reviewReady && m.canSign)
        assertTrue("every press is \"… anyway\"", m.signAnyway)
        assertEquals("Vela couldn't check what this transaction changes.", reviewUncheckedLine(m.door, words))
        // Its Try again reads this proposal again.
        assertEquals("review_again", m.reviewAgain["type"].toString().trim('"'))
        // A draft: Propose and approve the same.
        val d = draftDoor(draftView(unchecked, ready).draft!!)
        assertTrue(d.unchecked && d.enabled && d.anyway && d.retry)
    }

    /** D8: under a shut Approve (the review being read), the fee line says nothing of approving. */
    @Test
    fun the_fee_line_never_says_you_can_approve_under_a_shut_approve() {
        fun view(state: String): MsProposalsView {
            val canExecute = item.replace(""""canExecute":[]""", """"canExecute":["$bob"]""")
            return MsProposalsView.of(json("""{"followed":"$safe","reviews":{"100:$hash":$allowed},"review_states":{"100:$hash":$state},
                "previews":{"100:${hash.lowercase()}:${bob.lowercase()}":{"kind":"unchecked"}},"items":[$canExecute],
                "chains":[{"chainId":100,"members":{"deployed":true,"owners":["$alice","$bob"],"threshold":2,"nonce":"4"},"items":[],"mailbox":true}]}"""))
        }
        val reading = MsProposalModel(view(loading).items.single(), view(loading), words, null)
        assertFalse(reading.canSign)
        assertEquals("Couldn’t check whether it would run — Execute checks again.", reading.feeLine(bob)?.first)
        val open = MsProposalModel(view(ready).items.single(), view(ready), words, null)
        assertTrue(open.canSign)
        assertEquals("Couldn’t check whether it would run — you can still approve; Execute checks again.", open.feeLine(bob)?.first)
    }

    @Test
    fun one_check_unread_alone_stays_a_plain_caution() {
        val m = model(proposalView(guardUnread, ready))
        assertFalse(m.door.unchecked || m.door.retry)
        assertTrue(m.canSign)
        assertFalse(m.signAnyway)
    }

    // -- the real proposals machine, over this shell ---------------------------------

    @Test
    fun the_real_machine_opens_the_drafts_door_only_once_read_and_again_after_try_again() = runBlocking {
        val code = File(repoRoot, "rust/crates/vela-core/tests/vectors/safe-code")
        val world = MultisigWorld { name -> File(code, "$name.hex").readText().trim() }
        world.locals += listOf(0, 1)
        val owners = (0..2).map(world::address)
        val g = world.genesis(owners, 2, "Quiet Harbor", "0x31")
        val address = g["address"].str!!
        world.deploy(100, address, MultisigWorld.Safe(owners, 2))
        world.store["vela.multisigAccounts"] = "[${world.storedRow(g, listOf(100))}]"
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        try {
            val hub = MultisigHub(
                live = world,
                facts = { MultisigHub.Facts(MultisigWorld.SERVICE, world.localAccounts(), listOf(100), listOf(100), emptyList()) },
                scope = scope,
            )
            // Every draft state the sheet would draw, and the door it would have had then.
            val seen = java.util.concurrent.CopyOnWriteArrayList<Pair<String?, Boolean>>()
            scope.launch {
                hub.proposalsView.collect { v -> v.draft?.let { d -> seen += d.reviewState.kind to draftDoor(d).enabled } }
            }
            withTimeout(30_000) { hub.boot() }
            hub.activate(address)
            withTimeout(10_000) { while (hub.proposalsView.value.followed == null) delay(20) }
            val transfer = mapOf("kind" to "transfers", "transfers" to listOf(mapOf("token" to null, "to" to bob, "amount" to "1000")))
            withTimeout(30_000) {
                hub.proposals.request(ev("draft", "chain_id" to 100, "proposal" to transfer)) { v -> v["draft"] as? JsonObject }
            }
            suspend fun settled(): MsSignDoor = withTimeout(60_000) {
                while (hub.proposalsView.value.draft?.reviewState.kind.let { it == null || it == "loading" }) delay(20)
                draftDoor(hub.proposalsView.value.draft!!)
            }
            val first = settled()
            assertEquals("the world reads it: ready", "ready", hub.proposalsView.value.draft!!.reviewState.kind)
            assertTrue("read and allowed: open", first.showsReview && first.enabled)
            // Try again: read again (loading at once), then open again — never open while loading.
            val before = seen.size
            hub.sendProposal(draftReviewAgain())
            withTimeout(10_000) { while (seen.drop(before).none { it.first == "loading" }) delay(20) }
            val again = settled()
            assertTrue(again.enabled)
            assertTrue("a draft door was open while its review was not ready: $seen", seen.none { (kind, enabled) -> enabled && kind != "ready" })
        } finally {
            scope.cancel()
        }
    }

    @Test
    fun the_views_decode_the_review_states_in_either_hash_case() {
        val view = MsProposalsView.of(json("""{"items":[$item],"review_states":{"100:${hash.uppercase().replace("0X", "0x")}":$ready}}"""))
        assertEquals(json(ready), view.reviewState(view.items.single()))
        assertEquals(json(loading), draftView(null, loading).draft!!.reviewState)
    }
}
