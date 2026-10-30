package app.getvela.wallet.feature.signing.trustedsigner

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import org.json.JSONObject
import uniffi.vela_core_uniffi.signingPlan
import uniffi.vela_core_uniffi.signingRoute
import uniffi.vela_core_uniffi.trustedSignerCeremonyKeyLabel
import uniffi.vela_core_uniffi.venueBlockLine

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
     * The key row of the hand-off card (the core's `KeyLabel`, D-17): "Confirm
     * with | <the key>" — the key's own label when the person gave it one that
     * is not the wallet's name, else its place's title. Never read off the
     * record here.
     */
    val keyLabel: KeyLabel? = null,
) {
    /**
     * The core's `KeyLabel` — one label | value row, drawn like the signing
     * sheet's other rows: the label is the translation of [labelKey]
     * ("Confirm with", or "New key on" while a ceremony makes the key), the
     * value [name] as it is, else the translation of [placeKey].
     */
    data class KeyLabel(val name: String?, val placeKey: String, val labelKey: String) {
        /** The row's label, in the person's words. */
        fun label(t: (String) -> String): String = labelKey.takeIf { it.isNotBlank() }?.let(t).orEmpty()

        /** The row's value: the key's own name, else its place. */
        fun value(t: (String) -> String): String = name?.takeIf { it.isNotBlank() } ?: placeKey.takeIf { it.isNotBlank() }?.let(t).orEmpty()

        companion object {
            /** A `KeyLabel` as the core wrote it (`{name?, place_key, label_key}`); `null` when it does not read. */
            fun of(json: JSONObject?): KeyLabel? {
                json ?: return null
                return KeyLabel(
                    name = json.optString("name").takeIf { json.has("name") && !json.isNull("name") && it.isNotBlank() },
                    placeKey = json.optString("place_key"),
                    labelKey = json.optString("label_key"),
                )
            }

            /**
             * A key ceremony's row while it waits on its page — the core's
             * `Ceremony::key_label`: "New key on | Phone or tablet" while a key
             * is made, "Confirm with | This device" when one signs in or
             * proves. `null` for an operation that is not a ceremony.
             */
            fun ofCeremony(operationJson: String): KeyLabel? =
                runCatching { trustedSignerCeremonyKeyLabel(operationJson) }.getOrNull()
                    ?.let { runCatching { JSONObject(it) }.getOrNull() }
                    ?.let(::of)
        }
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
                keyLabel = KeyLabel.of(plan.optJSONObject("key_label")),
            )
        }
    }
}

/**
 * R7 (owner, 2026-10-30: the page everywhere) — where ONE request goes: the
 * core's `signing_route(account, method)` over the stored record, asked for
 * every signature the app makes — the wallet's own send (`""`), a site's
 * `eth_sendTransaction`, a batch (`wallet_sendCalls`), `personal_sign` and
 * typed data — exactly as the web's `signChallenge` asks it. Nothing here
 * decides where a signature goes; it only reads the core's answer.
 */
sealed interface SigningRoute {
    /** Vela's own sheet already showed it: the account's passkey signs here. */
    data object InVela : SigningRoute

    /** The account's trusted signing page, at [url]: it reviews and signs. */
    data class Page(val url: String) : SigningRoute

    /** Nothing on this device can reach the keys: the core's `VenueBlock`. */
    data class Blocked(val block: JSONObject) : SigningRoute

    companion object {
        /**
         * The core's route for [method] by the account stored as
         * [accountJson]; `null` when the core routes it nowhere — a method no
         * venue signs (`eth_sign`) or a record it cannot read. A caller whose
         * account reviews on a page refuses then, never signs in Vela behind
         * the person's choice (the web's `unsupported`).
         */
        fun of(accountJson: String?, method: String): SigningRoute? {
            val json = accountJson?.let { runCatching { signingRoute(it, method) }.getOrNull() } ?: return null
            val route = runCatching { JSONObject(json) }.getOrNull() ?: return null
            return when (route.optString("type")) {
                "in_vela" -> InVela
                "page" -> route.optString("url").ifEmpty { null }?.let(::Page)
                "blocked" -> route.optJSONObject("block")?.let(::Blocked)
                else -> null
            }
        }
    }
}

/**
 * The account's trusted signing page did not sign (spec 102, round 7) — the
 * web's `PageDeclinedError` / `PageSignFailedError` in one shape. [page] is
 * `null` when the PERSON said no there — declined on the page, or closed it
 * unsigned (`SignSubmitOutcome.PageDeclined`; a send keeps its confirm open,
 * `passkey_cancelled`) — and otherwise the core's `PageFailure`: it never
 * answered, its answer was not this request's signature, its own rules
 * refused, or its check failed so it was not opened. Nothing was signed.
 *
 * Still a [app.getvela.wallet.feature.onboarding.core.PasskeyFailure] of kind
 * `Cancelled`, so a caller that only knows passkeys keeps the request open as
 * before; the signing paths read [page] and tell the core which it was.
 */
class PageSignFailure(
    val page: app.getvela.wallet.feature.signing.core.PageFailure?,
    sentence: String,
) : app.getvela.wallet.feature.onboarding.core.PasskeyFailure(
    app.getvela.wallet.feature.onboarding.core.FailureKind.Cancelled,
    sentence,
) {
    /** The person's own "no" on the page. */
    val declined: Boolean get() = page == null
}

/**
 * Why a venue cannot be used for an account — the core's `VenueBlock`
 * (`signing_venue.rs`), carried typed on the sign and send wires
 * (`SignSubmitOutcome.VenueBlocked`, `SendSubmitFailure.VenueBlocked`,
 * `SignErrorNotice.venue_block`, `SendView.tx_venue_block`). Read here only to
 * carry it; its sentence is the core's (`venueBlockLine`: the corpus key and
 * the values it takes) — nothing here decides which words a block gets.
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

    /** The block as the core writes it, for the core to read back. */
    fun json(): String = Wire.json.encodeToString(serializer(), this)

    /** The reason in the person's words: the core's line (`venueBlockLine`), translated. */
    fun words(t: (String, Map<String, String>) -> String): String = VenueWords.line(json(), t)

    companion object {
        /** A block as the core wrote it (`signing_plan`'s `blocked`); `null` when it does not read. */
        fun of(json: JSONObject): VenueBlock? =
            runCatching { Wire.json.decodeFromString(serializer(), json.toString()) }.getOrNull()
    }
}

/** The words of a venue: an unreachable venue's reason. */
object VenueWords {
    /**
     * A venue's refusal in the person's words — the core's `venue_block_line`
     * (`VenueBlock::key()` with `VenueBlock::vars()`), translated. Empty only
     * when the core cannot read [block]; a caller that disables a choice for
     * it still does, by the block's presence.
     */
    fun block(block: JSONObject, t: (String, Map<String, String>) -> String): String = line(block.toString(), t)

    internal fun line(blockJson: String, t: (String, Map<String, String>) -> String): String =
        runCatching { venueBlockLine(blockJson) }.getOrNull()?.let { t(it.key, it.vars) }.orEmpty()
}
