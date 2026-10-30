package app.getvela.wallet.macrobenchmark

import androidx.benchmark.macro.BaselineProfileMode
import androidx.benchmark.macro.CompilationMode
import androidx.benchmark.macro.ExperimentalMetricApi
import androidx.benchmark.macro.MemoryUsageMetric
import androidx.benchmark.macro.StartupMode
import androidx.benchmark.macro.StartupTimingMetric
import androidx.benchmark.macro.TraceSectionMetric
import androidx.benchmark.macro.junit4.MacrobenchmarkRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import app.getvela.wallet.perf.Journeys
import app.getvela.wallet.perf.Journeys.awaitHome
import app.getvela.wallet.perf.Journeys.seedReturningPerson
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Journey 1: a returning person's cold start, to the cached home.
 *
 * TTID is the first frame (the launch animation's); TTFD is `reportFullyDrawn`,
 * which the home sends once it stands on rows (`ReportDrawnWhen` in
 * VelaNavHost). `vela.launchAnimation` is how long the spec 012 animation
 * keeps the home covered after that — the time a person who does not tap
 * waits to SEE the home.
 *
 * Two compilations: `shipped` is what a Play install runs (the baseline
 * profile, if the APK carries one); `noProfile` is the floor without it.
 */
@OptIn(ExperimentalMetricApi::class)
@RunWith(AndroidJUnit4::class)
class StartupBenchmark {
    @get:Rule
    val rule = MacrobenchmarkRule()

    @Test
    fun coldStartHomeShipped() = coldStartHome(CompilationMode.Partial(BaselineProfileMode.UseIfAvailable))

    @Test
    fun coldStartHomeNoProfile() = coldStartHome(CompilationMode.None())

    private fun coldStartHome(compilation: CompilationMode) = rule.measureRepeated(
        packageName = Journeys.PACKAGE,
        metrics = listOf(
            StartupTimingMetric(),
            TraceSectionMetric("vela.launchAnimation", TraceSectionMetric.Mode.First),
            MemoryUsageMetric(MemoryUsageMetric.Mode.Last),
        ),
        compilationMode = compilation,
        startupMode = StartupMode.COLD,
        iterations = ITERATIONS,
        setupBlock = {
            // Once per iteration: cheap, and every launch reads the same store.
            seedReturningPerson()
        },
    ) {
        startActivityAndWait()
        awaitHome()
    }

    private companion object {
        const val ITERATIONS = 10
    }
}
