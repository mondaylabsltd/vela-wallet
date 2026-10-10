package app.getvela.wallet.feature.settings.components

import app.getvela.wallet.core.designsystem.tokens.VelaFontFeaturesTabular
import android.graphics.Bitmap
import android.view.accessibility.AccessibilityManager
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.MutableTransitionState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.LocalTextStyle
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.remember
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.InputMode
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalInputModeManager
import androidx.compose.ui.platform.LocalViewConfiguration
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.min
import androidx.compose.ui.unit.sp
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.components.VelaLabelBesideValue
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaMotion
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.diagnostics.BugReport
import app.getvela.wallet.core.diagnostics.ScreenshotTray
import app.getvela.wallet.core.diagnostics.ScreenshotViewerRules
import app.getvela.wallet.core.platform.VelaHaptic
import app.getvela.wallet.core.platform.rememberVelaHaptic
import app.getvela.wallet.feature.settings.FeedbackModel
import kotlinx.coroutines.flow.first

/** One tile as the sheet draws it: its id, and its thumbnail once prepared. */
data class ScreenshotTileView(val id: Long, val thumbnail: Bitmap?, val ready: Boolean)

/**
 * Where focus goes after the viewer closed (spec 078 §C4): the tile [id], or
 * the add target when null. [seq] makes a second return to the same tile a
 * new request.
 */
data class ScreenshotFocusReturn(val id: Long?, val seq: Int)

private const val TILE_MOTION_MS = 150

/**
 * An icon that belongs to a line of text is sized in sp, so it grows with the
 * app's text size instead of shrinking beside it (spec 078 round 3, A8).
 */
@Composable
fun textIconSize(size: TextUnit): Dp = with(LocalDensity.current) { size.toDp() }

/** A glyph that sits on a text line and scales with it. */
@Composable
fun FeedbackLineIcon(icon: ImageVector, tint: Color, size: TextUnit, modifier: Modifier = Modifier) {
    Icon(icon, contentDescription = null, tint = tint, modifier = modifier.size(textIconSize(size)))
}

/**
 * The report's screenshots section (spec 078 round 3, the founder's ask, v2):
 * header with the count; either one wide add target or ONE row of five equal
 * square columns (the tiles, then the add tile while fewer than five); under
 * them a refusal while one stands, and — whenever an image is attached — the
 * "screenshots are public" warning, which a refusal never replaces.
 *
 * The warning is never behind a disclosure: the founder ruled screenshots
 * public on the issue, and the person reads that before 发送, not after.
 */
