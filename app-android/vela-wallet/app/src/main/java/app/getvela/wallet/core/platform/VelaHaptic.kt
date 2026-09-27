package app.getvela.wallet.core.platform

import android.os.Build
import android.view.HapticFeedbackConstants
import android.view.View
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalView
import app.getvela.wallet.core.diagnostics.VelaLog

/**
 * The five haptics this app performs — and the policy that says where
 * (spec 048, `docs/design-system/haptics.md`):
 *
 * - [Press]   a button under the finger (the founder's rule: press = deformation + haptic),
 *             and a tab that switches the destination (founder, 2026-09-26)
 * - [Detent]  a slider step crossed, a picker snapping, the signing slider's threshold
 * - [Select]  a selection that takes effect: a switch, a filter, a network / fee-token /
 *             account pick, a favourite, a copy
 * - [Success] / [Reject]  an outcome the core decided
 *
 * Never for scrolling, a row tap that navigates, re-tapping the tab in force, Back, a sheet
 * opening or closing, typing, animation, or a value the app changed itself.
 * One per gesture. Through [View.performHapticFeedback], so the system's
 * haptics switch is honoured, and every one leaves a log line the scripted
 * device pass can count.
 */
enum class VelaHaptic {
    Press,
    Detent,
    Select,
    Success,
    Reject,
    ;

    internal val constant: Int
        get() = when (this) {
            Press -> HapticFeedbackConstants.VIRTUAL_KEY
            Detent -> if (Build.VERSION.SDK_INT >= 34) HapticFeedbackConstants.SEGMENT_TICK else HapticFeedbackConstants.CLOCK_TICK
            Select -> if (Build.VERSION.SDK_INT >= 30) HapticFeedbackConstants.CONFIRM else HapticFeedbackConstants.KEYBOARD_TAP
            Success -> if (Build.VERSION.SDK_INT >= 30) HapticFeedbackConstants.CONFIRM else HapticFeedbackConstants.LONG_PRESS
            Reject -> if (Build.VERSION.SDK_INT >= 30) HapticFeedbackConstants.REJECT else HapticFeedbackConstants.LONG_PRESS
        }
}

fun View.velaHaptic(kind: VelaHaptic) {
    VelaLog.event("haptic", kind.name.lowercase())
    runCatching { performHapticFeedback(kind.constant) }
}

/** The haptic performer for a composable: `val haptic = rememberVelaHaptic(); haptic(VelaHaptic.Select)`. */
@Composable
fun rememberVelaHaptic(): (VelaHaptic) -> Unit {
    val view = LocalView.current
    return remember(view) { { kind -> view.velaHaptic(kind) } }
}
