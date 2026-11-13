package app.getvela.wallet.core.designsystem.tokens

import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.SpringSpec
import androidx.compose.animation.core.spring

/** core.motion — durations in ms, press scales, spring parameters. */
object VelaMotion {
    const val durationFast: Int = 150
    const val durationNormal: Int = 250
    const val durationSlow: Int = 400
    const val sheetIn: Int = 220
    const val sheetOut: Int = 180
    const val sheetDrag: Int = 200
    const val entranceFade: Int = 300
    const val entranceFadeUp: Int = 400
    const val entranceStagger: Int = 50

    const val pressScaleButton: Float = 0.97f
    const val pressScaleRow: Float = 0.98f
    const val pressScaleFab: Float = 0.92f

    /**
     * Spec 014: how long the address strip shows its transient "copied"
     * confirmation before reverting (visual feedback only — not a retry timer;
     * not in the DTCG export).
     */
    const val copiedFeedbackHold: Int = 1500

    // motion.spring / springGentle — RN Reanimated parameters (damping/stiffness/mass),
    // kept verbatim for the drift test. Compose springs below are the perceptual
    // translation (research D12): Reanimated and Compose units are not compatible.
    const val springDamping: Float = 15f
    const val springStiffness: Float = 150f
    const val springMass: Float = 0.8f
    const val springGentleDamping: Float = 20f
    const val springGentleStiffness: Float = 120f
    const val springGentleMass: Float = 1f

    /** Interactive press feedback: fast settle, no oscillation. */
    val pressSpring: SpringSpec<Float> = spring(dampingRatio = 0.75f, stiffness = 600f)

    // -- Screen motion (native parity Phase 3; not in the DTCG export) ----------
    // Material 3's in-app predictive back and shared-axis entrance, in the
    // springs above: settled, no overshoot — a wallet's screens do not bounce.

    /** The screen in front at a fully held back gesture (M3: 90 %). */
    const val predictiveBackScale: Float = 0.9f

    /** How far an entering level fades from (1 − this → 1). */
    const val entranceFadeDepth: Float = 0.6f

    /** A level settling in: critically damped, medium-low stiffness (~300 ms). */
    val entranceSpring: SpringSpec<Float> = spring(
        dampingRatio = Spring.DampingRatioNoBouncy,
        stiffness = Spring.StiffnessMediumLow,
    )

    /** The back layer following the finger, and letting go: quick, no overshoot. */
    val backSettleSpring: SpringSpec<Float> = spring(
        dampingRatio = Spring.DampingRatioNoBouncy,
        stiffness = Spring.StiffnessMedium,
    )

    /** Layout/gentle transitions. */
    val gentleSpring: SpringSpec<Float> = spring(
        dampingRatio = Spring.DampingRatioNoBouncy,
        stiffness = Spring.StiffnessMediumLow,
    )
}
