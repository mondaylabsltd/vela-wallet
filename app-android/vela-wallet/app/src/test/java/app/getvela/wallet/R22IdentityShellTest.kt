package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.i18n.I18nCatalogs
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.flows.FlowLive
import app.getvela.wallet.feature.multisig.MultisigWords
import app.getvela.wallet.feature.multisig.core.MsProposalsView
import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.onboarding.core.IdentityCheck
import app.getvela.wallet.feature.onboarding.core.RegistryClient
import app.getvela.wallet.feature.onboarding.core.RegistryResolver
import app.getvela.wallet.feature.onboarding.core.SessionView
import app.getvela.wallet.feature.send.core.FeeView
import app.getvela.wallet.feature.signing.SigningBlock
import app.getvela.wallet.feature.signing.SigningLive
import app.getvela.wallet.feature.signing.SigningTone
import app.getvela.wallet.feature.signing.core.ClearSigningView
import app.getvela.wallet.feature.signing.core.GuardView
import app.getvela.wallet.feature.signing.core.SignErrorKind
import app.getvela.wallet.feature.signing.core.SignView
import app.getvela.wallet.feature.signing.core.SigningController
import app.getvela.wallet.feature.wallet.core.AccountIdentityCheck
import app.getvela.wallet.feature.wallet.core.ReceiveStanding
import com.sun.net.httpserver.HttpServer
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.runBlocking
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File
import java.net.InetSocketAddress

/**
 * Round 22 (§37) on Android: what the core decided about identity, approvals,
 * sign-in and the multisig mailbox reaches this shell's screens as the core
 * wrote it.
 *
 * - review 1-7: the unit walk's `verified_by` passes through to the core
 *   untouched, the confirm walk is answered with the registry walks' own
 *   reader (the shared lying-index vector, replayed here), the stored record
 *   keeps its `identity`, and Receive shows no address the core says not to;
 * - review 1-12: a batch's kept-unlimited legs and the core's warning key;
 * - review 1-11: a refused sign-in in its own words;
 * - review 1-13: the non-members' proposals behind their disclosure.
 */
