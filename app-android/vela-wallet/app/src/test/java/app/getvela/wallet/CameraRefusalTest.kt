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

    @Test
    fun `r14-10 the button says Open Settings once it opens Settings, Grant Permission while the system still asks`() {
        assertEquals("Open Settings", CameraRefusal.label(GrantAction.OpenSettings, grant = "Grant Permission", openSettings = "Open Settings"))
        assertEquals("Grant Permission", CameraRefusal.label(GrantAction.Ask, grant = "Grant Permission", openSettings = "Open Settings"))
    }

    @Test
    fun `r14-10 the label follows the same rule as the action`() {
        for (asked in listOf(false, true)) for (rationale in listOf(false, true)) {
            val action = CameraRefusal.grantAction(askedBefore = asked, showRationale = rationale)
            val label = CameraRefusal.label(action, grant = "grant", openSettings = "settings")
            assertEquals("asked=$asked rationale=$rationale", if (action == GrantAction.OpenSettings) "settings" else "grant", label)
        }
    }
}
