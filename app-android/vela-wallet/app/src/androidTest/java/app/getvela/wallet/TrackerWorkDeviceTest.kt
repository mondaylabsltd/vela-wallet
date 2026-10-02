package app.getvela.wallet

import android.os.SystemClock
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.work.WorkInfo
import androidx.work.WorkManager
import app.getvela.wallet.feature.wallet.core.TrackerWorker
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Spec 088 FR-005 on a real platform: the background clock is enqueued exactly
 * the way the wallet enqueues it when the app leaves with a send in flight, and
 * it must run to SUCCEEDED — on Android 10/11 too, where an expedited request
 * without `getForegroundInfo()` used to die. Run it on the API 30 emulator:
 *
 *   adb -s emulator-5554 shell am instrument -w \
 *     -e class app.getvela.wallet.TrackerWorkDeviceTest \
 *     app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 */
@RunWith(AndroidJUnit4::class)
class TrackerWorkDeviceTest {
    @Test
    fun theBackgroundClockRunsToTheEnd() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val work = WorkManager.getInstance(context)
        TrackerWorker.enqueue(context)
        val deadline = SystemClock.elapsedRealtime() + 90_000
        var state: WorkInfo.State? = null
        while (SystemClock.elapsedRealtime() < deadline) {
            state = work.getWorkInfosForUniqueWork(TrackerWorker.WORK_NAME).get().firstOrNull()?.state
            if (state?.isFinished == true) break
            Thread.sleep(250)
        }
        assertEquals(WorkInfo.State.SUCCEEDED, state)
    }
}
