package app.getvela.wallet.baselineprofile

import androidx.benchmark.macro.junit4.BaselineProfileRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
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
 * The baseline profile: what a returning person's launch and the four
 * Phase 4 journeys run, AOT-compiled at install. `includeInStartupProfile`
 * also feeds the startup profile (DEX layout) for the launch itself.
 *
 *   ./gradlew :app:generateBaselineProfile -PvelaSkipRustBuild
 */
@LargeTest
@RunWith(AndroidJUnit4::class)
class BaselineProfileGenerator {
    @get:Rule
    val rule = BaselineProfileRule()

    @Test
    fun startup() = rule.collect(
        packageName = Journeys.PACKAGE,
        includeInStartupProfile = true,
    ) {
        seedReturningPerson()
        pressHome()
        startActivityAndWait()
        awaitHome()
    }

    @Test
    fun journeys() = rule.collect(
        packageName = Journeys.PACKAGE,
        includeInStartupProfile = false,
    ) {
        seedReturningPerson()
        pressHome()
        startActivityAndWait()
        awaitHome()
        scrollHistory(openHistory())
        back()
        openSigningSheet()
        closeSigningSheet()
        openSendToConfirm()
    }
}
