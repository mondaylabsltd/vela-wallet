package app.getvela.wallet.feature.wallet.core

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.feature.onboarding.core.IDENTITY_CONFIRMED
import app.getvela.wallet.feature.onboarding.core.IDENTITY_UNCONFIRMED
import app.getvela.wallet.feature.onboarding.core.IdentityCheck
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.serialization.Serializable
import org.json.JSONObject

/**
 * §37 (review 1-7): may this account be received at — the core's
 * `ReceiveStanding`, decoded as it writes it. The rule is the core's
 * (`receive_standing`): a multi-key group only the key index vouched for is
 * `unconfirmed` (no address, no QR, no request, no copy or share — and the
 * chain is asked again first), one a chain proved another wallet's is
 * `refuted`. This shell draws what it says and decides none of it.
 */
@Serializable
data class ReceiveStanding(
    val receivable: Boolean = false,
    val identity: String = IDENTITY_UNCONFIRMED,
    val title_key: String? = null,
    val body_key: String? = null,
    val check_first: Boolean = false,
) {
    companion object {
        /**
         * A record the core could not read: no address either (the core's
         * `None`), worded as not confirmed, with nothing to ask again.
         */
        val UNREADABLE = ReceiveStanding(
            receivable = false,
            identity = IDENTITY_UNCONFIRMED,
            title_key = I18nKeys.Flows.RECEIVE_UNCONFIRMED_TITLE,
            body_key = I18nKeys.Flows.RECEIVE_UNCONFIRMED_BODY,
            check_first = false,
        )

        /**
         * The stored [accountJson]'s standing; `null` when there is no record
         * to judge (a drawn screen, a multisig in front — whose address is
         * its own Safe's, not a key group's).
         */
        fun of(accountJson: String, door: (String) -> String? = { uniffi.vela_core_uniffi.receiveStanding(it) }): ReceiveStanding? {
            if (accountJson.isBlank()) return null
            val answer = runCatching { door(accountJson) }.getOrNull() ?: return UNREADABLE
            return runCatching { Wire.json.decodeFromString(serializer(), answer) }.getOrDefault(UNREADABLE)
        }
    }
}

/**
 * §37 (review 1-7): the re-check of an account the chain has not confirmed —
 * when Receive opens on it, on its Check again, and once in the background
 * at the app's start. The walk is the core's
 * (`registry_resolve::confirm_step`, driven by
 * [app.getvela.wallet.feature.onboarding.core.RegistryClient.confirmIdentity]
 * with the registry walks' own reader); when it says something other than the
 * record does, the core's `accountWithIdentity` of that record is saved and
 * the session re-reads it, so every screen follows.
 */
class AccountIdentityCheck(
    /** The confirm walk: `(keys[0].public_key_hex, address)` → its `Done`. */
    private val confirm: suspend (publicKeyHex: String, address: String) -> IdentityCheck,
    /** Write the updated record and hand the session the store's list. */
    private val save: suspend (JSONObject) -> Unit,
    private val withIdentity: (accountJson: String, identity: String) -> String? =
        { json, identity -> uniffi.vela_core_uniffi.accountWithIdentity(json, identity) },
    private val standingOf: (String) -> ReceiveStanding? = { ReceiveStanding.of(it) },
) {
    private val _checking = MutableStateFlow<Set<String>>(emptySet())

    /** The addresses (lowercase) whose walk is out now: "Checking on-chain…". */
    val checking: StateFlow<Set<String>> = _checking.asStateFlow()

    fun isChecking(address: String): Boolean = address.lowercase() in _checking.value

    /**
     * Ask the chain about [accountJson] now. Returns what it said, or `null`
     * when there was nothing to ask (no key, no address, a walk for it
     * already out). Never throws for a chain that did not answer — that is
     * `unconfirmed`, and the record stays as it was.
     */
    suspend fun check(accountJson: String): String? {
        val record = runCatching { JSONObject(accountJson) }.getOrNull() ?: return null
        val address = record.optString("address")
        val key = record.optJSONArray("keys")?.optJSONObject(0)?.optString("public_key_hex")?.takeIf { it.isNotBlank() }
            ?: record.optString("public_key_hex")
        if (address.isBlank() || key.isBlank()) return null
        val stored = standingOf(accountJson)?.identity ?: IDENTITY_CONFIRMED
        val id = address.lowercase()
        var claimed = false
        _checking.update { held -> if (id in held) held else { claimed = true; held + id } }
        if (!claimed) return null
        try {
            val found = confirm(key, address)
            if (found.identity != stored) {
                val updated = withIdentity(accountJson, found.identity)
                if (updated != null) {
                    save(JSONObject(updated))
                    VelaLog.event("receive", "identity", "was" to stored, "now" to found.identity, "verified_by" to found.verifiedBy)
                }
            }
            return found.identity
        } finally {
            _checking.update { it - id }
        }
    }
}
