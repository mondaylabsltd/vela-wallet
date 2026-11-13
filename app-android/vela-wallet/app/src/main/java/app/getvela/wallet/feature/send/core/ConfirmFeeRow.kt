package app.getvela.wallet.feature.send.core

import org.json.JSONObject
import uniffi.vela_core_uniffi.multisigLinkCall

/**
 * r12 S4: the confirm screen's fee row as the core words it (`fee.row`,
 * through the link door): the label's key and what goes before the amount.
 * Android drew "~0.12" where the web and the desktop drew "0.12" for the
 * same quote; the rule is the core's (§13: a quoted fee is drawn bare).
 */
object ConfirmFeeRow {
    /** What precedes the confirm's fee amount — the core's `amount_prefix` ("" for a quote). */
    fun prefix(quoted: Boolean = true): String = row(quoted)?.optString("amount_prefix", null) ?: ""

    /** The row's label key — `send.estFeeLabel`, or `send.feeTokenEstimate` for one not quoted; `null` from a core without the rule. */
    fun labelKey(quoted: Boolean = true): String? = row(quoted)?.optString("label_key", "")?.takeIf { it.isNotEmpty() }

    private fun row(quoted: Boolean): JSONObject? = runCatching {
        val answer = multisigLinkCall("fee.row", JSONObject().put("quoted", quoted).toString())
        if (answer.trim() == "null") null else JSONObject(answer)
    }.getOrNull()
}
