package app.getvela.wallet.feature.wallet.components

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.text.style.TextOverflow
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.wallet.BalanceRefreshModel

/**
 * The hero's "↻ Updated 2m" (issue 462) — the same control on all four
 * shells, under the total and its status line.
 *
 * Tapping it reads every chain again (`RefreshRequested{force, pull}` plus
 * the activity tick — the caller's [onRefresh]). While that is out
 * ([spinning], which the screen holds for at least 650 ms) the glyph turns,
 * the words read "Updating…", and a second tap does nothing.
 *
 * **Nothing moves.** Both labels are laid out in the same box, the one not
 * showing invisible, so the box is the wider one's width in both states,
 * and its height never changes: the control does not slide out from under
 * the finger that tapped it. Quiet ink like the status line — a figure that
 * is fresh is the normal case.
 *
 * [onRefresh] `null` (the gallery) draws the control and accepts no tap.
 */
@Composable
fun BalanceRefreshControl(
    model: BalanceRefreshModel,
    spinning: Boolean,
    onRefresh: (() -> Unit)?,
    modifier: Modifier = Modifier,
) {
    val colors = VelaTheme.colors
    val ink = colors.fgSubtle
    Row(
        modifier = modifier
            .testTag(TEST_TAG)
            .heightIn(min = VelaSizing.controlSm)
            .clip(RoundedCornerShape(VelaRadius.md))
            .clickable(enabled = onRefresh != null && !spinning, role = Role.Button) { onRefresh?.invoke() }
            .padding(end = VelaSpacing.sm),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.sm),
    ) {
        if (spinning) {
            TurningGlyph(tint = ink)
        } else {
            Icon(
                imageVector = VelaIcons.RefreshCw,
                contentDescription = null,
                tint = ink,
                modifier = Modifier.size(VelaIconSize.sm),
            )
        }
        Box {
            Label(text = model.updated.orEmpty(), shown = !spinning)
            Label(text = model.updating, shown = spinning)
        }
    }
}

/**
 * The turning glyph — composed only while turning, so an idle home runs no
 * frame clock. The angle is read in the layer, not in composition.
 */
@Composable
private fun TurningGlyph(tint: androidx.compose.ui.graphics.Color) {
    val turn = rememberInfiniteTransition(label = "balance-refresh")
    val angle by turn.animateFloat(
        initialValue = 0f,
        targetValue = 360f,
        animationSpec = infiniteRepeatable(tween(durationMillis = TURN_MS, easing = LinearEasing)),
        label = "balance-refresh-angle",
    )
    Icon(
        imageVector = VelaIcons.RefreshCw,
        contentDescription = null,
        tint = tint,
        modifier = Modifier
            .size(VelaIconSize.sm)
            .graphicsLayer { rotationZ = angle },
    )
}

@Composable
private fun Label(text: String, shown: Boolean) {
    Text(
        text = text,
        color = VelaTheme.colors.fgSubtle,
        fontFamily = VelaFontFamily,
        fontWeight = VelaFontWeight.medium,
        fontSize = VelaTextSize.sm,
        maxLines = 1,
        overflow = TextOverflow.Ellipsis,
        // The one not showing keeps its room and says nothing to TalkBack.
        modifier = if (shown) Modifier else Modifier.alpha(0f).clearAndSetSemantics {},
    )
}

/** The control's test hook — the desktop's element id for the same control. */
const val BALANCE_REFRESH_TEST_TAG = "balance-refresh"
private const val TEST_TAG = BALANCE_REFRESH_TEST_TAG

/** One calm turn — the fee card's refresh turns at the same pace. */
private const val TURN_MS = 1200
