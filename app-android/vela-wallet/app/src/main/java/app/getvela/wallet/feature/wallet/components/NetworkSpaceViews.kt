package app.getvela.wallet.feature.wallet.components

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import kotlinx.coroutines.launch
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.times
import app.getvela.wallet.core.designsystem.components.VelaModalSheet
import app.getvela.wallet.core.designsystem.components.VelaPrimaryButton
import app.getvela.wallet.core.designsystem.components.VelaSecondaryButton
import app.getvela.wallet.core.designsystem.components.sheetScrolls
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaLeading
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.settings.core.NetworkSpace

/**
 * The testnet space's bar (spec 105): across the top of every route, under
 * the status bar, "Testnet space · nothing here has real value". In the flow
 * of the layout, as the web's is — it pushes the page down rather than
 * covering its header (the host consumes the status bar's inset beneath it).
 * Tapping it offers the switch back.
 */
@Composable
fun NetworkSpaceBar(onClick: () -> Unit, modifier: Modifier = Modifier) {
    val colors = VelaTheme.colors
    val strings = LocalVelaStrings.current
    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(colors.warningSoft)
            .statusBarsPadding()
            .testTag(TESTNET_BAR_TAG),
    ) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .clickable(onClick = onClick)
                .padding(horizontal = VelaSpacing.lg, vertical = VelaSpacing.sm)
                .semantics { liveRegion = LiveRegionMode.Polite },
            contentAlignment = Alignment.Center,
        ) {
            Text(
                text = strings.t(I18nKeys.NetworkSpace.BAR),
                color = colors.warningBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.sm,
                textAlign = TextAlign.Center,
            )
        }
        HorizontalDivider(thickness = VelaBorder.hairline, color = colors.warningBorder)
    }
}

/** The testnet bar (spec 105). */
const val TESTNET_BAR_TAG = "testnet-bar"

/**
 * The switch prompt (spec 105), opened by the home's space label, the testnet
 * bar or Settings → Networks: what the other space is, then the switch. The
 * answer is the caller's to carry — through the `set_network_space` intent,
 * the road an agent's request takes. Going back to the mainnet space is the
 * same prompt with the mainnet's own line: nothing to warn about.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun NetworkSpaceSwitchSheet(
    /** The space the switch goes TO. */
    to: NetworkSpace,
    onAnswer: (switching: Boolean) -> Unit,
) {
    val strings = LocalVelaStrings.current
    val colors = VelaTheme.colors
    // The sheet slides away FIRST and the answer follows: while it is going
    // its window is still on top and takes a quick second tap, which used to
    // fall through to the screen beneath as the space changed under it. One
    // answer, whichever comes first — the confirm, Cancel, a swipe, Back.
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val scope = rememberCoroutineScope()
    var answered by remember { mutableStateOf(false) }
    val answer = { switching: Boolean ->
        if (!answered) {
            answered = true
            scope.launch { sheetState.hide() }.invokeOnCompletion { onAnswer(switching) }
        }
    }
    VelaModalSheet(onDismissRequest = { answer(false) }, sheetState = sheetState, containerColor = colors.bgRaised) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .sheetScrolls()
                .padding(start = VelaSizing.screenPaddingX, end = VelaSizing.screenPaddingX, bottom = VelaSpacing.xl4),
        ) {
            Text(
                text = strings.t(I18nKeys.NetworkSpace.TITLE),
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl2,
            )
            Spacer(modifier = Modifier.height(VelaSpacing.lg))
            Text(
                text = strings.t(if (to.testnet) I18nKeys.NetworkSpace.TESTNET_HINT else I18nKeys.NetworkSpace.MAINNET_HINT),
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                lineHeight = VelaLeading.normal * VelaTextSize.base,
            )
            Spacer(modifier = Modifier.height(VelaSpacing.xl4))
            VelaPrimaryButton(
                text = strings.t(if (to.testnet) I18nKeys.NetworkSpace.TO_TESTNET else I18nKeys.NetworkSpace.TO_MAINNET),
                onClick = { answer(true) },
                modifier = Modifier.fillMaxWidth().testTag(SPACE_SWITCH_CONFIRM_TAG),
            )
            Spacer(modifier = Modifier.height(VelaSpacing.lg))
            VelaSecondaryButton(
                text = strings.t(I18nKeys.Common.CANCEL),
                onClick = { answer(false) },
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}

/** The switch prompt's confirm (spec 105). */
const val SPACE_SWITCH_CONFIRM_TAG = "network-space-switch"
