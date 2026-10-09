package app.getvela.wallet.feature.signing.trustedsigner

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import org.json.JSONObject
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
    /**
     * What "Confirm with {{key}}" names (the core's `KeyLabel`, D-17): the
     * key's own label when the person gave it one that is not the wallet's
     * name, else its place's title — never read off the record here.
     */
    val keyLabel: KeyLabel? = null,
) {
    /** The core's `KeyLabel`: [name] drawn as it is, else the translation of [placeKey]. */
    data class KeyLabel(val name: String?, val placeKey: String) {
        fun text(t: (String) -> String): String = name?.takeIf { it.isNotBlank() } ?: placeKey.takeIf { it.isNotBlank() }?.let(t).orEmpty()
    }

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
                keyLabel = plan.optJSONObject("key_label")?.let { label ->
                    KeyLabel(
                        name = label.optString("name").takeIf { label.has("name") && !label.isNull("name") && it.isNotBlank() },
                        placeKey = label.optString("place_key"),
                    )
                },
            )
        }
    }
}

/**
 * Why a venue cannot be used for an account — the core's `VenueBlock`
 * (`signing_venue.rs`), carried typed on the sign and send wires
 * (`SignSubmitOutcome.VenueBlocked`, `SendSubmitFailure.VenueBlocked`,
 * `SignErrorNotice.venue_block`, `SendView.tx_venue_block`). The sentence is
 * [key] with [vars]; nothing here decides which.
 */
@Serializable
sealed class VenueBlock {
    /** Vela's own sheet signs with `getvela.app` keys only, and the account's live on [domain]. */
    @Serializable
    @SerialName("app_cannot_reach")
    data class AppCannotReach(val domain: String) : VenueBlock()

    /** The page lives on [page_domain] and the account's keys on [domain]. */
    @Serializable
    @SerialName("page_on_other_domain")
    data class PageOnOtherDomain(val page_domain: String, val domain: String) : VenueBlock()

    /** The web opens no signing page — never a phone's R1 answer; read so a block from anywhere reads. */
    @Serializable
    @SerialName("not_on_web")
    data object NotOnWeb : VenueBlock()

    /** The core's `VenueBlock::key()`. */
    fun key(): String = when (this) {
        is AppCannotReach -> "settings.venue.blockedApp"
        is PageOnOtherDomain -> "settings.venue.blockedPage"
        NotOnWeb -> "settings.venue.blockedWeb"
    }

    /** The facts [key]'s sentence names. */
    fun vars(): Map<String, String> = when (this) {
        is AppCannotReach -> mapOf("domain" to domain)
        is PageOnOtherDomain -> mapOf("pageDomain" to page_domain, "domain" to domain)
        NotOnWeb -> emptyMap()
    }

    /** The reason in the person's words. */
    fun words(t: (String, Map<String, String>) -> String): String = t(key(), vars())

    companion object {
        /** A block as the core wrote it (`signing_plan`'s `blocked`); `null` when it does not read. */
        fun of(json: JSONObject): VenueBlock? =
            runCatching { Wire.json.decodeFromString(serializer(), json.toString()) }.getOrNull()
    }
}

/** The words of a venue: an unreachable venue's reason. */
object VenueWords {
    /**
     * A venue's refusal in the person's words: the core's `VenueBlock::key()`
     * — `settings.venue.blockedApp` (`{{domain}}`), `settings.venue.blockedPage`
     * (`{{pageDomain}}`, `{{domain}}`) or `settings.venue.blockedWeb` (no
     * facts; never an R1 answer on a phone, handled so a block from anywhere
     * reads). This only fills the facts in.
     */
    fun block(block: JSONObject, t: (String, Map<String, String>) -> String): String =
        (VenueBlock.of(block) ?: VenueBlock.AppCannotReach(block.optString("domain"))).words(t)
}
