package app.getvela.wallet

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.data.VelaStore
import app.getvela.wallet.feature.settings.core.SettingsController
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith

/**
 * SC-001, on a real device and across a real process boundary.
 *
 * The two halves are **meant to be run as separate `am instrument`
 * invocations**, which is what makes this evidence rather than a round-trip
 * through a warm object graph: each invocation is a new process, so the second
 * half reads what survived the first one dying.
 *
 * ```bash
 * P=app.getvela.wallet/androidx.test.runner.AndroidJUnitRunner
 * T=app.getvela.wallet.CurrencyPersistenceTest
 * adb shell am instrument -w -e class "$T#aChoiceIsMade" "$P"
 * adb shell am force-stop app.getvela.wallet
 * adb shell am instrument -w -e class "$T#theChoiceSurvivedTheProcess" "$P"
 * ```
 *
 * Run together in one invocation they still pass, and prove less: the second
 * half would then be reading a DataStore the first half left in memory. The
 * force-stop between them is the assertion.
 */
@RunWith(AndroidJUnit4::class)
class CurrencyPersistenceTest {

    private val context get() = InstrumentationRegistry.getInstrumentation().targetContext

    private fun controller() = SettingsController(
        context = context,
        scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate),
    )

    @Test
    fun aChoiceIsMade() = runBlocking {
        val store = VelaStore(context)
        store.remove(KeyValueStore.Keys.DISPLAY_CURRENCY)

        val settings = controller()
        settings.refreshCurrency()
        settings.chooseCurrency(CHOICE)

        // The machine has committed when its view says so — the write is one
        // of the effects it runs on the way there.
        withTimeout(TIMEOUT_MS) {
            settings.currency.first { it.code == CHOICE && it.committed }
        }

        assertEquals(
            "the choice must reach the shared key the other clients read",
            CHOICE,
            store.read(KeyValueStore.Keys.DISPLAY_CURRENCY),
        )
    }

    @Test
    fun theChoiceSurvivedTheProcess() = runBlocking {
        val settings = controller()
        // A fresh machine in a fresh process, told only to look.
        settings.refreshCurrency()

        val view = withTimeout(TIMEOUT_MS) { settings.currency.first { it.committed } }

        assertEquals("the currency chosen before the process died", CHOICE, view.code)
        // Still unpriced: pricing needs the network layer spec 041 brings, and
        // `null` is not `1` — the core must not have invented a rate.
        assertEquals(null, view.rate)
    }

    private companion object {
        /**
         * Not USD, deliberately. USD is the core's placeholder, so a bug that
         * lost the preference entirely would still read back "USD" and pass.
         */
        const val CHOICE = "JPY"
        const val TIMEOUT_MS = 10_000L
    }
}
