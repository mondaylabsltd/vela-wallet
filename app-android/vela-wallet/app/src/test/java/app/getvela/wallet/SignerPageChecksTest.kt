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
import kotlinx.coroutines.launch
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
        // The launch is built by the admission, from exactly the URL that was
        // checked — and names the app's language, so the page speaks it.
        val url = admission!!.urlLaunch("""{"intent":{}}""", "tok", "zh-HK", System.currentTimeMillis().toULong())
        assertTrue(url, url.startsWith(OfficialDist.LAUNCH_URL + "?"))
        assertTrue(url, url.substringBefore('#').contains("lang=zh-HK"))
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

    /** A self-hosted deployment serving one build of its own, listed by its index. */
    private class OwnDeployment(val base: String, body: String = "<!doctype html><title>my own signing page</title>") {
        val bytes = body.toByteArray()
        val version: String = MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }
        suspend fun fetch(url: String): SignerPageChecks.Fetched = when (url) {
            "${base}index.json" -> SignerPageChecks.Fetched.Body(200, """{"versions":["$version"]}""".toByteArray())
            "${base}b/$version/sign.html" -> SignerPageChecks.Fetched.Body(200, bytes)
            else -> SignerPageChecks.Fetched.Body(404, ByteArray(0))
        }
    }

    /** Checks whose trusted versions are the pages core's own list, as the app wires them. */
    private fun checksOverPages(host: CoreHost<SigningPagesView>, store: FakeStore, fetch: suspend (String) -> SignerPageChecks.Fetched) =
        SignerPageChecks(
            fetch = fetch,
            store = store,
            savedPages = {
                app.getvela.wallet.core.crux.Wire.json.encodeToString(
                    kotlinx.serialization.builtins.ListSerializer(SigningPage.serializer()),
                    host.view.value.saved,
                )
            },
            recordTrust = { url, version ->
                host.dispatch(SigningPagesEvent.VersionTrusted(url, version), SigningPagesEvent.serializer())
                withTimeout(10_000L) { host.view.first { view -> view.saved.any { page -> version in page.trusted } } }
            },
        )

    /**
     * D-7 / D-15: a self-hosted page's own build, proposed by its index, can
     * only end in asking the person — and once they trust it, it opens. The
     * version is the check's own (`versionToTrust`), and it is stored on THAT
     * page by the pages core — never in a device-wide list — so the same bytes
     * served by another page still ask.
     */
    @Test
    fun `a self-hosted build asks to be trusted, and opens once trusted on that page only`() {
        val own = OwnDeployment("https://sign.example.com/")
        val other = OwnDeployment("https://other.example.org/")
        val store = FakeStore()
        val host = pagesHost(store)
        host.start()
        host.dispatch(SigningPagesEvent.Refresh, SigningPagesEvent.serializer())
        host.settle { it.loaded }
        val checks = checksOverPages(host, store) { url -> if (url.startsWith(own.base)) own.fetch(url) else other.fetch(url) }

        val check = runBlocking { checks.check(own.base) }
        assertEquals(SignerIntegrityState.ASK_TO_TRUST, checks.line(own.base).state)
        assertFalse(checks.line(own.base).opens)
        assertTrue(check.proposed)
        assertEquals("the question carries the full version", own.version, checks.versionToTrust(own.base))

        runBlocking { checks.trust(own.base) }
        assertEquals(SignerIntegrityState.TRUSTED_HERE, checks.line(own.base).state)
        assertTrue(checks.line(own.base).opens)
        assertNull("nothing left to ask", checks.versionToTrust(own.base))
        // Stored on the page (saved by the answer, though it was never added).
        val saved = host.settle { view -> view.saved.any { it.url == own.base } }.saved.single { it.url == own.base }
        assertEquals(listOf(own.version), saved.trusted)
        store.settles(KeyValueStore.Keys.SIGNING_PAGES) { it?.contains(own.version) == true }
        assertFalse("no device-wide list", store.values.containsKey("vela.signerPage.trusted"))
        // A rename keeps it: the trust rides every write of the list.
        host.dispatch(SigningPagesEvent.PageRenamed(own.base, "Home"), SigningPagesEvent.serializer())
        assertEquals(listOf(own.version), host.settle { view -> view.saved.any { it.name == "Home" } }.saved.single().trusted)

        // The same bytes on another page vouch for nothing there.
        runBlocking { checks.check(other.base) }
        assertEquals(SignerIntegrityState.ASK_TO_TRUST, checks.line(other.base).state)
    }

    @Test
    fun `the official page is never asked about, so there is nothing to trust`() {
        val checks = OfficialDist.checks()
        runBlocking { checks.ensure(official) }
        assertNull(checks.versionToTrust(official))
        // Asking anyway records nothing and changes nothing.
        var recorded = false
        val guarded = SignerPageChecks(fetch = OfficialDist::fetch, store = FakeStore(), recordTrust = { _, _ -> recorded = true })
        runBlocking { guarded.trust(official) }
        assertFalse(recorded)
        assertEquals(SignerIntegrityState.MATCHES, guarded.line(official).state)
    }

    // --- D-14: freshness and the background refresh ---------------------------

    @Test
    fun `a page is refreshed in the background past half a day, and not before`() {
        var now = 1_760_000_000_000L
        val checks = OfficialDist.checks(clock = { now })
        assertTrue("never checked: due", checks.refreshDue(official))
        runBlocking { checks.refresh(listOf(official)) }
        assertEquals(SignerIntegrityState.MATCHES, checks.line(official).state)
        val fetched = OfficialDist.fetches.get()
        assertFalse("just checked", checks.refreshDue(official))

        val schedule = uniffi.vela_core_uniffi.signerPageRefreshSchedule()
        now += schedule.refreshAfterMs.toLong() - 1
        runBlocking { checks.refresh(listOf(official)) }
        assertEquals("not yet half a day: nothing fetched", fetched, OfficialDist.fetches.get())

        now += 2
        assertTrue(checks.refreshDue(official))
        runBlocking { checks.refresh(listOf(official, official.trimEnd('/'))) }
        assertTrue("past half a day: fetched again, once per page", OfficialDist.fetches.get() > fetched)
        assertEquals(now.toULong(), checks.line(official).checkedAtMs)
    }

    /**
     * D-14: an offline refresh keeps a check that still vouches — the line
     * does not drop to "couldn't check" and Open still opens — and the next
     * attempt rests; one that completed replaces it, whatever it says.
     */
    @Test
    fun `an offline refresh keeps a check that still vouches, a completed one replaces it`() {
        var now = 1_760_000_000_000L
        var offline = false
        var tampered = false
        val checks = SignerPageChecks(
            fetch = { url ->
                when {
                    offline -> SignerPageChecks.Fetched.Unreachable
                    tampered && !url.endsWith("index.json") -> SignerPageChecks.Fetched.Body(200, "tampered".toByteArray())
                    else -> OfficialDist.fetch(url)
                }
            },
            store = FakeStore(),
            clock = { now },
        )
        runBlocking { checks.ensure(official) }
        val first = checks.line(official).checkedAtMs

        now += 13 * 60 * 60 * 1000L
        offline = true
        runBlocking { checks.refresh(listOf(official)) }
        assertEquals("kept", SignerIntegrityState.MATCHES, checks.line(official).state)
        assertEquals(first, checks.line(official).checkedAtMs)
        assertNotNull("still opens", checks.admitted(official))
        assertFalse("an attempt that could not complete rests", checks.refreshDue(official))

        now += uniffi.vela_core_uniffi.signerPageRefreshSchedule().retryAfterMs.toLong() + 1
        offline = false
        tampered = true
        runBlocking { checks.refresh(listOf(official)) }
        assertEquals("a completed refresh wins, a mismatch included", SignerIntegrityState.MISMATCH, checks.line(official).state)
        assertNull(checks.admitted(official))
    }

    /**
     * While a check runs, the line is the core's for that moment: the previous
     * verdict while it still vouches (a background refresh never flickers a
     * good line), "checking" once it does not — and Open waits for it.
     */
    @Test
    fun `while a check runs, a good line holds and a stale one reads checking`() {
        var now = 1_760_000_000_000L
        val gate = kotlinx.coroutines.CompletableDeferred<Unit>()
        var held = false
        val checks = SignerPageChecks(
            fetch = { url ->
                if (held && !url.endsWith("index.json")) gate.await()
                OfficialDist.fetch(url)
            },
            store = FakeStore(),
            clock = { now },
        )
        runBlocking { checks.ensure(official) }
        held = true
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default).also { scopes += it }

        now += 13 * 60 * 60 * 1000L
        val refresh = scope.launch { checks.check(official) }
        runBlocking { withTimeout(5_000L) { while (checks.checks.value[SignerPageChecks.key(official)]?.running != true) kotlinx.coroutines.delay(5) } }
        assertEquals("a fresh verdict holds", SignerIntegrityState.MATCHES, checks.line(official).state)
        assertNotNull("and still opens", checks.admitted(official))

        now += 12 * 60 * 60 * 1000L
        assertEquals("a stale one reads checking", SignerIntegrityState.CHECKING, checks.line(official).state)
        assertFalse(checks.line(official).opens)
        assertNull(checks.admitted(official))

        gate.complete(Unit)
        runBlocking { withTimeout(5_000L) { refresh.join() } }
        assertEquals(SignerIntegrityState.MATCHES, checks.line(official).state)
    }

    // --- D-13: the time a check ran -----------------------------------------

    @Test
    fun `checked at is the core's moment - the clock time today, the date before today`() {
        val zone = java.util.TimeZone.getTimeZone("UTC")
        val formats = app.getvela.wallet.core.format.Formats(
            date = app.getvela.wallet.core.format.DateFormatKey.MdySlash,
            time = app.getvela.wallet.core.format.TimeFormatKey.H24,
        )
        val now = java.time.Instant.parse("2026-10-09T18:00:00Z").toEpochMilli()
        fun matches(at: String) = uniffi.vela_core_uniffi.SignerIntegrityLine(
            SignerIntegrityState.MATCHES, "0ba8ee8c", java.time.Instant.parse(at).toEpochMilli().toULong(),
            "componentsUi.signing.integrity.matches", true,
        )
        assertEquals(
            // Each "·" binds to the word before it (U+00A0).
            "Version 0ba8ee8c\u00a0· matches Vela's published build list\u00a0· checked 14:32",
            SignerPageChecks.words(matches("2026-10-09T14:32:00Z"), strings, now, formats, zone),
        )
        assertEquals(
            // … and the moment is one unbreakable unit: its own space is U+00A0 too.
            "Version 0ba8ee8c\u00a0· matches Vela's published build list\u00a0· checked 10/08/2026,\u00a014:32",
            SignerPageChecks.words(matches("2026-10-08T14:32:00Z"), strings, now, formats, zone),
        )
        val zh = app.getvela.wallet.core.i18n.I18nRuntime { lang -> java.io.File(System.getProperty("vela.repo.root"), "assets/i18n/$lang.json").readBytes() }
            .apply { initialize("zh") }
        assertEquals("版本 0ba8ee8c\u00a0· 与 Vela 公布的构建清单一致\u00a0· 检查于 14:32", SignerPageChecks.words(matches("2026-10-09T14:32:00Z"), zh, now, formats, zone))
        // A line with no time (still checking) fills nothing in.
        assertEquals("Checking the page…", SignerPageChecks.words(SignerPageChecks.CHECKING, strings, now, formats, zone))
    }

    @Test
    fun `the check asks for the page as a browser navigates to it`() {
        val headers = uniffi.vela_core_uniffi.signerPageCheckHeaders()
        assertEquals(listOf("Accept"), headers.map { it.name })
        assertTrue(headers.single().value.startsWith("text/html"))
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

    /**
     * D-17: the hand-off card's key row is the plan's `key_label` — its label
     * the corpus key the core names ("Confirm with"), its value the key's own
     * name when the person gave it one that is not the wallet's, else its
     * place's title (the card's "Signing account" row already names the wallet).
     */
    @Test
    fun `the hand-off card names the key by the plan's key label`() {
        val named = JSONObject(record("""{"type":"page","url":"https://sign.getvela.app/"}""", domain = "getvela.app"))
        named.getJSONArray("keys").getJSONObject(0).put("name", "YubiKey 5C")
        val label = SigningPlan.of(named.toString())!!.keyLabel!!
        assertEquals("Confirm with", label.label(strings::t))
        assertEquals("YubiKey 5C", label.value(strings::t))
        // The founding key carries the wallet's name ("Savings"): named by its place.
        val founding = SigningPlan.of(record("""{"type":"page","url":"https://sign.getvela.app/"}""", domain = "getvela.app"))!!
        assertNull(founding.keyLabel!!.name)
        assertEquals("This device", founding.keyLabel!!.value(strings::t))
        val zh = app.getvela.wallet.core.i18n.I18nRuntime { lang -> java.io.File(System.getProperty("vela.repo.root"), "assets/i18n/$lang.json").readBytes() }
            .apply { initialize("zh") }
        // A label | value row, not a sentence: 「确认方式 | YubiKey 5C」.
        assertEquals("确认方式" to "YubiKey 5C", label.label(zh::t) to label.value(zh::t))
        assertEquals("确认方式" to "这台设备", founding.keyLabel!!.label(zh::t) to founding.keyLabel!!.value(zh::t))
    }

    /**
     * Spec 102 integration: a key ceremony's row is the core's
     * (`trustedSignerCeremonyKeyLabel`) — "New key on | <place>" while a key
     * is made, "Confirm with | <place>" when one signs in or proves; never a
     * name, and nothing for an operation that is not a ceremony.
     */
    @Test
    fun `a ceremony's key row is the core's`() {
        val create = SigningPlan.KeyLabel.ofCeremony("""{"type":"register_passkey","name":"Savings","method":"hybrid"}""")!!
        assertNull(create.name)
        assertEquals("New key on" to "Phone or tablet", create.label(strings::t) to create.value(strings::t))
        val signIn = SigningPlan.KeyLabel.ofCeremony("""{"type":"authenticate_passkey","method":"security_key"}""")!!
        assertEquals("Confirm with" to "USB security key", signIn.label(strings::t) to signIn.value(strings::t))
        assertNull(SigningPlan.KeyLabel.ofCeremony("""{"type":"sign_user_op"}"""))
    }

    /** The refusal's words are the core's (`venueBlockLine`): its key and the values it takes — no table here. */
    @Test
    fun `a venue refusal reads in the person's language, the web's included`() {
        val t = { key: String, vars: Map<String, String> -> strings.t(key, vars) }
        assertEquals(
            "Vela can't reach keys on sign.example.com.",
            app.getvela.wallet.feature.signing.trustedsigner.VenueBlock.AppCannotReach("sign.example.com").words(t),
        )
        assertEquals(
            "This page is on sign.example.com; this account's keys are on getvela.app.",
            app.getvela.wallet.feature.signing.trustedsigner.VenueBlock.PageOnOtherDomain("sign.example.com", "getvela.app").words(t),
        )
        assertEquals("Signing pages open from the Vela apps, not the web.", VenueWords.block(JSONObject().put("type", "not_on_web"), t))
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
