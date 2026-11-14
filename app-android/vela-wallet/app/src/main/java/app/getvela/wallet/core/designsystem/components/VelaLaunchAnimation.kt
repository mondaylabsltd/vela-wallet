package app.getvela.wallet.core.designsystem.components

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameMillis
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.unit.dp
import app.getvela.wallet.core.designsystem.tokens.VelaLaunch
import app.getvela.wallet.core.designsystem.tokens.VelaLaunchScene
import kotlinx.coroutines.delay
import kotlin.math.min

/**
 * The most the launch clock advances in one frame: two frames at 60 Hz. See
 * the clock in [VelaLaunchAnimation].
 */
private const val MAX_STEP_MS = 34f

/**
 * The launch animation, drawn with Compose's own Canvas and clock — no
 * animation library. The scene (paths, colours, timings) is
 * `docs/design/onboarding/launch/launch-scene.json`, generated into
 * [VelaLaunchScene]; [VelaLaunchTimeline] says where everything is at a moment.
 *
 * Everything about the shape of this is shared with the other three platforms:
 * play once → hold the finished lockup → cross-dissolve into Welcome, with any
 * input cutting straight to the dissolve and every path ending on Welcome.
 *
 * @param onProgress fraction of the cross-dissolve completed, 0..1. The HOST
 *   applies this to the Welcome content's alpha so the two layers dissolve into
 *   each other. It is not the animation's own progress.
 * @param onFinished called EXACTLY once, for every outcome — completion, skip,
 *   ceiling, reduce-motion. The host does not distinguish between them; it only
 *   removes the overlay.
 */
