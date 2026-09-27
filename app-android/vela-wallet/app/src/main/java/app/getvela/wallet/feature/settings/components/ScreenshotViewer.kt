package app.getvela.wallet.feature.settings.components

import android.graphics.BitmapFactory
import android.view.WindowManager
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.FastOutLinearInEasing
import androidx.compose.animation.core.LinearOutSlowInEasing
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animate
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.calculateCentroid
import androidx.compose.foundation.gestures.calculatePan
import androidx.compose.foundation.gestures.calculateZoom
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.LocalTextStyle
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.isSpecified
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.PointerInputScope
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChange
import androidx.compose.ui.input.pointer.positionChanged
import androidx.compose.ui.input.pointer.util.VelocityTracker
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.window.DialogWindowProvider
import androidx.core.view.WindowCompat
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.tokens.VelaColorsDark
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaMotion
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.diagnostics.ScreenshotViewerRules
import app.getvela.wallet.core.diagnostics.ScreenshotViewerRules.Drag
import app.getvela.wallet.core.diagnostics.ScreenshotViewerRules.UNZOOMED
import app.getvela.wallet.core.diagnostics.ScreenshotViewerRules.Zoom
import app.getvela.wallet.core.platform.VelaHaptic
import app.getvela.wallet.core.platform.rememberVelaHaptic
import app.getvela.wallet.feature.contacts.components.rememberReducedMotion
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * One image the viewer pages through: the PREPARED bytes — exactly what will be
 * sent, so what the person inspects is what goes public (spec 078 §C5) — and
 * TalkBack's name for it ("View screenshot 2").
 */
class ViewerImage(val id: Long, val jpeg: ByteArray, val width: Int, val height: Int, val label: String)

/** The viewer is black in both themes, so its danger is the light-on-dark one. */
private val ViewerDanger = VelaColorsDark.errorBase
private val ViewerCounter = Color.White.copy(alpha = 0.7f)

/**
 * Tap a screenshot to see it large (spec 078 §C, phone): a full-screen viewer
 * on pure black, above the report sheet.
 *
 * - The picture is fitted BETWEEN the top bar (the counter "2 / 5" centred,
 *   the ✕ at the trailing edge, Android's habit) and the bottom bar (Remove,
 *   in the light-on-dark danger colour), so the white controls never sit on
 *   a light screenshot.
 * - Sideways pages; pinch zooms, double tap goes to 2× and back; zoomed, a
 *   drag pans and neither pages nor dismisses.
 * - Down (not zoomed) dismisses: the controls go the moment the drag starts,
 *   the black fades as the picture follows the finger; past ~20% of the
 *   height or on a flick it closes, else it springs back. System Back closes.
 * - Opens and closes with a 180 ms fade and a slight scale; reduced motion
 *   (animator duration scale 0) keeps the fade and drops the scale.
 * - Remove shows the next image (the previous when it was the last);
 *   removing the only one closes it.
 *
 * A Dialog with its own full-screen window, not a sheet: the app has one sheet
 * host ([app.getvela.wallet.core.designsystem.components.VelaModalSheet]) and a
 * viewer is not one. The window takes the app's text size with it (a dialog's
 * window starts from the system's), and its status bar icons are light.
 *
 * [onClosed] runs once the close has finished animating; the caller then
 * drops the viewer and puts focus back on the tile.
 */
