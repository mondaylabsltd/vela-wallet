package app.getvela.wallet

import app.getvela.wallet.feature.wallet.core.TrackerWorker
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Spec 088 FR-005 (audit A18): below API 31 an expedited request runs as a
 * foreground service, which needs a `getForegroundInfo()` the tracker does not
 * have — so on Android 10 and 11 it must be a plain request. From 31 on it is
 * an expedited JOB, which needs no notification.
 */
class TrackerWorkRequestTest {
    @Test
    fun `android 10 and 11 get a plain request`() {
        for (sdk in 29..30) {
            assertFalse("API $sdk", TrackerWorker.expedites(sdk))
            assertFalse("API $sdk", TrackerWorker.request(sdk).workSpec.expedited)
        }
    }

    @Test
    fun `android 12 and later keep the expedited job`() {
        for (sdk in 31..36) {
            assertTrue("API $sdk", TrackerWorker.expedites(sdk))
            assertTrue("API $sdk", TrackerWorker.request(sdk).workSpec.expedited)
        }
    }
}
