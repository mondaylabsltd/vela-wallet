package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.feature.settings.core.SigningPage
import app.getvela.wallet.feature.settings.core.SigningPagesEvent
import app.getvela.wallet.feature.settings.core.SigningPagesExecutor
import app.getvela.wallet.feature.settings.core.SigningPagesOperation
import app.getvela.wallet.feature.settings.core.SigningPagesShellResult
import app.getvela.wallet.feature.settings.core.SigningPagesView
import app.getvela.wallet.feature.signing.trustedsigner.SignerPageChecks
import app.getvela.wallet.feature.signing.trustedsigner.SigningPlan
import app.getvela.wallet.feature.signing.trustedsigner.VenueWords
import java.security.MessageDigest
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.SignerIntegrityState
import uniffi.vela_core_uniffi.SigningPagesCore

/**
 * Spec 102 on the phone: the integrity check (P2-06, R6), the signing pages
 * this device keeps (P2-08), and the signing plan the shell reads (R1/R4/R5).
 * Every rule is the core's; these pin that the shell asks it, carries the
 * bytes faithfully, and opens nothing it did not admit.
 */
class SignerPageChecksTest {

    private val official = "https://sign.getvela.app/"

    // --- R6: the check -------------------------------------------------------

    @Test
    fun `the official page is checked against the published bytes and admitted`() {
        val checks = OfficialDist.checks()
        val admission = runBlocking { checks.ensure(official) }
        assertNotNull("the committed dist/ bytes are a version this build accepts", admission)
        val line = checks.line(official)
        assertEquals(SignerIntegrityState.MATCHES, line.state)
        assertEquals(OfficialDist.VERSION.take(8), line.version)
        assertTrue(line.opens)
        assertNotNull(line.checkedAtMs)
        // The launch is built by the admission, from exactly the URL that was checked.
        val url = admission!!.urlLaunch("""{"intent":{}}""", "tok", System.currentTimeMillis().toULong())
        assertTrue(url, url.startsWith(OfficialDist.LAUNCH_URL + "?"))
    }

    @Test
    fun `a fresh check is reused, and one past a day is run again before anything opens`() {
        var now = 1_760_000_000_000L
        val checks = OfficialDist.checks(clock = { now })
        runBlocking { checks.ensure(official) }
        val first = OfficialDist.fetches.get()
        runBlocking { checks.ensure(official) }
        assertEquals("a fresh check is not fetched again", first, OfficialDist.fetches.get())

        now += 24 * 60 * 60 * 1000L + 1
        assertEquals("a stale check vouches for nothing", SignerIntegrityState.CHECKING, checks.line(official).state)
        assertNull("…and admits nothing until it is run again", checks.admitted(official))
        assertNotNull(runBlocking { checks.ensure(official) })
        assertTrue("the stale page was fetched again", OfficialDist.fetches.get() > first)
        assertTrue(checks.line(official).opens)
    }

    @Test
    fun `every way a fetch can fail opens nothing`() {
        for (failure in listOf(SignerPageChecks.Fetched.Unreachable, SignerPageChecks.Fetched.Unreadable, SignerPageChecks.Fetched.Body(500, ByteArray(0)))) {
            val checks = SignerPageChecks(
                fetch = { url -> if (url.endsWith("index.json")) OfficialDist.fetch(url) else failure },
                store = FakeStore(),
            )
            assertNull("$failure", runBlocking { checks.ensure(official) })
            assertEquals(SignerIntegrityState.COULD_NOT_CHECK, checks.line(official).state)
            assertFalse(checks.line(official).opens)
        }
    }

    @Test
    fun `an index that never answered falls back to the launch version, which is still checked`() {
        val checks = SignerPageChecks(
            fetch = { url -> if (url.endsWith("index.json")) SignerPageChecks.Fetched.Unreachable else OfficialDist.fetch(url) },
            store = FakeStore(),
        )
        assertNotNull(runBlocking { checks.ensure(official) })
        assertEquals(SignerIntegrityState.MATCHES, checks.line(official).state)
    }

    @Test
    fun `a version blocked on this device is never the one opened`() {
        // One blocked: the next published version this build accepts opens instead.
        val one = OfficialDist.checks(store = FakeStore(mapOf(KeyValueStore.Keys.SIGNER_PAGE_BLOCKED to JSONArray(listOf(OfficialDist.VERSION)).toString())))
        assertNotNull(runBlocking { one.ensure(official) })
        assertFalse(OfficialDist.VERSION.startsWith(one.line(official).version))
        // Every one blocked: nothing opens, and the line says it is this device's own list.
        val every = JSONArray(uniffi.vela_core_uniffi.signerPageAllowed()).toString()
        val all = OfficialDist.checks(store = FakeStore(mapOf(KeyValueStore.Keys.SIGNER_PAGE_BLOCKED to every)))
        assertNull(runBlocking { all.ensure(official) })
        assertFalse(all.line(official).opens)
        assertEquals(SignerIntegrityState.ALL_BLOCKED, all.line(official).state)
    }

