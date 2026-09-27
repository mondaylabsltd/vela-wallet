package app.getvela.wallet.core.diagnostics

import kotlin.math.abs
import kotlin.math.hypot
import kotlin.math.max
import kotlin.math.min

/**
 * Tap a screenshot to see it large (spec 078 §C; the founder: 「上传的截图要能点击
 * 放大预览吧」) — the rules with no Android in them, so the JVM tests hold them.
 *
 * **Which image, and where focus goes.** Only a prepared tile opens — it shows
 * the bytes that will be sent, so what the person inspects is exactly what goes
 * public — and nothing opens while the report is sending. Remove in the viewer
 * shows the next image, or the previous when it was the last; removing the only
 * one closes it. Closing puts focus back on the tile that opened it — or, when
 * that one was removed in the viewer, on the tile that took its place, else
 * the add target.
 *
 * **Gestures, as numbers** (the web's `viewer-gesture.ts`, in px and px/s):
 * sideways pages; down (not zoomed) dismisses, the black fading as the picture
 * follows the finger, closing past [DISMISS_FRACTION] of the height or on a
 * flick; zoomed, a drag pans and never pages or dismisses; pinch zooms about
 * the fingers, [MIN_ZOOM]–[MAX_ZOOM]; a double tap goes to [DOUBLE_TAP_ZOOM]
 * about the tap, or back to 1.
 */
object ScreenshotViewerRules {

    /** Released past this fraction of the viewer's height: it closes (the spec's "~20%"). */
    const val DISMISS_FRACTION: Float = 0.2f

    /** Released moving down faster than this (dp/s — the sheets' 0.5 px/ms): it closes. */
    const val FLICK_VELOCITY_DP: Float = 500f

    /** A flick still has to have moved the picture at least this far (dp). */
    const val FLICK_MIN_DISTANCE_DP: Float = 16f

    const val MIN_ZOOM: Float = 1f
    const val MAX_ZOOM: Float = 4f

    /** Where a double tap zooms to. */
    const val DOUBLE_TAP_ZOOM: Float = 2f

    /** Two taps this close (dp) are one double tap. */
    const val DOUBLE_TAP_DISTANCE_DP: Float = 24f

    /** Below this, a pinch let go is no zoom at all. */
    const val ZOOM_EPSILON: Float = 1.02f

    /** Open and close: a fade and a slight scale, inside the spec's 150–200 ms. */
    const val MOTION_MS: Int = 180

    /** The picture's scale at the start of the open (and the end of the close). */
    const val MOTION_START_SCALE: Float = 0.94f

    // --- which image -----------------------------------------------------

    /** Whether tapping tile [id] opens the viewer: it is there, it is prepared, and nothing is being sent. */
    fun <R> opens(tiles: List<ScreenshotTray.Tile<R>>, id: Long, sending: Boolean): Boolean =
        !sending && tiles.any { it.id == id && it.ready != null }

    /** The tiles the viewer pages through: the prepared ones, in tile order. */
    fun <R> viewable(tiles: List<ScreenshotTray.Tile<R>>): List<ScreenshotTray.Tile<R>> =
        tiles.filter { it.ready != null }

    /**
     * After [removed] leaves [shown] (the viewer's order): the image to show —
     * the next, or the previous when it was the last — or null: it was the
     * only one, so the viewer closes.
     */
    fun shownAfterRemove(shown: List<Long>, removed: Long): Long? {
        val at = shown.indexOf(removed)
        if (at < 0) return shown.firstOrNull()
        return shown.getOrNull(at + 1) ?: shown.getOrNull(at - 1)
    }

    /**
     * Where focus goes when the viewer closes, after [removed] was taken out
     * of [tiles] (all of them, in tile order): unchanged, unless it was the
     * tile focus was to return to — then the tile that took its place, or
     * null (the add target) when none did.
     */
    fun returnAfterRemove(tiles: List<Long>, returnTo: Long?, removed: Long): Long? {
        if (returnTo != removed) return returnTo
        val at = tiles.indexOf(removed)
        return if (at < 0) null else tiles.getOrNull(at + 1)
    }

    /** "2 / 5" — shown only when there is more than one image. */
    fun counter(index: Int, count: Int): String? = if (count > 1) "${index + 1} / $count" else null

    // --- gestures ----------------------------------------------------------

    enum class Drag { Undecided, Pan, Page, Dismiss, None }

    /**
     * What one finger's drag is for, once it has moved past [slop]. [Drag.None]
     * is a push UP while not zoomed: nothing to do, and it must not page.
     */
    fun takesDrag(dx: Float, dy: Float, zoomed: Boolean, slop: Float): Drag = when {
        abs(dx) < slop && abs(dy) < slop -> Drag.Undecided
        zoomed -> Drag.Pan
        abs(dx) > abs(dy) -> Drag.Page
        dy > 0f -> Drag.Dismiss
        else -> Drag.None
    }

    /** iOS's rubber band: follows [distance] less and less, approaching [dimension]. */
    fun rubberBand(distance: Float, dimension: Float, c: Float = 0.55f): Float {
        if (!(distance > 0f) || !(dimension > 0f)) return 0f
        return (1f - 1f / (distance * c / dimension + 1f)) * dimension
    }

