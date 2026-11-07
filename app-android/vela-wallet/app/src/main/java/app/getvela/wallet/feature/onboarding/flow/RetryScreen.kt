package app.getvela.wallet.feature.onboarding.flow

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.times
import app.getvela.wallet.core.designsystem.components.VelaPrimaryButton
import app.getvela.wallet.core.designsystem.components.VelaSecondaryButton
import app.getvela.wallet.core.designsystem.components.VelaTechDetails
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaLeading
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.LocalVelaStrings

/**
 * The keys were minted; the group never landed.
 *
 * Nothing is lost and nothing is re-minted. The core keeps the whole founding
 * set and a pending record it wrote BEFORE the first publish attempt, so a retry
 * resumes at the publish — which is why this screen offers a retry rather than
 * starting over, and why "start over" is the quiet secondary rather than the
 * obvious escape.
 */
@Composable
fun ColumnScope.RetryScreen(
    /**
     * The publish's own error, forwarded verbatim — it goes into the bug report,
     * so prettifying it here would lose the only detail worth filing.
     */
    detail: String?,
    busy: Boolean,
    /** r18-8 (core §32): the core's own sentence for this failure (`sync_error_key`), in the message's place. */
    sentenceKey: String? = null,
    onRetry: () -> Unit,
    onStartOver: () -> Unit,
    onEditEndpoint: () -> Unit,
) {
    val strings = LocalVelaStrings.current
    val colors = VelaTheme.colors
    var detailsExpanded by remember { mutableStateOf(false) }

    Column(
        modifier = Modifier.weight(1f).fillMaxWidth().verticalScroll(rememberScrollState()),
    ) {
        Text(
            text = strings.t(I18nKeys.Create.SYNC_FAILED_TITLE),
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.bold,
            fontSize = VelaTextSize.xl3,
        )
        Spacer(modifier = Modifier.height(VelaSpacing.md))
        Text(
            text = strings.t(sentenceKey ?: I18nKeys.Create.SYNC_FAILED_MESSAGE),
            color = colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.lg,
            lineHeight = VelaLeading.normal * VelaTextSize.lg,
        )
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
        Text(
            text = strings.t(I18nKeys.Create.SYNC_FAILED_HINT),
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
            lineHeight = VelaLeading.normal * VelaTextSize.base,
        )

        if (detail != null) {
            Spacer(modifier = Modifier.height(VelaSpacing.xl3))
            VelaTechDetails(
                label = strings.t(I18nKeys.Create.TECHNICAL_DETAILS),
                code = detail,
                context = strings.t(I18nKeys.Create.STATUS_SYNCING_KEY),
                endpoint = null,
                expanded = detailsExpanded,
                onToggle = { detailsExpanded = !detailsExpanded },
            )
        }
        Spacer(modifier = Modifier.height(VelaSpacing.xl3))
    }

    // r18-8 (§32): a refused publish keeps Retry upload — the person's own
    // press, never the core's by itself (`sync_error_key` says why above).
    VelaPrimaryButton(
        text = strings.t(I18nKeys.Create.RETRY_UPLOAD_BTN),
        onClick = onRetry,
        loading = busy,
        modifier = Modifier.fillMaxWidth(),
    )
    Spacer(modifier = Modifier.height(VelaSpacing.lg))
    VelaSecondaryButton(
        text = strings.t(I18nKeys.Flow.EDIT_INDEX_ENDPOINT),
        onClick = onEditEndpoint,
        enabled = !busy,
        modifier = Modifier.fillMaxWidth(),
    )
    Spacer(modifier = Modifier.height(VelaSpacing.lg))
    VelaSecondaryButton(
        text = strings.t(I18nKeys.Create.START_OVER_BTN),
        onClick = onStartOver,
        enabled = !busy,
        modifier = Modifier.fillMaxWidth(),
    )
    Spacer(modifier = Modifier.height(VelaSpacing.xl))
}
