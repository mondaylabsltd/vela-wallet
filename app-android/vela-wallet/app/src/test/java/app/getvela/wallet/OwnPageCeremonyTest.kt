package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreDriver
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.feature.onboarding.core.CreateView
import app.getvela.wallet.feature.onboarding.core.OnboardingExecutor
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.CreateWalletCore
import uniffi.vela_core_uniffi.LoginCore
import uniffi.vela_core_uniffi.trustedSignerCeremonyRequest

/**
 * Spec 102 R3, through the real create and sign-in machines and the events
 * this shell sends: "Use my own signing page" (`signing_page_chosen`,
 * `sign_in {method, page}`) puts the page on the ceremony op for a custom
 * domain, and on nothing for `getvela.app` — which is exactly what the
 * executor routes by (`OnboardingExecutor.pageOf`). The place rides along as
 * `method`, never a fourth one.
 */
class OwnPageCeremonyTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    @After
    fun stop() = scope.cancel()

    private class Run(val driver: CoreDriver, val ops: MutableStateFlow<List<JSONObject>>, val view: MutableStateFlow<JSONObject?>)

    /** A machine whose ceremonies are recorded and never answered. */
    private fun drive(bridge: app.getvela.wallet.core.crux.CoreBridge): Run {
        val ops = MutableStateFlow<List<JSONObject>>(emptyList())
        val view = MutableStateFlow<JSONObject?>(null)
        val driver = CoreDriver(
            bridge = bridge,
            scope = scope,
            perform = { op ->
                ops.value = ops.value + op
                when (op.optString("type")) {
                    "check_passkey_support" -> """{"type":"passkey_support","supported":true}"""
                    "generate_group_key" -> {
                        val seed = "11".repeat(32)
                        JSONObject().put("type", "group_key_generated").put("seed_hex", seed)
                            .put("group_public_key_hex", uniffi.vela_core_uniffi.registryGroupPublicKeyFromSeed(seed)).toString()
                    }
                    "load_accounts" -> """{"type":"accounts_loaded","accounts":[]}"""
                    "probe_index_health" -> """{"type":"index_health","ok":true}"""
                    else -> CompletableDeferred<String>().await()
                }
            },
            onView = { view.value = it },
            escapedFailure = OnboardingExecutor::escapedFailure,
            onFault = { throw AssertionError("core fault", it) },
        )
        return Run(driver, ops, view)
    }

    private fun Run.send(event: JSONObject) = driver.dispatch(event.toString())

    private fun Run.op(type: String): JSONObject = runBlocking {
        withTimeout(10_000) { ops.first { list -> list.any { it.optString("type") == type } } }
            .first { it.optString("type") == type }
    }

    private fun Run.created(predicate: (CreateView) -> Boolean): CreateView = runBlocking {
        kotlinx.coroutines.withTimeoutOrNull(10_000) { view.first { json -> json != null && predicate(CreateView.from(json)) } }
            ?.let { CreateView.from(it) }
            ?: throw AssertionError("never reached: ops=${ops.value.map { it.optString("type") }} view=${view.value}")
    }

    /** The form filled and submitted: the keys screen, no key yet. */
    private fun toKeys(page: String?): Run {
        val run = drive(CreateWalletCore().asBridge())
        run.driver.dispatch("""{"type":"start"}""")
        if (page != null) run.send(JSONObject().put("type", "signing_page_chosen").put("url", page))
        run.send(JSONObject().put("type", "name_changed").put("name", "Everyday"))
        // Every acknowledgement the form asks for, however many the core has.
        val acks = run.created { true }.acks.size
        repeat(acks) { index -> run.send(JSONObject().put("type", "ack_toggled").put("index", index)) }
        run.send(JSONObject().put("type", "submit"))
        run.created { it.stage == app.getvela.wallet.feature.onboarding.core.CreateStage.AddKeys }
        return run
    }

    private fun create(page: String?): Run {
        val run = toKeys(page)
        run.send(JSONObject().put("type", "add_key").put("name", "").put("method", "security_key"))
        return run
    }

    /**
     * Issue #475: the heading over the three places and whether they are
     * pinned open are the core's (`add_heading_key`, `methods_pinned`), read
     * off the real machine — "Choose where it lives" and open with no key
     * yet (the heading was "Add a passkey", the screen's own title said
     * again: the integration's note 17); the same words, no longer pinned,
     * while the first ceremony is in flight. And the "Added n / 7" counter
     * is the core's too (`key_count_shown`, note 22): not drawn with no key.
     */
    @Test
    fun `the keys screen's heading and its pin are the core's words`() {
        val run = toKeys(null)
        val empty = run.created { it.stage == app.getvela.wallet.feature.onboarding.core.CreateStage.AddKeys && it.canAddKey }
        assertEquals("onboarding.create.keyPlaceHeading", empty.addHeadingKey)
        assertTrue("no key yet: the three places are open, nothing to fold", empty.methodsPinned)
        assertFalse("no key, no counter", empty.keyCountShown)
        run.send(JSONObject().put("type", "add_key").put("name", "").put("method", "security_key"))
        val inFlight = run.created { !it.canAddKey }
        assertEquals("onboarding.create.keyPlaceHeading", inFlight.addHeadingKey)
        assertFalse("not pinned open over a ceremony", inFlight.methodsPinned)
        assertFalse("a ceremony in flight is not a key yet", inFlight.keyCountShown)
    }

    @Test
    fun `a custom domain's page mints the key there, and the place rides along`() {
        val run = create("https://sign.example.com")
        val register = run.op("register_passkey")
        assertEquals("https://sign.example.com/", OnboardingExecutor.pageOf(register))
        assertEquals("security_key", register.getString("method"))
        val view = run.created { it.signingPage != null }
        assertEquals("sign.example.com", view.signingDomain)
        // The core builds the page's request from the op as it came — the place is the page's hint.
        val request = trustedSignerCeremonyRequest(register.toString(), "id", "Everyday", "https://index.example", null)
        assertNotNull(request)
        assertTrue(request!!, request.contains("security-key"))
    }

    @Test
    fun `the official page runs the ceremony in the app`() {
        val run = create("https://sign.getvela.app/")
        val register = run.op("register_passkey")
        assertNull("R3: a getvela.app page runs in the app", OnboardingExecutor.pageOf(register))
        val view = CreateView.from(run.view.value!!)
        assertEquals("getvela.app", view.signingDomain)
        assertEquals("https://sign.getvela.app/", view.signingPage)
    }

    @Test
    fun `no page chosen is the app, and a page may no longer be chosen once a key is in flight`() {
        val run = create(null)
        val register = run.op("register_passkey")
        assertNull(OnboardingExecutor.pageOf(register))
        val view = CreateView.from(run.view.value!!)
        assertEquals("getvela.app", view.signingDomain)
        assertFalse(view.canChoosePage)
        assertEquals(3, view.addMethods.size)
    }

    @Test
    fun `a sign-in on a custom domain's page authenticates there`() {
        val run = drive(LoginCore().asBridge())
        run.driver.dispatch("""{"type":"start"}""")
        run.send(JSONObject().put("type", "sign_in").put("method", "hybrid").put("page", "https://sign.example.com/"))
        val auth = run.op("authenticate_passkey")
        assertEquals("https://sign.example.com/", OnboardingExecutor.pageOf(auth))
        assertEquals("hybrid", auth.getString("method"))

        val inApp = drive(LoginCore().asBridge())
        inApp.driver.dispatch("""{"type":"start"}""")
        inApp.send(JSONObject().put("type", "sign_in").put("method", "platform").put("page", JSONObject.NULL))
        assertNull(OnboardingExecutor.pageOf(inApp.op("authenticate_passkey")))
    }
}
