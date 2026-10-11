package app.getvela.wallet.feature.wallet.components

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
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
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaMotion
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.wallet.BalanceRefreshModel

/**
 * The hero's refresh control (issue 462) — the same control on all four
 * shells: the glyph at the trailing edge of the line under the total.
 *
 * Tapping it reads every chain again (`RefreshRequested{force, pull}` plus
 * the activity tick — the caller's [onRefresh]). While that is out
 * ([spinning], which the screen holds for at least 650 ms) the glyph turns
 * and a second tap does nothing.
 *
 * **One line with the status, and it never moves** (the 2026-10-11 phone
 * walk, item 4). The control used to be a row of its own UNDER the status
 * line's place: on a cold start a lone ⟳ sat on its own line beneath
 * 「● 正在检查…」, and once the read settled an empty line stood between the
 * figure and 「⟳ 上次更新 · 刚刚」. It is the glyph alone now, in a box of one
 * size at the END of that line ([BalanceDisplay] composes the two), and its
 * words — "Updated 2m", "Updating…" — are said in the line's own place when
 * nothing more pressing is ([BalanceRefreshStamp]). So the hero is one line
 * shorter, the same height in every state, and the glyph is where the finger
 * left it whatever the line says.
 *
 * **Always named.** Its words are its name whether or not they are on
 * screen: "Updating…" while it turns, "Updated 2m" once a read has settled,
 * and before any has, [BalanceRefreshModel.idleLabel] ("Refresh balance") —
 * never a bare "Button", and never "Updating…" over a control at rest.
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
    val ink = VelaTheme.colors.fgSubtle
    val name = if (spinning) model.updating else model.updated ?: model.idleLabel
    Box(
        modifier = modifier
            .testTag(TEST_TAG)
            // One box in every state: a finger's width, the line's height —
            // the glyph at its end, on the screen's margin.
            .size(width = VelaSizing.hitTarget, height = VelaSizing.controlSm)
            .clip(RoundedCornerShape(VelaRadius.md))
            .clickable(enabled = onRefresh != null && !spinning, role = Role.Button) { onRefresh?.invoke() }
            .semantics { contentDescription = name },
        contentAlignment = Alignment.CenterEnd,
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
    }
}

/**
 * The refresh control's words, in the status line's place: "Updated 2m", or
 * "Updating…" while a refresh the person asked for is out. Drawn only when
 * the line has nothing more pressing to say ([BalanceDisplay]) — a status, a
 * first read, the empty wallet's live line each take the place instead.
 *
 * Both are laid out in the same box, the one not showing invisible, so the
 * words swap without the line changing. A tap on them refreshes, as a tap on
 * the glyph does. Unsaid to TalkBack: the control at the line's end carries
 * the same words as its name, and would say them twice.
 */
@Composable
fun BalanceRefreshStamp(
    model: BalanceRefreshModel,
    spinning: Boolean,
    onRefresh: (() -> Unit)?,
    modifier: Modifier = Modifier,
) {
    Box(
        modifier = modifier
            .testTag(BALANCE_REFRESH_STAMP_TAG)
            .clickable(enabled = onRefresh != null && !spinning) { onRefresh?.invoke() }
            .clearAndSetSemantics { },
    ) {
        Label(text = model.updated.orEmpty(), shown = !spinning)
        Label(text = model.updating, shown = spinning)
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
        // The one not showing keeps its room.
        modifier = if (shown) Modifier else Modifier.alpha(0f),
    )
}

/** The control's test hook — the desktop's element id for the same control. */
const val BALANCE_REFRESH_TEST_TAG = "balance-refresh"

/** The control's words where the line says them ([BalanceRefreshStamp]). */
const val BALANCE_REFRESH_STAMP_TAG = "balance-refresh-stamp"
private const val TEST_TAG = BALANCE_REFRESH_TEST_TAG

/**
 * One turn at the web/iOS pace (motion slow × 2 = 800 ms; the desktop's
 * spinner turns at it too), so a press reads the same on every shell. The fee
 * card's refresh turns at the same pace.
 */
private const val TURN_MS = VelaMotion.durationSlow * 2
