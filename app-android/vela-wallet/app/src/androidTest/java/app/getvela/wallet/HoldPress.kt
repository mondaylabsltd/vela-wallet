package app.getvela.wallet

import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performSemanticsAction

/**
 * R22: a sign / confirm button as a person presses it. A plain button is a
 * tap. A button the core holds (`hold_ms > 0`, "… anyway" or danger — it says
 * "Hold to confirm" as its state) is pressed through its accessible path, as
 * TalkBack would: the first activation arms it, the second confirms. A test
 * that tapped it once would have done nothing — which is the point of it.
 */
fun SemanticsNodeInteraction.confirmPress() {
    if (!isHeld()) {
        performClick()
        return
    }
    performSemanticsAction(SemanticsActions.OnClick)
    performSemanticsAction(SemanticsActions.OnClick)
}

/** R22: the first activation of a held button only — it arms and says "Press again to confirm". */
fun SemanticsNodeInteraction.armHeld() {
    performSemanticsAction(SemanticsActions.OnClick)
}

/** Whether this button is held by the core's friction (it carries the hold's state). */
fun SemanticsNodeInteraction.isHeld(): Boolean =
    fetchSemanticsNode().config.getOrNull(SemanticsProperties.StateDescription) != null
