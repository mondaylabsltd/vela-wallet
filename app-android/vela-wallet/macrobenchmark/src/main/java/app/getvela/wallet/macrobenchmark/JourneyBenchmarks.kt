package app.getvela.wallet.macrobenchmark

import androidx.benchmark.macro.BaselineProfileMode
import androidx.benchmark.macro.CompilationMode
import androidx.benchmark.macro.ExperimentalMetricApi
import androidx.benchmark.macro.FrameTimingMetric
import androidx.benchmark.macro.MemoryUsageMetric
import androidx.benchmark.macro.junit4.MacrobenchmarkRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import app.getvela.wallet.perf.Journeys
import app.getvela.wallet.perf.Journeys.awaitHome
import app.getvela.wallet.perf.Journeys.back
import app.getvela.wallet.perf.Journeys.closeSigningSheet
import app.getvela.wallet.perf.Journeys.openHistory
import app.getvela.wallet.perf.Journeys.openSendToConfirm
import app.getvela.wallet.perf.Journeys.openSigningSheet
import app.getvela.wallet.perf.Journeys.scrollHistory
import app.getvela.wallet.perf.Journeys.seedReturningPerson
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Journeys 2–4, frame timing (jank) and memory, on the shipped compilation.
 * Each iteration starts from a cold launch onto the cached home (not measured)
 * and measures only the journey itself.
 */
@OptIn(ExperimentalMetricApi::class)
@RunWith(AndroidJUnit4::class)
class JourneyBenchmarks {
    @get:Rule
    val rule = MacrobenchmarkRule()

    private val shipped = CompilationMode.Partial(BaselineProfileMode.UseIfAvailable)

    private fun journey(name: String, block: androidx.benchmark.macro.MacrobenchmarkScope.() -> Unit) = rule.measureRepeated(
        packageName = Journeys.PACKAGE,
        metrics = listOf(FrameTimingMetric(), MemoryUsageMetric(MemoryUsageMetric.Mode.Max)),
        compilationMode = shipped,
        // Not COLD: that kills the process between setup and measure. The setup
        // makes its own cold start (the seed kills the process first).
        startupMode = null,
        iterations = ITERATIONS,
        setupBlock = {
            seedReturningPerson()
            startActivityAndWait()
            awaitHome()
        },
        measureBlock = block,
    )

    /** Journey 2: open the 200-row history and fling through it, down and back. */
    @Test
    fun activityHistoryScroll() = journey("history") {
        val list = openHistory()
        scrollHistory(list)
        back()
    }

    /** Journey 3: Send, a token, a recipient and an amount, to Confirm. */
    @Test
    fun sendToConfirm() = journey("send") {
        openSendToConfirm()
    }

    /** Journey 4: a page's `personal_sign` opens the signing sheet. */
    @Test
    fun signingSheetOpen() = journey("sign") {
        openSigningSheet()
        closeSigningSheet()
    }

    private companion object {
        const val ITERATIONS = 5
    }
}