    /**
     * D-7: a self-hosted page's own build, proposed by its index, can only end
     * in asking the person — and once they trust it on this device, it opens.
     */
    @Test
    fun `a custom page's own build asks to be trusted, and opens once trusted on this device`() {
        val own = "https://sign.example.com/"
        val bytes = "<!doctype html><title>my own signing page</title>".toByteArray()
        val version = MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }
        val store = FakeStore()
        val checks = SignerPageChecks(
            fetch = { url ->
                when (url) {
                    "${own}index.json" -> SignerPageChecks.Fetched.Body(200, """{"versions":["$version"]}""".toByteArray())
                    "${own}b/$version/sign.html" -> SignerPageChecks.Fetched.Body(200, bytes)
                    else -> SignerPageChecks.Fetched.Body(404, ByteArray(0))
                }
            },
            store = store,
        )
        val check = runBlocking { checks.check(own) }
        assertEquals(SignerIntegrityState.ASK_TO_TRUST, checks.line(own).state)
        assertFalse(checks.line(own).opens)
        assertTrue(check.proposed)
        assertEquals(version, check.version)

        runBlocking { checks.trust(own, check.version) }
        assertEquals(SignerIntegrityState.TRUSTED_HERE, checks.line(own).state)
        assertTrue(checks.line(own).opens)
        assertEquals(JSONArray(listOf(version)).toString(), store.values[KeyValueStore.Keys.SIGNER_PAGE_TRUSTED])
    }

    @Test
    fun `the index is read in both of its shapes, and nothing else lists a version`() {
        assertEquals(listOf("a", "b"), SignerPageChecks.listed("""{"versions":["a","b"]}""".toByteArray()))
        assertEquals(listOf("c"), SignerPageChecks.listed("""["c"]""".toByteArray()))
        assertEquals(emptyList<String>(), SignerPageChecks.listed("<html>".toByteArray()))
    }

    // --- R1 / R4 / R5: the plan the shell reads ------------------------------

    private fun record(venue: String?, signInKey: Boolean = true, origin: String? = null, domain: String? = null) = JSONObject()
        .put("id", "a1b2c3d4")
        .put("name", "Savings")
        .put("address", "0x88cCA0EeDbF2C4426110bbFc998F048689266894")
        .put("public_key_hex", "04" + "ab".repeat(64))
        .put("created_at_iso", "2026-09-30T10:00:00.000Z")
        .put(
            "keys",
            JSONArray().put(
                JSONObject().put("credential_id", "a1b2c3d4").put("public_key_hex", "04" + "ab".repeat(64))
                    .put("name", "Savings").put("transports", "hybrid,internal")
                    .apply { origin?.let { put("signer_origin", it) } },
            ),
        )
        .apply {
            if (signInKey) put("sign_in_key", JSONObject().put("credential_id", "a1b2c3d4").put("method", "platform").put("transports", "hybrid,internal"))
            domain?.let { put("signing_domain", it) }
            venue?.let { put("signing_venue", JSONObject(it)) }
        }
        .toString()

    @Test
    fun `the plan names the venue, the domain and the key route`() {
        val inVela = SigningPlan.of(record("""{"type":"in_vela"}""", domain = "getvela.app"))!!
        assertNull(inVela.page)
        assertEquals("getvela.app", inVela.domain)
        assertEquals("a1b2c3d4", inVela.credentialId)
        assertEquals(app.getvela.wallet.feature.onboarding.core.KeyMethod.Platform, inVela.method)
        assertTrue(inVela.keyRouteJson!!.contains("client-device"))

        val onPage = SigningPlan.of(record("""{"type":"page","url":"https://sign.getvela.app/"}""", domain = "getvela.app"))!!
        assertEquals(official, onPage.page)
        assertNull(onPage.blocked)
        // An unreadable record is no plan: the spine signs as a record without one.
        assertNull(SigningPlan.of("not json"))
    }

    @Test
    fun `a record from before 102 migrates — the official page's account keeps its page, without a fourth method`() {
        val legacy = JSONObject(record(null, signInKey = false, origin = "https://sign.getvela.app"))
            .put("signed_in_with", JSONObject().put("credential_id", "a1b2c3d4").put("method", "trusted_signer").put("transports", "hybrid,internal").put("signer_origin", "https://sign.getvela.app"))
            .toString()
        val plan = SigningPlan.of(legacy)!!
        assertEquals("getvela.app", plan.domain)
        assertEquals(official, plan.page)
        assertEquals("the place comes from the key's transports", app.getvela.wallet.feature.onboarding.core.KeyMethod.Platform, plan.method)
    }

    @Test
    fun `a custom-domain account whose page is unknown here is blocked, with the core's reason`() {
        val plan = SigningPlan.of(record("""{"type":"in_vela"}""", domain = "sign.example.com"))!!
        assertNotNull(plan.blocked)
        assertEquals("Vela can't reach keys on sign.example.com.", VenueWords.block(plan.blocked!!) { key, vars -> en(key, vars) })
    }

    private val strings by lazy {
        app.getvela.wallet.core.i18n.I18nRuntime { lang -> java.io.File(System.getProperty("vela.repo.root"), "assets/i18n/$lang.json").readBytes() }
            .apply { initialize("en") }
    }

    private fun en(key: String, vars: Map<String, String>) = strings.t(key, vars)

    @Test
    fun `the hand-off card names the key, else its place`() {
        assertEquals("Savings", VenueWords.keyLabel("Savings", app.getvela.wallet.feature.onboarding.core.KeyMethod.Platform) { strings.t(it) })
        assertEquals("This device", VenueWords.keyLabel("", app.getvela.wallet.feature.onboarding.core.KeyMethod.Platform) { strings.t(it) })
        assertEquals("", VenueWords.keyLabel(" ", null) { strings.t(it) })
        assertEquals("Savings", VenueWords.keyName(record(null), "A1B2C3D4"))
    }

    // --- P2-08: the signing pages machine ------------------------------------

    private val scopes = mutableListOf<CoroutineScope>()

    @After
    fun tearDown() = scopes.forEach { it.cancel() }

    private fun pagesHost(store: KeyValueStore): CoreHost<SigningPagesView> {
        val executor = SigningPagesExecutor(store)
        return CoreHost(
            bridge = SigningPagesCore().asBridge(),
            scope = CoroutineScope(SupervisorJob() + Dispatchers.Default).also { scopes += it },
            initial = SigningPagesView(),
            serializer = SigningPagesView.serializer(),
            perform = JsonShell.perform(SigningPagesOperation.serializer(), SigningPagesShellResult.serializer(), executor::perform),
            escapedFailure = JsonShell.escapedFailure(
                SigningPagesOperation.serializer(),
                SigningPagesShellResult.serializer(),
                fallback = SigningPagesShellResult.Stored(),
                answer = executor::neutralAnswer,
            ),
            onFault = { error -> throw AssertionError("shell fault: $error", error) },
        )
    }

    private fun <V : Any> CoreHost<V>.settle(predicate: (V) -> Boolean): V =
        runBlocking { withTimeout(10_000L) { view.first(predicate) } }

    private fun FakeStore.settles(key: String, predicate: (String?) -> Boolean) =
        runBlocking { withTimeout(10_000L) { while (!predicate(values[key])) kotlinx.coroutines.delay(20) } }

    @Test
    fun `the 071 page is imported once, and the old key goes`() {
        val store = FakeStore(mapOf(KeyValueStore.Keys.TRUSTED_SIGNER_URL to "https://my.signer.test/"))
        val host = pagesHost(store)
        host.start()
        host.dispatch(SigningPagesEvent.Refresh, SigningPagesEvent.serializer())
        val view = host.settle { it.loaded && it.pages.size == 2 }
        assertTrue("official first", view.pages.first().official)
        assertEquals("https://my.signer.test/", view.pages[1].url)
        assertEquals("my.signer.test", view.pages[1].domain)
        store.settles(KeyValueStore.Keys.TRUSTED_SIGNER_URL) { it == null }
        assertEquals(listOf(SigningPage("https://my.signer.test/")), view.saved)
        assertTrue(store.values[KeyValueStore.Keys.SIGNING_PAGES]!!.contains("https://my.signer.test/"))
    }

    @Test
    fun `pages are added, refused, renamed and removed by the core's rules`() {
        val store = FakeStore()
        val host = pagesHost(store)
        host.start()
        host.dispatch(SigningPagesEvent.Refresh, SigningPagesEvent.serializer())
        host.settle { it.loaded }

        host.dispatch(SigningPagesEvent.PageAdded("http://192.168.1.4/"), SigningPagesEvent.serializer())
        assertEquals("insecure", host.settle { it.add_error != null }.add_error)
        host.dispatch(SigningPagesEvent.PageAdded("https://sign.getvela.app/"), SigningPagesEvent.serializer())
        assertEquals("duplicate", host.settle { it.add_error == "duplicate" }.add_error)

        host.dispatch(SigningPagesEvent.PageAdded("https://sign.example.com", "Mine"), SigningPagesEvent.serializer())
        val added = host.settle { it.pages.size == 2 }
        assertNull(added.add_error)
        assertEquals("https://sign.example.com/", added.pages[1].url)
        // A page's keys belong to its own host; only `*.getvela.app` folds to the apps' domain.
        assertEquals("sign.example.com", added.pages[1].domain)
        store.settles(KeyValueStore.Keys.SIGNING_PAGES) { it?.contains("sign.example.com") == true }

        host.dispatch(SigningPagesEvent.PageRenamed("https://sign.example.com/", "Home"), SigningPagesEvent.serializer())
        assertEquals("Home", host.settle { it.pages.getOrNull(1)?.name == "Home" }.pages[1].name)
        host.dispatch(SigningPagesEvent.PageRemoved("https://sign.example.com/"), SigningPagesEvent.serializer())
        host.settle { it.pages.size == 1 }
        store.settles(KeyValueStore.Keys.SIGNING_PAGES) { it == "[]" }
    }
}
