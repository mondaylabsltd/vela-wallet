package app.getvela.wallet.feature.onboarding.core

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.diagnostics.VelaLog
import kotlinx.serialization.json.JsonElement
import org.json.JSONArray
import org.json.JSONObject

/**
 * The session machine's twelve operations.
 *
 * A separate vocabulary from onboarding's eighteen, and a separate executor,
 * because the session machine is **app-resident** — one per process, outliving
 * every screen — while an onboarding core exists only for the length of a flow.
 *
 * Typed ([SessionOperation] → [SessionShellResult], `SessionWire.kt`) and so
 * exhaustive by the compiler: it used to be a `when` over strings, outside
 * `CoreWireDriftTest`, and ob-2's `publish_pending_upload` reached it as an
 * operation nobody handled (the executor threw, and a wallet saved before its
 * publish never finished registering).
 *
 * The writes are best effort by contract: the session is already in the state
 * the write was meant to record, and a failed write cannot put it back. That is
 * why so many branches below discard their error — deliberately, and only where
 * the contract says the shell swallows it.
 *
 * Four belong to the landing watch (issue #409, ob-2): the session — still here
 * when the create screen is gone — reads the outbox, waits on each accepted
 * record's registry task or REPLAYS a publish that never landed, and removes a
 * record once it has landed. Reads, a replay of proofs already made, and a
 * local write; no passkey anywhere.
 */
