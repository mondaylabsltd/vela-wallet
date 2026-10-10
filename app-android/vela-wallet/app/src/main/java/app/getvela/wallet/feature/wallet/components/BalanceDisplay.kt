package app.getvela.wallet.feature.wallet.components

import androidx.compose.ui.platform.testTag
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.semantics.clearAndSetSemantics
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.wallet.BalanceModel
import app.getvela.wallet.feature.wallet.BalanceStateKind

/**
 * Hero balance (spec vocabulary #4): label line (总余额 · USD), amount with
 * de-emphasised decimals, and exactly one of normal / zero-live / loading /
 * hidden, then the status line's place, then the refresh control (issue 462)
 * under it. The refresh the control starts adds no status line
 * (WalletLive.balanceStatus), so nothing above it moves when it is tapped.
 *
 * **The line under the figure keeps its place from the first frame** (the
 * integration's note 26b). It comes and goes with the wallet's state — "Some
 * balances are still updating." on every cold start with a cached total,
 * "Can't reach Polygon right now" when a network drops, the empty wallet's
 * "live" line — and each arrival used to push the refresh control, Receive,
 * Send and the whole page down by its height (32 dp), and each departure
 * pulled them back up. One line of room is always there now; a status (or
 * the live line) is drawn in it, and nothing under the hero moves.
 */
@Composable
fun BalanceDisplay(
    model: BalanceModel,
    modifier: Modifier = Modifier,
    onToggleVisibility: () -> Unit = {},
    onStatusClick: () -> Unit = {},
    /** Issue 462: the refresh control's tap; `null` (the gallery) draws it inert. */
    onRefresh: (() -> Unit)? = null,
    /** Whether the refresh glyph turns — the screen's held spin; the model's own flag by default. */
    refreshSpinning: Boolean = model.refresh?.refreshing == true,
) {
    val colors = VelaTheme.colors
    Column(modifier = modifier) {
        Text(
            // The currency is named once it is known — the stored choice
            // while its rate is on the way, the committed one after; before
            // either, the label stands alone rather than say "USD" for a
            // person who never chose it.
            text = if (model.currency.isBlank()) model.label else "${model.label} · ${model.currency}",
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.medium,
            fontSize = VelaTextSize.sm,
        )
        Spacer(modifier = Modifier.height(VelaSpacing.sm))
        when (model.state) {
            BalanceStateKind.Normal, BalanceStateKind.ZeroLive -> HeroLine { AmountRow(model, onToggleVisibility) }
            BalanceStateKind.Loading -> HeroLine { SkeletonBalanceBlock() }
            BalanceStateKind.Hidden -> HeroLine { HiddenRow(model, onToggleVisibility) }
        }
        // The status line's place: one line of the row a status draws, kept
        // whether or not there is one. What is said in it is the status —
        // the most actionable thing — else the empty wallet's live line.
        Spacer(modifier = Modifier.height(VelaSpacing.md))
        Box(modifier = Modifier.testTag(BALANCE_STATUS_PLACE_TAG), contentAlignment = Alignment.CenterStart) {
            BalanceStatusRoom()
            val status = model.status
            when {
                status != null -> BalanceStatusLine(model = status, onClick = onStatusClick)
                model.state == BalanceStateKind.ZeroLive -> LiveIndicatorRow(model.liveText.orEmpty())
            }
        }
        model.refresh?.let { refresh ->
            Spacer(modifier = Modifier.height(VelaSpacing.sm))
            BalanceRefreshControl(model = refresh, spinning = refreshSpinning, onRefresh = onRefresh)
        }
    }
}

/** The status line's place under the figure — there in every state. */
const val BALANCE_STATUS_PLACE_TAG = "balance-status-place"

/**
 * The figure's own line, held in every state (PR 2 polish): the mask and the
 * skeleton stand in the height the figure takes, so hiding or showing the
 * balance moves nothing under it. The mask's row (its eye's 48 dp target over
 * a smaller mask) was 6 px off the figure's on the Xiaomi, and Receive, Send
 * and the activity moved by that much at every toggle.
 */
@Composable
private fun HeroLine(content: @Composable () -> Unit) {
    Box(contentAlignment = Alignment.CenterStart) {
        Text(
            text = "0",
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.bold,
            fontSize = VelaTextSize.xl5,
            modifier = Modifier.alpha(0f).clearAndSetSemantics {},
        )
        content()
    }
}

@Composable
private fun AmountRow(model: BalanceModel, onToggle: () -> Unit = {}) {
    val colors = VelaTheme.colors
    // Spec 048: the amount itself hides the figures (the web's BalanceDisplay button).
    Row(modifier = Modifier.clickable(onClick = onToggle)) {
        Text(
            text = model.integer.orEmpty(),
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.bold,
            fontSize = VelaTextSize.xl5,
            modifier = Modifier.alignByBaseline(),
        )
        model.decimals?.let { decimals ->
            Text(
                text = model.decimalMark + decimals,
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl3,
                modifier = Modifier.alignByBaseline(),
            )
        }
    }
}

@Composable
private fun LiveIndicatorRow(text: String) {
    val colors = VelaTheme.colors
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
    ) {
        Box(
            modifier = Modifier
                .size(WalletMetrics.liveDotSize)
                .alpha(rememberPulseAlpha())
                .background(colors.successBase, CircleShape),
        )
        Text(
            text = text,
            color = colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.medium,
            fontSize = VelaTextSize.sm,
        )
    }
}

@Composable
private fun HiddenRow(model: BalanceModel, onToggleVisibility: () -> Unit) {
    val colors = VelaTheme.colors
    Row(
        modifier = Modifier.clickable(onClick = onToggleVisibility),verticalAlignment = Alignment.CenterVertically) {
        Text(
            text = model.integer.orEmpty(),
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.bold,
            fontSize = VelaTextSize.xl4,
        )
        Spacer(modifier = Modifier.width(VelaSpacing.md))
        IconButton(onClick = onToggleVisibility) {
            Icon(
                imageVector = VelaIcons.EyeOff,
                // While hidden, the affordance reveals: announce "show balance".
                contentDescription = model.a11yShow,
                tint = colors.fgMuted,
                modifier = Modifier.size(VelaIconSize.lg),
            )
        }
    }
}
