package app.getvela.wallet.feature.flows.components

import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.text.style.TextAlign
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.flows.ScanModel
import app.getvela.wallet.feature.flows.ScanTool

/**
 * The scanner (spec 021 component 27) — S1, full screen.
 *
 * The camera feed is out of scope here, so the frame holds an inert surface.
 * What IS in scope is the frame itself: four corner brackets and nothing else,
 * so the thing being aimed at stays visible. A closed rectangle around a QR
 * code competes with the code's own quiet zone.
 */
@Composable
fun ScanSurface(
    model: ScanModel,
    modifier: Modifier = Modifier,
    onClose: () -> Unit = {},
    onTool: (ScanTool) -> Unit = {},
    /** Spec 046: the camera behind the brackets, and one status line (permission, no code found). */
    preview: (@Composable () -> Unit)? = null,
    status: String? = null,
    statusAction: String? = null,
    onStatusAction: () -> Unit = {},
) {
    val colors = VelaTheme.colors
    Column(
        modifier = modifier
            .fillMaxSize()
            .background(colors.bgBase)
            .statusBarsPadding(),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = VelaSizing.screenPaddingX, vertical = VelaSpacing.xl),
            horizontalArrangement = Arrangement.End,
        ) {
            Box(
                modifier = Modifier
                    .size(VelaSizing.controlSm)
                    .background(colors.bgRaised, CircleShape)
                    .clickable(onClick = onClose),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    imageVector = VelaIcons.Close,
                    contentDescription = model.closeLabel,
                    tint = colors.fgBase,
                    modifier = Modifier.size(VelaIconSize.lg),
                )
            }
        }

        Spacer(modifier = Modifier.weight(1f))
        ScanFrame(modifier = Modifier.align(Alignment.CenterHorizontally), preview = preview)
        Text(
            text = model.hint,
            color = colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
            textAlign = TextAlign.Center,
            modifier = Modifier
                .fillMaxWidth()
                .padding(vertical = VelaSpacing.xl),
        )
        status?.let {
            Text(
                text = it,
                color = colors.warningBase,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
                textAlign = TextAlign.Center,
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = VelaSizing.screenPaddingX)
                    .semantics { contentDescription = "scan-status" },
            )
            statusAction?.let { action ->
                Text(
                    text = action,
                    color = colors.accentBase,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.sm,
                    textAlign = TextAlign.Center,
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable(onClick = onStatusAction)
                        .padding(vertical = VelaSpacing.md),
                )
            }
        }
        Spacer(modifier = Modifier.weight(1f))

        Row(
            modifier = Modifier
                .fillMaxWidth()
                .navigationBarsPadding()
                .padding(bottom = VelaSpacing.xl5),
            horizontalArrangement = Arrangement.Center,
        ) {
            model.tools.forEach { tool ->
                Column(
                    modifier = Modifier
                        .padding(horizontal = VelaSpacing.xl3)
                        .clickable { onTool(tool.id) },
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Box(
                        modifier = Modifier
                            .size(VelaSizing.controlMd)
                            .background(colors.bgRaised, CircleShape),
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(
                            imageVector = when (tool.id) {
                                ScanTool.Gallery -> VelaIcons.Image
                                ScanTool.Torch -> VelaIcons.Zap
                                ScanTool.Flip -> VelaIcons.RotateCcw
                            },
                            contentDescription = null,
                            tint = colors.fgBase,
                            modifier = Modifier.size(VelaIconSize.md),
                        )
                    }
                    Spacer(modifier = Modifier.height(VelaSpacing.sm))
                    Text(
                        text = tool.label,
                        color = colors.fgMuted,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.sm,
                    )
                }
            }
        }
    }
}

/** Corner brackets over an inert feed placeholder. */
@Composable
private fun ScanFrame(modifier: Modifier = Modifier, preview: (@Composable () -> Unit)? = null) {
    val colors = VelaTheme.colors
    BoxWithConstraints(modifier = modifier.fillMaxWidth(fraction = 0.68f)) {
        val bracket = VelaIconSize.xl2
        preview?.let { camera ->
            Box(
                modifier = Modifier
                    .aspectRatio(1f)
                    .clip(RoundedCornerShape(VelaRadius.md)),
            ) { camera() }
        }
        Box(
            modifier = Modifier
                .aspectRatio(1f)
                .then(if (preview == null) Modifier.background(colors.bgSunken, RoundedCornerShape(VelaRadius.md)) else Modifier)
                .drawBehind {
                    val stroke = VelaBorder.emphasis.toPx() * 2
                    val arm = bracket.toPx()
                    val w = size.width
                    val h = size.height
                    val ink = colors.fgBase
                    fun line(from: Offset, to: Offset) =
                        drawLine(ink, from, to, strokeWidth = stroke, cap = StrokeCap.Round)
                    // top-left
                    line(Offset(0f, arm), Offset(0f, 0f))
                    line(Offset(0f, 0f), Offset(arm, 0f))
                    // top-right
                    line(Offset(w - arm, 0f), Offset(w, 0f))
                    line(Offset(w, 0f), Offset(w, arm))
                    // bottom-left
                    line(Offset(0f, h - arm), Offset(0f, h))
                    line(Offset(0f, h), Offset(arm, h))
                    // bottom-right
                    line(Offset(w - arm, h), Offset(w, h))
                    line(Offset(w, h), Offset(w, h - arm))
                },
        )
    }
}
