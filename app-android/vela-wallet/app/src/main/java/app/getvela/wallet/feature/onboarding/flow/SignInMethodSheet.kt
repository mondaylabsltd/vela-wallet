package app.getvela.wallet.feature.onboarding.flow

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.unit.times
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaLeading
import app.getvela.wallet.core.designsystem.tokens.VelaOpacity
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.onboarding.core.KeyMethod

/**
 * The four ways to sign in — this device, a nearby device by scan, a hardware
 * security key, and the Trusted Signer — the SAME set creating a wallet offers
 * per key. A wallet that lives on a security key is reachable even when a
 * platform passkey is also present, which the plain system route would use
 * silently.
 *
 * The scan (`Hybrid`) is "sign in with your phone" over caBLE (spec 019): this
 * device shows a QR, the phone that holds the passkey scans it, and the ceremony
 * runs over the BLE/tunnel channel that phone opens. The Trusted Signer (spec 075)
 * is a page that asks the person to pick their key and returns an assertion over
 * a challenge it derived itself. The rows are the same shape as the create
 * picker's, deliberately.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SignInMethodSheet(
    onPick: (KeyMethod) -> Unit,
    onDismiss: () -> Unit,
) {
    val strings = LocalVelaStrings.current
    val colors = VelaTheme.colors

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(),
        containerColor = colors.bgRaised,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = VelaSpacing.xl2)
                .padding(bottom = VelaSpacing.xl2),
            verticalArrangement = Arrangement.spacedBy(VelaSpacing.sm),
        ) {
            Text(
                text = strings.t(I18nKeys.Login.HEADER),
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl2,
                modifier = Modifier.padding(bottom = VelaSpacing.md),
            )
            KeyMethod.entries.forEach { method ->
                // All four routes are live: platform (this device), scan (a
                // phone over caBLE), a security key, and the Trusted Signer.
                val available = true
                val (titleKey, bodyKey) = signInMethodCopy(method)
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable(enabled = available) { onPick(method) }
                        .padding(vertical = VelaSpacing.lg)
                        .alpha(if (available) 1f else VelaOpacity.disabled),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(modifier = Modifier.weight(1f)) {
                        Text(
                            text = strings.t(titleKey),
                            color = colors.fgBase,
                            fontFamily = VelaFontFamily,
                            fontWeight = VelaFontWeight.semibold,
                            fontSize = VelaTextSize.lg,
                        )
                        Text(
                            text = strings.t(
                                if (available) bodyKey else I18nKeys.Create.METHOD_HYBRID_UNAVAILABLE,
                            ),
                            color = colors.fgMuted,
                            fontFamily = VelaFontFamily,
                            fontSize = VelaTextSize.sm,
                            lineHeight = VelaLeading.normal * VelaTextSize.sm,
                        )
                    }
                    if (available) {
                        Icon(
                            imageVector = VelaIcons.ChevronRight,
                            contentDescription = null,
                            tint = colors.fgSubtle,
                            modifier = Modifier.size(VelaIconSize.lg),
                        )
                    }
                }
            }
        }
    }
}

/**
 * The create picker's copy, reused: the same four methods, the same words —
 * including the Trusted Signer (spec 075), which is a passkey route like the
 * other three and is offered wherever they are.
 */
internal fun signInMethodCopy(method: KeyMethod): Pair<String, String> = methodCopy(method)
