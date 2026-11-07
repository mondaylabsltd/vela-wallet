package app.getvela.wallet.feature.onboarding.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject

/**
 * The session machine's wire (`vela_core::app::session`), as kotlinx sealed
 * hierarchies — so [SessionExecutor]'s `when` is exhaustive by the compiler,
 * and `CoreWireDriftTest` holds every variant against the generated mirror
 * (`SessionOperation.ts`, `SessionShellResult.ts`, `SessionEvent.ts`).
 *
 * It used to be hand-built `org.json`, outside the gate: `publish_pending_upload`
 * (ob-2) arrived as an operation nobody had a branch for, and the executor
 * threw. A new core variant is now a compile error here or a failing test, not
 * a sign-in that never finishes.
 *
 * The account and outbox records cross VERBATIM ([JsonObject] / [JsonElement]):
 * the core owns their shape (`Account`, `OutboxRecord`), and a field-by-field
 * copy here is exactly how a wallet's `keys` were once dropped.
 */
@Serializable
sealed class SessionOperation {
    @Serializable
    @SerialName("load_accounts")
    data object LoadAccounts : SessionOperation()

    @Serializable
    @SerialName("load_active_index")
    data object LoadActiveIndex : SessionOperation()

    @Serializable
    @SerialName("save_account")
    data class SaveAccount(val account: JsonObject) : SessionOperation()

    @Serializable
    @SerialName("save_active_index")
    data class SaveActiveIndex(val index: Int) : SessionOperation()

    @Serializable
    @SerialName("check_pending_uploads")
    data object CheckPendingUploads : SessionOperation()

    @Serializable
    @SerialName("clear_signed_in_wallet")
    data object ClearSignedInWallet : SessionOperation()

    @Serializable
    @SerialName("remove_account")
    data class RemoveAccount(val address: String) : SessionOperation()

    @Serializable
    @SerialName("clear_extension_cache")
    data object ClearExtensionCache : SessionOperation()

    @Serializable
    @SerialName("load_pending_uploads")
    data object LoadPendingUploads : SessionOperation()

    @Serializable
    @SerialName("await_registry_landing")
    data class AwaitRegistryLanding(val task_id: String) : SessionOperation()

    @Serializable
    @SerialName("remove_pending_upload")
    data class RemovePendingUpload(val credential_id: String) : SessionOperation()

    /** ob-2: replay a wallet's publish that never landed — every member's proof rides in it, so no passkey. */
    @Serializable
    @SerialName("publish_pending_upload")
    data class PublishPendingUpload(val publish: PendingPublish) : SessionOperation()
}

/**
 * `PendingPublish` — a registry publish that can be replayed without a prompt.
 * Members cross verbatim: [PendingPublisher] reads them as the create's own
 * publish does ([PublishMember.from]).
 */
@Serializable
data class PendingPublish(
    val metadata_hex: String,
    val members: List<JsonObject> = emptyList(),
    val group_seed_hex: String,
    val group_public_key_hex: String,
    /** Spec 102 R3: the page the unit is filed under. `null`: the app. */
    val page: String? = null,
)

@Serializable
sealed class SessionShellResult {
    @Serializable
    @SerialName("accounts_loaded")
    data class AccountsLoaded(val accounts: List<JsonElement> = emptyList()) : SessionShellResult()

    @Serializable
    @SerialName("accounts_unavailable")
    data object AccountsUnavailable : SessionShellResult()

    @Serializable
    @SerialName("active_index_loaded")
    data class ActiveIndexLoaded(val index: Int) : SessionShellResult()

    @Serializable
    @SerialName("account_saved")
    data object AccountSaved : SessionShellResult()

    @Serializable
    @SerialName("account_removed")
    data object AccountRemoved : SessionShellResult()

    @Serializable
    @SerialName("active_index_saved")
    data object ActiveIndexSaved : SessionShellResult()

    @Serializable
    @SerialName("pending_uploads")
    data class PendingUploads(val has_pending: Boolean) : SessionShellResult()

    @Serializable
    @SerialName("pending_uploads_unavailable")
    data object PendingUploadsUnavailable : SessionShellResult()

    @Serializable
    @SerialName("signed_in_wallet_cleared")
    data object SignedInWalletCleared : SessionShellResult()

    @Serializable
    @SerialName("extension_cache_cleared")
    data object ExtensionCacheCleared : SessionShellResult()

    /** The outbox as stored; a record this build cannot read is `null` and costs only itself. */
    @Serializable
    @SerialName("pending_uploads_loaded")
    data class PendingUploadsLoaded(val records: List<JsonElement> = emptyList()) : SessionShellResult()

    @Serializable
    @SerialName("registry_landed")
    data object RegistryLanded : SessionShellResult()

    @Serializable
    @SerialName("registry_landing_unconfirmed")
    data class RegistryLandingUnconfirmed(
        val message: String,
        /** r18-8 (§28, §32): this test build refused the publish to the live index — the session asks no more by itself. */
        val refused: Boolean? = null,
    ) : SessionShellResult()

    @Serializable
    @SerialName("pending_upload_removed")
    data object PendingUploadRemoved : SessionShellResult()
}

/** What the shell tells the session. `mode` / `venue` are the core's own objects, forwarded untouched. */
@Serializable
sealed class SessionEvent {
    @Serializable
    @SerialName("boot")
    data object Boot : SessionEvent()

    @Serializable
    @SerialName("switch_account")
    data class SwitchAccount(val index: Int) : SessionEvent()

    @Serializable
    @SerialName("account_established")
    data class AccountEstablished(val mode: JsonObject) : SessionEvent()

    @Serializable
    @SerialName("remove_account")
    data class RemoveAccount(val index: Int) : SessionEvent()

    @Serializable
    @SerialName("signing_venue_chosen")
    data class SigningVenueChosen(val address: String, val venue: JsonObject) : SessionEvent()

    @Serializable
    @SerialName("sign_out")
    data object SignOut : SessionEvent()

    @Serializable
    @SerialName("sign_out_confirmed")
    data object SignOutConfirmed : SessionEvent()

    @Serializable
    @SerialName("sign_out_dismissed")
    data object SignOutDismissed : SessionEvent()

    /** ob-2: try the outbox again now — the connection came back, a retry timer fired, a screen that names unsynced wallets opened. */
    @Serializable
    @SerialName("sync_pending")
    data object SyncPending : SessionEvent()
}

/** `SessionUnsynced` — a wallet on this device whose registration has not landed (ob-2). */
@Serializable
data class SessionUnsynced(
    val name: String = "",
    val address: String = "",
    /** The core can finish it on its own: the shell keeps asking (`sync_pending`) on a widening timer. */
    val retrying: Boolean = false,
)
