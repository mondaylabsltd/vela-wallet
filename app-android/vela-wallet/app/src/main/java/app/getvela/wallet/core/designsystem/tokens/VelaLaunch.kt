package app.getvela.wallet.core.designsystem.tokens

import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/**
 * Launch-animation constants (spec 012): the transition around the scene.
 *
 * The scene itself — canvases, paths, colours, its own timings — is
 * [VelaLaunchScene], generated from `docs/design/onboarding/launch/launch-scene.json`;
 * what is here either comes from it or is the hand-off the four apps share
 * (hold, dissolve, ceiling, fit rule). `LaunchGeometryTest` asserts research
 * D1's table so a transcription slip surfaces here rather than on a user's screen.
 */
object VelaLaunch {
    /** Authored length of the animation, the scene's: the play the hold follows. */
    val durationMs: Int = VelaLaunchScene.durationMs.toInt()

    /**
     * Hold on the finished lockup before the hand-off, so the brand registers
     * instead of flashing past (FR-012a). Skippable by input; bypassed under
     * reduce-motion. Tried at 2000 ms and cut to 400 on seeing it.
     */
    const val holdMs: Long = 400

    /**
     * Cross-dissolve into Welcome — [VelaMotion.durationSlow].
     * [VelaMotion.sheetOut] (180 ms) was the first choice and reads as a cut at
     * full-screen scale rather than a dissolve.
     */
    const val exitCrossfadeMs: Int = VelaMotion.durationSlow

    /**
     * FR-015: measured from the first presented frame, not from construction.
     * Nominal is 1700 play + 400 hold + 400 dissolve = 2500; the rest is slack.
     */
    const val hardCeilingMs: Long = 3000

    /**
     * Form-factor threshold. Android is `screenOrientation="portrait"`-locked and
     * phones are narrower than this, so a phone always resolves to the phone
     * canvas; a ≥ 768 dp tablet picks up the large-screen one with no
     * special-casing.
     *
     * Deliberately NOT a layout breakpoint reused from elsewhere — this governs
     * which animation is authored for the screen, not how a screen lays out.
     */
    val largeScreenMinWidth: Dp = 768.dp

    /**
     * Core canvases (research D0), the scene's: the cropped framings that are
     * drawn. The full-bleed widths exist only to pin the box ratios below.
     */
    const val phoneCanvasW: Float = VelaLaunchScene.canvasW
    const val phoneCanvasH: Float = VelaLaunchScene.canvasH
    const val largeCanvasW: Float = VelaLaunchScene.largeCanvasW
    const val largeCanvasH: Float = VelaLaunchScene.largeCanvasH

    /**
     * Box width as a fraction of viewport width: the core canvas divided by the
     * full-bleed canvas it was cropped from. NOT a judgement call — at 390 dp the
     * phone lockup lands at exactly the authored 80.7 % of screen width.
     */
    const val phoneBoxWidthRatio: Float = VelaLaunchScene.canvasW / VelaLaunchScene.phoneFullBleedW
    const val largeBoxWidthRatio: Float = VelaLaunchScene.largeCanvasW / VelaLaunchScene.largeFullBleedW

    /** The shared predicate, in device-independent units. */
    fun isLargeScreen(widthDp: Dp, heightDp: Dp): Boolean =
        widthDp >= heightDp || widthDp >= largeScreenMinWidth

    /**
     * Box size for a viewport, per the shared fit rule. Centred by the caller;
     * nothing is clipped or clamped, because the canvas is cropped to the
     * motion — the box *is* the canvas, scaled to fit it.
     */
    fun boxSize(viewportWidth: Dp, largeScreen: Boolean): Pair<Dp, Dp> {
        val ratio = if (largeScreen) largeBoxWidthRatio else phoneBoxWidthRatio
        val canvasW = if (largeScreen) largeCanvasW else phoneCanvasW
        val canvasH = if (largeScreen) largeCanvasH else phoneCanvasH
        val w = viewportWidth * ratio
        return w to (w * (canvasH / canvasW))
    }
}
