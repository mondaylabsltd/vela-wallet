package app.getvela.wallet

import androidx.compose.runtime.Composable
import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.dev.FixtureAccount
import app.getvela.wallet.dev.ParallelSpaceHook
import app.getvela.wallet.dev.ParallelSpaceProvider
import app.getvela.wallet.feature.browser.core.BrowserAccounts
import app.getvela.wallet.feature.browser.core.BrowserExecutor
import app.getvela.wallet.feature.browser.core.DbrEvent
import app.getvela.wallet.feature.browser.core.DbrOperation
import app.getvela.wallet.feature.browser.core.DbrShellResult
import app.getvela.wallet.feature.browser.core.DbrView
import app.getvela.wallet.feature.send.core.UserOpSigner
import java.util.concurrent.CopyOnWriteArrayList
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeoutOrNull
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.DappBrowserCore

/**
 * Device walk 2026-10-11, item 5: a walk through the parallel space dropped
 * the owner's Arc Orbs connection, and every earlier walk had done the same
 * to whatever was connected.
 *
 * Through the REAL `dapp_browser` machine and this shell's REAL
 * [BrowserExecutor], over the store the executor suites use, with the account
 * events exactly as the app makes them ([BrowserAccounts.events], which asks
 * [ParallelSpaceHook] whose account is in front): the person's sites are in
 * the store → the space is entered → a site is connected inside (one the
 * person had connected too, and a new one) and switched to another network →
 * the space is left. Every `vela.perm.*` / `vela.chain.*` value is then byte
 * for byte what it was, and NOTHING was written or removed in between.
 *
 * It fails without the fix: the same events with no `space` re-pin both
 * grants to the test wallet on the way in (two writes) and remove them on the
 * way out.
 */
class ParallelSpaceSitesTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    private val ann = "0x1F9840a85d5aF5bf1D1762F925BDADdC4201F984"
    private val bo = "0x6B175474E89094C44Da98b954EedeAC495271d0F"
    private val fixture = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    /** The device walks' own test dApp — connected by the person, and again inside the space. */
    private val testDapp = "http://127.0.0.1:8137"

    /** The connection the owner lost. */
    private val orbs = "https://arcorbs.game3.app"

    /** A site first met inside the space. */
    private val fresh = "https://new.example"

    /** The space as the debug build has it: a flag, and the test wallet's address — which is not the flag's to change. */
    private class Space(private val address: String) : ParallelSpaceProvider {
        @Volatile var on = false
        override fun active() = on
        override fun enter() { on = true }
        override fun leave() { on = false }
        override fun signer(): UserOpSigner = error("nothing signs here")
        override fun fixtureAccount() = FixtureAccount(0, "cred-fixture", "04ef", address)

        @Composable
        override fun Badge() = Unit
    }

    /** [FakeStore], with every change asked of it written down. */
    private class WatchedStore(val inner: FakeStore) : KeyValueStore by inner {
        val changes = CopyOnWriteArrayList<String>()
        override suspend fun write(key: String, value: String): Boolean {
            changes += "write $key = $value"
            return inner.write(key, value)
        }

        override suspend fun remove(vararg keys: String): Boolean {
            keys.forEach { changes += "remove $it" }
            return inner.remove(*keys)
        }

        /** The site store alone, as it is on disk. */
        fun sites(): Map<String, String> = inner.values
            .filterKeys { it.startsWith(BrowserExecutor.GRANT_PREFIX) || it.startsWith(BrowserExecutor.CHAIN_PREFIX) }
            .toSortedMap()
    }

    private val space = Space(fixture)
    private val delivered = CopyOnWriteArrayList<Pair<String, JSONObject>>()

    /** How many messages the pages have been sent: what [answer] wakes on. */
    private val deliveries = MutableStateFlow(0)

    @After
    fun stop() {
        ParallelSpaceHook.install(null)
        scope.cancel()
    }

    private fun personsStore() = WatchedStore(
        FakeStore(
            mapOf(
                BrowserExecutor.grantKey(testDapp) to """{"origin":"$testDapp","address":"$bo","chain_id":100,"granted_at_ms":1.7e12}""",
                BrowserExecutor.chainKey(testDapp) to "100",
                BrowserExecutor.grantKey(orbs) to """{"origin":"$orbs","address":"$bo","chain_id":8453,"granted_at_ms":1.6e12}""",
                BrowserExecutor.chainKey(orbs) to "8453",
                "vela.accounts" to "[]",
            ),
        ),
    )

    private fun host(store: KeyValueStore): CoreHost<DbrView> {
        val executor = BrowserExecutor(
            store,
            object : BrowserExecutor.Ports {
                override fun deliver(tab: String, messageJson: String) {
                    delivered += tab to JSONObject(messageJson)
                    deliveries.update { it + 1 }
                }
                override suspend fun read(chainId: Int, method: String, params: JSONArray, bundler: Boolean): BrowserExecutor.ReadAnswer =
                    BrowserExecutor.ReadAnswer.Body(JSONObject().put("jsonrpc", "2.0").put("id", 1).put("result", "0x10"))
                override suspend fun userOpTxHash(chainId: Int, userOpHash: String): String? = null
                override fun forwardToSigning(operation: DbrOperation.ForwardToSigning) = Unit
                override fun cancelSigning(tab: String, id: String) = Unit
                override fun saveConnectionRecord(row: JSONObject) = Unit
            },
        )
        return CoreHost(
            bridge = DappBrowserCore().asBridge(), scope = scope, initial = DbrView(), serializer = DbrView.serializer(),
            perform = JsonShell.perform(DbrOperation.serializer(), DbrShellResult.serializer(), executor::perform),
            escapedFailure = JsonShell.escapedFailure(DbrOperation.serializer(), DbrShellResult.serializer(), fallback = DbrShellResult.Ack, answer = executor::neutralAnswer),
        )
    }

    private var clock = 1.7e12

    /**
     * The session's accounts changed: what the app tells the browser machine
     * ([BrowserAccounts.events], as `BrowserController.accountsChanged` sends
     * them). [switchFirst] states the two in the other order — the core must
     * not depend on which arrives first.
     */
    private fun accounts(h: CoreHost<DbrView>, addresses: List<String>, active: String, switchFirst: Boolean) {
        clock += 1_000
        val events = BrowserAccounts.events(addresses, active, clock)
        for (event in if (switchFirst) events.reversed() else events) h.dispatch(event, DbrEvent.serializer())
    }

    private fun page(h: CoreHost<DbrView>, tab: String, origin: String, message: JSONObject) =
        h.dispatch(DbrEvent.PageMessage(tab = tab, frame_origin = origin, is_main_frame = true, message_json = message.toString()), DbrEvent.serializer())

    private fun ask(h: CoreHost<DbrView>, tab: String, origin: String, id: String, method: String, params: JSONArray = JSONArray()) =
        page(h, tab, origin, JSONObject().put("t", "req").put("doc", "d-$tab").put("id", id).put("method", method).put("params", params))

    /**
     * The page's answer to [id]. Also the barrier every "nothing was written"
     * below stands on: the driver starts effects in the order the core asked
     * for them and a store write never suspends, so once a LATER request's
     * answer has been delivered, every write an earlier event asked for is
     * done (`CoreDriver`, property 1).
     */
    private suspend fun answer(id: String): JSONObject = settled("the page was never answered for $id — delivered so far: $delivered") {
        deliveries.first { delivered.any { it.second.optString("id") == id } }
        delivered.first { it.second.optString("id") == id }.second
    }

    /**
     * Woken by what it waits for, never by a clock. [HANG_MS] is not a
     * deadline anything is measured against: it only turns a machine that
     * never answers into a failure that says so, instead of a hung run.
     */
    private suspend fun <T> settled(orElse: String, wait: suspend () -> T): T =
        withTimeoutOrNull(HANG_MS) { wait() } ?: throw AssertionError(orElse)

    private suspend fun connected(h: CoreHost<DbrView>, tab: String, origin: String, id: String): List<String> {
        ask(h, tab, origin, id, "eth_accounts")
        val result = answer(id).getJSONArray("result")
        return (0 until result.length()).map { result.getString(it) }
    }

    private suspend fun connect(h: CoreHost<DbrView>, tab: String, origin: String, id: String): String {
        ask(h, tab, origin, id, "eth_requestAccounts")
        val consent = settled("no consent sheet for $origin") { h.view.first { it.consent?.origin == origin } }.consent!!
        clock += 1_000
        h.dispatch(DbrEvent.ConsentApproved(clock), DbrEvent.serializer())
        val granted = answer(id).getJSONArray("result").getString(0)
        assertEquals("the consent named the account it granted", consent.address, granted)
        return granted
    }

    private fun walk(switchFirst: Boolean) = runBlocking<Unit> {
        ParallelSpaceHook.install(space)
        val store = personsStore()
        val before = store.sites()
        val h = host(store)
        h.dispatch(DbrEvent.Start, DbrEvent.serializer())
        h.dispatch(DbrEvent.NetworksChanged(listOf(1, 100, 8453)), DbrEvent.serializer())
        accounts(h, listOf(ann, bo), bo, switchFirst)
        settled("the stored sites were never read") { h.view.first { it.ready && it.sites.size == 2 } }
        for ((tab, origin) in listOf("t-dapp" to testDapp, "t-orbs" to orbs, "t-new" to fresh)) {
            page(h, tab, origin, JSONObject().put("t", "hello").put("doc", "d-$tab"))
        }
        assertEquals("the person's site, before the space", listOf(bo), connected(h, "t-dapp", testDapp, "a0"))
        assertEquals("the launch itself rewrote nothing", emptyList<String>(), store.changes.toList())

        // -- in: the session appends the test wallet and puts it in front ------------
        space.enter()
        accounts(h, listOf(ann, bo, fixture), fixture, switchFirst)
        assertEquals("inside, no site is connected until it is connected THERE", emptyList<String>(), connected(h, "t-dapp", testDapp, "a1"))
        assertEquals(emptyList<String>(), connected(h, "t-orbs", orbs, "a2"))

        // -- inside: the same site the person had, and a new one ---------------------
        assertEquals(fixture.lowercase(), connect(h, "t-dapp", testDapp, "c1").lowercase())
        assertEquals(fixture.lowercase(), connect(h, "t-new", fresh, "c2").lowercase())
        ask(h, "t-dapp", testDapp, "s1", "wallet_switchEthereumChain", JSONArray().put(JSONObject().put("chainId", "0x2105")))
        assertTrue("the switch inside is answered: ${answer("s1")}", answer("s1").has("result"))
        ask(h, "t-dapp", testDapp, "q1", "eth_chainId")
        assertEquals("the space's page is on the network it picked", "0x2105", answer("q1").getString("result"))
        assertEquals("nothing reached the store from inside the space", emptyList<String>(), store.changes.toList())
        assertEquals(before, store.sites())

        // -- out: the flag goes first, the record a moment later ---------------------
        space.leave()
        assertFalse("the flag already says outside", ParallelSpaceHook.active())
        // The session speaks while the test wallet is still in front (any change of its view).
        accounts(h, listOf(ann, bo, fixture), fixture, switchFirst)
        assertEquals("still the space's own site", listOf(fixture.lowercase()), connected(h, "t-new", fresh, "a3").map { it.lowercase() })
        // The record is gone; the wallet the person was on is in front again.
        accounts(h, listOf(ann, bo), bo, switchFirst)

        assertEquals("the person's sites are theirs again", listOf(bo), connected(h, "t-dapp", testDapp, "a4"))
        assertEquals(listOf(bo), connected(h, "t-orbs", orbs, "a5"))
        assertEquals("what was connected inside ended with the visit", emptyList<String>(), connected(h, "t-new", fresh, "a6"))
        ask(h, "t-dapp", testDapp, "q2", "eth_chainId")
        assertEquals("and the person's page is back on its own network", "0x64", answer("q2").getString("result"))

        assertEquals("not one write or removal, in, inside or out", emptyList<String>(), store.changes.toList())
        assertEquals("the site store, byte for byte", before, store.sites())
        assertEquals("Settings' list is the person's again", setOf(testDapp, orbs), h.view.value.sites.map { it.origin }.toSet())
        assertTrue(h.view.value.sites.all { it.address == bo })
    }

    @Test
    fun `a walk through the space leaves the person's sites byte for byte, accounts first`() = walk(switchFirst = false)

    @Test
    fun `a walk through the space leaves the person's sites byte for byte, the switch first`() = walk(switchFirst = true)

    /**
     * What is said to the core: `space` exactly for the test wallet, by its
     * ADDRESS — in any letter case, with the flag on or already cleared — and
     * never for an account of the person's, in the space or out of it.
     */
    @Test
    fun `the switch says space for the test wallet's address and for nothing else`() {
        fun said(active: String) = BrowserAccounts.events(listOf(ann, bo, fixture), active, 1.0)
            .filterIsInstance<DbrEvent.AccountSwitched>().single().space

        assertFalse("no space in this build: an ordinary switch", said(fixture))
        ParallelSpaceHook.install(space)
        space.on = true
        assertTrue(said(fixture))
        assertTrue(said(fixture.lowercase()))
        assertFalse("the person switched to their own wallet inside the space", said(ann))
        space.on = false
        assertTrue("the flag is cleared before the record is removed: still the test wallet", said(fixture))
        assertFalse(said(bo))
        assertEquals(
            "no account in front: only the list is stated",
            listOf<DbrEvent>(DbrEvent.AccountsUpdated(listOf(ann))),
            BrowserAccounts.events(listOf(ann), null, 1.0),
        )
    }

    private companion object {
        /** A hang limit, not a deadline (see [settled]). */
        const val HANG_MS = 120_000L
    }

    /** The wire: `space` is on the event the core reads, and absent means false (an older shell's event). */
    @Test
    fun `space travels on the event`() {
        val json = app.getvela.wallet.core.crux.Wire.json
        val said = json.encodeToString(DbrEvent.serializer(), DbrEvent.AccountSwitched(fixture, 5.0, space = true))
        assertTrue(said, JSONObject(said).getBoolean("space"))
        assertEquals("account_switched", JSONObject(said).getString("type"))
        val plain = json.encodeToString(DbrEvent.serializer(), DbrEvent.AccountSwitched(bo, 5.0))
        assertFalse(plain, JSONObject(plain).optBoolean("space", false))
    }
}