    /** Where the picture sits while dragged down: 1:1 down, rubber-banded back up. */
    fun dismissOffset(dy: Float, height: Float): Float = if (dy >= 0f) dy else -rubberBand(-dy, height)

    /** On release: close, or spring back? [velocity] in px/s, positive = down. */
    fun shouldDismiss(offset: Float, height: Float, velocity: Float, flickVelocity: Float, flickMinDistance: Float): Boolean {
        if (!(offset > 0f)) return false
        if (offset > height * DISMISS_FRACTION) return true
        return velocity > flickVelocity && offset >= flickMinDistance
    }

    /** How much of the black is left while the picture is dragged down — gone well before it reaches the bottom. */
    fun backdrop(offset: Float, height: Float): Float {
        if (!(offset > 0f)) return 1f
        return max(0f, 1f - offset / max(1f, height * 0.6f))
    }

    /** The picture shrinks a little as it is pulled away, as the platform viewers do. */
    fun dismissScale(offset: Float, height: Float): Float {
        if (!(offset > 0f) || !(height > 0f)) return 1f
        return 1f - min(0.2f, offset / height * 0.4f)
    }

    /** The picture's scale and its offset (px) from centred. */
    data class Zoom(val scale: Float = 1f, val x: Float = 0f, val y: Float = 0f) {
        val zoomed: Boolean get() = scale > ZOOM_EPSILON
    }

    val UNZOOMED: Zoom = Zoom()

    /** The picture fitted into the box, aspect kept — scaled up as well as down. */
    fun fitSize(width: Float, height: Float, boxWidth: Float, boxHeight: Float): Pair<Float, Float> {
        if (!(width > 0f && height > 0f && boxWidth > 0f && boxHeight > 0f)) return boxWidth to boxHeight
        val scale = min(boxWidth / width, boxHeight / height)
        return width * scale to height * scale
    }

    fun clampScale(scale: Float): Float = scale.coerceIn(MIN_ZOOM, MAX_ZOOM)

    /** A zoomed picture's edges stay at or beyond the box's: never a gap at a side. */
    fun clampPan(zoom: Zoom, fittedWidth: Float, fittedHeight: Float, boxWidth: Float, boxHeight: Float): Zoom {
        val maxX = max(0f, (fittedWidth * zoom.scale - boxWidth) / 2f)
        val maxY = max(0f, (fittedHeight * zoom.scale - boxHeight) / 2f)
        return zoom.copy(x = zoom.x.coerceIn(-maxX, maxX), y = zoom.y.coerceIn(-maxY, maxY))
    }

    /**
     * Zoom to [scale] keeping the picture's point under the focus ([fx], [fy]
     * in px from the box's centre) exactly where it is — what makes a pinch
     * or a double tap land on the thing the finger is on.
     */
    fun zoomAbout(from: Zoom, scale: Float, fx: Float, fy: Float): Zoom {
        val ratio = scale / from.scale
        return Zoom(scale, fx - (fx - from.x) * ratio, fy - (fy - from.y) * ratio)
    }

    /** A double tap toggles: into [DOUBLE_TAP_ZOOM] about the tap, or back out to 1. */
    fun doubleTapZoom(from: Zoom, fx: Float, fy: Float, fittedWidth: Float, fittedHeight: Float, boxWidth: Float, boxHeight: Float): Zoom {
        if (from.zoomed) return UNZOOMED
        return clampPan(zoomAbout(from, DOUBLE_TAP_ZOOM, fx, fy), fittedWidth, fittedHeight, boxWidth, boxHeight)
    }

    /** Between two zooms, [t] of the way (an animated double tap). */
    fun lerp(from: Zoom, to: Zoom, t: Float): Zoom = Zoom(
        from.scale + (to.scale - from.scale) * t,
        from.x + (to.x - from.x) * t,
        from.y + (to.y - from.y) * t,
    )

    /**
     * A control that acts on a TAP, never on a touch that turned into a
     * scroll (078; found on the iPhone 2026-09-27: a scroll that began on a
     * screenshot's remove badge deleted the screenshot — the badge's hit area
     * reaches into where a thumb starts scrolling the sheet). Compose's
     * `clickable` drops the click once a PARENT consumes the drag, but a
     * sideways drag, or one the sheet's scroll did not claim, can still end
     * inside the control's bounds and click. So the control decides for
     * itself: a press that moved past [slop] from where it went down is not a
     * tap. No press at all (TalkBack's double tap, a keyboard's Enter) is —
     * so a press is forgotten once its gesture has [end]ed, whether or not it
     * clicked, and a later click with no pointer behind it is never refused.
     */
    class TapSlop(private val slop: Float) {
        private var downX = 0f
        private var downY = 0f
        private var pressed = false
        private var spoiled = false

        fun down(x: Float, y: Float) {
            downX = x
            downY = y
            pressed = true
            spoiled = false
        }

        fun move(x: Float, y: Float) {
            if (pressed && hypot(x - downX, y - downY) > slop) spoiled = true
        }

        /** The pointer was taken (cancelled): not a tap. */
        fun cancel() {
            if (pressed) spoiled = true
        }

        /** The gesture is over (after the click, if any, was decided). */
        fun end() {
            pressed = false
            spoiled = false
        }

        /** Is the click that just arrived a real tap (or no pointer's at all)? */
        fun accept(): Boolean = !(pressed && spoiled)
    }
}
