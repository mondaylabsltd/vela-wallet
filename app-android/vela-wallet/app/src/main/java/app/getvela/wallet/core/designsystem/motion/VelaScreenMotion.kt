package app.getvela.wallet.core.designsystem.motion

import android.provider.Settings
import androidx.activity.BackEventCompat
import androidx.activity.compose.PredictiveBackHandler
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import app.getvela.wallet.core.designsystem.tokens.VelaMotion
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import kotlin.coroutines.cancellation.CancellationException

/**
 * Screen motion for the phone (native parity Phase 3, Material 3 guidance):
 *
 * - **Predictive back** on every in-app level ([VelaBackHandler] +
 *   [predictiveBack]): while the back gesture is held, the screen in front
 *   shrinks toward the gesture's edge on a rounded card (the M3 in-app
 *   pattern — 90 %, an 8 dp shift, the sheet radius), and lets go of it
 *   exactly where the person does: committed, the level beneath takes its
 *   place; cancelled, it springs back. Sheets get the same from Material's
 *   own `ModalBottomSheet` once the manifest opts in
 *   (`enableOnBackInvokedCallback`).
 * - **Entrances** ([screenEntrance]): a level entered slides in a step from
 *   the side it came from and settles on a spring with no overshoot — once
 *   per level (DESIGN-LANGUAGE rule 10: never replayed by a recomposition).
 *
 * Both are still when the person asked for less motion ([rememberReducedMotion]:
 * the system's "Remove animations", i.e. an animator duration scale of 0);
 * Back itself works the same either way.
 */
@Stable
class PredictiveBackState {
    /** 0 at rest, toward 1 as the back gesture is held. */
    var progress by mutableFloatStateOf(0f)
        internal set

    /** [BackEventCompat.EDGE_LEFT] or [BackEventCompat.EDGE_RIGHT]: where the gesture started. */
    var edge by mutableIntStateOf(BackEventCompat.EDGE_LEFT)
        internal set
}

@Composable
fun rememberPredictiveBackState(): PredictiveBackState = remember { PredictiveBackState() }

/**
 * [androidx.activity.compose.BackHandler] with the gesture's progress told to
 * [state] (`null`: no layer follows it). [onBack] runs once, when the gesture
 * commits — or at once for a button-press Back, which has no progress.
 */
@Composable
fun VelaBackHandler(
    enabled: Boolean = true,
    state: PredictiveBackState? = null,
    onBack: () -> Unit,
) {
    val current by rememberUpdatedState(onBack)
    PredictiveBackHandler(enabled = enabled) { events ->
        try {
            events.collect { event ->
                state?.edge = event.swipeEdge
                state?.progress = event.progress
            }
            current()
        } catch (cancelled: CancellationException) {
            throw cancelled
        } finally {
            // Committed or cancelled, the layer settles back from where it is.
            state?.progress = 0f
        }
    }
}

/** The person asked for less motion: the system's animator scale is 0 ("Remove animations"). */
@Composable
fun rememberReducedMotion(): Boolean {
    val resolver = LocalContext.current.contentResolver
    return remember(resolver) { reducedMotion(Settings.Global.getFloat(resolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f)) }
}

/** The rule, on its own so it is tested: any scale of 0 or less is "no motion". */
fun reducedMotion(animatorScale: Float): Boolean = animatorScale <= 0f

/** How the held gesture draws the screen in front — a pure function of the gesture, tested on the JVM. */
data class BackLayer(val scale: Float, val shiftFraction: Float, val cornerFraction: Float) {
    companion object {
        val REST = BackLayer(1f, 0f, 0f)

        /** [progress] 0…1 → the M3 in-app shape: scale to 90 %, shift toward the edge, corners to the sheet radius. */
        fun of(progress: Float, reduced: Boolean): BackLayer {
            if (reduced) return REST
            val eased = FastOutSlowInEasing.transform(progress.coerceIn(0f, 1f))
            return BackLayer(
                scale = 1f - (1f - VelaMotion.predictiveBackScale) * eased,
                shiftFraction = eased,
                cornerFraction = if (eased > 0f) 1f else 0f,
            )
        }
    }
}

/** The screen in front follows [state]'s gesture (see [BackLayer]). */
fun Modifier.predictiveBack(state: PredictiveBackState, reduced: Boolean): Modifier = composed {
    val progress by animateFloatAsState(
        targetValue = state.progress,
        animationSpec = VelaMotion.backSettleSpring,
        label = "predictiveBack",
    )
    val layer = BackLayer.of(progress, reduced)
    if (layer == BackLayer.REST) return@composed this
    val toward = if (state.edge == BackEventCompat.EDGE_RIGHT) -1f else 1f
    graphicsLayer {
        scaleX = layer.scale
        scaleY = layer.scale
        translationX = toward * layer.shiftFraction * VelaSpacing.md.toPx()
        shape = RoundedCornerShape(VelaRadius.xl2 * layer.cornerFraction)
        clip = true
    }
}

/**
 * A level's entrance: from [VelaSpacing.xl3] toward the side it came from,
 * fading up, settled by [VelaMotion.entranceSpring] — keyed on [level], so it
 * plays once per level and never on a recomposition. [depth] (a stack's size,
 * a journey's step) says which side: deeper or level is entered from the end
 * side, shallower (a Back) from the start side.
 */
fun Modifier.screenEntrance(
    level: Any?,
    depth: Int,
    reduced: Boolean,
    /** `false`: the first level shown (a tab's own root, which cuts in) does not slide. */
    animateFirst: Boolean = true,
): Modifier = composed {
    val lastDepth = remember { intArrayOf(depth) }
    val seen = remember { booleanArrayOf(false) }
    val forward = remember(level) { (depth >= lastDepth[0]).also { lastDepth[0] = depth } }
    val first = remember(level) { !seen[0].also { seen[0] = true } }
    val offset = remember(level) { Animatable(if (reduced || (first && !animateFirst)) 0f else 1f) }
    LaunchedEffect(level) {
        if (offset.value != 0f) offset.animateTo(0f, VelaMotion.entranceSpring)
    }
    val direction = if (forward) 1f else -1f
    graphicsLayer {
        translationX = direction * offset.value * VelaSpacing.xl3.toPx()
        alpha = 1f - offset.value * VelaMotion.entranceFadeDepth
    }
}

/** A flow level: it follows the held back gesture, and enters from its side once. */
fun Modifier.flowMotion(level: Any?, depth: Int, back: PredictiveBackState, reduced: Boolean, animateFirst: Boolean = true): Modifier =
    predictiveBack(back, reduced).screenEntrance(level, depth, reduced, animateFirst)
