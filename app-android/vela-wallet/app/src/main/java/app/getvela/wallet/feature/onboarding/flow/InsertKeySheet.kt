package app.getvela.wallet.feature.onboarding.flow

import app.getvela.wallet.core.designsystem.components.VelaModalSheet
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.times
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.components.VelaSecondaryButton
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaLeading
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.LocalVelaStrings

/**
 * "Insert your security key" — shown while the USB ceremony polls for a key.
 *
 * A missing key is a waitable state, not an error: the ceremony keeps probing
 * and this sheet closes ITSELF the moment the key enumerates, so plugging it
 * in is the whole gesture. [otgLooksOff] adds the one hint that turns an
 * apparently dead key into a working one on phones whose OTG switch turns
 * itself off (the OnePlus 5T auto-disables it after ten minutes — the key is
 * fine, the port is asleep). No confirm button: the key IS the confirm.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun InsertKeySheet(
    otgLooksOff: Boolean,
    onCancel: () -> Unit,
) {
    val strings = LocalVelaStrings.current
    val colors = VelaTheme.colors

    VelaModalSheet(onDismissRequest = onCancel, containerColor = colors.bgRaised) {
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
                text = strings.t(I18nKeys.Flow.INSERT_KEY_TITLE),
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl2,
            )
            Spacer(modifier = Modifier.height(VelaSpacing.lg))
            Text(
                text = strings.t(I18nKeys.Flow.INSERT_KEY_BODY),
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                lineHeight = VelaLeading.normal * VelaTextSize.base,
            )
            if (otgLooksOff) {
                Spacer(modifier = Modifier.height(VelaSpacing.xl))
                Row(verticalAlignment = Alignment.Top) {
                    Icon(
                        imageVector = VelaIcons.TriangleAlert,
                        contentDescription = null,
                        tint = colors.warningBase,
                        modifier = Modifier.size(VelaIconSize.base),
                    )
                    Spacer(modifier = Modifier.size(VelaSpacing.md))
                    Text(
                        text = strings.t(I18nKeys.Flow.OTG_OFF_HINT),
                        color = colors.fgBase,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.base,
                        lineHeight = VelaLeading.normal * VelaTextSize.base,
                    )
                }
            }
            Spacer(modifier = Modifier.height(VelaSpacing.xl4))
            VelaSecondaryButton(
                text = strings.t(I18nKeys.Flow.CLOSE),
                onClick = onCancel,
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}
