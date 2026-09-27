package app.getvela.wallet.feature.onboarding

import app.getvela.wallet.core.designsystem.components.VelaModalSheet
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.RadioButton
import androidx.compose.material3.RadioButtonDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import app.getvela.wallet.core.data.ThemePreference
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaLetterSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.LocalVelaStrings

/** Light / Dark / Auto picker (US3); corpus keys onboarding.settings.*. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ThemeSettingsSheet(
    current: ThemePreference,
    onSelect: (ThemePreference) -> Unit,
    onDismiss: () -> Unit,
) {
    val strings = LocalVelaStrings.current
    val colors = VelaTheme.colors
    VelaModalSheet(
        onDismissRequest = onDismiss,
        containerColor = colors.bgRaised,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(
                    start = VelaSizing.screenPaddingX,
                    end = VelaSizing.screenPaddingX,
                    bottom = VelaSpacing.xl4,
                ),
        ) {
            Text(
                text = strings.t(I18nKeys.Settings.TITLE),
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl2,
            )
            Spacer(modifier = Modifier.height(VelaSpacing.xl))
            Text(
                text = strings.t(I18nKeys.Settings.SECTION_APPEARANCE),
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.sm,
                letterSpacing = VelaLetterSpacing.sectionLabel,
            )
            Spacer(modifier = Modifier.height(VelaSpacing.md))
            ThemeOptionRow(
                label = strings.t(I18nKeys.Settings.THEME_LIGHT),
                selected = current == ThemePreference.Light,
                onClick = { onSelect(ThemePreference.Light) },
            )
            ThemeOptionRow(
                label = strings.t(I18nKeys.Settings.THEME_DARK),
                selected = current == ThemePreference.Dark,
                onClick = { onSelect(ThemePreference.Dark) },
            )
            ThemeOptionRow(
                label = strings.t(I18nKeys.Settings.THEME_AUTO),
                selected = current == ThemePreference.Auto,
                onClick = { onSelect(ThemePreference.Auto) },
            )
        }
    }
}

@Composable
private fun ThemeOptionRow(
    label: String,
    selected: Boolean,
    onClick: () -> Unit,
) {
    val colors = VelaTheme.colors
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = VelaSizing.hitTarget)
            // selectable (not clickable): publishes Role.RadioButton + selected state
            // so TalkBack announces which theme is active (FR-010).
            .selectable(
                selected = selected,
                role = Role.RadioButton,
                onClick = onClick,
            ),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        RadioButton(
            selected = selected,
            onClick = null,
            colors = RadioButtonDefaults.colors(
                selectedColor = colors.accentBase,
                unselectedColor = colors.fgSubtle,
            ),
        )
        Spacer(modifier = Modifier.width(VelaSpacing.lg))
        Text(
            text = label,
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.regular,
            fontSize = VelaTextSize.lg,
        )
    }
}
