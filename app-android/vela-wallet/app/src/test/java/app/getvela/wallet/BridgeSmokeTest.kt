package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.feature.browser.core.BhistEvent
import app.getvela.wallet.feature.browser.core.BhistOperation
import app.getvela.wallet.feature.browser.core.BhistShellResult
import app.getvela.wallet.feature.browser.core.BhistView
import app.getvela.wallet.feature.browser.core.DpermEvent
import app.getvela.wallet.feature.browser.core.DpermOperation
import app.getvela.wallet.feature.browser.core.DpermShellResult
import app.getvela.wallet.feature.browser.core.DpermView
import app.getvela.wallet.feature.browser.core.ExploreEvent
import app.getvela.wallet.feature.browser.core.ExploreOperation
import app.getvela.wallet.feature.browser.core.ExploreShellResult
import app.getvela.wallet.feature.browser.core.ExploreView
import app.getvela.wallet.feature.signing.core.ClearOperation
import app.getvela.wallet.feature.signing.core.ClearShellResult
import app.getvela.wallet.feature.signing.core.ClearSigningEvent
import app.getvela.wallet.feature.signing.core.ClearSigningView
import app.getvela.wallet.feature.signing.core.GuardEvent
import app.getvela.wallet.feature.signing.core.GuardOperation
import app.getvela.wallet.feature.signing.core.GuardShellResult
import app.getvela.wallet.feature.signing.core.GuardView
import app.getvela.wallet.feature.signing.core.SignEvent
import app.getvela.wallet.feature.signing.core.SignOperation
import app.getvela.wallet.feature.signing.core.SignShellResult
import app.getvela.wallet.feature.signing.core.SignView
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.serialization.KSerializer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.ApprovalGuardCore
import uniffi.vela_core_uniffi.BrowserHistoryCore
import uniffi.vela_core_uniffi.ClearSigningCore
import uniffi.vela_core_uniffi.DappPermissionsCore
import uniffi.vela_core_uniffi.ExploreSitesCore
import uniffi.vela_core_uniffi.SignRequestCore
import uniffi.vela_core_uniffi.dappOriginOf

/**
 * Spec 044 T009: each of the six browser-and-signing machines is created,
 * receives its first event, answers a view, and faults on nothing. The
 * executors are stubs that answer the neutral shape; the point is the
 * bridge and the wire, not the arms.
 */
class BridgeSmokeTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val faults = ArrayList<String>()

    @After
    fun stop() = scope.cancel()

    private fun <V : Any, O : Any, R : Any> host(
        bridge: app.getvela.wallet.core.crux.CoreBridge,
        initial: V,
        view: KSerializer<V>,
        op: KSerializer<O>,
        res: KSerializer<R>,
        answer: (O) -> R,
    ): CoreHost<V> = CoreHost(
        bridge = bridge,
        scope = scope,
        initial = initial,
        serializer = view,
        perform = JsonShell.perform(op, res) { operation -> answer(operation) },
        escapedFailure = JsonShell.escapedFailure(op, res, fallback = answer(dummyOf(op)), answer = answer),
        onFault = { error -> faults += error.toString() },
    )

    @Suppress("UNCHECKED_CAST")
    private fun <O : Any> dummyOf(op: KSerializer<O>): O = when (op.descriptor.serialName) {
        DpermOperation.serializer().descriptor.serialName -> DpermOperation.ReadGrant("") as O
        ExploreOperation.serializer().descriptor.serialName -> ExploreOperation.ReadExplore as O
        BhistOperation.serializer().descriptor.serialName -> BhistOperation.ReadHistory as O
        SignOperation.serializer().descriptor.serialName -> SignOperation.SwitchActiveAccount(0) as O
        ClearOperation.serializer().descriptor.serialName -> ClearOperation.Now as O
        GuardOperation.serializer().descriptor.serialName -> GuardOperation.ReadTokenMetadata(1, emptyList()) as O
        else -> error("no dummy for ${op.descriptor.serialName}")
    }

    @Test
    fun `the six machines answer their first event without a fault`() = runBlocking {
        val explore = host(ExploreSitesCore().asBridge(), ExploreView(), ExploreView.serializer(), ExploreOperation.serializer(), ExploreShellResult.serializer()) { op ->
            when (op) {
                ExploreOperation.ReadExplore -> ExploreShellResult.Loaded(null)
                is ExploreOperation.WriteExplore -> ExploreShellResult.Written
            }
        }
        explore.dispatch(ExploreEvent.Start, ExploreEvent.serializer())
        assertTrue(withTimeout(10_000) { explore.view.first { it.ready } }.ready)

        // The history machine records a visit only once its store has answered
        // (Start → ReadHistory → Loaded); the view has no "ready" flag, so the
        // executor's answer is the signal.
        val historyLoaded = kotlinx.coroutines.CompletableDeferred<Unit>()
        val bhist = host(BrowserHistoryCore().asBridge(), BhistView(), BhistView.serializer(), BhistOperation.serializer(), BhistShellResult.serializer()) { op ->
            when (op) {
                BhistOperation.ReadHistory -> BhistShellResult.Loaded().also { historyLoaded.complete(Unit) }
                is BhistOperation.WriteHistory -> BhistShellResult.Written
                BhistOperation.RemoveHistory -> BhistShellResult.Written
            }
        }
        bhist.dispatch(BhistEvent.Start, BhistEvent.serializer())
        withTimeout(10_000) { historyLoaded.await() }
        kotlinx.coroutines.delay(50)
        bhist.dispatch(BhistEvent.VisitRecorded("https://app.uniswap.org/swap", "Uniswap", null, 1.0e12), BhistEvent.serializer())
        val visited = withTimeout(10_000) { bhist.view.first { it.entries.isNotEmpty() } }
        assertEquals("https://app.uniswap.org", visited.entries.single().origin)

        val dperm = host(DappPermissionsCore().asBridge(), DpermView(), DpermView.serializer(), DpermOperation.serializer(), DpermShellResult.serializer()) { op ->
            when (op) {
                is DpermOperation.ReadGrant -> DpermShellResult.GrantRead(op.origin, null)
                else -> DpermShellResult.Ack
            }
        }
        dperm.dispatch(DpermEvent.NavigationStarted("https://app.uniswap.org/swap?x=1"), DpermEvent.serializer())
        val navigated = withTimeout(10_000) { dperm.view.first { it.current_origin != null } }
        assertEquals("https://app.uniswap.org", navigated.current_origin)

        val sign = host(SignRequestCore().asBridge(), SignView(), SignView.serializer(), SignOperation.serializer(), SignShellResult.serializer()) { op ->
            when (op) {
                is SignOperation.SendResponse -> SignShellResult.Responded
                is SignOperation.CheckBundlerFunding -> SignShellResult.PreCheck(null)
                is SignOperation.AttemptSponsorship -> SignShellResult.Sponsorship(app.getvela.wallet.feature.signing.core.SignSponsorship.Denied(null))
                is SignOperation.SignAndSubmit -> SignShellResult.Submit(app.getvela.wallet.feature.signing.core.SignSubmitOutcome.Failed("stub"), 0.0)
                is SignOperation.PersistRecord -> SignShellResult.RecordPersisted
                is SignOperation.UpdateRecord -> SignShellResult.RecordUpdated
                is SignOperation.SwitchActiveAccount -> SignShellResult.AccountSwitched
            }
        }
        sign.dispatch(SignEvent.NetworksChanged(listOf(1, 100)), SignEvent.serializer())
        val networks = withTimeout(10_000) { sign.view.first { true } }
        assertEquals(SignView().surface, networks.surface)

        val clear = host(ClearSigningCore().asBridge(), ClearSigningView(), ClearSigningView.serializer(), ClearOperation.serializer(), ClearShellResult.serializer()) { op ->
            when (op) {
                is ClearOperation.HttpGet -> ClearShellResult.DescriptorFetched(op.path, null)
                is ClearOperation.RpcEthCall -> ClearShellResult.RpcAnswer(op.probe, op.chain_id, op.to, null, true)
                is ClearOperation.SelectorDbLookup -> ClearShellResult.SelectorCandidates()
                is ClearOperation.Timer -> ClearShellResult.TimedOut(op.token)
                ClearOperation.Now -> ClearShellResult.Clock(1.0e12)
            }
        }
        clear.dispatch(ClearSigningEvent.Cleared, ClearSigningEvent.serializer())
        assertEquals(ClearSigningView().surface, withTimeout(10_000) { clear.view.first { true } }.surface)

        val guard = host(ApprovalGuardCore().asBridge(), GuardView(), GuardView.serializer(), GuardOperation.serializer(), GuardShellResult.serializer()) { op ->
            when (op) {
                is GuardOperation.ReadTokenMetadata -> GuardShellResult.MetaResolved(null)
                is GuardOperation.ReadErc20Allowance -> GuardShellResult.AllowanceRead(null)
                is GuardOperation.ReadErc20Balance -> GuardShellResult.BalanceRead(null)
            }
        }
        guard.dispatch(GuardEvent.RevokeChosen, GuardEvent.serializer())
        withTimeout(10_000) { guard.view.first { true } }

        assertTrue("no core faults: $faults", faults.isEmpty())
    }

    @Test
    fun `the origin rule is the core's`() {
        assertEquals("https://app.uniswap.org", dappOriginOf("https://app.uniswap.org/swap?a=1#x"))
        assertEquals("http://127.0.0.1:8137", dappOriginOf("http://127.0.0.1:8137/?v=3"))
        assertEquals("https://example.com", dappOriginOf("HTTPS://user@Example.com:443/"))
        assertNull(dappOriginOf("about:blank"))
    }
}