class SessionExecutor(
    private val store: AccountStore,
    /** The registry the landing watch reads, at the endpoint Settings names. */
    private val registry: suspend () -> RegistryClient = { RegistryClient(store.registryUrlOrDefault()) },
    /** ob-2's replay; a seam so a test can stand in for the registry. */
    private val publisher: suspend (PendingPublish) -> Unit = { PendingPublisher(registry).publish(it) },
) {

    suspend fun perform(operation: SessionOperation): SessionShellResult = when (operation) {
        SessionOperation.LoadAccounts -> try {
            SessionShellResult.AccountsLoaded(elements(store.loadAccounts()))
        } catch (error: Throwable) {
            if (error is kotlinx.coroutines.CancellationException) throw error
            SessionShellResult.AccountsUnavailable
        }

        SessionOperation.LoadActiveIndex ->
            SessionShellResult.ActiveIndexLoaded(runCatching { store.loadActiveIndex() }.getOrDefault(0))

        // A best-effort migration write-back. If it fails, the in-memory
        // correction the core made still stands.
        is SessionOperation.SaveAccount -> {
            runCatching { store.saveAccount(JSONObject(operation.account.toString())) }
            SessionShellResult.AccountSaved
        }

        is SessionOperation.SaveActiveIndex -> {
            runCatching { store.saveActiveIndex(operation.index) }
            SessionShellResult.ActiveIndexSaved
        }

        SessionOperation.CheckPendingUploads -> try {
            SessionShellResult.PendingUploads(store.hasPendingUploads())
        } catch (error: Throwable) {
            if (error is kotlinx.coroutines.CancellationException) throw error
            // Fail closed: the sign-out dialog simply does not open, so no
            // unwarned logout path appears.
            SessionShellResult.PendingUploadsUnavailable
        }

        is SessionOperation.RemoveAccount -> {
            runCatching { store.removeAccountAtAddress(operation.address) }
            SessionShellResult.AccountRemoved
        }

        SessionOperation.ClearSignedInWallet -> {
            runCatching { store.clearSignedInWallet() }
            SessionShellResult.SignedInWalletCleared
        }

        // A no-op wherever no extension exists, which is every Android: the
        // account snapshot the Safari extension reads is an iOS artifact.
        // Answered rather than skipped, because the core is waiting for the
        // ack and would otherwise never leave the sign-out.
        SessionOperation.ClearExtensionCache -> SessionShellResult.ExtensionCacheCleared

        // Issue #409 — the landing watch. The outbox goes over as stored;
        // the core decides which records a read can settle, and a record it
        // cannot read costs only itself.
        SessionOperation.LoadPendingUploads -> try {
            SessionShellResult.PendingUploadsLoaded(elements(store.loadPendingUploads()))
        } catch (error: Throwable) {
            if (error is kotlinx.coroutines.CancellationException) throw error
            SessionShellResult.PendingUploadsUnavailable
        }

        // The same poll, interval and budget the create's publish always
        // waited with — now after "Wallet created" instead of before it.
        is SessionOperation.AwaitRegistryLanding -> landing("wait", operation.task_id) {
            registry().awaitTask(operation.task_id)
        }

        // Only ever after `registry_landed` — the core's rule. Best effort:
        // a record that survives is confirmed again next launch.
        is SessionOperation.RemovePendingUpload -> {
            runCatching { store.removePendingUpload(operation.credential_id) }
            VelaLog.event("registry.landing", "record removed", "cred" to VelaLog.shortId(operation.credential_id))
            SessionShellResult.PendingUploadRemoved
        }

        // ob-2: the publish the create would have sent, replayed — every
        // member's creation-time proof and the group key ride in the record,
        // so no passkey and no sheet. It waits for the landing itself: only a
        // landed group lets the record go.
        is SessionOperation.PublishPendingUpload -> landing("replay", null) {
            publisher(operation.publish)
        }
    }

    /** A registry wait or replay, told as the core hears it: landed, or not confirmed with why. */
    private suspend fun landing(what: String, task: String?, wait: suspend () -> Unit): SessionShellResult {
        val started = System.currentTimeMillis()
        VelaLog.event("registry.landing", what, "task" to task)
        return try {
            wait()
            VelaLog.event("registry.landing", "landed", "task" to task, "waitedMs" to System.currentTimeMillis() - started)
            SessionShellResult.RegistryLanded
        } catch (error: Throwable) {
            if (error is kotlinx.coroutines.CancellationException) throw error
            VelaLog.event(
                "registry.landing",
                "unconfirmed",
                "task" to task,
                "waitedMs" to System.currentTimeMillis() - started,
                "reason" to error.message,
            )
            // r18-8 (§28, §32): a publish this test build refused before it
            // left says so — the session never asks again by itself.
            SessionShellResult.RegistryLandingUnconfirmed(
                error.message ?: error.javaClass.simpleName,
                refused = true.takeIf { (error as? RegistryFailure)?.refused == true },
            )
        }
    }

    companion object {
        /** Every session operation's wire name — the mirror's, by the drift gate. */
        val OPERATIONS: List<String> = listOf(
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
            "publish_pending_upload",
        )

        private fun elements(array: JSONArray): List<JsonElement> =
            (0 until array.length()).map { index ->
                val value = array.opt(index)
                if (value == null || value == JSONObject.NULL) {
                    kotlinx.serialization.json.JsonNull
                } else {
                    Wire.json.parseToJsonElement(
                        when (value) {
                            is JSONObject, is JSONArray -> value.toString()
                            else -> JSONObject.quote(value.toString())
                        },
                    )
                }
            }

        /**
         * The session's own failure for an operation whose answer the core
         * refused (spec 048) — the web's `sessionFailure`, variant for variant:
         * a list that cannot be read is `accounts_unavailable` (→ onboarding),
         * never an onboarding-shaped `storage_failed` the session machine would
         * refuse a second time and leave the route on `loading`.
         */
        fun failure(operation: SessionOperation, error: Throwable): SessionShellResult = when (operation) {
            SessionOperation.LoadAccounts -> SessionShellResult.AccountsUnavailable
            SessionOperation.LoadActiveIndex -> SessionShellResult.ActiveIndexLoaded(0)
            SessionOperation.CheckPendingUploads -> SessionShellResult.PendingUploadsUnavailable
            is SessionOperation.SaveAccount -> SessionShellResult.AccountSaved
            is SessionOperation.SaveActiveIndex -> SessionShellResult.ActiveIndexSaved
            is SessionOperation.RemoveAccount -> SessionShellResult.AccountRemoved
            SessionOperation.ClearSignedInWallet -> SessionShellResult.SignedInWalletCleared
            SessionOperation.ClearExtensionCache -> SessionShellResult.ExtensionCacheCleared
            // The landing watch fails toward keeping the record: the watch
            // stops, the warning stays, the next launch asks again.
            SessionOperation.LoadPendingUploads -> SessionShellResult.PendingUploadsUnavailable
            is SessionOperation.AwaitRegistryLanding ->
                SessionShellResult.RegistryLandingUnconfirmed(error.message ?: error.javaClass.simpleName)
            is SessionOperation.PublishPendingUpload ->
                SessionShellResult.RegistryLandingUnconfirmed(error.message ?: "the publish did not answer")
            is SessionOperation.RemovePendingUpload -> SessionShellResult.PendingUploadRemoved
        }

        /** [failure] over the driver's JSON; an operation this build has no name for keeps the route off `loading`. */
        fun escapedFailure(operation: JSONObject, error: Throwable): String {
            val result = runCatching {
                failure(Wire.json.decodeFromString(SessionOperation.serializer(), operation.toString()), error)
            }.getOrDefault(SessionShellResult.AccountsUnavailable)
            return Wire.json.encodeToString(SessionShellResult.serializer(), result)
        }
    }
}