@Composable
fun ScreenshotViewer(
    images: List<ViewerImage>,
    startId: Long,
    closeLabel: String,
    removeLabel: String,
    onRemove: (id: Long) -> Unit,
    onClosed: () -> Unit,
) {
    val appDensity = LocalDensity.current
    val reduceMotion = rememberReducedMotion()
    val scope = rememberCoroutineScope()
    val presence = remember { Animatable(0f) }
    var closing by remember { mutableStateOf(false) }
    val closed by rememberUpdatedState(onClosed)
    val drag = remember { ViewerDrag() }
    LaunchedEffect(Unit) {
        presence.animateTo(1f, tween(ScreenshotViewerRules.MOTION_MS, easing = LinearOutSlowInEasing))
    }
    /** Fade (and scale) out, then [after] — the removal of the only image — then hand back. */
    val close: (after: () -> Unit, height: Float) -> Unit = { after, height ->
        if (!closing) {
            closing = true
            scope.launch {
                // A dismissing drag carries on down while it fades.
                if (drag.offset > 0f && height > 0f) {
                    launch {
                        animate(drag.offset, drag.offset + height * 0.25f, animationSpec = tween(ScreenshotViewerRules.MOTION_MS)) { v, _ -> drag.offset = v }
                    }
                }
                presence.animateTo(0f, tween(ScreenshotViewerRules.MOTION_MS, easing = FastOutLinearInEasing))
                after()
                closed()
            }
        }
    }
    val startLabel = remember(startId) { images.firstOrNull { it.id == startId }?.label.orEmpty() }
    Dialog(
        onDismissRequest = { close({}, 0f) },
        properties = DialogProperties(
            dismissOnBackPress = true,
            dismissOnClickOutside = false,
            usePlatformDefaultWidth = false,
            decorFitsSystemWindows = false,
            // What TalkBack says as the window comes up: "View screenshot 2".
            windowTitle = startLabel,
        ),
    ) {
        val window = (LocalView.current.parent as? DialogWindowProvider)?.window
        DisposableEffect(window) {
            window?.let {
                // Nothing dims the sheet behind: the viewer's own black is the
                // only ground, and it is what fades while the picture is pulled down.
                it.clearFlags(WindowManager.LayoutParams.FLAG_DIM_BEHIND)
                it.setDimAmount(0f)
                // The open and close are drawn here, not by the window manager.
                it.setWindowAnimations(0)
                WindowCompat.getInsetsController(it, it.decorView).apply {
                    isAppearanceLightStatusBars = false
                    isAppearanceLightNavigationBars = false
                }
            }
            onDispose {}
        }
        CompositionLocalProvider(LocalDensity provides appDensity) {
            ViewerContent(
                images = images,
                startId = startId,
                closeLabel = closeLabel,
                removeLabel = removeLabel,
                presence = { presence.value },
                reduceMotion = reduceMotion,
                closing = closing,
                drag = drag,
                onClose = { height -> close({}, height) },
                onRemove = onRemove,
                onRemoveLast = { id -> close({ onRemove(id) }, 0f) },
            )
        }
    }
}

/** The pull-down in progress: how far, and whether a finger is on it. */
@Stable
private class ViewerDrag {
    var offset by mutableFloatStateOf(0f)
    var active by mutableStateOf(false)
}

@Composable
private fun ViewerContent(
    images: List<ViewerImage>,
    startId: Long,
    closeLabel: String,
    removeLabel: String,
    presence: () -> Float,
    reduceMotion: Boolean,
    closing: Boolean,
    drag: ViewerDrag,
    onClose: (height: Float) -> Unit,
    onRemove: (Long) -> Unit,
    onRemoveLast: (Long) -> Unit,
) {
    val density = LocalDensity.current
    val scope = rememberCoroutineScope()
    val haptic = rememberVelaHaptic()
    val pager = rememberPagerState(initialPage = images.indexOfFirst { it.id == startId }.coerceAtLeast(0)) { images.size }
    var height by remember { mutableFloatStateOf(0f) }
    var zoomed by remember { mutableStateOf(false) }
    // After a remove: the image the rules say comes next, once the list has caught up.
    var pendingShow by remember { mutableStateOf<Long?>(null) }
    LaunchedEffect(images) {
        val target = pendingShow ?: return@LaunchedEffect
        pendingShow = null
        val at = images.indexOfFirst { it.id == target }
        if (at >= 0 && at != pager.currentPage) pager.scrollToPage(at)
    }
    // The controls go the moment a pull-down starts, and come back when it springs back.
    val controls by animateFloatAsState(
        targetValue = if (drag.active || drag.offset > 1f) 0f else 1f,
        animationSpec = tween(if (drag.active) 90 else VelaMotion.durationFast),
        label = "viewerControls",
    )
    val current = images.getOrNull(pager.currentPage.coerceAtMost(images.lastIndex))
    val dismissEnd: (Float) -> Unit = { velocity ->
        val flick = with(density) { ScreenshotViewerRules.FLICK_VELOCITY_DP.dp.toPx() }
        val minDistance = with(density) { ScreenshotViewerRules.FLICK_MIN_DISTANCE_DP.dp.toPx() }
        if (ScreenshotViewerRules.shouldDismiss(drag.offset, height, velocity, flick, minDistance)) {
            onClose(height)
        } else {
            scope.launch {
                animate(drag.offset, 0f, animationSpec = spring(dampingRatio = 0.85f, stiffness = Spring.StiffnessMediumLow)) { v, _ -> drag.offset = v }
            }
        }
    }
    Box(
        modifier = Modifier
            .fillMaxSize()
            .onSizeChanged { height = it.height.toFloat() }
            .semantics { current?.let { paneTitle = it.label } },
    ) {
        // The ground: pure black, fading with the open/close and with a pull-down.
        Box(
            modifier = Modifier
                .fillMaxSize()
                .graphicsLayer { alpha = presence() * ScreenshotViewerRules.backdrop(drag.offset, height) }
                .background(Color.Black),
        )
        Column(modifier = Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing)) {
            ViewerTopBar(
                counter = ScreenshotViewerRules.counter(pager.currentPage, images.size),
                closeLabel = closeLabel,
                alpha = { presence() * controls },
                enabled = !closing && !drag.active,
                onClose = { onClose(height) },
            )
            HorizontalPager(
                state = pager,
                modifier = Modifier
                    .weight(1f)
                    .fillMaxWidth()
                    .graphicsLayer {
                        val p = presence()
                        val open = if (reduceMotion) 1f else ScreenshotViewerRules.MOTION_START_SCALE + (1f - ScreenshotViewerRules.MOTION_START_SCALE) * p
                        val s = open * ScreenshotViewerRules.dismissScale(drag.offset, height)
                        scaleX = s
                        scaleY = s
                        translationY = drag.offset
                        alpha = p
                    },
                key = { images[it].id },
                beyondViewportPageCount = 1,
                pageSpacing = VelaSpacing.lg,
                userScrollEnabled = !zoomed && !drag.active && !closing,
            ) { page ->
                val image = images[page]
                ZoomablePage(
                    image = image,
                    current = page == pager.currentPage,
                    enabled = !closing,
                    drag = drag,
                    height = { height },
                    onZoomed = { if (page == pager.currentPage) zoomed = it },
                    onDismissEnd = dismissEnd,
                )
            }
            ViewerBottomBar(
                label = removeLabel,
                alpha = { presence() * controls },
                enabled = !closing && !drag.active && current != null,
                onRemove = remove@{
                    val shown = current ?: return@remove
                    haptic(VelaHaptic.Press)
                    if (images.size <= 1) {
                        onRemoveLast(shown.id)
                    } else {
                        pendingShow = ScreenshotViewerRules.shownAfterRemove(images.map { it.id }, shown.id)
                        zoomed = false
                        onRemove(shown.id)
                    }
                },
            )
        }
    }
}

