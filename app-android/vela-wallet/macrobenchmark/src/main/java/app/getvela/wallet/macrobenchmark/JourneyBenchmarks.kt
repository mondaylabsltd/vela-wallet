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
            // The launch's own tail (the home's reads, the names, the first
            // GC and heap trim) is the startup benchmark's, not the journey's.
            // The 2.7 s launch animation used to hide part of it; since it
            // plays once a week (core-perf-1) the home is up in ~0.4 s, and a
            // journey started there measured the tail as its own frames and
            // memory (history scroll: heap 30 -> 57 MB, jank 17 -> 44 % on
            // the same build). Waited out, the comparison is the journey's.
            Thread.sleep(LAUNCH_TAIL_MS)
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

        /**
         * Long enough for the launch's reads to land and the heap to be
         * trimmed on an emulator (2.7 s, the old animation's length, was not:
         * nat-android-6 measured both), so runs before and after core-perf-1 compare.
         */
        const val LAUNCH_TAIL_MS = 10_000L
    }
}
