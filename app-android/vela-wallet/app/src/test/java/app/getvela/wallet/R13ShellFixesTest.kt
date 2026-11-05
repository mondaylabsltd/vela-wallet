package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.i18n.I18nCatalogs
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.feature.multisig.LookAsk
import app.getvela.wallet.feature.multisig.MemberRowTitles
import app.getvela.wallet.feature.multisig.MultisigWords
import app.getvela.wallet.feature.multisig.core.MsAccountsView
import app.getvela.wallet.feature.multisig.core.MultisigHub
import app.getvela.wallet.feature.multisig.core.get
import app.getvela.wallet.feature.multisig.core.kind
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import java.io.File
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.async
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.serialization.json.JsonObject
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Round 13's Android shell fixes: a member change's Activity title said in
 * the language in front (r13-5), a link handed over before the multisig
 * finished booting (r13-6), one sentence for a network an invite needs, and
 * an open sheet's fee asked again only when something asks (S4).
 */
class R13ShellFixesTest {

    private val repoRoot = File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set"))
    private val assets = File(repoRoot, "assets/i18n")

    private fun runtime(tag: String = "en"): I18nRuntime = I18nRuntime(
        readOnDemand = { namespace, lang -> File(assets, I18nCatalogs.assetPath(lang, namespace).removePrefix("i18n/")).takeIf { it.isFile }?.readBytes() },
    ) { lang -> File(assets, "$lang.json").readBytes() }.apply {
        initialize(tag)
        include(I18nCatalogs.MULTISIG)
    }

    private val frank = "0x78e6AE9A0b5D3b9B7e4d7aB2f1A9Fb83a344f0E3"
    private val safe = "0x777E989C821a7F1b031696f044a2fF114261875a"
    private val hash = "0x" + "cd".repeat(32)
    private val scopes = mutableListOf<CoroutineScope>()

    @After
    fun tearDown() = scopes.forEach { it.cancel() }

    private fun words(strings: I18nRuntime, labels: Map<String, String> = emptyMap()) = MultisigWords(
        strings = strings,
        accounts = MsAccountsView(),
        localNames = labels,
        networkName = { if (it == 5042002) "Arc Testnet" else null },
        nativeSymbol = { "USDC" },
        tokenFacts = { _, _ -> null },
    )

    private fun memberRow(id: String = "multisig:5042002:$hash:0", intent: String = "Add 0x78e6AE…44f0E3", action: Boolean = true) = JSONObject()
        .put("id", id).put("type", "dapp_tx").put("chainId", 5042002).put("timestamp", 1_762_300_000)
        .put("status", "confirmed").put("from", safe).put("to", safe).put("value", "0").put("symbol", "").put("intent", intent)
        .apply { if (action) put("multisigChange", JSONObject().put("kind", "addOwner").put("owner", frank).put("threshold", "2")) }

    // -- r13-5: a member change's Activity title, in the language in front ----------

    /**
     * r13-5 (r12-8 still open): zh Activity said "Add 0x78e6AE…44f0E3" — the
     * English of the moment the row was written. The row keeps its action
     * and is worded again when the language moves, by the core's one rule.
     */
    @Test
    fun a_member_change_row_is_worded_again_when_the_language_switches_after_it_exists() = runBlocking {
        val store = FakeStore(mapOf(KeyValueStore.Keys.TRANSACTIONS to JSONArray().put(memberRow()).put(memberRow("multisig:5042002:old:0", "Add 0xOld", action = false)).toString()))
        val feed = FeedExecutor(store, ownAccounts = { emptyList() })
        val strings = runtime("en")
        val reword: suspend () -> Int = { feed.rewordRecords { row -> MemberRowTitles.reword(row) { words(strings).memberChangeTitle(it) } } }

        // The row exists, written in English: worded by the core's rule once,
        // then nothing moves while nothing does.
        val en = words(strings).memberChangeTitle(Wire.json.parseToJsonElement("""{"kind":"addOwner","owner":"$frank","threshold":"2"}"""))!!
        assertTrue(en, en.startsWith("Add "))
        reword()
        assertEquals(0, reword())

        // The language switches AFTER the row exists: its title follows.
        strings.setLocale("zh")
        assertEquals(1, reword())
        val rows = JSONArray(store.read(KeyValueStore.Keys.TRANSACTIONS)!!)
        val zh = rows.getJSONObject(0).getString("intent")
        assertNotEquals(en, zh)
        assertFalse(zh, zh.startsWith("Add "))
        assertTrue(zh, zh.lowercase().contains("0x78e6ae"))
        assertEquals(words(strings).memberChangeTitle(Wire.json.parseToJsonElement(rows.getJSONObject(0).getJSONObject("multisigChange").toString())), zh)
        // A row written before the action was kept keeps its own title.
        assertEquals("Add 0xOld", rows.getJSONObject(1).getString("intent"))

        // A name landing (the accounts machine's labels) re-titles it too.
        val named = feed.rewordRecords { row -> MemberRowTitles.reword(row) { words(strings, mapOf(frank.lowercase() to "Frank")).memberChangeTitle(it) } }
        assertEquals(1, named)
        assertTrue(JSONArray(store.read(KeyValueStore.Keys.TRANSACTIONS)!!).getJSONObject(0).getString("intent").contains("Frank"))

        // And back to English.
        strings.setLocale("en")
        assertEquals(1, reword())
        assertEquals(en, JSONArray(store.read(KeyValueStore.Keys.TRANSACTIONS)!!).getJSONObject(0).getString("intent"))
    }