@Composable
private fun ViewerTopBar(counter: String?, closeLabel: String, alpha: () -> Float, enabled: Boolean, onClose: () -> Unit) {
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .graphicsLayer { this.alpha = alpha() },
    ) {
        if (counter != null) {
            Text(
                text = counter,
                color = ViewerCounter,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                fontWeight = VelaFontWeight.medium,
                // Tabular digits: "1 / 5" → "2 / 5" never shifts.
                style = LocalTextStyle.current.copy(fontFeatureSettings = "tnum"),
                modifier = Modifier.align(Alignment.Center).semantics { liveRegion = LiveRegionMode.Polite },
            )
        }
        PressableGlyph(
            label = closeLabel,
            enabled = enabled,
            onClick = onClose,
            modifier = Modifier.align(Alignment.CenterEnd).padding(end = VelaSpacing.xs),
        )
    }
}

/** The ✕: a white glyph in a 48dp round target (no disc — the black is its ground). */
@Composable
private fun PressableGlyph(label: String, enabled: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val haptic = rememberVelaHaptic()
    val interaction = remember { MutableInteractionSource() }
    val pressed by interaction.collectIsPressedAsState()
    val scale by animateFloatAsState(if (pressed) VelaMotion.pressScaleFab else 1f, VelaMotion.pressSpring, label = "viewerClosePress")
    Box(
        modifier = modifier
            .size(48.dp)
            .graphicsLayer {
                scaleX = scale
                scaleY = scale
            }
            .clip(CircleShape)
            .clickable(interactionSource = interaction, indication = null, enabled = enabled, role = Role.Button) {
                haptic(VelaHaptic.Press)
                onClick()
            }
            .semantics { contentDescription = label },
        contentAlignment = Alignment.Center,
    ) {
        Icon(VelaIcons.Close, contentDescription = null, tint = Color.White, modifier = Modifier.size(24.dp))
    }
}

