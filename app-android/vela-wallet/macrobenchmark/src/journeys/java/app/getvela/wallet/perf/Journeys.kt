package app.getvela.wallet.perf

import androidx.benchmark.macro.MacrobenchmarkScope
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.BySelector
import androidx.test.uiautomator.Direction
import androidx.test.uiautomator.UiObject2
import androidx.test.uiautomator.Until
import java.io.File

/**
 * The Phase 4 journeys (macrobenchmark/README.md), one definition shared by
 * the baseline-profile generator and the macrobenchmarks:
 *
 * 1. cold start to the cached home (a returning person's launch);
 * 2. the 200-row activity history, scrolled;
 * 3. Send opened and walked to its Confirm;
 * 4. the signing sheet opened.
 *
 * The app under test is `benchmarkRelease`: release code, plus the
 * `src/benchmark` doors that put a fixture wallet in the store
 * (`BenchSeedActivity`) and ask a signing sheet open (`BenchSignActivity`).
 * Everything is found by test tag (resource id), so any language works.
 * A step that does not arrive leaves a screenshot and the window hierarchy
 * beside the results before it fails.
 */
object Journeys {
    /**
     * The app under test: `app.getvela.wallet`, unless the run names another
     * (`-e vela.targetPackage app.getvela.wallet.devicetest`) — a phone that
     * already holds the store app is measured on the `-PvelaDeviceTest` build
     * installed beside it, never over it (scripts/run-macrobenchmarks.sh).
     */
    val PACKAGE: String =
        InstrumentationRegistry.getArguments().getString("vela.targetPackage")?.takeIf { it.isNotBlank() }
            ?: "app.getvela.wallet"
    private val SEED = "$PACKAGE/app.getvela.wallet.bench.BenchSeedActivity"
    private val SIGN = "$PACKAGE/app.getvela.wallet.bench.BenchSignActivity"
    const val ROWS = 200
    private const val WAIT_MS = 30_000L

    /** Puts the fixture wallet in the store and kills the process, so what follows is a cold start on it. */
    fun MacrobenchmarkScope.seedReturningPerson(rows: Int = ROWS) {
        device.executeShellCommand("am start -W -n $SEED --ei vela.bench.rows $rows")
        expect(By.text("vela-bench-seeded"), "the fixture wallet was not seeded")
        killProcess()
    }

    /** The home is on screen, standing on rows (cached or live), and the launch animation is gone. */
    fun MacrobenchmarkScope.awaitHome() {
        expect(By.res("wallet-home"), "the home never appeared")
        // FR-016: a tap cuts the animation short — but a journey must not depend
        // on that; it waits the animation out like a person who does not tap.
        device.wait(Until.gone(By.res("launch-animation")), WAIT_MS)
    }

    fun MacrobenchmarkScope.openHistory(): UiObject2 {
        expect(By.res("home-activity-all"), "the home's Activity · All is not there").click()
        return expect(By.res("activity-list"), "the history never opened")
    }

    /** Down to the end of 200 rows and back, by flings — the frames FrameTimingMetric reads. */
    fun MacrobenchmarkScope.scrollHistory(list: UiObject2) {
        // Clear of the system gesture areas at the edges.
        list.setGestureMargin(device.displayWidth / 5)
        repeat(FLINGS) { list.fling(Direction.DOWN); device.waitForIdle() }
        repeat(FLINGS) { list.fling(Direction.UP); device.waitForIdle() }
    }

    fun MacrobenchmarkScope.back() {
        device.pressBack()
        device.waitForIdle()
    }

    /** Asks a `personal_sign` sheet open and waits for its confirm. */
    fun MacrobenchmarkScope.openSigningSheet() {
        device.executeShellCommand("am start -n $SIGN")
        expect(By.res("signing-confirm"), "the signing sheet never opened")
    }

    fun MacrobenchmarkScope.closeSigningSheet() {
        // The sheet's own Close (BACK does not dismiss a signing request).
        device.findObject(By.res("signing-close"))?.click() ?: device.pressBack()
        device.wait(Until.gone(By.res("signing-confirm")), WAIT_MS)
    }

    /** Home → Send → a token → recipient and amount → Confirm. */
    fun MacrobenchmarkScope.openSendToConfirm() {
        expect(By.res("home-send"), "the home's Send is not there").click()
        expect(By.res("send-token:$TOKEN"), "Send's token list never offered $TOKEN").click()
        expect(By.res("send-amount"), "Send's form never appeared").text = AMOUNT
        expect(By.res("send-recipient"), "Send's recipient field is not there").text = RECIPIENT
        device.waitForIdle()
        // The keyboard may cover Continue.
        if (!device.hasObject(By.res("send-continue"))) device.pressBack()
        // Confirm needs the live relay's quote. When the relay refuses for the
        // moment ("gas price far above the network's", a quote that would not
        // build) the app says so in a dialog and retries by itself; the journey
        // closes it and asks again, a few times, before it gives up — a relay's
        // bad minute must not read as the app's regression.
        repeat(CONFIRM_ATTEMPTS) { attempt ->
            expect(By.res("send-continue").enabled(true), "Continue never enabled").click()
            if (device.wait(Until.hasObject(By.res("send-confirm")), WAIT_MS)) return
            if (attempt < CONFIRM_ATTEMPTS - 1) {
                device.pressBack() // the refusal dialog
                device.wait(Until.hasObject(By.res("send-continue")), WAIT_MS)
                Thread.sleep(RELAY_RETRY_MS)
            }
        }
        expect(By.res("send-confirm"), "Send's Confirm never appeared (the relay never quoted)")
    }

    private const val CONFIRM_ATTEMPTS = 4
    private const val RELAY_RETRY_MS = 5_000L

    /** Waits for [selector]; on a timeout leaves a screenshot and the hierarchy, then fails with [what]. */
    private fun MacrobenchmarkScope.expect(selector: BySelector, what: String): UiObject2 {
        device.wait(Until.findObject(selector), WAIT_MS)?.let { return it }
        val dir = InstrumentationRegistry.getInstrumentation().context.externalMediaDirs.firstOrNull()
        if (dir != null) {
            val stamp = System.currentTimeMillis()
            runCatching { device.takeScreenshot(File(dir, "journey-failure-$stamp.png")) }
            runCatching { File(dir, "journey-failure-$stamp.xml").outputStream().use { device.dumpWindowHierarchy(it) } }
        }
        throw IllegalStateException(what)
    }

    /** The fixture Safe's one real holding (Gnosis): Send can quote it for real. */
    private const val TOKEN = "xDAI"

    /** Dust, to a public address with no checksum to get wrong (all lower case). */
    private const val AMOUNT = "0.001"
    private const val RECIPIENT = "0x1f9840a85d5af5bf1d1762f925bdaddc4201f984"

    private const val FLINGS = 6
}
