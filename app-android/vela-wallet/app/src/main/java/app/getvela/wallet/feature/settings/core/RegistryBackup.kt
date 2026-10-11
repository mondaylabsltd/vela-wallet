package app.getvela.wallet.feature.settings.core

import app.getvela.wallet.core.i18n.I18nKeys
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
import org.json.JSONArray
import org.json.JSONObject

/**
 * Where a wallet's founding record stands on Ethereum, and — when it is not
 * there — the one call that would put it there (spec 062 §5a). The Android
 * transport for `vela_core::registry_backup`.
 *
 * The walk is the core's: five `eth_call`s against the registry contract, on
 * Gnosis (where the record lives) and on Ethereum (where the backup goes). The
 * index service is never asked — a backup that needed our server to work would
 * not be a backup from it — and no passkey is involved: the registry stored the
 * original calldata and froze its signature domain, so the Gnosis bytes verify
 * on Ethereum as they are. Nothing is cached: "not backed up" next to a fee
 * must never be stale.
 */
class RegistryBackup(
    /**
     * `eth_call` on one chain: the RAW `result` hex — a bare `0x` included,
     * which is a chain saying "no such contract here" — or `null` when nobody
     * answered. Silence is never a verdict.
     */
    private val ethCall: suspend (chainId: Int, to: String, data: String) -> String?,
    /** `registryBackupStep` from the core. A seam so the JVM tests need no native library. */
    private val step: (address: String, foundingKeyHex: String, answersJson: String, targetChain: UInt?) -> String,
) {
    /**
     * [NotCopyable]: Gnosis answered, and what it holds can never be copied
     * (a wallet registered before registry V13 stored no payload). Asking
     * again gets the same answer — a calm end, nothing to tap. It used to
     * fall into [CouldNotCheck], whose row retries: such a wallet said
     * "Couldn't check" for ever.
     */
    enum class State { Unavailable, NotRegistered, BackedUp, NotBackedUp, CouldNotCheck, NotCopyable }

    /** The one transaction that performs the backup. */
    data class Call(val chainId: Int, val to: String, val data: String)

    /** The row's second line: never a caution — a copy is optional and costs a fee. */
    enum class Tone { Neutral, Positive }

    /** What a tap on the row does: nothing, open the sheet with [Check.call], or run the walk again. */
    enum class Action { None, Copy, Retry }

    /**
     * The Keys block's row as the CORE words it (`BackupState::row`, riding
     * on the step's `done`): its title and second line (corpus keys), the
     * line's tone, and the tap. One rule for every shell — this one draws it
     * and maps nothing.
     *
     * [explainKey]: the paragraph under the row for this state — what a copy
     * publishes, what it costs and how it is made — or `null` where it does
     * not apply: under a wallet that can never be copied it described a
     * thing the row above had just said cannot be done. No paragraph is
     * drawn then, and no room is kept for one.
     */
    data class Row(val titleKey: String, val subtitleKey: String, val tone: Tone, val action: Action, val explainKey: String? = null)

    /** [row] `null`: nothing is drawn (no registry on Ethereum, or no record to copy). */
    data class Check(val state: State, val call: Call?, val row: Row? = null)

    suspend fun check(address: String, foundingKeyHex: String, targetChain: Int? = null): Check {
        val answers = JSONArray()
        repeat(MAX_ROUNDS) {
            val next = runCatching { JSONObject(step(address, foundingKeyHex, answers.toString(), targetChain?.toUInt())) }
                .getOrNull() ?: return COULD_NOT
            if (next.optString("type") == "ask") {
                val requests = next.optJSONArray("requests") ?: return COULD_NOT
                coroutineScope {
                    (0 until requests.length()).map { index ->
                        val request = requests.getJSONObject(index)
                        async { perform(request) }
                    }.awaitAll()
                }.forEach(answers::put)
                return@repeat
            }
            val state = when (next.optString("state")) {
                "unavailable" -> State.Unavailable
                "not_registered" -> State.NotRegistered
                "backed_up" -> State.BackedUp
                "not_backed_up" -> State.NotBackedUp
                "not_copyable" -> State.NotCopyable
                "could_not_check" -> State.CouldNotCheck
                // A state this build does not know is not a verdict it can draw.
                else -> return COULD_NOT
            }
            // `optString` turns a JSON null into "null" (org.json); `optJSONObject` says "absent".
            val call = next.optJSONObject("call")?.let {
                Call(chainId = it.getInt("chain_id"), to = it.getString("to"), data = it.getString("data"))
            }
            val row = next.optJSONObject("row")?.let(::rowOf)
            // The core offers a call exactly when there is something to do.
            return if (state == State.NotBackedUp && call == null) COULD_NOT else Check(state, call, row)
        }
        return COULD_NOT
    }

    private fun rowOf(json: JSONObject) = Row(
        titleKey = json.optString("title_key"),
        subtitleKey = json.optString("subtitle_key"),
        tone = if (json.optString("tone") == "positive") Tone.Positive else Tone.Neutral,
        action = when (json.optString("action")) {
            "copy" -> Action.Copy
            "retry" -> Action.Retry
            else -> Action.None
        },
        // The core leaves the key out where no paragraph applies (`optString`
        // reads an absent key as "", and a JSON null as "null").
        explainKey = json.optString("explain_key").takeIf { it.isNotEmpty() && !json.isNull("explain_key") },
    )

    private suspend fun perform(request: JSONObject): JSONObject {
        val id = request.optString("id")
        fun answer(outcome: String, body: String? = null) =
            JSONObject().put("id", id).put("outcome", outcome).put("body", body ?: JSONObject.NULL)
        return runCatching {
            val result = ethCall(request.getInt("chain_id"), request.getString("to"), request.getString("data"))
            if (result == null) answer("failed") else answer("ok", result)
        }.getOrElse { answer("failed") }
    }

    companion object {
        private const val MAX_ROUNDS = 16

        /**
         * This transport's own "could not check": the walk never reached the
         * core's `done` (it threw, ran out of rounds, or said something this
         * build cannot read), so there is no core row to draw. It is the
         * core's row for `could_not_check`, word for word — ask again.
         */
        internal val COULD_NOT = Check(
            State.CouldNotCheck,
            null,
            Row(I18nKeys.SettingsUi.BACKUP_TITLE, I18nKeys.SettingsUi.BACKUP_COULD_NOT_CHECK, Tone.Neutral, Action.Retry, I18nKeys.SettingsUi.BACKUP_EXPLAIN),
        )
    }
}
