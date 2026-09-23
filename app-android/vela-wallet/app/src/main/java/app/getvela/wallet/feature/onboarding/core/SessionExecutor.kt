package app.getvela.wallet.feature.onboarding.core

import org.json.JSONObject

/**
 * The session machine's seven operations.
 *
 * A separate vocabulary from onboarding's eighteen, and a separate executor,
 * because the session machine is **app-resident** — one per process, outliving
 * every screen — while an onboarding core exists only for the length of a flow.
 *
 * Five of the seven are best effort by contract: the session is already in the
 * state the write was meant to record, and a failed write cannot put it back.
 * That is why so many branches below discard their error — deliberately, and
 * only where the contract says the shell swallows it.
 */
class SessionExecutor(private val store: AccountStore) {

    suspend fun perform(operation: JSONObject): String =
        when (val type = operation.getString("type")) {
            "load_accounts" -> try {
                result("accounts_loaded") { put("accounts", store.loadAccounts()) }
            } catch (error: Throwable) {
                if (error is kotlinx.coroutines.CancellationException) throw error
                result("accounts_unavailable") {}
            }

            "load_active_index" -> result("active_index_loaded") {
                put("index", runCatching { store.loadActiveIndex() }.getOrDefault(0))
            }

            // A best-effort migration write-back. If it fails, the in-memory
            // correction the core made still stands.
            "save_account" -> {
                runCatching { store.saveAccount(operation.getJSONObject("account")) }
                result("account_saved") {}
            }

            "save_active_index" -> {
                runCatching { store.saveActiveIndex(operation.optInt("index")) }
                result("active_index_saved") {}
            }

            "check_pending_uploads" -> try {
                result("pending_uploads") { put("has_pending", store.hasPendingUploads()) }
            } catch (error: Throwable) {
                if (error is kotlinx.coroutines.CancellationException) throw error
                // Fail closed: the sign-out dialog simply does not open, so no
                // unwarned logout path appears.
                result("pending_uploads_unavailable") {}
            }

            "remove_account" -> {
                runCatching { store.removeAccountAtAddress(operation.optString("address")) }
                result("account_removed") {}
            }

            "clear_signed_in_wallet" -> {
                runCatching { store.clearSignedInWallet() }
                result("signed_in_wallet_cleared") {}
            }

            // A no-op wherever no extension exists, which is every Android: the
            // account snapshot the Safari extension reads is an iOS artifact.
            // Answered rather than skipped, because the core is waiting for the
            // ack and would otherwise never leave the sign-out.
            "clear_extension_cache" -> result("extension_cache_cleared") {}

            else -> error("unhandled session operation: $type")
        }.toString()

    companion object {
        /** Every session operation this executor is required to handle (contract §2). */
        val OPERATIONS = listOf(
            "load_accounts",
            "load_active_index",
            "save_account",
            "save_active_index",
            "check_pending_uploads",
            "remove_account",
            "clear_signed_in_wallet",
            "clear_extension_cache",
        )

        private inline fun result(type: String, fill: JSONObject.() -> Unit): JSONObject =
            JSONObject().put("type", type).apply(fill)

        /**
         * The session's own failure for an operation whose answer the core
         * refused (spec 048) — the web's `sessionFailure`, variant for variant:
         * a list that cannot be read is `accounts_unavailable` (→ onboarding),
         * never an onboarding-shaped `storage_failed` the session machine would
         * refuse a second time and leave the route on `loading`.
         */
        fun escapedFailure(operation: JSONObject, error: Throwable): String {
            val type = operation.optString("type")
            val body = when (type) {
                "load_accounts" -> JSONObject().put("type", "accounts_unavailable")
                "load_active_index" -> JSONObject().put("type", "active_index_loaded").put("index", 0)
                "check_pending_uploads" -> JSONObject().put("type", "pending_uploads_unavailable")
                "save_account" -> JSONObject().put("type", "account_saved")
                "save_active_index" -> JSONObject().put("type", "active_index_saved")
                "remove_account" -> JSONObject().put("type", "account_removed")
                "clear_signed_in_wallet" -> JSONObject().put("type", "signed_in_wallet_cleared")
                "clear_extension_cache" -> JSONObject().put("type", "extension_cache_cleared")
                else -> JSONObject().put("type", "accounts_unavailable")
            }
            return body.toString()
        }
    }
}
