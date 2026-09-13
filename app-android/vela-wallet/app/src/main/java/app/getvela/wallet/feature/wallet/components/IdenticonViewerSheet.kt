package app.getvela.wallet.feature.wallet.components

import app.getvela.wallet.core.platform.Clipboard
import android.content.Context
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.times
import app.getvela.wallet.core.designsystem.components.VelaPrimaryButton
import app.getvela.wallet.core.designsystem.components.VelaSecondaryButton
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaLeading
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.LocalVelaStrings

/** How long the copy confirmation holds before the label reverts. */
private const val COPIED_FEEDBACK_MS = 1_500L

/**
 * The identicon, big, above the address that drew it (founder call,
 * 2026-08-26).
 *
 * The artwork is a fingerprint of the address: the same address always draws
 * the same pattern, which is only useful once somebody has seen the two
 * together often enough to recognise one from the other. A 40dp avatar in a
 * header never teaches that. This does — and it opens from the artwork itself,
 * wherever the artwork is drawn, rather than from a settings page nobody
 * visits.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun IdenticonViewerSheet(
    /** The seed, verbatim: what the artwork was drawn from. */
    address: String,
    onDismiss: () -> Unit,
    /** Spec 049: the name the opener showed, so the initials style draws the same disc big. */
    name: String? = null,
) {
    val strings = LocalVelaStrings.current
    val colors = VelaTheme.colors
    val context = LocalContext.current
    var copied by remember { mutableStateOf(false) }

    LaunchedEffect(copied) {
        if (copied) {
            kotlinx.coroutines.delay(COPIED_FEEDBACK_MS)
            copied = false
        }
    }

    ModalBottomSheet(onDismissRequest = onDismiss, containerColor = colors.bgRaised) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(
                    start = VelaSizing.screenPaddingX,
                    end = VelaSizing.screenPaddingX,
                    bottom = VelaSpacing.xl4,
                ),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
        ) {
            IdenticonAvatar(
                tappable = false,
                seed = address,
                size = WalletMetrics.identiconViewerSize,
                contentDescription = null,
                name = name,
            )

            Text(
                text = strings.t(I18nKeys.Wallet.IDENTICON_TITLE),
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl,
            )

            Text(
                text = strings.t(I18nKeys.Wallet.IDENTICON_CAPTION),
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.regular,
                fontSize = VelaTextSize.base,
                lineHeight = VelaLeading.relaxed * VelaTextSize.base,
                textAlign = TextAlign.Center,
            )

            // The WHOLE address, wrapped rather than middle-truncated: a
            // fingerprint you can only see half of teaches half a habit.
            Text(
                text = address,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.regular,
                fontSize = VelaTextSize.sm,
                lineHeight = VelaLeading.relaxed * VelaTextSize.sm,
                textAlign = TextAlign.Center,
                modifier = Modifier
                    .fillMaxWidth()
                    .background(colors.bgSunken, RoundedCornerShape(VelaRadius.lg))
                    .padding(VelaSpacing.lg),
            )

            VelaPrimaryButton(
                text = strings.t(
                    if (copied) I18nKeys.Wallet.IDENTICON_COPIED else I18nKeys.Wallet.COPY_ADDRESS,
                ),
                onClick = {
                    copyToClipboard(context, address)
                    copied = true
                },
                modifier = Modifier.fillMaxWidth(),
            )
            VelaSecondaryButton(
                text = strings.t(I18nKeys.Wallet.IDENTICON_CLOSE),
                onClick = onDismiss,
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}

private fun copyToClipboard(context: Context, address: String): Boolean =
    Clipboard.copy(context, "address", address)
