package app.getvela.wallet.feature.wallet.gallery

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
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
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.core.identicon.IdenticonImage
import app.getvela.wallet.feature.wallet.WalletFixtures
import app.getvela.wallet.feature.wallet.WalletScreen
import app.getvela.wallet.feature.wallet.WalletScreenState

/**
 * Preview gallery (spec 015 FR-004, research D4): every mobile H-state plus a
 * component board and the identicon board, each reachable in ≤2 interactions.
 * Reached only via the `vela.startDestination` intent extra — production
 * navigation never links here.
 *
 * Chip labels are state codes / component names (data, not translations); the
 * theme-toggle chip reuses the onboarding theme labels so nothing user-visible
 * bypasses the corpus.
 */
private enum class GalleryEntry(val label: String) {
    H1("H1"),
    H1S("H1s"),
    H2("H2"),
    H3("H3"),
    H4("H4"),
    H5("H5"),
    H6("H6"),
    H7("H7"),
    H7X("H7x 1.35×"),
    H8("H8"),
    H9("H9"),
    H10("H10"),
    H11("H11"),
    H12("H12"),
    Components("Components"),
    Identicons("Identicon"),
    ;

    val screenState: WalletScreenState?
        get() = when (this) {
            H1 -> WalletScreenState.H1
            H1S -> WalletScreenState.H1S
            H2 -> WalletScreenState.H2
            H3 -> WalletScreenState.H3
            H4 -> WalletScreenState.H4
            H5 -> WalletScreenState.H5
            H6 -> WalletScreenState.H6
            H7 -> WalletScreenState.H7
            H7X -> WalletScreenState.H7X
            H8 -> WalletScreenState.H8
            H9 -> WalletScreenState.H9
            H10 -> WalletScreenState.H10
            H11 -> WalletScreenState.H11
            H12 -> WalletScreenState.H12
            Components, Identicons -> null
        }
}

@Composable
fun GalleryScreen(systemDarkTheme: Boolean) {
    var dark by rememberSaveable { mutableStateOf(systemDarkTheme) }
    var entry by rememberSaveable { mutableStateOf(GalleryEntry.H1) }

    // Local VelaTheme override: the gallery flips appearance without touching
    // the persisted preference (research D4 appearance toggle).
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
                GalleryEntry.entries.forEach { candidate ->
                    GalleryChip(
                        label = candidate.label,
                        selected = candidate == entry,
                        onClick = { entry = candidate },
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
                when (entry) {
                    GalleryEntry.Components -> ComponentBoard()
                    GalleryEntry.Identicons -> IdenticonBoard()
                    else -> {
                        val state = entry.screenState ?: WalletScreenState.H1
                        val model = remember(strings, state) {
                            WalletFixtures.buildMobileState(state, strings)
                        }
                        WalletScreen(
                            model = model,
                            onSheetDismiss = { entry = GalleryEntry.H1 },
                        )
                    }
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
            .clip(RoundedCornerShape(VelaRadius.full))
            .background(if (selected) colors.accentSoft else colors.bgRaised)
            .clickable(onClick = onClick)
            .padding(horizontal = VelaSpacing.lg, vertical = VelaSpacing.md),
    ) {
        Text(
            text = label,
            color = if (selected) colors.accentBase else colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.medium,
            fontSize = VelaTextSize.base,
            maxLines = 1,
        )
    }
}

/**
 * Identicon board (spec US3): the fixed seed set rendered through the shared
 * vela-core rasterizer, for cross-platform eyeball parity, plus a locale note
 * (resolved corpus string + active system tag) proving live i18n.
 */
@Composable
private fun IdenticonBoard() {
    val colors = VelaTheme.colors
    val strings = LocalVelaStrings.current
    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = VelaSizing.screenPaddingX, vertical = VelaSpacing.xl),
        verticalArrangement = Arrangement.spacedBy(VelaSpacing.xl),
    ) {
        WalletFixtures.IDENTICON_BOARD_SEEDS.forEach { seed ->
            Row(verticalAlignment = Alignment.CenterVertically) {
                IdenticonImage(seed = seed, size = VelaSizing.emptyStateCircle)
                Spacer(modifier = Modifier.width(VelaSpacing.xl))
                Text(
                    // Seeds are fixture data; the empty seed renders the shared placeholder.
                    text = seed.ifEmpty { "«empty» → placeholder" },
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.regular,
                    fontSize = VelaTextSize.sm,
                    modifier = Modifier.weight(1f),
                )
            }
        }
        Spacer(modifier = Modifier.height(VelaSpacing.md))
        // Locale note: system tag (data) + one corpus-resolved string so a
        // locale switch is visible from this board.
        Text(
            text = "i18n · " + java.util.Locale.getDefault().toLanguageTag() +
                " · " + strings.t(I18nKeys.Wallet.ALL_NETWORKS),
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.regular,
            fontSize = VelaTextSize.sm,
        )
    }
}
