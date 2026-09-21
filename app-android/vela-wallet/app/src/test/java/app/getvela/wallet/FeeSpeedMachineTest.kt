package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.feature.send.core.FeeAssetView
import app.getvela.wallet.feature.send.core.FeeEstimateView
import app.getvela.wallet.feature.send.core.FeeSpeedEvent
import app.getvela.wallet.feature.send.core.FeeSpeedView
import app.getvela.wallet.feature.send.core.FeeTier
import app.getvela.wallet.feature.send.core.TierPreviewQuote
import app.getvela.wallet.feature.send.core.TierQuote
import app.getvela.wallet.feature.settings.core.FeeTierExecutor
import app.getvela.wallet.feature.settings.core.FeeTierPrefEvent
import app.getvela.wallet.feature.settings.core.FeeTierPrefOperation
import app.getvela.wallet.feature.settings.core.FeeTierPrefShellResult
import app.getvela.wallet.feature.settings.core.FeeTierPrefView
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Spec 069 on Android: the two machines behind the speed control, driven
 * through the real core over uniffi — the stored default (`fee_tier_pref`)
 * and the control's decisions (`fee_speed`). The rules themselves are pinned
 * in `app_fee_speed.rs`; what these pin is that Android's wire carries them
 * whole, and that its executor stores the preference where every client does.
 */
class FeeSpeedMachineTest {
    private val scopes = mutableListOf<CoroutineScope>()

    @After
    fun stopEveryMachine() {
        scopes.forEach { it.cancel() }
        scopes.clear()
    }

    private fun scope() = CoroutineScope(SupervisorJob() + Dispatchers.Default).also { scopes += it }

    private fun speedHost(): CoreHost<FeeSpeedView> = CoreHost(
        bridge = uniffi.vela_core_uniffi.FeeSpeedCore().asBridge(),
        scope = scope(),
        initial = FeeSpeedView(),
        serializer = FeeSpeedView.serializer(),
        perform = { operation -> error("fee_speed asked for $operation") },
        escapedFailure = { operation, error -> throw AssertionError("fee_speed $operation", error) },
        onFault = { error -> throw AssertionError("shell fault: $error", error) },
    )

    private fun prefHost(store: KeyValueStore): CoreHost<FeeTierPrefView> {
        val executor = FeeTierExecutor(store)
        return CoreHost(
            bridge = uniffi.vela_core_uniffi.FeeTierPrefCore().asBridge(),
            scope = scope(),
            initial = FeeTierPrefView(),
            serializer = FeeTierPrefView.serializer(),
            perform = JsonShell.perform(FeeTierPrefOperation.serializer(), FeeTierPrefShellResult.serializer(), executor::perform),
            escapedFailure = JsonShell.escapedFailure(
                FeeTierPrefOperation.serializer(),
                FeeTierPrefShellResult.serializer(),
                fallback = FeeTierPrefShellResult.StoredTier(null),
                answer = executor::neutralAnswer,
            ),
            onFault = { error -> throw AssertionError("shell fault: $error", error) },
        )
    }

    private fun <V : Any> CoreHost<V>.settle(predicate: (V) -> Boolean): V =
        runBlocking { withTimeout(TIMEOUT_MS) { view.first(predicate) } }

    /** Optimism-shaped: every tier the same fee, each its own gas bid. */
    private fun quote(tier: FeeTier, low: String, high: String) = FeeEstimateView(
        chain_id = 10,
        total_wei = "10000",
        max_fee_per_gas = high,
        network_fee_per_gas = "0",
        relayer_fee_per_gas = "0",
        bundler_gas_price = "0",
        in_band_gas_basis = "0",
        effective_gas_price = low,
        max_gas_price = high,
        total_gas = "0",
        deployed = true,
        tier = tier,
        quoted = true,
        fee_asset = FeeAssetView.Native,
        fee_recipient = "0xfee",
    )

    @Test
    fun `a slower default goes Fast where Fast costs no more, and says why`() {
        val host = speedHost()
        host.start()
        host.dispatch(FeeSpeedEvent.Configure(FeeTier.Slow, "comma_dot"), FeeSpeedEvent.serializer())
        host.dispatch(FeeSpeedEvent.StageChanged(on_form = true), FeeSpeedEvent.serializer())
        // The partner the core wants priced beside the slower default.
        val asked = host.settle { it.preferred == FeeTier.Slow && it.previews.isNotEmpty() }
        assertEquals(listOf(FeeTier.Fast), asked.previews)
        host.dispatch(
            FeeSpeedEvent.QuotesChanged(
                chain_id = 10,
                in_force = TierQuote(busy = false, fee = quote(FeeTier.Slow, "1937", "4500")),
                previews = listOf(TierPreviewQuote(FeeTier.Fast, busy = false, fee = quote(FeeTier.Fast, "3244", "9000"))),
            ),
            FeeSpeedEvent.serializer(),
        )
        val view = host.settle { it.tier == FeeTier.Fast }
        assertTrue(view.free)
        assertTrue(view.free_note)
        assertFalse(view.picked)
    }

