package app.getvela.wallet.feature.flows.gallery

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.flows.FlowFixtures
import app.getvela.wallet.feature.flows.FlowHost
import app.getvela.wallet.feature.flows.FlowState

/**
 * Wallet-flow preview gallery (spec 021 FR-004): all thirty states, each
 * reachable in ≤2 interactions, driven by fixtures alone and fully offline.
 * Reached only via the `vela.startDestination` intent extra — production
 * navigation never links here.
 *
 * Chip labels are state codes (data, not translations); the theme chip reuses
 * the onboarding theme labels, so nothing user-visible bypasses the corpus.
 *
 * The gallery hosts the same [FlowHost] the wallet route does, so a state seen
 * here is the state that ships — there is no second render path to drift.
 */
@Composable
fun FlowGalleryScreen(systemDarkTheme: Boolean, initialState: String? = null) {
    var dark by rememberSaveable { mutableStateOf(systemDarkTheme) }
    // An unknown or absent name is R1, not a crash: the extra is a convenience
    // for a device walkthrough, and a typo should not take the gallery down.
    var state by rememberSaveable {
        mutableStateOf(
            FlowState.entries.firstOrNull { it.name.equals(initialState?.trim(), true) }
                ?: FlowState.R1,
        )
    }

    VelaTheme(darkTheme = dark) {
        val colors = VelaTheme.colors
        val strings = LocalVelaStrings.current
        Column(
            modifier = Modifier
                .fillMaxSize()
                .background(colors.bgBase)
                .statusBarsPadding(),
        ) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .horizontalScroll(rememberScrollState())
                    .padding(horizontal = VelaSpacing.xl, vertical = VelaSpacing.md),
                horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                FlowState.entries.forEach { candidate ->
                    GalleryChip(
                        label = candidate.name,
                        selected = candidate == state,
                        onClick = { state = candidate },
                    )
                }
                GalleryChip(
                    label = strings.t(
                        if (dark) I18nKeys.Settings.THEME_LIGHT else I18nKeys.Settings.THEME_DARK,
                    ),
                    selected = false,
                    onClick = { dark = !dark },
                )
            }
            Box(modifier = Modifier.weight(1f)) {
                val model = remember(state, strings) { FlowFixtures.build(state, strings) }
                // Navigation is a sink here: the gallery swaps fixture states
                // through its chips, and a screen that pushed one would take
                // the chip row's selection out of sync with what is drawn.
                FlowHost(model = model)
                // A board's alert, through the dialog the live send draws (PR 2 note 13).
                FlowFixtures.alert(state)?.let { kind ->
                    app.getvela.wallet.navigation.SendAlertDialog(kind = kind, strings = strings, chainName = FlowFixtures.BOARD_CHAIN, onDismiss = {})
                }
            }
        }
    }
}

@Composable
private fun GalleryChip(label: String, selected: Boolean, onClick: () -> Unit) {
    val colors = VelaTheme.colors
    Box(
        modifier = Modifier
            .clip(RoundedCornerShape(VelaRadius.md))
            .background(if (selected) colors.fgBase else colors.bgRaised)
            .clickable(onClick = onClick)
            .padding(horizontal = VelaSpacing.lg, vertical = VelaSpacing.md),
    ) {
        Text(
            text = label,
            color = if (selected) colors.bgBase else colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontWeight = if (selected) VelaFontWeight.semibold else VelaFontWeight.medium,
            fontSize = VelaTextSize.sm,
            maxLines = 1,
        )
    }
}
