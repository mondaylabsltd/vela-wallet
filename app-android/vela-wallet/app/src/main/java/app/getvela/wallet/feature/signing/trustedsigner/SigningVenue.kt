package app.getvela.wallet.feature.signing.trustedsigner

import app.getvela.wallet.feature.onboarding.core.KeyMethod
import org.json.JSONArray
import org.json.JSONObject
import uniffi.vela_core_uniffi.keyMethodWords
import uniffi.vela_core_uniffi.signingPlan

/**
 * Spec 102: how an account signs on this device — the core's `SigningPlan`
 * (`signing_plan(account)`), read once for the shell. Every rule in it is the
 * core's: R1 reachability (an unreachable venue never comes back here, only
 * [blocked]), R4 the venue for transactions and messages, R5 the key route.
 *
 * [page] is the venue's page, `null` for Vela's own sheet; [keyRouteJson] the
 * plan's `key` exactly as it came — it is handed back to the core
 * (`TrustedSignerInput.keyRouteJson`) rather than rebuilt here.
 */
data class SigningPlan(
    /** The RP ID the account's keys live under. */
    val domain: String,
    /** Where its transactions and messages are reviewed and signed; `null` — in Vela. */
    val page: String?,
    /** Set only when nothing on this device can reach the keys: the core's `VenueBlock`. */
    val blocked: JSONObject?,
    /** The key route (`{credential_id, method, transports, hints}`); `null` for an old record. */
    val keyRouteJson: String?,
    val credentialId: String?,
    val transports: String,
    /** Where the key lives; `null` when the plan names no key (or a place this build does not know). */
    val method: KeyMethod?,
) {
    companion object {
        /** The plan for a stored account record, or `null` when the core cannot read the record. */
        fun of(accountJson: String?): SigningPlan? {
            val json = accountJson?.let { runCatching { signingPlan(it) }.getOrNull() } ?: return null
            val plan = runCatching { JSONObject(json) }.getOrNull() ?: return null
            val venue = plan.optJSONObject("venue")
            val key = plan.optJSONObject("key")
            return SigningPlan(
                domain = plan.optString("domain"),
                page = venue?.takeIf { it.optString("type") == "page" }?.optString("url")?.ifEmpty { null },
                blocked = plan.optJSONObject("blocked"),
                keyRouteJson = key?.toString(),
                credentialId = key?.optString("credential_id")?.ifEmpty { null },
                transports = key?.optString("transports").orEmpty(),
                method = key?.optString("method")?.let { wire -> KeyMethod.entries.firstOrNull { it.wire == wire } },
            )
        }
    }
}

/** The words of a venue: the hand-off card's key line and an unreachable venue's reason. */
object VenueWords {
    /**
     * R1's reason in the person's words: `settings.venue.blockedApp`
     * (`{{domain}}`) or `settings.venue.blockedPage` (`{{pageDomain}}`,
     * `{{domain}}`). The core names which (`VenueBlock::key`); this only
     * fills the two facts in.
     */
    fun block(block: JSONObject, t: (String, Map<String, String>) -> String): String = when (block.optString("type")) {
        "page_on_other_domain" -> t(
            "settings.venue.blockedPage",
            mapOf("pageDomain" to block.optString("page_domain"), "domain" to block.optString("domain")),
        )
        else -> t("settings.venue.blockedApp", mapOf("domain" to block.optString("domain")))
    }

    /**
     * What "Confirm with {{key}}" names (D4): the key's own name, else where it
     * lives ("This device", "Phone or tablet", "USB security key" — the core's
     * sign-in words for the place), else nothing.
     */
    fun keyLabel(name: String, method: KeyMethod?, t: (String) -> String): String =
        name.trim().ifEmpty {
            method?.let { keyMethodWords(it.wire, "sign_in", "other")?.titleKey }?.let(t).orEmpty()
        }

    /** The name the record gives the key with [credentialId] (a founding key's label), or empty. */
    fun keyName(accountJson: String?, credentialId: String?): String {
        if (accountJson == null || credentialId.isNullOrEmpty()) return ""
        val keys = runCatching { JSONObject(accountJson).optJSONArray("keys") }.getOrNull() ?: JSONArray()
        for (index in 0 until keys.length()) {
            val key = keys.optJSONObject(index) ?: continue
            if (key.optString("credential_id").equals(credentialId, ignoreCase = true)) return key.optString("name")
        }
        return ""
    }
}
