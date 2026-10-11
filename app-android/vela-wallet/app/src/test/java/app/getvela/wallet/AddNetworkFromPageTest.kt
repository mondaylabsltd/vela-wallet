package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.browser.ExploreLive
import app.getvela.wallet.feature.browser.core.BrowserExecutor
import app.getvela.wallet.feature.browser.core.DappAddOutcome
import app.getvela.wallet.feature.browser.core.DbrEvent
import app.getvela.wallet.feature.browser.core.DbrOperation
import app.getvela.wallet.feature.browser.core.DbrReason
import app.getvela.wallet.feature.browser.core.DbrShellResult
import app.getvela.wallet.feature.browser.core.DbrView
import app.getvela.wallet.feature.settings.core.NetDappAddPhase
import app.getvela.wallet.feature.settings.core.NetEvent
import app.getvela.wallet.feature.settings.core.NetOperation
import app.getvela.wallet.feature.settings.core.NetRawChainData
import app.getvela.wallet.feature.settings.core.NetShellResult
import app.getvela.wallet.feature.settings.core.NetView
import app.getvela.wallet.feature.settings.core.NetworkAdminExecutor
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.DappBrowserCore
import uniffi.vela_core_uniffi.NetworkAdminCore

/**
 * Spec 100: a page's `wallet_addEthereumChain` through BOTH real machines and
 * this shell's executors, carried between them the way
 * `VelaWalletApplication` carries it — `forward_to_add_network` →
 * `dapp_add_requested`, `dapp_add_settled` → `add_network_answered`. A fake
 * network answers the sheet's probes; everything else is the real thing. (The
 * rules themselves are the core's `app_dapp_add_network_100` tests.)
 */
class AddNetworkFromPageTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val safe = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private val origin = "http://127.0.0.1:8137"
    private val sepolia = 11_155_111L
    private val catalogRpc = "https://rpc.sepolia.example"
    private val strings: VelaStrings = run {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
    }

    private val delivered = java.util.concurrent.CopyOnWriteArrayList<JSONObject>()
    private val probed = java.util.concurrent.CopyOnWriteArrayList<String>()

    @After
    fun stop() = scope.cancel()

    /** The two machines and the relay between them. [deployed]: whether the fake chain carries Vela's contracts. */
    private class Rig(val browser: CoreHost<DbrView>, val admin: CoreHost<NetView>, val store: FakeStore)

    /** [p256]: whether the fake chain has the P-256 verifier at 0x100. */
    private fun rig(deployed: Boolean, p256: Boolean = true): Rig {
        val store = FakeStore()
        lateinit var browser: CoreHost<DbrView>
        lateinit var admin: CoreHost<NetView>
        val adminExecutor = NetworkAdminExecutor(
            store,
            dappAddSettled = {
                { tab, id, outcome -> browser.dispatch(DbrEvent.AddNetworkAnswered(tab, id, outcome, 1.0e12), DbrEvent.serializer()) }
            },
        )
        // The network this test stands up: the catalog knows Sepolia and every
        // probe answers. Store writes and the ending are the real executor's.
        val network: suspend (NetOperation) -> NetShellResult = { operation ->
            when (operation) {
                is NetOperation.FetchChainInfo -> NetShellResult.ChainInfo(
                    operation.chain_id,
                    NetRawChainData(chain_id = sepolia, name = "Ethereum Sepolia", native_currency_symbol = "ETH", rpc = listOf(catalogRpc)),
                )
                is NetOperation.ProbeRpc -> {
                    probed += operation.url
                    NetShellResult.Probed(operation.url, sepolia, 20)
                }
                is NetOperation.RpcGetCode -> NetShellResult.Code(operation.url, operation.address, if (deployed) "0x6080604052" else "0x")
                is NetOperation.RpcCallP256 -> NetShellResult.P256Call(operation.url, if (p256) "0x" + "0".repeat(63) + "1" else "0x")
                else -> adminExecutor.perform(operation)
            }
        }
        admin = CoreHost(
            bridge = NetworkAdminCore().asBridge(), scope = scope, initial = NetView(), serializer = NetView.serializer(),
            perform = JsonShell.perform(NetOperation.serializer(), NetShellResult.serializer(), network),
            escapedFailure = JsonShell.escapedFailure(NetOperation.serializer(), NetShellResult.serializer(), fallback = NetShellResult.Written, answer = adminExecutor::neutralAnswer),
            onFault = { error -> throw AssertionError("network_admin fault: $error", error) },
        )
        val browserExecutor = BrowserExecutor(
            store,
            object : BrowserExecutor.Ports {
                override fun deliver(tab: String, messageJson: String) { delivered += JSONObject(messageJson) }
                override suspend fun read(chainId: Int, method: String, params: JSONArray, bundler: Boolean) = BrowserExecutor.ReadAnswer.NoAnswer()
                override suspend fun userOpTxHash(chainId: Int, userOpHash: String): String? = null
                override fun forwardToSigning(operation: DbrOperation.ForwardToSigning) = Unit
                override fun cancelSigning(tab: String, id: String) = Unit
                override fun saveConnectionRecord(row: JSONObject) = Unit
                override fun forwardToAddNetwork(operation: DbrOperation.ForwardToAddNetwork) =
                    admin.dispatch(NetEvent.DappAddRequested(operation.tab, operation.id, operation.origin, operation.ask), NetEvent.serializer())
                override fun cancelAddNetwork(tab: String, id: String) =
                    admin.dispatch(NetEvent.DappAddCancelled(tab, id), NetEvent.serializer())
            },
        )
        browser = CoreHost(
            bridge = DappBrowserCore().asBridge(), scope = scope, initial = DbrView(), serializer = DbrView.serializer(),
            perform = JsonShell.perform(DbrOperation.serializer(), DbrShellResult.serializer(), browserExecutor::perform),
            escapedFailure = JsonShell.escapedFailure(DbrOperation.serializer(), DbrShellResult.serializer(), fallback = DbrShellResult.Ack, answer = browserExecutor::neutralAnswer),
        )
        admin.dispatch(NetEvent.Started, NetEvent.serializer())
        browser.dispatch(DbrEvent.Start, DbrEvent.serializer())
        browser.dispatch(DbrEvent.NetworksChanged(listOf(1, 100, 8453)), DbrEvent.serializer())
        browser.dispatch(DbrEvent.AccountsUpdated(listOf(safe)), DbrEvent.serializer())
        browser.dispatch(DbrEvent.AccountSwitched(safe, 1.0e12), DbrEvent.serializer())
        page(browser, JSONObject().put("t", "hello").put("doc", "d1"))
        return Rig(browser, admin, store)
    }

    private fun page(h: CoreHost<DbrView>, message: JSONObject) =
        h.dispatch(DbrEvent.PageMessage(tab = "t1", frame_origin = origin, is_main_frame = true, message_json = message.toString()), DbrEvent.serializer())

    private fun addSepolia(h: CoreHost<DbrView>, id: String) = page(
        h,
        JSONObject().put("t", "req").put("doc", "d1").put("id", id).put("method", "wallet_addEthereumChain").put(
            "params",
            JSONArray().put(JSONObject().put("chainId", "0xaa36a7").put("chainName", "My Sepolia").put("rpcUrls", JSONArray().put("https://rpc.invalid"))),
        ),
    )

    private suspend fun answerFor(id: String): JSONObject {
        withTimeout(10_000) { while (delivered.none { it.optString("id") == id && it.optString("dir") == "res" }) delay(10) }
        return delivered.first { it.optString("id") == id && it.optString("dir") == "res" }
    }

    private fun CoreHost<NetView>.sheet(phase: NetDappAddPhase) =
        runBlocking { withTimeout(10_000) { view.first { it.dapp_add?.phase == phase } } }.dapp_add!!

    @Test
    fun `an approved add is saved the Settings way and answers the page null with chainChanged`() = runBlocking<Unit> {
        val rig = rig(deployed = true)
        addSepolia(rig.browser, "a1")
        val sheet = rig.admin.sheet(NetDappAddPhase.Ready)
        assertEquals("Ethereum Sepolia", sheet.name)
        assertFalse("the page's RPC is never asked for a chain the catalog knows", probed.any { it.contains("rpc.invalid") })
        // The sheet, in Settings' words.
        val model = ExploreLive.addNetwork(sheet, strings)
        assertEquals("127.0.0.1:8137 asks to add a network", model.lead)
        assertEquals("Add Network", model.add)
        assertEquals("Ethereum Sepolia", model.rows.first().second)
        assertNull(model.fromSite)

        rig.admin.dispatch(NetEvent.DappAddApproved("2026-10-04T12:00:00Z"), NetEvent.serializer())
        val answer = answerFor("a1")
        assertTrue("answered null", answer.has("result") && answer.isNull("result"))
        withTimeout(10_000) { while (delivered.none { it.optString("event") == "chainChanged" }) delay(10) }
        assertEquals("0xaa36a7", delivered.first { it.optString("event") == "chainChanged" }.optString("data"))
        assertTrue(
            "saved where every reader of custom networks looks",
            rig.store.values[KeyValueStore.Keys.CUSTOM_NETWORKS].orEmpty().contains("11155111"),
        )
        assertEquals("11155111", rig.store.values["vela.chain.$origin"])
        assertNull(rig.admin.view.value.dapp_add)
    }

    @Test
    fun `an incompatible chain is refused 4902 and recorded, and nothing is saved`() = runBlocking<Unit> {
        val rig = rig(deployed = false)
        addSepolia(rig.browser, "a1")
        val sheet = rig.admin.sheet(NetDappAddPhase.NotCompatible)
        val model = ExploreLive.addNetwork(sheet, strings)
        assertNull("no Add for a chain this wallet refuses", model.add)
        assertEquals("Done", model.dismiss)
        // The verifier is there and contracts are not: the core's line, and
        // Chain Setup opened on THIS chain.
        assertEquals("missing_contracts", sheet.compat?.blocker)
        assertEquals(strings.t("settingsModals.addNetwork.incompatibleHint"), model.note)
        assertEquals("Open Chain Setup Tool", model.setupTool?.label)
        assertEquals("https://getvela.app/chain-setup?chain=11155111", model.setupTool?.url)

        rig.admin.dispatch(NetEvent.DappAddDeclined, NetEvent.serializer())
        assertEquals(4902, answerFor("a1").getJSONObject("error").getInt("code"))
        assertFalse(rig.store.values[KeyValueStore.Keys.CUSTOM_NETWORKS].orEmpty().contains("11155111"))
        rig.browser.dispatch(DbrEvent.InspectorOpened("t1"), DbrEvent.serializer())
        val row = withTimeout(10_000) { rig.browser.view.first { it.inspector?.rows?.any { r -> r.id == "a1" && r.reason != null } == true } }
            .inspector!!.rows.first { it.id == "a1" }
        assertEquals(DbrReason.NotCompatible, row.reason)
        assertEquals(4902L, row.code)
    }

    /**
     * A chain with no P-256 verifier, through the real machine: refused for
     * THAT (it wins over missing contracts), said in its own words, and with
     * no Chain Setup button — nothing can be deployed to make it work.
     */
    @Test
    fun `a chain with no P-256 verifier is refused in its own words, with nothing to deploy`() = runBlocking<Unit> {
        val rig = rig(deployed = false, p256 = false)
        addSepolia(rig.browser, "a1")
        val sheet = rig.admin.sheet(NetDappAddPhase.NotCompatible)
        assertEquals("no_p256", sheet.compat?.blocker)
        assertNull(sheet.compat?.setup_url)
        val model = ExploreLive.addNetwork(sheet, strings)
        assertNull(model.add)
        assertEquals(strings.t("settingsModals.addNetwork.noP256Hint"), model.note)
        assertTrue(model.note!!, model.note!!.contains("Vela wallets can't work here"))
        assertNull("no Chain Setup for a chain nothing can be deployed on", model.setupTool)
        assertFalse(model.checks.last().ok)

        rig.admin.dispatch(NetEvent.DappAddDeclined, NetEvent.serializer())
        assertEquals(4902, answerFor("a1").getJSONObject("error").getInt("code"))
    }

    @Test
    fun `a second add while the sheet is open is -32002`() = runBlocking<Unit> {
        val rig = rig(deployed = true)
        addSepolia(rig.browser, "a1")
        rig.admin.sheet(NetDappAddPhase.Ready)
        addSepolia(rig.browser, "a2")
        assertEquals(-32002, answerFor("a2").getJSONObject("error").getInt("code"))
        rig.admin.dispatch(NetEvent.DappAddDeclined, NetEvent.serializer())
        assertEquals(4001, answerFor("a1").getJSONObject("error").getInt("code"))
    }

    @Test
    fun `the outcome wire round-trips every ending`() {
        for (outcome in listOf(DappAddOutcome.Added(sepolia.toInt()), DappAddOutcome.Declined, DappAddOutcome.NotCompatible, DappAddOutcome.BadRpc, DappAddOutcome.Busy)) {
            val json = app.getvela.wallet.core.crux.Wire.json.encodeToString(DappAddOutcome.serializer(), outcome)
            assertEquals(outcome, app.getvela.wallet.core.crux.Wire.json.decodeFromString(DappAddOutcome.serializer(), json))
        }
    }
}
