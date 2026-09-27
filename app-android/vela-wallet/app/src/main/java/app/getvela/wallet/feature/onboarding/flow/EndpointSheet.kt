package app.getvela.wallet.feature.onboarding.flow

import app.getvela.wallet.core.designsystem.components.VelaModalSheet
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.times
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.components.VelaPrimaryButton
import app.getvela.wallet.core.designsystem.components.VelaSecondaryButton
import app.getvela.wallet.core.designsystem.components.VelaTextField
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
 * Point this wallet at a different passkey index.
 *
 * Opened automatically when the core reports `endpoint_unreachable`, and
 * reachable by hand from the retry screen. **Sign-in is still permitted while it
 * is open** (data-model §4): an unreachable index is not a locked door, and a
 * person whose wallet is already on this device can often get in regardless.
 *
 * The warning is not decoration. A wrong endpoint does not corrupt anything, but
 * it makes every key lookup answer "not found" — which presents as a wallet that
 * has vanished, and is the single most alarming wrong answer this app can give.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun EndpointSheet(
    current: String,
    defaultUrl: String,
    onSave: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    val strings = LocalVelaStrings.current
    val colors = VelaTheme.colors
    var draft by remember(current) { mutableStateOf(current) }

    VelaModalSheet(onDismissRequest = onDismiss, containerColor = colors.bgRaised) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .imePadding()
                .padding(
                    start = VelaSizing.screenPaddingX,
                    end = VelaSizing.screenPaddingX,
                    bottom = VelaSpacing.xl4,
                ),
        ) {
            Text(
                text = strings.t(I18nKeys.Settings.SECTION_PASSKEY_INDEX),
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl2,
            )
            Spacer(modifier = Modifier.height(VelaSpacing.md))
            Text(
                text = strings.t(I18nKeys.Settings.PASSKEY_HINT),
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                lineHeight = VelaLeading.normal * VelaTextSize.base,
            )

            Spacer(modifier = Modifier.height(VelaSpacing.xl3))
            VelaTextField(
                value = draft,
                onValueChange = { draft = it },
                label = strings.t(I18nKeys.Settings.ENDPOINT_URL_LABEL),
                placeholder = defaultUrl,
            )

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
                    text = strings.t(I18nKeys.Settings.WARNING_TEXT),
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.sm,
                    lineHeight = VelaLeading.normal * VelaTextSize.sm,
                )
            }

            Spacer(modifier = Modifier.height(VelaSpacing.xl4))
            VelaPrimaryButton(
                text = strings.t(I18nKeys.Flow.RETRY),
                onClick = { onSave(draft) },
                modifier = Modifier.fillMaxWidth(),
            )
            Spacer(modifier = Modifier.height(VelaSpacing.lg))
            VelaSecondaryButton(
                text = strings.t(I18nKeys.Settings.RESET_TO_DEFAULT),
                onClick = { onSave(defaultUrl) },
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}