@Composable
private fun ViewerBottomBar(label: String, alpha: () -> Float, enabled: Boolean, onRemove: () -> Unit) {
    val interaction = remember { MutableInteractionSource() }
    val pressed by interaction.collectIsPressedAsState()
    val scale by animateFloatAsState(if (pressed) VelaMotion.pressScaleButton else 1f, VelaMotion.pressSpring, label = "viewerRemovePress")
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 64.dp)
            .graphicsLayer { this.alpha = alpha() },
        contentAlignment = Alignment.Center,
    ) {
        // A text button: the danger colour carries it, no filled pill.
        Text(
            text = label,
            color = ViewerDanger,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
            fontWeight = VelaFontWeight.semibold,
            modifier = Modifier
                .graphicsLayer {
                    scaleX = scale
                    scaleY = scale
                }
                .clip(RoundedCornerShape(VelaRadius.full))
                .clickable(interactionSource = interaction, indication = null, enabled = enabled, role = Role.Button, onClick = onRemove)
                .heightIn(min = 48.dp)
                .padding(horizontal = VelaSpacing.xl, vertical = VelaSpacing.md),
        )
    }
}

/**
 * One image: decoded from the bytes that will be sent (off the main thread),
 * fitted into the page, and its gestures — pinch, double tap, pan while zoomed,
 * the pull-down that dismisses. A sideways drag is left to the pager.
 */
@Composable
private fun ZoomablePage(
    image: ViewerImage,
    current: Boolean,
    enabled: Boolean,
    drag: ViewerDrag,
    height: () -> Float,
    onZoomed: (Boolean) -> Unit,
    onDismissEnd: (velocity: Float) -> Unit,
) {
    val scope = rememberCoroutineScope()
    var zoom by remember { mutableStateOf(UNZOOMED) }
    var zoomJob by remember { mutableStateOf<Job?>(null) }
    // A page paged away from comes back at its full, unzoomed size.
    LaunchedEffect(current) { if (!current) { zoomJob?.cancel(); zoom = UNZOOMED } }
    LaunchedEffect(current, zoom.zoomed) { if (current) onZoomed(zoom.zoomed) }
    val bitmap by produceState<ImageBitmap?>(null, image.id) {
        value = withContext(Dispatchers.Default) {
            runCatching { BitmapFactory.decodeByteArray(image.jpeg, 0, image.jpeg.size)?.asImageBitmap() }.getOrNull()
        }
    }
    val live by rememberUpdatedState(enabled)
    val dismissEnd by rememberUpdatedState(onDismissEnd)
    Box(
        modifier = Modifier
            .fillMaxSize()
            // Zoomed, the picture stays between the bars: the white controls
            // are never drawn over a light screenshot.
            .clipToBounds()
            .pointerInput(image.id) {
                viewerGestures(
                    enabled = { live },
                    zoom = { zoom },
                    setZoom = { zoomJob?.cancel(); zoom = it },
                    animateZoom = { target ->
                        zoomJob?.cancel()
                        val from = zoom
                        zoomJob = scope.launch {
                            animate(0f, 1f, animationSpec = tween(VelaMotion.durationFast + 50)) { t, _ -> zoom = ScreenshotViewerRules.lerp(from, target, t) }
                        }
                    },
                    natural = image.width.toFloat() to image.height.toFloat(),
                    drag = drag,
                    height = height,
                    onDismissEnd = { dismissEnd(it) },
                )
            }
            .semantics { contentDescription = image.label },
        contentAlignment = Alignment.Center,
    ) {
        val picture = bitmap
        if (picture != null) {
            Image(
                bitmap = picture,
                contentDescription = null,
                contentScale = ContentScale.Fit,
                modifier = Modifier
                    .fillMaxSize()
                    .graphicsLayer {
                        scaleX = zoom.scale
                        scaleY = zoom.scale
                        translationX = zoom.x
                        translationY = zoom.y
                    },
            )
        } else {
            CircularProgressIndicator(color = ViewerCounter, strokeWidth = 2.dp, modifier = Modifier.size(28.dp))
        }
    }
}

private enum class Mode { Undecided, Pan, Dismiss, Pinch, Pass }

/**
 * The page's gestures in one loop, so their order is ours: the pager (the
 * parent) sees every event after this page, and takes a drag only when the
 * page left it unconsumed — a sideways one while not zoomed.
 */
