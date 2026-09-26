package app.getvela.wallet.feature.settings.components

import android.graphics.Bitmap
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.MutableTransitionState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
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
import androidx.compose.runtime.key
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
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
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.diagnostics.BugReport
import app.getvela.wallet.core.diagnostics.ScreenshotTray
import app.getvela.wallet.feature.settings.FeedbackModel

/** One tile as the sheet draws it: its id, and its thumbnail once prepared. */
data class ScreenshotTileView(val id: Long, val thumbnail: Bitmap?, val ready: Boolean)

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
    /** False while the report is sending: nothing here may change (v3 B9). */
    enabled: Boolean = true,
) {
    val colors = VelaTheme.colors
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
                    style = LocalTextStyle.current.copy(fontFeatureSettings = "tnum"),
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
                                enabled = enabled,
                                onRemoved = { onRemove(item.id) },
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
 */
@Composable
private fun ScreenshotTile(tile: ScreenshotTileView, size: Dp, removeLabel: String, enabled: Boolean, onRemoved: () -> Unit) {
    val colors = VelaTheme.colors
    val shape = RoundedCornerShape(VelaRadius.lg)
    val shown = remember { MutableTransitionState(false).apply { targetState = true } }
    LaunchedEffect(shown.isIdle, shown.currentState, shown.targetState) {
        if (shown.isIdle && !shown.currentState && !shown.targetState) onRemoved()
    }
    AnimatedVisibility(
        visibleState = shown,
        enter = fadeIn(tween(TILE_MOTION_MS)) + scaleIn(tween(TILE_MOTION_MS), initialScale = 0.85f),
        exit = fadeOut(tween(TILE_MOTION_MS)) + scaleOut(tween(TILE_MOTION_MS), targetScale = 0.85f),
    ) {
        Box(modifier = Modifier.size(size)) {
            Box(
                modifier = Modifier
                    .matchParentSize()
                    .clip(shape)
                    .background(colors.bgSunken)
                    .border(VelaBorder.hairline, colors.borderBase, shape),
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
                    .clickable(enabled = enabled, role = Role.Button) { shown.targetState = false }
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
