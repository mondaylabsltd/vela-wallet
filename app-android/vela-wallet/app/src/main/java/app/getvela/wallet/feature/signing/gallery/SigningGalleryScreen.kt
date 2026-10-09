package app.getvela.wallet.feature.signing.gallery

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
import app.getvela.wallet.feature.signing.SigningFixtures
import app.getvela.wallet.feature.signing.SigningScreenState
import app.getvela.wallet.feature.signing.SigningSheetContent

/**
 * The signing sheet's state gallery: every drawn scenario (spec 022's CS
 * canon, and CS36 — the wallet's own backup), fixtures only and offline,
 * through the SAME [SigningSheetContent] the live sheet mounts, so a state
 * seen here is the one that ships. Reached only by the debug-build
 * `vela.startDestination` extra, with `vela.signingState` to open on one:
 *
 * ```
 * adb shell am start -n app.getvela.wallet/.MainActivity \
 *   --es vela.startDestination signing-gallery --es vela.signingState CS36
 * ```
 *
 * The ✕ and the confirm are drawn, as on the live sheet, and do nothing.
 * Chip labels are state codes (data, not translations).
 */
@Composable
fun SigningGalleryScreen(systemDarkTheme: Boolean, initialState: String? = null) {
    var dark by rememberSaveable { mutableStateOf(systemDarkTheme) }
    // An unknown or absent name is CS1, not a crash: the extra is a
    // convenience for a device pass, and a typo should not take it down.
    var state by rememberSaveable {
        mutableStateOf(
            SigningScreenState.entries.firstOrNull { it.name.equals(initialState?.trim(), true) }
                ?: SigningScreenState.CS1,
        )
    }

    VelaTheme(darkTheme = dark) {
        val colors = VelaTheme.colors
        val strings = LocalVelaStrings.current
        Column(
            modifier = Modifier
                .fillMaxSize()
                .background(colors.bgRaised)
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
                SigningScreenState.entries.forEach { candidate ->
                    GalleryChip(label = candidate.name, selected = candidate == state, onClick = { state = candidate })
                }
                GalleryChip(
                    label = strings.t(if (dark) I18nKeys.Settings.THEME_LIGHT else I18nKeys.Settings.THEME_DARK),
                    selected = false,
                    onClick = { dark = !dark },
                )
            }
            Box(modifier = Modifier.weight(1f).padding(top = VelaSpacing.lg)) {
                // CS40/CS41: the hand-off card a send raises on its own;
                // CS43/CS44: a key ceremony waiting on its page.
                val standalone = remember(state, strings) { SigningFixtures.standaloneHandoff(state, strings) }
                val ceremony = remember(state, strings) { SigningFixtures.standaloneCeremony(state, strings) }
                if (standalone != null) {
                    app.getvela.wallet.feature.signing.HandoffSheetContent(
                        standalone, onOpen = {}, onCancel = {}, cancel = strings.t("common.cancel"),
                    )
                } else if (ceremony != null) {
                    app.getvela.wallet.feature.signing.components.TrustedSignerWaiting(
                        ceremony, onReopen = {}, onCancel = {},
                        modifier = Modifier.padding(horizontal = app.getvela.wallet.core.designsystem.tokens.VelaSizing.screenPaddingX),
                    )
                } else {
                    val model = remember(state, strings) { SigningFixtures.build(state, strings) }
                    SigningSheetContent(model = model, onConfirm = {}, onClose = {})
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
            .background(if (selected) colors.fgBase else colors.bgSunken)
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