class R22IdentityShellTest {
    private val root: String = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
    private val strings: VelaStrings = I18nRuntime(
        readOnDemand = { namespace, tag -> File(root, "assets/" + I18nCatalogs.assetPath(tag, namespace)).takeIf { it.isFile }?.readBytes() },
    ) { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply {
        initialize("en")
        include(I18nCatalogs.MULTISIG)
    }
    private val chains: JSONObject by lazy {
        JSONObject(File(root, "app-web/vela-wallet/src/lib/onboarding/core/__fixtures__/registry-chain.json").readText())
    }
    private val vector: JSONObject by lazy {
        JSONObject(File(root, "rust/crates/vela-core/tests/vectors/rules/lying-index.json").readText())
    }
    private var server: HttpServer? = null

    @After
    fun stop() { server?.stop(0) }

    private fun reader(down: Set<Int>): suspend (Int, String, String) -> String? = { chainId, _, data ->
        if (chainId in down) null else chains.getJSONObject("answers").optJSONObject(chainId.toString())?.optString(data)?.takeIf { it.isNotEmpty() }
    }

    /** A client whose index answers every unit query with [unit] (a real local server). */
    private fun client(unit: String?, down: Set<Int>): RegistryClient {
        server?.stop(0)
        val http = HttpServer.create(InetSocketAddress("127.0.0.1", 0), 0)
        http.createContext("/api/query") { exchange ->
            val bytes = (unit ?: "{}").toByteArray()
            exchange.sendResponseHeaders(if (unit == null) 503 else 200, bytes.size.toLong())
            exchange.responseBody.use { it.write(bytes) }
        }
        http.start()
        server = http
        return RegistryClient("http://127.0.0.1:${http.address.port}", RegistryResolver(reader(down)))
    }

    private fun downOf(case: JSONObject): Set<Int> = case.optJSONArray("down")?.let { a -> (0 until a.length()).map(a::getInt).toSet() } ?: emptySet()

    // -- A. identity ----------------------------------------------------------

    @Test
    fun `the lying-index vector replays through this shell's resolver loop`() = runBlocking {
        val cases = vector.getJSONArray("cases")
        val key = vector.getString("publicKey")
        var units = 0
        var confirms = 0
        for (i in 0 until cases.length()) {
            val case = cases.getJSONObject(i)
            val expect = case.getJSONObject("expect")
            val name = case.getString("name")
            when (case.getString("kind")) {
                "unit" -> {
                    val unit = client(case.getJSONObject("index").toString(), downOf(case)).queryUnit(case.getLong("unitId"))
                    assertEquals(name, expect.getString("verified_by"), unit.verifiedBy)
                    assertEquals(name, expect.getInt("members"), unit.members.size)
                    units++
                }
                "confirm" -> {
                    val check = RegistryClient("http://127.0.0.1:9", RegistryResolver(reader(downOf(case))))
                        .confirmIdentity(key, case.getString("address"))
                    assertEquals(name, IdentityCheck(expect.getString("identity"), expect.getString("verified_by")), check)
                    confirms++
                }
            }
        }
        assertTrue("both kinds replayed", units > 0 && confirms > 0)
    }

    @Test
    fun `the confirm walk asks only the chain, never the index`() = runBlocking {
        var indexAsked = false
        val http = HttpServer.create(InetSocketAddress("127.0.0.1", 0), 0)
        http.createContext("/") { exchange -> indexAsked = true; exchange.sendResponseHeaders(503, -1); exchange.close() }
        http.start()
        server = http
        val asked = mutableListOf<Int>()
        val client = RegistryClient(
            "http://127.0.0.1:${http.address.port}",
            RegistryResolver({ chainId, to, data -> asked += chainId; reader(emptySet())(chainId, to, data) }),
        )
        val confirm = vector.getJSONArray("cases").let { a -> (0 until a.length()).map(a::getJSONObject) }.first { it.getString("kind") == "confirm" }
        client.confirmIdentity(vector.getString("publicKey"), confirm.getString("address"))
        assertFalse("the index is never asked", indexAsked)
        assertTrue(asked.isNotEmpty())
        // No resolver at all: nothing can vouch — unconfirmed, never confirmed.
        assertEquals(IdentityCheck("unconfirmed", "none"), RegistryClient("http://127.0.0.1:9", null).confirmIdentity("04ab", confirm.getString("address")))
    }

    /** A record the core reads, with [identity] (absent = confirmed). */
    private fun record(identity: String? = null, address: String = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"): JSONObject {
        val key = vector.getString("publicKey")
        return JSONObject()
            .put("id", "76656c612d666978747572652d3031").put("name", "Parallel space").put("address", address)
            .put("public_key_hex", key).put("created_at_iso", "2026-11-08T00:00:00Z")
            .put("keys", JSONArray().put(JSONObject().put("credential_id", "76656c612d666978747572652d3031").put("public_key_hex", key)))
            .apply { if (identity != null) put("identity", identity) }
    }

    @Test
    fun `the stored record keeps its identity through every write, and confirmed is never written`() = runBlocking {
        val store = AccountStore(FakeStore())
        store.saveAccount(record("unconfirmed"))
        val stored = store.loadAccounts().getJSONObject(0)
        assertEquals("unconfirmed", stored.optString("identity"))
        // The session's row carries the record whole, identity included.
        val view = SessionView.from(
            JSONObject().put("loading", false).put("has_wallet", true).put("address", stored.getString("address")).put("active_index", 0)
                .put("allowed_route", "wallet").put("accounts", JSONArray().put(JSONObject().put("index", 0).put("account", stored))),
        )
        assertEquals("unconfirmed", JSONObject(view.activeRow!!.accountJson).optString("identity"))
        // The core's re-save with what the chain found: confirmed drops the field.
        val confirmed = JSONObject(uniffi.vela_core_uniffi.accountWithIdentity(stored.toString(), "confirmed")!!)
        assertFalse("confirmed is not written", confirmed.has("identity"))
        store.saveAccount(confirmed)
        assertFalse(store.loadAccounts().getJSONObject(0).has("identity"))
        store.saveAccount(JSONObject(uniffi.vela_core_uniffi.accountWithIdentity(confirmed.toString(), "refuted")!!))
        assertEquals("refuted", store.loadAccounts().getJSONObject(0).optString("identity"))
    }

    @Test
    fun `Receive stands where the core says`() {
        val confirmed = ReceiveStanding.of(record().toString())!!
        assertTrue(confirmed.receivable)
        assertNull(FlowLive.receiveStanding(confirmed, checking = false, strings))

        val unconfirmed = ReceiveStanding.of(record("unconfirmed").toString())!!
        assertFalse(unconfirmed.receivable)
        assertTrue(unconfirmed.check_first)
        val shown = FlowLive.receiveStanding(unconfirmed, checking = false, strings)!!
        assertEquals(strings.t(I18nKeys.Flows.RECEIVE_UNCONFIRMED_TITLE), shown.title)
        assertEquals(strings.t(I18nKeys.Flows.RECEIVE_UNCONFIRMED_BODY), shown.body)
        assertEquals(strings.t(I18nKeys.Flows.RECEIVE_UNCONFIRMED_CHECK_AGAIN), shown.checkAgain)
        assertNull(shown.checking)
        assertFalse(shown.refuted)
        assertEquals(strings.t(I18nKeys.Flows.RECEIVE_UNCONFIRMED_CHECKING), FlowLive.receiveStanding(unconfirmed, checking = true, strings)!!.checking)

        val refuted = FlowLive.receiveStanding(ReceiveStanding.of(record("refuted").toString()), checking = false, strings)!!
        assertEquals(strings.t(I18nKeys.Flows.RECEIVE_REFUTED_TITLE), refuted.title)
        assertEquals(strings.t(I18nKeys.Flows.RECEIVE_REFUTED_BODY), refuted.body)
        assertTrue(refuted.refuted)
        assertNull("nothing to ask again", refuted.checkAgain)

        // No record to judge (a drawn screen, a multisig in front): nothing said.
        assertNull(ReceiveStanding.of(""))
        // A record the core cannot read shows no address either.
        assertFalse(ReceiveStanding.of("{\"not\":\"an account\"}")!!.receivable)
        // Every word is in the corpus.
        listOf(
            I18nKeys.Flows.RECEIVE_UNCONFIRMED_TITLE, I18nKeys.Flows.RECEIVE_UNCONFIRMED_BODY, I18nKeys.Flows.RECEIVE_UNCONFIRMED_CHECK_AGAIN,
            I18nKeys.Flows.RECEIVE_UNCONFIRMED_CHECKING, I18nKeys.Flows.RECEIVE_REFUTED_TITLE, I18nKeys.Flows.RECEIVE_REFUTED_BODY,
        ).forEach { assertTrue(it, strings.t(it) != it) }
    }

    @Test
    fun `a re-check saves what the chain found, through the core, and says it is checking meanwhile`() = runBlocking {
        val saved = mutableListOf<JSONObject>()
        val gate = CompletableDeferred<Unit>()
        val check = AccountIdentityCheck(
            confirm = { key, address ->
                assertEquals(vector.getString("publicKey"), key)
                assertEquals("0x88cCA0EeDbF2C4426110bbFc998F048689266894", address)
                gate.await()
                IdentityCheck("confirmed", "gnosis")
            },
            save = { saved += it },
        )
        val unconfirmed = record("unconfirmed").toString()
        val running = async(Dispatchers.Default) { check.check(unconfirmed) }
        while (!check.isChecking("0x88cCA0EeDbF2C4426110bbFc998F048689266894")) Thread.sleep(5)
        assertNull("one walk at a time per account", check.check(unconfirmed))
        gate.complete(Unit)
        assertEquals("confirmed", running.await())
        assertFalse(check.isChecking("0x88cCA0EeDbF2C4426110bbFc998F048689266894"))
        assertTrue(ReceiveStanding.of(saved.single().toString())!!.receivable)
        assertFalse(saved.single().has("identity"))

        // The chain still silent: nothing new, nothing written.
        val quiet = AccountIdentityCheck(confirm = { _, _ -> IdentityCheck("unconfirmed", "none") }, save = { saved += it })
        assertEquals("unconfirmed", quiet.check(unconfirmed))
        assertEquals(1, saved.size)
    }

    // -- B. approvals ---------------------------------------------------------

    @Test
    fun `a batch's kept-unlimited legs ride the approve, and the warning is the core's key`() {
        val guard = Wire.json.decodeFromString(
            GuardView.serializer(),
            """{"surface":"approval_editor","confirm_allowed":true,"unlimited_consented":false,"unlimited_consented_legs":[1,3],
               "unlimited_warning":true,"unlimited_warning_key":"componentsUi.signing.farAboveBalanceWarning",
               "detected":{"kind":"erc20_approve","spender":"0x1111111111111111111111111111111111111111","locus":{"type":"calldata_word","word_index":1},"nested_at":2}}""",
        )
        assertEquals(listOf(1, 3), guard.unlimited_consented_legs)
        assertEquals(2, guard.detected?.nested_at)
        val opts = SigningController.approveOpts(FeeView(), ClearSigningView(), guard)
        assertEquals(listOf(1, 3), opts.unlimited_approved_legs)
        assertFalse(opts.unlimited_approved)
        assertTrue(Wire.json.encodeToString(app.getvela.wallet.feature.signing.core.SignApproveOpts.serializer(), opts).contains("\"unlimited_approved_legs\":[1,3]"))
        val warnings = SigningLive.guardBlocks(guard, strings).filterIsInstance<SigningBlock.Warning>().map { it.text }
        assertTrue(warnings.toString(), strings.t("componentsUi.signing.farAboveBalanceWarning") in warnings)
        assertFalse(strings.t("componentsUi.signing.unlimitedWarning") in warnings)
        // A view from before §37 names no key: the unlimited sentence, as before.
        val old = Wire.json.decodeFromString(GuardView.serializer(), """{"surface":"permit_sign","unlimited_warning":true}""")
        assertTrue(strings.t("componentsUi.signing.unlimitedWarning") in SigningLive.guardBlocks(old, strings).filterIsInstance<SigningBlock.Warning>().map { it.text })
        assertTrue(old.unlimited_consented_legs.isEmpty())
    }

    // -- C. sign-in -----------------------------------------------------------

    @Test
    fun `a refused sign-in says why in its own words, never the self-call text`() {
        assertEquals(SignErrorKind.SignInRefused, Wire.json.decodeFromString(SignErrorKind.serializer(), "\"sign_in_refused\""))
        val sign = Wire.json.decodeFromString(
            SignView.serializer(),
            """{"surface":"sheet","confirm_gate_open":false,
               "blocked":{"function":"signIn","selector":"","leg_index":null,"nested":false,"parent":null,"reason":null,
                 "sign_in":{"reason":"domain","key":"componentsUi.signing.signIn.wrongDomain","params":{"domain":"bank.example","origin":"https://evil.example"}}}}""",
        )
        val blocks = SigningLive.statusBlocks(sign, strings)
        val title = blocks.filterIsInstance<SigningBlock.Intent>().single()
        assertEquals(strings.t("componentsUi.signing.signIn.refusedTitle"), title.text)
        assertEquals(SigningTone.Danger, title.tone)
        val body = blocks.filterIsInstance<SigningBlock.Warning>().single().text
        assertEquals(strings.t("componentsUi.signing.signIn.wrongDomain", mapOf("domain" to "bank.example", "origin" to "https://evil.example")), body)
        assertTrue(body, body.contains("bank.example") && body.contains("https://evil.example"))
        assertFalse(blocks.any { it is SigningBlock.Intent && it.text == strings.t("componentsUi.signing.selfCallBlockedTitle") })
        // The self-call refusal is untouched.
        val selfCall = SigningLive.statusBlocks(sign.copy(blocked = sign.blocked!!.copy(function = "enableModule", sign_in = null)), strings)
        assertEquals(strings.t("componentsUi.signing.selfCallBlockedTitle"), selfCall.filterIsInstance<SigningBlock.Intent>().single().text)
    }

    // -- D. the multisig mailbox ---------------------------------------------

    private val safe = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private val bob = "0x031d7D57c99CAF891e1C250554691Fd12D84772b"
    private val words = MultisigWords(strings = strings, networkName = { if (it == 100) "Gnosis" else null })

    private fun proposals(nonMembers: Int, showing: Boolean, fromNonMember: Boolean) = MsProposalsView.of(
        Wire.json.parseToJsonElement(
            """{"followed":"$safe","non_members":$nonMembers,"showing_non_members":$showing,
               "items":[{"chainId":100,"safeTxHash":"0x${"ab".repeat(32)}","nonce":"7","proposer":"$bob","approvals":[],"approved":0,"threshold":2,
                 "status":"pending","canApprove":[],"canExecute":[],"cancelledBy":null,"sameNonce":[],"executing":false,"createdMs":1,
                 "unsent":false,"renameUndeployed":false,"fromNonMember":$fromNonMember}]}""",
        ),
    )

    @Test
    fun `the non-members' proposals sit behind their disclosure, each with its warning`() {
        val closed = proposals(nonMembers = 2, showing = false, fromNonMember = false)
        assertEquals(2, closed.nonMembers)
        assertEquals(strings.t("home.multisig.panel.nonMembersShow", mapOf("total" to "2")), words.nonMembersDisclosure(closed))
        assertNull(words.nonMemberWarning(closed.items.single()))
        val open = proposals(nonMembers = 2, showing = true, fromNonMember = true)
        assertTrue(open.showingNonMembers)
        assertEquals(strings.t("home.multisig.panel.nonMembersHide"), words.nonMembersDisclosure(open))
        assertEquals(strings.t("home.multisig.panel.nonMembersWarning"), words.nonMemberWarning(open.items.single()))
        assertNull("none left out: no row", words.nonMembersDisclosure(proposals(nonMembers = 0, showing = false, fromNonMember = false)))
        // The words are the corpus's, not keys.
        assertNotNull(words.nonMembersDisclosure(closed)?.takeIf { !it.startsWith("home.") })
    }

    @Test
    fun `a member nothing binds to its creation is worded like the other authority refusals`() {
        val finding = Wire.json.parseToJsonElement("""{"from":"genesis","chainId":100,"reason":{"kind":"ownerNotBound","address":"$bob"}}""")
        assertEquals(strings.t("home.multisig.authority.ownerNotBound", mapOf("network" to "Gnosis", "address" to bob)), words.finding(finding))
        assertTrue(words.finding(finding).contains(bob))
    }
}
