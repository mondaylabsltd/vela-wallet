package app.getvela.wallet.feature.wallet

import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.wallet.core.PayRefusal

/**
 * A `/pay` link the core refused (spec 039 Part B), said as the web's `/pay`
 * page says it: "Invalid payment link", and why — the core's `pay_refusal`
 * (recipient, network, token or amount), worded from the corpus. The link
 * arrived by an App Link, a share or a scan; whichever way, it is refused
 * by the same machine and told the same way, never dropped in silence.
 */
@Immutable
data class PayLinkRefusalModel(val title: String, val body: String, val dismiss: String)

object PayLinkRefusalWords {
    /** Why the core refused the link, in the person's language (the web's `refusalBody`). */
    fun body(refusal: PayRefusal?, s: VelaStrings): String = s.t(
        when (refusal) {
            PayRefusal.Token -> I18nKeys.PayLink.INVALID_TOKEN
            PayRefusal.Amount -> I18nKeys.PayLink.INVALID_AMOUNT
            // Recipient, network — and a refusal with no reason, which the
            // core never sends; the general sentence is still true of it.
            PayRefusal.Recipient, PayRefusal.Network, null -> I18nKeys.PayLink.INVALID_BODY
        },
    )

    fun model(refusal: PayRefusal?, s: VelaStrings): PayLinkRefusalModel = PayLinkRefusalModel(
        title = s.t(I18nKeys.PayLink.INVALID_TITLE),
        body = body(refusal, s),
        dismiss = s.t(I18nKeys.Common.GOT_IT),
    )
}

/** The refusal over the wallet, the way the app's other refusals are said (the send's alert). */
@Composable
fun PayLinkRefusalDialog(model: PayLinkRefusalModel, onDismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        modifier = Modifier.testTag("pay-link-refused"),
        confirmButton = { TextButton(onClick = onDismiss) { Text(model.dismiss) } },
        title = { Text(model.title) },
        text = { Text(model.body) },
    )
}
