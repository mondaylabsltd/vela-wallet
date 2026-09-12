package app.getvela.wallet.feature.contacts.components

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaLeading
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.contacts.ContactNoticeModel

/** One sentence from the core and a Close (spec 045 US6): the import report, or the refusal. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ContactNoticeSheet(
    model: ContactNoticeModel,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = VelaTheme.colors
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        containerColor = colors.bgRaised,
        modifier = modifier,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = VelaSpacing.xl)
                .navigationBarsPadding(),
        ) {
            Text(
                text = model.title,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl,
            )
            Spacer(modifier = Modifier.height(VelaSpacing.md))
            Text(
                text = model.body,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                lineHeight = VelaTextSize.base * VelaLeading.normal,
                modifier = Modifier.semantics { contentDescription = "contacts-import-report" },
            )
            Spacer(modifier = Modifier.height(VelaSpacing.xl))
            Text(
                text = model.close,
                color = colors.accentBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.base,
                textAlign = TextAlign.Center,
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable(onClick = onDismiss)
                    .padding(vertical = VelaSpacing.md),
            )
            Spacer(modifier = Modifier.height(VelaSpacing.lg))
        }
    }
}