@Composable
fun VelaLaunchAnimation(
    darkTheme: Boolean,
    reduceMotion: Boolean,
    backgroundColor: Color,
    onProgress: (Float) -> Unit,
    onFinished: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val configuration = LocalConfiguration.current
    val largeScreen = VelaLaunch.isLargeScreen(
        configuration.screenWidthDp.dp,
        configuration.screenHeightDp.dp,
    )
    val (boxWidth, boxHeight) = VelaLaunch.boxSize(configuration.screenWidthDp.dp, largeScreen)

    // Built once: the scene never changes, only where and how opaque it is.
    val markPaths = remember { VelaLaunchScene.markPaths.map { it.fill to it.ops.toPath() } }
    val letterPaths = remember { VelaLaunchScene.letters.map { it.ops.toPath() } }
    val colors = if (darkTheme) VelaLaunchScene.dark else VelaLaunchScene.light

    // Latched so a completion racing a timeout cannot dismiss twice
    // (contract behaviour 1). `rememberUpdatedState` keeps the timers pointed at
    // the current lambda without restarting them.
    val finish by rememberUpdatedState(onFinished)
    var finished by remember { mutableStateOf(false) }
    val finishOnce = remember {
        {
            if (!finished) {
                finished = true
                finish()
            }
        }
    }

    var exiting by remember { mutableStateOf(false) }

    // The animation's clock, in milliseconds of the authored timeline. Linear:
    // the easing belongs to each tween (VelaLaunchTimeline), not to time.
    // Read only in the draw pass below, so a frame redraws without recomposing.
    var clock by remember { mutableFloatStateOf(0f) }

    // Play → hold → dissolve. Reduce-motion shows the finished lockup and skips
    // the hold entirely: the point of the setting is less time spent on motion
    // (FR-019/FR-020).
    //
    // The clock follows Compose's frame clock, but advances at most
    // [MAX_STEP_MS] per frame. A cold start's first frames are the busiest the
    // main thread has (the screen behind is being built), and a wall-clock
    // timeline jumped across them: on a slow phone the mark froze, then leapt
    // with the letters already half there. Capped, a stall pauses the motion
    // instead of skipping it; the ceiling below still bounds the whole.
    LaunchedEffect(reduceMotion) {
        if (reduceMotion) {
            clock = VelaLaunchScene.durationMs
        } else {
            var last = withFrameMillis { it }
            while (clock < VelaLaunchScene.durationMs && !exiting) {
                val now = withFrameMillis { it }
                clock = min(VelaLaunchScene.durationMs, clock + min((now - last).toFloat(), MAX_STEP_MS))
                last = now
            }
            if (!exiting) delay(VelaLaunch.holdMs)
        }
        // A skip already froze the lockup where it stood (the loop above stops).
        exiting = true
    }

    // FR-015: hard ceiling from the first presented frame.
    LaunchedEffect(Unit) {
        withFrameMillis { }
        delay(VelaLaunch.hardCeilingMs)
        exiting = true
    }

    // Cross-dissolve. The HOST fades the Welcome content in by the same fraction
    // over a background that never fades, which is what stops the transition
    // washing out through to the bare window.
    val dissolve by animateFloatAsState(
        targetValue = if (exiting) 1f else 0f,
        animationSpec = tween(durationMillis = VelaLaunch.exitCrossfadeMs),
        label = "launchDissolve",
    )
    LaunchedEffect(dissolve) {
        onProgress(dissolve)
        if (exiting && dissolve >= 1f) finishOnce()
    }

    Box(
        modifier = modifier
            .fillMaxSize()
            // Opaque until the dissolve starts — Welcome must not be visible
            // through it (FR-013). During the dissolve the host's identically
            // coloured background takes over, so this fades with the overlay.
            .background(backgroundColor.copy(alpha = 1f - dissolve))
            .alpha(1f - dissolve)
            // FR-016: any tap cuts to the dissolve. `pointerInput` rather than
            // `clickable` so there is no ripple and no accessibility affordance
            // on what is decoration.
            .pointerInput(Unit) {
                awaitPointerEventScope {
                    while (true) {
                        awaitPointerEvent()
                        exiting = true
                    }
                }
            },
        contentAlignment = Alignment.Center,
    ) {
        // The box IS the canvas (phone or large), scaled to fit it.
        Canvas(modifier = Modifier.size(boxWidth, boxHeight)) {
            val ms = clock
            val canvasW = if (largeScreen) VelaLaunch.largeCanvasW else VelaLaunch.phoneCanvasW
            val fit = size.width / canvasW
            withTransform({
                scale(fit, fit, pivot = Offset.Zero)
                // The large canvas draws the same scene, framed: translate then scale.
                if (largeScreen) {
                    translate(VelaLaunchScene.largeOffsetX, VelaLaunchScene.largeOffsetY)
                    scale(VelaLaunchScene.largeScale, VelaLaunchScene.largeScale, pivot = Offset.Zero)
                }
            }) {
                letterPaths.forEachIndexed { i, path ->
                    val alpha = VelaLaunchTimeline.letterAlpha(i, ms)
                    if (alpha > 0f) drawPath(path, colors.wordmark, alpha = alpha)
                }
                translate(VelaLaunchTimeline.markX(ms), VelaLaunchTimeline.markY(ms)) {
                    markPaths.forEach { (fill, path) -> drawPath(path, colors[fill]) }
                }
            }
        }
    }
}

/**
 * Nonzero fill (the [Path] default) on purpose: the crossbars of "e" and "t"
 * are separate contours wound the same way as their outlines, and even-odd
 * would punch them out.
 */
private fun List<VelaLaunchScene.Op>.toPath(): Path = Path().also { path ->
    for (op in this) {
        when (op) {
            is VelaLaunchScene.Op.Move -> path.moveTo(op.x, op.y)
            is VelaLaunchScene.Op.Line -> path.lineTo(op.x, op.y)
            is VelaLaunchScene.Op.Curve -> path.cubicTo(op.x1, op.y1, op.x2, op.y2, op.x, op.y)
            VelaLaunchScene.Op.Close -> path.close()
        }
    }
}