@Composable
fun FeedbackScreenshotsSection(
    model: FeedbackModel,
    tiles: List<ScreenshotTileView>,
    notice: ScreenshotTray.Notice?,
    onAdd: () -> Unit,
    onRemove: (id: Long) -> Unit,
    modifier: Modifier = Modifier,
    /** False while the report is sending: nothing here may change or open (v3 B9, §C5). */
    enabled: Boolean = true,
    /** A prepared tile was tapped: open the viewer on it (spec 078 §C1). */
    onOpen: (id: Long) -> Unit = {},
    /** The viewer closed: focus goes here — the tile, or the add target (§C4) — for TalkBack or a keyboard. */
    focusReturn: ScreenshotFocusReturn? = null,
) {
    val colors = VelaTheme.colors
    val tileFocus = remember { mutableMapOf<Long, FocusRequester>() }
    val addFocus = remember { FocusRequester() }
    val windowInfo = LocalWindowInfo.current
    val context = LocalContext.current
    val inputModes = LocalInputModeManager.current
    LaunchedEffect(focusReturn) {
        val back = focusReturn ?: return@LaunchedEffect
        // Only for someone who moves by focus — TalkBack, or a keyboard. A
        // finger needs no focus back, and a focused add target would wear the
        // ripple's grey focus veil (seen on the Xiaomi after removing the last image).
        val talkBack = context.getSystemService(AccessibilityManager::class.java)?.isTouchExplorationEnabled == true
        if (!talkBack && inputModes.inputMode != InputMode.Keyboard) return@LaunchedEffect
        // The viewer's window is going away: focus can only land once the
        // sheet's window has it back.
        snapshotFlow { windowInfo.isWindowFocused }.first { it }
        val target = back.id?.let { tileFocus[it] } ?: addFocus
        runCatching { target.requestFocus() }
    }
    Column(modifier = modifier.fillMaxWidth()) {
        VelaLabelBesideValue(
            modifier = Modifier.fillMaxWidth(),
            gap = VelaSpacing.md,
            stackedValue = Alignment.Start,
            label = {
                Text(
                    text = model.screenshotsLabel,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.sm,
                    fontWeight = VelaFontWeight.semibold,
                )
            },
            value = {
                Text(
                    text = if (tiles.isEmpty()) model.screenshotsHint else "${tiles.size} / ${BugReport.MAX_SCREENSHOTS}",
                    // fg.muted, not subtle: subtle failed 4.5:1 (v3 B8).
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.sm,
                    // Tabular digits: "1 / 5" → "2 / 5" never shifts.
                    style = LocalTextStyle.current.copy(fontFeatureSettings = VelaFontFeaturesTabular),
                )
            },
        )
        Spacer(modifier = Modifier.height(VelaSpacing.md))
        if (tiles.isEmpty()) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .heightIn(min = VelaSizing.screenshotAdd)
                    .clip(RoundedCornerShape(VelaRadius.lg))
                    .background(colors.bgSunken)
                    .dashedOutline(colors.borderStrong)
                    // Focus returns here when the viewer removed the last tile.
                    .focusRequester(addFocus)
                    .focusProperties { canFocus = enabled }
                    .clickable(enabled = enabled, role = Role.Button, onClick = onAdd)
                    .padding(horizontal = VelaSpacing.lg, vertical = VelaSpacing.md),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md, Alignment.CenterHorizontally),
            ) {
                FeedbackLineIcon(VelaIcons.ImagePlus, colors.fgMuted, 20.sp)
                Text(
                    text = model.addScreenshots,
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                )
            }
        } else {
            // ONE row, five equal columns: images + the add tile never exceed
            // five, and a tile is 72dp or whatever a fifth of the row leaves.
            // A tile the viewer removed never ran its own exit: forget its focus handle here.
            tileFocus.keys.retainAll(tiles.mapTo(HashSet()) { it.id })
            BoxWithConstraints(modifier = Modifier.fillMaxWidth()) {
                // 12dp, as iOS and the web draw it.
                val gap = VelaSpacing.lg
                val tile = min(VelaSizing.screenshotTile, (maxWidth - gap * (BugReport.MAX_SCREENSHOTS - 1)) / BugReport.MAX_SCREENSHOTS)
                Row(horizontalArrangement = Arrangement.spacedBy(gap)) {
                    tiles.forEachIndexed { index, item ->
                        key(item.id) {
                            ScreenshotTile(
                                tile = item,
                                size = tile,
                                removeLabel = model.removeScreenshot.replace("{{index}}", (index + 1).toString()),
                                viewLabel = model.viewScreenshot.replace("{{index}}", (index + 1).toString()),
                                enabled = enabled,
                                focus = tileFocus.getOrPut(item.id) { FocusRequester() },
                                onOpen = { onOpen(item.id) },
                                onRemoved = {
                                    tileFocus.remove(item.id)
                                    onRemove(item.id)
                                },
                            )
                        }
                    }
                    if (tiles.size < BugReport.MAX_SCREENSHOTS) {
                        Box(
                            modifier = Modifier
                                .size(tile)
                                .clip(RoundedCornerShape(VelaRadius.lg))
                                .background(colors.bgSunken)
                                .dashedOutline(colors.borderStrong)
                                .focusRequester(addFocus)
                                .focusProperties { canFocus = enabled }
                                .clickable(enabled = enabled, role = Role.Button, onClick = onAdd)
                                .semantics { contentDescription = model.addScreenshots },
                            contentAlignment = Alignment.Center,
                        ) {
                            FeedbackLineIcon(VelaIcons.ImagePlus, colors.fgMuted, 20.sp)
                        }
                    }
                }
            }
        }
        // A refusal is its own line, above the warning, until the next change.
        val refusal = when (notice) {
            ScreenshotTray.Notice.Limit -> model.screenshotsLimit
            ScreenshotTray.Notice.Unsupported -> model.screenshotUnsupported
            null -> null
        }
        refusal?.let { NoteLine(VelaIcons.TriangleAlert, it, colors.warningBase) }
        if (tiles.isNotEmpty()) NoteLine(VelaIcons.Eye, model.screenshotsPublic, colors.fgMuted)
    }
}

