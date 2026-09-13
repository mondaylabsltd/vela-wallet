package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.feature.settings.core.CurrencyEvent
import app.getvela.wallet.feature.settings.core.CurrencyExecutor
import app.getvela.wallet.feature.settings.core.CurrencyOperation
import app.getvela.wallet.feature.settings.core.CurrencyShellResult
import app.getvela.wallet.feature.settings.core.CurrencyView
import java.util.Locale
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * The whole road, end to end, on the **real** `display_currency` machine.
 *
 * The host dylib is on `jna.library.path` (wired in `app/build.gradle.kts` for
 * spec 005's engine tests), so a JVM test can drive the same Rust that runs on
 * the phone. Everything between a tap and a stored preference is exercised
 * here: event encoding, the uniffi bridge, the effect loop, the executor, the
 * result encoding, and the view decode.
 *
 * What it deliberately does NOT prove is Android storage — that is
 * `CurrencyPersistenceTest`, which needs a device and a real process death.
 * What it does prove is that when the storage answers, the machine behaves.
 */
class CurrencyMachineTest {
    /**
     * Cancelled after every test.
     *
     * Each host keeps a driver alive for the life of its scope. A suite that
     * leaves a dozen of them running competes with itself for
     * `Dispatchers.Default`, and the symptom lands somewhere else entirely — a
     * different test timing out on a budget it had never come close to. A
     * leaked scope is not untidiness; it is a flake with somebody else's name
     * on it.
     */
    private val scopes = mutableListOf<CoroutineScope>()

    @After
    fun stopEveryMachine() {
        scopes.forEach { it.cancel() }
        scopes.clear()
    }


    private fun host(store: KeyValueStore, locale: Locale?): CoreHost<CurrencyView> {
        val executor = CurrencyExecutor(store, primaryLocale = { locale })
        return CoreHost(
            bridge = uniffi.vela_core_uniffi.DisplayCurrencyCore().asBridge(),
            scope = CoroutineScope(SupervisorJob() + Dispatchers.Default).also { scopes += it },
            initial = CurrencyView(code = "USD", rate = null, committed = false),
            serializer = CurrencyView.serializer(),
            perform = JsonShell.perform(
                CurrencyOperation.serializer(),
                CurrencyShellResult.serializer(),
                executor::perform,
            ),
            escapedFailure = JsonShell.escapedFailure(
                CurrencyOperation.serializer(),
                CurrencyShellResult.serializer(),
                fallback = CurrencyShellResult.StoredCode(null),
                answer = executor::neutralAnswer,
            ),
            onFault = { error -> throw AssertionError("shell fault: $error", error) },
        )
    }

    private fun CoreHost<CurrencyView>.settle(predicate: (CurrencyView) -> Boolean): CurrencyView =
        runBlocking { withTimeout(TIMEOUT_MS) { view.first(predicate) } }

    @Test
    fun aFirstLaunchInJapanStaysOnDollarsUntilRatesExist() {
        // Not the result the phase set out to produce, and the core is right.
        //
        // `display_currency.rs:12` — "the seed is persisted ONLY after a real
        // rate resolves, because a seeded currency rendering at the rate-1
        // fallback (₫78 instead of ₫2,000,000) is strictly worse than staying
        // on USD". Spec 040 has no way to price anything, so `resolve_rate`
        // answers null and the seed is declined by design.
        //
        // What this pins is the CONSEQUENCE: answering `read_device_currency`
        // honestly (research D8) buys a person in Tokyo nothing until spec 041
        // brings rates — and it buys them everything the moment it does,
        // without another line of Kotlin. A test asserting JPY here would be
        // asserting a rate this build cannot fetch.
        val store = FakeStore()
        val host = host(store, Locale.JAPAN)
        host.dispatch(CurrencyEvent.Refresh, CurrencyEvent.serializer())

        val view = host.settle { it.committed }
        assertEquals("USD", view.code)
        // And nothing was written: an unpriced seed must not be persisted, or
        // the next launch would read it back as a settled choice.
        assertNull(store.values[KeyValueStore.Keys.DISPLAY_CURRENCY])
    }

    @Test
    fun aStoredChoiceOutranksTheRegion() {
        // Both signals present and disagreeing. The stored one wins, and the
        // rule that says so is in Rust — this test's job is to prove the shell
        // hands the core both facts rather than deciding between them.
        val store = FakeStore(mapOf(KeyValueStore.Keys.DISPLAY_CURRENCY to "GBP"))
        val host = host(store, Locale.JAPAN)
        host.dispatch(CurrencyEvent.Refresh, CurrencyEvent.serializer())

        assertEquals("GBP", host.settle { it.committed }.code)
    }

    @Test
    fun aPickIsCommittedAndStored() {
        val store = FakeStore()
        val host = host(store, Locale.US)
        host.dispatch(CurrencyEvent.Refresh, CurrencyEvent.serializer())
        host.settle { it.committed }

        host.dispatch(CurrencyEvent.UserChose("EUR"), CurrencyEvent.serializer())
        val view = host.settle { it.code == "EUR" }

        assertEquals("EUR", view.code)
        assertEquals("EUR", store.values[KeyValueStore.Keys.DISPLAY_CURRENCY])
    }

    @Test
    fun aSecondMachineOverTheSameStoreAgrees() {
        // The in-process half of SC-001: a machine that never saw the pick
        // reads it back from where the first one put it. The other half — that
        // the bytes survive the process dying — needs a device.
        val store = FakeStore()
        host(store, Locale.US).let { first ->
            first.dispatch(CurrencyEvent.Refresh, CurrencyEvent.serializer())
            first.settle { it.committed }
            first.dispatch(CurrencyEvent.UserChose("KRW"), CurrencyEvent.serializer())
            first.settle { it.code == "KRW" }
        }

        val second = host(store, Locale.US)
        second.dispatch(CurrencyEvent.Refresh, CurrencyEvent.serializer())
        assertEquals("KRW", second.settle { it.committed }.code)
    }

    @Test
    fun aChosenCurrencyIsShownUnpriced() {
        // `resolve_rate` is fail-closed for the whole of 040, so a currency
        // that is not the dollar arrives with NO rate — which is exactly what
        // the shell must render, and the reason `CurrencyView.rate` is
        // nullable. USD is the one code with a rate here, and it is 1 because
        // USD/USD is 1, not because anything was fetched.
        val store = FakeStore(mapOf(KeyValueStore.Keys.DISPLAY_CURRENCY to "GBP"))
        val host = host(store, Locale.US)
        host.dispatch(CurrencyEvent.Refresh, CurrencyEvent.serializer())

        val view = host.settle { it.committed }
        assertEquals("GBP", view.code)
        assertNull("an unpriced currency must not arrive priced", view.rate)
    }

    private companion object {
        const val TIMEOUT_MS = 10_000L
    }
}
