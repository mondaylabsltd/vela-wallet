package app.getvela.wallet.feature.onboarding.flow

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.times
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.components.VelaDangerButton
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
 * The way back out of a signed-in wallet.
 *
 * **This exists because wiring a route guard without wiring its exit produces an
 * app you cannot leave.** Spec 019's Phase 5 hit exactly this on desktop —
 * signing in worked and then stranded the person, because `allowed_route` sends
 * a signed-in client to the wallet and nothing anywhere sent it back. The
 * desktop got a sidebar row; the two phones shipped without one and hit the same
 * wall on the founder's device within a minute of the first successful create.
 *
 * Two things here are the core's, not this sheet's:
 *
 * - **The warning.** `pendingUploadWarning` is the session machine's answer after
 *   it asks storage whether any public key is still unconfirmed — not this
 *   screen's guess. The dialog does not open until the machine has one, which is
 *   why the caller renders on `signOut != null` rather than on a local flag.
 * - **What sign-out clears.** The account list and the active index, and nothing
 *   else; contacts, history, tokens and settings belong to the account, and the
 *   account comes back because its address derives from the passkey.
 *
 * `settings.signOut.desc` is deliberately skipped, as on desktop: it ends "your
 * passkey stays in Face ID / fingerprint", and `keeps` says the load-bearing part
 * without naming another platform's biometric.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SignOutSheet(
    pendingUploadWarning: Boolean,
    accountCount: Int = 1,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
) {
    val strings = LocalVelaStrings.current
    val colors = VelaTheme.colors

    ModalBottomSheet(onDismissRequest = onDismiss, containerColor = colors.bgRaised) {
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
                text = strings.t(I18nKeys.Settings.SIGN_OUT_TITLE),
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl2,
            )
            // How many wallets this takes, when it is more than one. `keeps`
            // below is true either way — the address returns, the history is
            // still there — and on a device holding six it was ALSO how the
            // sheet managed to say nothing about signing in six times
            // (owner, 2026-09-23).
            if (accountCount > 1) {
                Spacer(modifier = Modifier.height(VelaSpacing.lg))
                Text(
                    text = strings.t(
                        I18nKeys.Settings.SIGN_OUT_DESC_MANY,
                        mapOf("count" to accountCount.toString()),
                    ),
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                    lineHeight = VelaLeading.normal * VelaTextSize.base,
                )
            }
            Spacer(modifier = Modifier.height(VelaSpacing.lg))
            Text(
                text = strings.t(I18nKeys.Settings.SIGN_OUT_KEEPS),
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                lineHeight = VelaLeading.normal * VelaTextSize.base,
            )

            if (pendingUploadWarning) {
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
                        text = strings.t(I18nKeys.Settings.SIGN_OUT_WARNING),
                        color = colors.fgBase,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.base,
                        lineHeight = VelaLeading.normal * VelaTextSize.base,
                    )
                }
            }

            Spacer(modifier = Modifier.height(VelaSpacing.xl4))
            // Red, like the fixture layer's `signOutSheet` declares
            // (`tone: danger`) and like the settings confirm this sheet now
            // stands in place of. Accent belongs to actions that move money;
            // leaving a wallet moves none.
            VelaDangerButton(
                // "Sign Out Anyway" when there is something to be anyway ABOUT;
                // plain "Sign Out" otherwise. Wording the risk into the button
                // is what makes the warning above more than decoration.
                text = strings.t(
                    if (pendingUploadWarning) {
                        I18nKeys.Settings.SIGN_OUT_ANYWAY
                    } else {
                        I18nKeys.Settings.SIGN_OUT_BUTTON
                    },
                ),
                onClick = onConfirm,
                modifier = Modifier.fillMaxWidth(),
            )
            Spacer(modifier = Modifier.height(VelaSpacing.lg))
            VelaSecondaryButton(
                text = strings.t(I18nKeys.Settings.SIGN_OUT_CANCEL),
                onClick = onDismiss,
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}