private suspend fun PointerInputScope.viewerGestures(
    enabled: () -> Boolean,
    zoom: () -> Zoom,
    setZoom: (Zoom) -> Unit,
    animateZoom: (Zoom) -> Unit,
    natural: Pair<Float, Float>,
    drag: ViewerDrag,
    height: () -> Float,
    onDismissEnd: (Float) -> Unit,
) {
    val slop = viewConfiguration.touchSlop
    val doubleTapTimeout = viewConfiguration.doubleTapTimeoutMillis
    val doubleTapDistance = ScreenshotViewerRules.DOUBLE_TAP_DISTANCE_DP.dp.toPx()
    var lastTap: Pair<Long, Offset>? = null
    fun box() = size.width.toFloat() to size.height.toFloat()
    fun fitted(): Pair<Float, Float> {
        val (bw, bh) = box()
        return ScreenshotViewerRules.fitSize(natural.first, natural.second, bw, bh)
    }
    fun clamped(z: Zoom): Zoom {
        val (fw, fh) = fitted()
        val (bw, bh) = box()
        return ScreenshotViewerRules.clampPan(z, fw, fh, bw, bh)
    }
    awaitEachGesture {
        val down = awaitFirstDown(requireUnconsumed = false)
        if (!enabled()) return@awaitEachGesture
        var mode = Mode.Undecided
        var total = Offset.Zero
        var pointer = down.id
        var multi = false
        var lastPosition = down.position
        var lastTime = down.uptimeMillis
        val tracker = VelocityTracker()
        tracker.addPosition(down.uptimeMillis, down.position)
        while (true) {
            val event = awaitPointerEvent()
            val pressed = event.changes.filter { it.pressed }
            if (pressed.isEmpty()) {
                event.changes.firstOrNull { it.id == pointer }?.let {
                    lastPosition = it.position
                    lastTime = it.uptimeMillis
                }
                break
            }
            if (pressed.size >= 2 && (mode == Mode.Undecided || mode == Mode.Pan)) {
                mode = Mode.Pinch
                multi = true
            }
            // The finger this gesture follows; a lifted one hands over to another.
            val change = event.changes.firstOrNull { it.id == pointer && it.pressed } ?: pressed.first().also { pointer = it.id }
            when (mode) {
                Mode.Pinch -> {
                    val factor = event.calculateZoom()
                    val pan = event.calculatePan()
                    val centroid = event.calculateCentroid(useCurrent = true)
                    if (centroid.isSpecified) {
                        val from = zoom()
                        val (bw, bh) = box()
                        val about = ScreenshotViewerRules.zoomAbout(
                            from,
                            ScreenshotViewerRules.clampScale(from.scale * factor),
                            centroid.x - bw / 2f,
                            centroid.y - bh / 2f,
                        )
                        setZoom(clamped(about.copy(x = about.x + pan.x, y = about.y + pan.y)))
                    }
                    event.changes.forEach { if (it.positionChanged()) it.consume() }
                }
                Mode.Undecided -> {
                    total += change.positionChange()
                    tracker.addPosition(change.uptimeMillis, change.position)
                    when (ScreenshotViewerRules.takesDrag(total.x, total.y, zoom().zoomed, slop)) {
                        Drag.Undecided -> Unit
                        Drag.Pan -> {
                            mode = Mode.Pan
                            change.consume()
                        }
                        Drag.Dismiss -> {
                            mode = Mode.Dismiss
                            drag.active = true
                            drag.offset = ScreenshotViewerRules.dismissOffset(total.y - slop, height())
                            change.consume()
                        }
                        // Sideways: the pager's. Up while not zoomed: nobody's.
                        Drag.Page, Drag.None -> mode = Mode.Pass
                    }
                }
                Mode.Pan -> {
                    val d = change.positionChange()
                    val from = zoom()
                    setZoom(clamped(from.copy(x = from.x + d.x, y = from.y + d.y)))
                    change.consume()
                }
                Mode.Dismiss -> {
                    total += change.positionChange()
                    tracker.addPosition(change.uptimeMillis, change.position)
                    drag.offset = ScreenshotViewerRules.dismissOffset(total.y - slop, height())
                    change.consume()
                }
                Mode.Pass -> Unit
            }
        }
        when (mode) {
            // A pinch let go below ~1× is no zoom at all.
            Mode.Pinch, Mode.Pan -> if (!zoom().zoomed && zoom() != UNZOOMED) animateZoom(UNZOOMED)
            Mode.Dismiss -> {
                drag.active = false
                onDismissEnd(tracker.calculateVelocity().y)
            }
            Mode.Undecided -> if (!multi) {
                val previous = lastTap
                if (previous != null && lastTime - previous.first <= doubleTapTimeout && (lastPosition - previous.second).getDistance() <= doubleTapDistance) {
                    lastTap = null
                    val (fw, fh) = fitted()
                    val (bw, bh) = box()
                    animateZoom(ScreenshotViewerRules.doubleTapZoom(zoom(), lastPosition.x - bw / 2f, lastPosition.y - bh / 2f, fw, fh, bw, bh))
                } else {
                    lastTap = lastTime to lastPosition
                }
            }
            Mode.Pass -> Unit
        }
    }
}