/** A small icon and a text-sm line under the tiles. */
@Composable
private fun NoteLine(icon: ImageVector, text: String, tint: Color) {
    Spacer(modifier = Modifier.height(VelaSpacing.md))
    Row(horizontalArrangement = Arrangement.spacedBy(VelaSpacing.sm)) {
        // Optical alignment with the first line.
        FeedbackLineIcon(icon, tint, VelaTextSize.sm * 1.2f, Modifier.padding(top = 1.dp))
        Text(
            text = text,
            color = tint,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.sm,
            lineHeight = VelaTextSize.sm * 1.4f,
        )
    }
}

/**
 * One square: the prepared image cropped to fill, or a sunken placeholder with
 * a small spinner while it is being prepared. The ✕ is the same in both
 * themes — an opaque neutral disc, a white cross, a ring in the page's colour so
 * it parts from any screenshot under it — overlapping the top-trailing corner by 4dp,
 * inside a 44dp hit area. The tile fades and scales in; a remove fades and
 * scales it out first, then takes it off the tray.
 *
 * The tile's body is a button (spec 078 §C1): a TAP on a prepared tile opens
 * the viewer on it, with the press answered by a small squeeze and a tick; a
 * tile still being prepared does not open. Both the body and the ✕ act only on
 * a real tap ([tapGuard]) — a scroll or a sideways drag that starts on a tile
 * or on its ✕ never opens or deletes anything (the iPhone's 2026-09-27 find).
 */
@Composable
private fun ScreenshotTile(
    tile: ScreenshotTileView,
    size: Dp,
    removeLabel: String,
    viewLabel: String,
    enabled: Boolean,
    focus: FocusRequester,
    onOpen: () -> Unit,
    onRemoved: () -> Unit,
) {
    val colors = VelaTheme.colors
    val shape = RoundedCornerShape(VelaRadius.lg)
    val shown = remember { MutableTransitionState(false).apply { targetState = true } }
    LaunchedEffect(shown.isIdle, shown.currentState, shown.targetState) {
        if (shown.isIdle && !shown.currentState && !shown.targetState) onRemoved()
    }
    val slop = LocalViewConfiguration.current.touchSlop
    val openTap = remember(slop) { ScreenshotViewerRules.TapSlop(slop) }
    val removeTap = remember(slop) { ScreenshotViewerRules.TapSlop(slop) }
    val haptic = rememberVelaHaptic()
    val interaction = remember { MutableInteractionSource() }
    val pressed by interaction.collectIsPressedAsState()
    val focused by interaction.collectIsFocusedAsState()
    val keyboard = LocalInputModeManager.current.inputMode == InputMode.Keyboard
    val scale by animateFloatAsState(if (pressed) VelaMotion.pressScaleButton else 1f, VelaMotion.pressSpring, label = "tilePress")
    AnimatedVisibility(
        visibleState = shown,
        enter = fadeIn(tween(TILE_MOTION_MS)) + scaleIn(tween(TILE_MOTION_MS), initialScale = 0.85f),
        exit = fadeOut(tween(TILE_MOTION_MS)) + scaleOut(tween(TILE_MOTION_MS), targetScale = 0.85f),
    ) {
        Box(modifier = Modifier.size(size)) {
            Box(
                modifier = Modifier
                    .matchParentSize()
                    .graphicsLayer {
                        scaleX = scale
                        scaleY = scale
                    }
                    .clip(shape)
                    .background(colors.bgSunken)
                    .border(
                        // The keyboard's focus ring; a finger never draws one.
                        if (focused && keyboard) 2.dp else VelaBorder.hairline,
                        if (focused && keyboard) colors.fixed.focusRingOuter else colors.borderBase,
                        shape,
                    )
                    .tapGuard(openTap)
                    // Focus comes back here when the viewer closes (§C4) — in
                    // touch mode too, so TalkBack lands on the tile it left.
                    .focusRequester(focus)
                    .focusProperties { canFocus = enabled }
                    .clickable(
                        interactionSource = interaction,
                        indication = null,
                        enabled = enabled && tile.ready,
                        role = Role.Button,
                    ) {
                        if (openTap.accept()) {
                            haptic(VelaHaptic.Press)
                            onOpen()
                        }
                    }
                    .semantics { contentDescription = viewLabel },
                contentAlignment = Alignment.Center,
            ) {
                val thumbnail = tile.thumbnail
                if (thumbnail != null) {
                    val image = remember(thumbnail) { thumbnail.asImageBitmap() }
                    Image(bitmap = image, contentDescription = null, contentScale = ContentScale.Crop, modifier = Modifier.matchParentSize())
                } else if (!tile.ready) {
                    CircularProgressIndicator(color = colors.fgMuted, strokeWidth = 2.dp, modifier = Modifier.size(20.dp))
                }
            }
            // The disc's edge sits 4dp past the corner. Its 44dp hit area grows
            // OUTWARD (v3 B10): centred on the disc across, and upward from the
            // disc's bottom edge — over the tile it covers little more than the
            // disc itself, never half the picture.
            val hit = VelaSizing.screenshotRemoveHit
            val disc = VelaSizing.screenshotRemove
            val discCentreIn = disc / 2 - 4.dp
            Box(
                modifier = Modifier
                    .align(Alignment.TopEnd)
                    .offset(x = hit / 2 - discCentreIn, y = -(hit - (discCentreIn + disc / 2)))
                    .size(hit)
                    // That outward area is where a thumb starts scrolling the
                    // sheet: only a tap removes.
                    .tapGuard(removeTap)
                    .clickable(enabled = enabled, role = Role.Button) {
                        if (removeTap.accept()) shown.targetState = false
                    }
                    .semantics { contentDescription = removeLabel },
                contentAlignment = Alignment.BottomCenter,
            ) {
                // Opaque (v3 B1): a see-through disc read two-toned over a
                // screenshot. fg.base on light (#1A1A18), border.strong on dark
                // (#3E3E38) — a solid neutral lighter than the page.
                val disc = if (VelaTheme.isDark) colors.borderStrong else colors.fgBase
                Box(
                    modifier = Modifier
                        .size(VelaSizing.screenshotRemove)
                        .clip(CircleShape)
                        .background(disc)
                        .border(2.dp, colors.bgBase, CircleShape),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(VelaIcons.Close, contentDescription = null, tint = Color.White, modifier = Modifier.size(12.dp))
                }
            }
        }
    }
}

