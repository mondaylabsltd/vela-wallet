package app.getvela.wallet.feature.onboarding.flow

import app.getvela.wallet.core.designsystem.components.VelaModalSheet
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.times
import app.getvela.wallet.core.designsystem.components.VelaPrimaryButton
import app.getvela.wallet.core.designsystem.components.VelaSecondaryButton
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaLeading
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.LocalVelaStrings

/**
 * "Turn on Location" — the explanation BEFORE the jump to system settings.
 *
 * API ≤30 withholds BLE scan results unless location services are on, so the
 * scan method genuinely needs the toggle — but an app that teleports someone
 * into system settings unannounced reads as broken (founder feedback,
 * 2026-08-28, OnePlus 5T). This sheet says why first; agreeing performs the
 * jump, and the ceremony resumes the moment they come back. The body carries
 * the load-bearing reassurance: Vela never reads the position.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LocationAskSheet(
    onAnswer: (Boolean) -> Unit,
) {
    val strings = LocalVelaStrings.current
    val colors = VelaTheme.colors

    VelaModalSheet(onDismissRequest = { onAnswer(false) }, containerColor = colors.bgRaised) {
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
                text = strings.t(I18nKeys.Flow.LOCATION_NEEDED_TITLE),
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl2,
            )
            Spacer(modifier = Modifier.height(VelaSpacing.lg))
            Text(
                text = strings.t(I18nKeys.Flow.LOCATION_NEEDED_BODY),
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                lineHeight = VelaLeading.normal * VelaTextSize.base,
            )
            Spacer(modifier = Modifier.height(VelaSpacing.xl4))
            VelaPrimaryButton(
                text = strings.t(I18nKeys.Flow.OPEN_LOCATION_SETTINGS),
                onClick = { onAnswer(true) },
                modifier = Modifier.fillMaxWidth(),
            )
            Spacer(modifier = Modifier.height(VelaSpacing.lg))
            VelaSecondaryButton(
                text = strings.t(I18nKeys.Flow.CLOSE),
                onClick = { onAnswer(false) },
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}
