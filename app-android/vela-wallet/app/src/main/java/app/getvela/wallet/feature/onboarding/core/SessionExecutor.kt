package app.getvela.wallet.feature.onboarding.core

import app.getvela.wallet.core.diagnostics.VelaLog
import org.json.JSONObject

/**
 * The session machine's eleven operations.
 *
 * A separate vocabulary from onboarding's eighteen, and a separate executor,
 * because the session machine is **app-resident** — one per process, outliving
 * every screen — while an onboarding core exists only for the length of a flow.
 *
 * The writes are best effort by contract: the session is already in the state
 * the write was meant to record, and a failed write cannot put it back. That is
 * why so many branches below discard their error — deliberately, and only where
 * the contract says the shell swallows it.
 *
 * Three belong to the landing watch (issue #409): a one-key wallet is entered
 * at the registry's 202, and the session — still here when the create screen is
 * gone — reads the outbox, waits on each accepted record's registry task, and
 * removes a record once its task has landed. A read and a local write; no
 * passkey anywhere.
 */
class SessionExecutor(
    private val store: AccountStore,
    /** The registry the landing watch reads, at the endpoint Settings names. */
    private val registry: suspend () -> RegistryClient = { RegistryClient(store.registryUrlOrDefault()) },
) {

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

            // Issue #409 — the landing watch. The outbox goes over as stored;
            // the core decides which records a read can settle, and a record it
            // cannot read costs only itself.
            "load_pending_uploads" -> try {
                result("pending_uploads_loaded") { put("records", store.loadPendingUploads()) }
            } catch (error: Throwable) {
                if (error is kotlinx.coroutines.CancellationException) throw error
                result("pending_uploads_unavailable") {}
            }

            // The same poll, interval and budget the create's publish always
            // waited with — now after "Wallet created" instead of before it.
            "await_registry_landing" -> {
                val task = operation.optString("task_id")
                val started = System.currentTimeMillis()
                VelaLog.event("registry.landing", "wait", "task" to task)
                try {
                    registry().awaitTask(task)
                    VelaLog.event(
                        "registry.landing",
                        "landed",
                        "task" to task,
                        "waitedMs" to System.currentTimeMillis() - started,
                    )
                    result("registry_landed") {}
                } catch (error: Throwable) {
                    if (error is kotlinx.coroutines.CancellationException) throw error
                    VelaLog.event(
                        "registry.landing",
                        "unconfirmed",
                        "task" to task,
                        "waitedMs" to System.currentTimeMillis() - started,
                        "reason" to error.message,
                    )
                    result("registry_landing_unconfirmed") {
                        put("message", error.message ?: error.javaClass.simpleName)
                    }
                }
            }

            // Only ever after `registry_landed` — the core's rule. Best effort:
            // a record that survives is confirmed again next launch.
            "remove_pending_upload" -> {
                val credentialId = operation.optString("credential_id")
                runCatching { store.removePendingUpload(credentialId) }
                VelaLog.event("registry.landing", "record removed", "cred" to VelaLog.shortId(credentialId))
                result("pending_upload_removed") {}
            }

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
            "load_pending_uploads",
            "await_registry_landing",
            "remove_pending_upload",
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
                // The landing watch: an unknown answer keeps the record — the
                // watch stops, the warning stays, the next launch asks again.
                "load_pending_uploads" -> JSONObject().put("type", "pending_uploads_unavailable")
                "await_registry_landing" -> JSONObject()
                    .put("type", "registry_landing_unconfirmed")
                    .put("message", error.message ?: error.javaClass.simpleName)
                "remove_pending_upload" -> JSONObject().put("type", "pending_upload_removed")
                else -> JSONObject().put("type", "accounts_unavailable")
            }
            return body.toString()
        }
    }
}