    /** The executed rows a member change writes keep its action (`multisigChange`); a transfer's do not. */
    @Test
    fun an_executed_member_change_row_keeps_its_action() = runBlocking {
        val written = mutableListOf<JSONObject>()
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default).also { scopes += it }
        val hub = MultisigHub(
            live = MultisigShellTest.FakePorts(),
            facts = { MultisigHub.Facts("http://127.0.0.1:1", emptyList(), listOf(5042002), listOf(5042002), emptyList()) },
            writeRows = { rows -> written += rows; true },
            scope = scope,
        )
        fun executed(action: String) = Wire.json.parseToJsonElement(
            """{"kind":"activity_rows","rows":[{"chainId":5042002,"safeTxHash":"$hash","nonce":"4","from":"$safe","txHash":"0x1","userOpHash":"0x2",
               "action":$action,"rows":[{"index":0,"kind":"call","to":"$safe","sends":"0","calls":1,"contract":"$safe"}]}]}""",
        ) as JsonObject
        hub.route(executed("""{"kind":"addOwner","owner":"$frank","threshold":"2"}"""))
        hub.route(executed("""{"kind":"rename","name":"Team"}"""))
        assertEquals(2, written.size)
        assertEquals("addOwner", written[0].getJSONObject("multisigChange").getString("kind"))
        assertEquals(frank, written[0].getJSONObject("multisigChange").getString("owner"))
        assertFalse(written[1].has("multisigChange"))
    }

    // -- r13-6: a link handed over before the boot is done ---------------------------

    /**
     * r13-6: launched, then a proposal link within ~1 s — the sheet said Arc
     * Testnet was "a network this device doesn't have" and the switcher
     * listed no multisig, until the process restarted. The link reached the
     * accounts machine before its `start` (a second `boot()` returned while
     * the first still waited for the app's networks) and nothing read it
     * again. Now every caller waits for the boot; a link delivered then is
     * read with the networks.
     */
    @Test
    fun a_link_delivered_before_the_boot_completes_is_read_with_the_networks() = runBlocking {
        val ports = MultisigShellTest.FakePorts()
        ports.store["vela.multisigAccounts"] = """[{"address":"$safe","owners":["0xD400866e00B055B20752a826CD5C89b811de130b","$frank"],"threshold":2,"chainIds":[5042002],"recorded":true,"addedMs":1,"genesis":{"factory":"0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67","singleton":"0x41675C099F32341bf84BFc5382aF534df5C7461a","saltNonce":"0x1","initializer":"0x","name":"R13 Team"}}]"""
        // The app's networks are still being read when the link arrives.
        val networksRead = CompletableDeferred<Unit>()
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default).also { scopes += it }
        val hub = MultisigHub(
            live = ports,
            facts = {
                networksRead.await()
                MultisigHub.Facts("http://127.0.0.1:1", emptyList(), listOf(1, 5042002), listOf(5042002), emptyList())
            },
            scope = scope,
        )
        val first = scope.async { hub.boot() }
        delay(200)
        // The link: what MultisigAppLayer does with it — boot, then open.
        val link = scope.async {
            hub.boot()
            hub.openLink("#ms=5042002.$safe.$hash")
        }
        delay(500)
        val waited = !link.isCompleted
        networksRead.complete(Unit)
        withTimeout(20_000) { first.await(); link.await() }
        val stand = withTimeout(10_000) { hub.accounts.until { v -> v["link"]?.kind } }
        assertNotEquals("missingNetwork", stand)
        delay(1_000)
        assertNotEquals("missingNetwork", hub.accountsView.value.link?.kind)
        assertTrue("the link waited for the boot", waited)
        // The multisig is listed (the switcher's rows).
        assertEquals(safe.lowercase(), hub.accountsView.value.accounts.single().account.address.lowercase())
    }

    /** A network list that moved while the boot waited is said once the machines have started. */
    @Test
    fun networks_that_move_during_the_boot_are_said_when_it_is_done() = runBlocking {
        val ports = MultisigShellTest.FakePorts()
        var networks = listOf(1)
        val gate = CompletableDeferred<Unit>()
        var asked = 0
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default).also { scopes += it }
        val hub = MultisigHub(
            live = ports,
            facts = {
                asked += 1
                // What the boot read first: the list before Arc Testnet was read in.
                val now = networks
                if (asked == 1) gate.await()
                MultisigHub.Facts("http://127.0.0.1:1", emptyList(), now, now, emptyList())
            },
            scope = scope,
        )
        val booting = scope.async { hub.boot() }
        delay(100)
        // The list moves while the boot waits, and the app says so (its
        // effect fires before `loaded`): nothing is lost.
        networks = listOf(1, 5042002)
        hub.networksChanged()
        gate.complete(Unit)
        withTimeout(20_000) { booting.await() }
        // The link is read against the networks as they stand after the boot.
        hub.openLink("#ms=5042002.$safe.$hash")
        val stand = withTimeout(10_000) { hub.accounts.until { v -> v["link"]?.kind } }
        assertNotEquals("missingNetwork", stand)
    }

    // -- r13-6: one sentence for the network an invite needs -------------------------

    @Test
    fun an_invites_missing_network_is_one_sentence() {
        val strings = runtime("en")
        val w = words(strings)
        val said = w.t("add.networkMissing", "network" to w.missingName(5042002))
        assertEquals("It's used on Arc Testnet, which this device doesn't have yet. Add the network; the invite opens again in your wallet once it's there.", said)
        assertFalse(said, said.contains("On Arc"))
        assertFalse(said, said.contains("doesn’t have, which"))
        // A network nobody names: "Chain N", in the page's words.
        assertTrue(w.missingName(777_777).contains("777777"))
        strings.setLocale("zh")
        val zh = w.t("add.networkMissing", "network" to w.missingName(5042002))
        assertTrue(zh, zh.startsWith("它用于 Arc Testnet，"))
    }

    // -- S4: an open sheet's fee is asked again only when something asks -------------

    @Test
    fun an_open_sheets_timer_asks_only_for_a_look_that_has_not_priced_it() {
        // None yet, unpriced, unchecked: asked, a few times.
        for (kind in listOf(null, "unpriced", "unchecked")) {
            assertTrue(LookAsk.timerAsks(kind, 0))
            assertFalse(LookAsk.timerAsks(kind, LookAsk.AUTO_LIMIT))
        }
        // A priced look, or one that says why it cannot run, stands.
        for (kind in listOf("priced", "fails", "refused", "spent")) assertFalse(kind, LookAsk.timerAsks(kind, 0))
        // Never an endless timer: 20 s of an open sheet asks nothing once priced.
        var asks = 0
        repeat((600_000 / LookAsk.ASK_EVERY_MS).toInt()) { if (LookAsk.timerAsks("priced", asks)) asks += 1 }
        assertEquals(0, asks)
        repeat((600_000 / LookAsk.ASK_EVERY_MS).toInt()) { if (LookAsk.timerAsks("unpriced", asks)) asks += 1 }
        assertEquals(LookAsk.AUTO_LIMIT, asks)
    }
}
