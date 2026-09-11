package app.getvela.wallet.core.designsystem.tokens

import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/**
 * Launch-animation constants (spec 012).
 *
 * Every value here is shared verbatim with the iOS, desktop and web apps — see
 * `specs/012-launch-animation-lottie/data-model.md` §4. Repeating them in four
 * languages is deliberate; `LaunchGeometryTest` asserts research D1's table so a
 * transcription slip surfaces here rather than on a user's screen.
 */
object VelaLaunch {
    /** Authored length of the animation: 102 frames ÷ 60 fps. */
    const val durationMs: Int = 1700

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

    /** FR-014: nothing presented by now → abandon the animation, show Welcome. */
    const val firstFrameBudgetMs: Long = 400

    /**
     * FR-015: measured from the first presented frame, not from construction.
     * Nominal is 1700 play + 400 hold + 400 dissolve = 2500; the rest is slack.
     */
    const val hardCeilingMs: Long = 3000

    /**
     * Form-factor threshold. Android is `screenOrientation="portrait"`-locked and
     * phones are narrower than this, so a phone always resolves to the phone
     * composition; a ≥ 768 dp tablet picks up the large-screen one with no
     * special-casing.
     *
     * Deliberately NOT a layout breakpoint reused from elsewhere — this governs
     * which animation is authored for the screen, not how a screen lays out.
     */
    val largeScreenMinWidth: Dp = 768.dp

    /**
     * Core canvases (research D0). These are the cropped framings that ship; the
     * full-bleed pair exists only to pin [boxWidthRatio] and is never loaded.
     */
    const val phoneCanvasW: Float = 350f
    const val phoneCanvasH: Float = 120f
    const val largeCanvasW: Float = 680f
    const val largeCanvasH: Float = 220f

    /**
     * Box width as a fraction of viewport width: the core canvas divided by the
     * full-bleed canvas it was cropped from. NOT a judgement call — at 390 dp the
     * phone lockup lands at exactly the authored 80.7 % of screen width, and
     * `scripts/lint-lottie-assets.mjs` fails if a re-crop moves either number.
     */
    const val phoneBoxWidthRatio: Float = 350f / 390f
    const val largeBoxWidthRatio: Float = 680f / 1920f

    /** Bundled asset names, synced from `docs/design/onboarding/launch/` at build time. */
    fun assetName(largeScreen: Boolean, darkTheme: Boolean): String {
        val form = if (largeScreen) "desktop" else "phone"
        val appearance = if (darkTheme) "dark" else "light"
        return "animations/vela-wallet-launch-$form-core-$appearance.json"
    }

    /** The shared predicate, in device-independent units. */
    fun isLargeScreen(widthDp: Dp, heightDp: Dp): Boolean =
        widthDp >= heightDp || widthDp >= largeScreenMinWidth

    /**
     * Box size for a viewport, per the shared fit rule. Centred by the caller;
     * nothing is clipped or clamped, because the shipped asset is cropped to the
     * motion — the box *is* the artwork.
     */
    fun boxSize(viewportWidth: Dp, largeScreen: Boolean): Pair<Dp, Dp> {
        val ratio = if (largeScreen) largeBoxWidthRatio else phoneBoxWidthRatio
        val canvasW = if (largeScreen) largeCanvasW else phoneCanvasW
        val canvasH = if (largeScreen) largeCanvasH else phoneCanvasH
        val w = viewportWidth * ratio
        return w to (w * (canvasH / canvasW))
    }
}
