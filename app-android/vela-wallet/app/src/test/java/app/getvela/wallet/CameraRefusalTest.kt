package app.getvela.wallet

import app.getvela.wallet.feature.scan.CameraRefusal
import app.getvela.wallet.feature.scan.GrantAction
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * NATIVE-LAUNCH-CHECKLIST C: a camera refused for good is not asked for again
 * (the system would show nothing) — the scanner's button opens Settings.
 */
class CameraRefusalTest {

    @Test
    fun `before any ask, and after one refusal, the dialog is shown again`() {
        assertEquals(GrantAction.Ask, CameraRefusal.grantAction(askedBefore = false, showRationale = false))
        assertEquals(GrantAction.Ask, CameraRefusal.grantAction(askedBefore = true, showRationale = true))
    }

    @Test
    fun `a refusal the system will not ask about again opens Settings`() {
        assertEquals(GrantAction.OpenSettings, CameraRefusal.grantAction(askedBefore = true, showRationale = false))
    }
}
