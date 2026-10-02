package app.getvela.wallet.feature.browser

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import app.getvela.wallet.core.designsystem.components.VelaModalSheet
import app.getvela.wallet.core.designsystem.components.VelaPrimaryButton
import app.getvela.wallet.core.designsystem.components.VelaSecondaryButton
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaMonoFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.browser.core.BrowserController

/**
 * Spec 088 FR-004: a page another app or website asked to open
 * (`velawallet://open?url=…`). It does not load until the person has seen
 * where it comes from — the host, large, as the core named it — and said
 * "Open in Vela Wallet". Dismissing is "no": nothing loads, nothing is kept.
 *
 * Words reused, not new: the host is data, the URL is data, the button is the
 * /pay page's own "Open in Vela Wallet" and the other is Cancel.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ExternalPageSheet(
    page: BrowserController.ExternalPage,
    strings: VelaStrings,
    onAnswer: (open: Boolean) -> Unit,
) {
    val colors = VelaTheme.colors
    VelaModalSheet(onDismissRequest = { onAnswer(false) }, containerColor = colors.bgRaised, followAppTextSize = true) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = VelaSpacing.xl)
                .navigationBarsPadding()
                .testTag("external-page-sheet"),
        ) {
            Text(
                text = page.host,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl,
            )
            Spacer(modifier = Modifier.height(VelaSpacing.sm))
            Text(
                text = page.url,
                color = colors.fgSubtle,
                fontFamily = VelaMonoFontFamily,
                fontSize = VelaTextSize.xs,
                maxLines = 3,
            )
            Spacer(modifier = Modifier.height(VelaSpacing.xl))
            VelaPrimaryButton(
                strings.t(I18nKeys.Explore.EXTERNAL_OPEN),
                onClick = { onAnswer(true) },
                modifier = Modifier.fillMaxWidth(),
            )
            Spacer(modifier = Modifier.height(VelaSpacing.lg))
            VelaSecondaryButton(
                strings.t(I18nKeys.Common.CANCEL),
                onClick = { onAnswer(false) },
                modifier = Modifier.fillMaxWidth(),
            )
            Spacer(modifier = Modifier.height(VelaSpacing.lg))
        }
    }
}