    @Test
    fun `open, every option carries its own fee and its gas bid in the person's preset`() {
        val host = speedHost()
        host.start()
        host.dispatch(FeeSpeedEvent.Configure(FeeTier.Fast, "dot_comma"), FeeSpeedEvent.serializer())
        host.dispatch(FeeSpeedEvent.Toggle, FeeSpeedEvent.serializer())
        host.dispatch(
            FeeSpeedEvent.QuotesChanged(
                chain_id = 10,
                in_force = TierQuote(fee = quote(FeeTier.Fast, "3244", "9000")),
                previews = listOf(
                    TierPreviewQuote(FeeTier.Standard, fee = quote(FeeTier.Standard, "2377", "6000")),
                    TierPreviewQuote(FeeTier.Slow, fee = quote(FeeTier.Slow, "1937", "4500")),
                ),
            ),
            FeeSpeedEvent.serializer(),
        )
        val view = host.settle { it.open && it.options.all { option -> option.gas_price != null } }
        assertEquals(listOf(FeeTier.Fast, FeeTier.Standard, FeeTier.Slow), view.options.map { it.tier })
        assertEquals(listOf("3.244 ~ 9.000 wei", "2.377 ~ 6.000 wei", "1.937 ~ 4.500 wei"), view.options.map { it.gas_price })
        assertEquals(FeeTier.Standard, view.options[1].fee?.tier)
        assertFalse(view.single)
    }

    @Test
    fun `a pick is one-shot and never offers the dead tier`() {
        val host = speedHost()
        host.start()
        host.dispatch(FeeSpeedEvent.Pick(FeeTier.Rapid), FeeSpeedEvent.serializer())
        host.dispatch(FeeSpeedEvent.Pick(FeeTier.Slow), FeeSpeedEvent.serializer())
        val picked = host.settle { it.picked }
        assertEquals(FeeTier.Slow, picked.tier)
        assertTrue(picked.options.none { it.tier == FeeTier.Rapid })
        host.dispatch(FeeSpeedEvent.Reset, FeeSpeedEvent.serializer())
        assertEquals(FeeTier.Fast, host.settle { !it.picked }.tier)
    }

    @Test
    fun `the default speed persists under vela feeTier and reads back`() {
        val store = FakeStore()
        val first = prefHost(store)
        first.start()
        first.dispatch(FeeTierPrefEvent.Refresh, FeeTierPrefEvent.serializer())
        assertEquals(FeeTier.Fast, first.settle { !it.committed }.tier)
        first.dispatch(FeeTierPrefEvent.UserChose(FeeTier.Standard), FeeTierPrefEvent.serializer())
        first.settle { it.committed && it.tier == FeeTier.Standard }
        runBlocking { withTimeout(TIMEOUT_MS) { while (store.values[KeyValueStore.Keys.FEE_TIER] == null) kotlinx.coroutines.delay(10) } }
        assertEquals("standard", store.values[KeyValueStore.Keys.FEE_TIER])

        val next = prefHost(store)
        next.start()
        next.dispatch(FeeTierPrefEvent.Refresh, FeeTierPrefEvent.serializer())
        assertEquals(FeeTier.Standard, next.settle { it.committed }.tier)
    }

    @Test
    fun `a stored name this build does not offer reads as the factory Fast`() {
        val store = FakeStore(mapOf(KeyValueStore.Keys.FEE_TIER to "rapid"))
        val host = prefHost(store)
        host.start()
        host.dispatch(FeeTierPrefEvent.Refresh, FeeTierPrefEvent.serializer())
        runBlocking { kotlinx.coroutines.delay(200) }
        val view = host.view.value
        assertEquals(FeeTier.Fast, view.tier)
        assertFalse(view.committed)
        assertEquals("rapid", store.values[KeyValueStore.Keys.FEE_TIER])
    }

    private companion object {
        const val TIMEOUT_MS = 10_000L
    }
}
