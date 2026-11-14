package app.getvela.wallet.core.designsystem.components

import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.Easing
import app.getvela.wallet.core.designsystem.tokens.VelaLaunchScene

/**
 * The launch lockup at a moment: where the mark is and how opaque each letter
 * is, `ms` milliseconds into the authored [VelaLaunchScene.durationMs].
 *
 * Pure, so it is held to the shared samples
 * (`docs/design/onboarding/launch/launch-timeline.vectors.json`) on the JVM —
 * the same samples the web's rendering was compared with, pixel for pixel,
 * against the animation files this replaced. [VelaLaunchAnimation] draws exactly
 * what this says.
 */
object VelaLaunchTimeline {
    /**
     * The scene's CSS cubic-bezier, eased by Compose itself. (0.4, 0, 0.2, 1)
     * is `FastOutSlowInEasing`; built from the scene so a change there cannot
     * leave this behind.
     */
    val easing: Easing = VelaLaunchScene.easing.let { CubicBezierEasing(it[0], it[1], it[2], it[3]) }

    class Frame(val markX: Float, val markY: Float, val letterAlpha: FloatArray)

    /** Eased progress of a tween running from [startMs] to [endMs]: 0 before, 1 after. */
    fun progress(ms: Float, startMs: Float, endMs: Float): Float = when {
        ms <= startMs -> 0f
        ms >= endMs -> 1f
        else -> easing.transform((ms - startMs) / (endMs - startMs))
    }

    private fun markProgress(ms: Float): Float =
        progress(ms, VelaLaunchScene.markStartMs, VelaLaunchScene.markEndMs)

    fun markX(ms: Float): Float =
        lerp(VelaLaunchScene.markFromX, VelaLaunchScene.markToX, markProgress(ms))

    fun markY(ms: Float): Float =
        lerp(VelaLaunchScene.markFromY, VelaLaunchScene.markToY, markProgress(ms))

    fun letterAlpha(index: Int, ms: Float): Float {
        val letter = VelaLaunchScene.letters[index]
        return progress(ms, letter.startMs, letter.endMs)
    }

    /** The whole state at once — for tests; the draw pass asks per field and allocates nothing. */
    fun state(ms: Float): Frame = Frame(
        markX = markX(ms),
        markY = markY(ms),
        letterAlpha = FloatArray(VelaLaunchScene.letters.size) { letterAlpha(it, ms) },
    )

    private fun lerp(from: Float, to: Float, t: Float): Float = from + (to - from) * t
}