/**
 * Watches a press without taking it (the Initial pass, nothing consumed — a
 * scroll that starts here still scrolls) so the control's click can ask
 * [ScreenshotViewerRules.TapSlop.accept] whether it was a tap. The press is
 * forgotten on the up's Final pass, after the click (if any) was decided, so a
 * later click with no finger behind it — TalkBack, a keyboard — always counts.
 */
internal fun Modifier.tapGuard(guard: ScreenshotViewerRules.TapSlop): Modifier = pointerInput(guard) {
    awaitEachGesture {
        val down = awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
        guard.down(down.position.x, down.position.y)
        while (true) {
            val event = awaitPointerEvent(PointerEventPass.Initial)
            val change = event.changes.firstOrNull { it.id == down.id }
            if (change == null) {
                guard.cancel()
                break
            }
            // A second finger is a pinch or a scroll, never a tap.
            if (event.changes.count { it.pressed } > 1) guard.cancel()
            guard.move(change.position.x, change.position.y)
            if (!change.pressed) break
        }
        awaitPointerEvent(PointerEventPass.Final)
        guard.end()
    }
}

/** A dashed hairline round the box, at the large radius — the "add something here" edge. */
private fun Modifier.dashedOutline(color: Color): Modifier = drawBehind {
    val stroke = VelaBorder.hairline.toPx()
    val radius = VelaRadius.lg.toPx()
    drawRoundRect(
        color = color,
        topLeft = Offset(stroke / 2, stroke / 2),
        size = Size(size.width - stroke, size.height - stroke),
        cornerRadius = CornerRadius(radius, radius),
        style = Stroke(width = stroke, pathEffect = PathEffect.dashPathEffect(floatArrayOf(6.dp.toPx(), 4.dp.toPx()))),
    )
}
