package app.getvela.wallet.feature.contacts.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * The `contacts` machine's wire types, in Kotlin.
 *
 * A transcription of `rust/crates/vela-core/src/app/contacts.rs` — the merge of
 * saved and history-derived entries, the tombstones, the sort, the group
 * membership rules, all 1,385 lines of them stay there. Checked against
 * `app-web/vela-wallet/src/lib/core/generated/Contact*.ts` by
 * `CoreWireDriftTest`.
 */

// -- what the screen renders -------------------------------------------------

@Serializable
data class ContactsView(
    val loaded: Boolean = false,
    /**
     * The unified book: saved ⊕ history-derived, tombstone-suppressed, sorted
     * favourites-first then most-recent.
     *
     * **This order is the core's and is not re-sorted here.** The A–Z sections
     * the phone draws are a presentation of it, not a replacement for it.
     */
    val contacts: List<Contact> = emptyList(),
    val groups: List<ContactGroupView> = emptyList(),
    val sections: List<ContactSection> = emptyList(),
    val last_import: ContactImportReport? = null,
    val import_failure: ContactImportFailure? = null,
    val export: ContactExportFile? = null,
    val recipient: ContactRecipientView? = null,
)

/** One letter of the A–Z directory: the addresses filed under it, in the book's order. */
@Serializable
data class ContactSection(val letter: String, val addresses: List<String> = emptyList())

/** A file the core wrote; sits in the view until `ExportTaken`. `contacts` is `u32`. */
@Serializable
data class ContactExportFile(
    val filename: String,
    val mime: String,
    val content: String,
    val contacts: Int = 0,
)

@Serializable
sealed class ContactExportScope {
    @Serializable
    @SerialName("all")
    data object All : ContactExportScope()

    @Serializable
    @SerialName("group")
    data class Group(val id: String) : ContactExportScope()
}

@Serializable
enum class ContactFileFormat {
    @SerialName("json") Json,
    @SerialName("csv") Csv,
}

/** Why an import file was refused, before anything was written. */
@Serializable
sealed class ContactImportFailure {
    @Serializable
    @SerialName("malformed_json")
    data object MalformedJson : ContactImportFailure()

    @Serializable
    @SerialName("no_address_column")
    data object NoAddressColumn : ContactImportFailure()

    @Serializable
    @SerialName("empty")
    data object Empty : ContactImportFailure()

    @Serializable
    @SerialName("unknown_group")
    data object UnknownGroup : ContactImportFailure()
}

/**
 * One address-book entry.
 *
 * **Two different numeric types, and the generated mirror cannot tell them
 * apart.** ts-rs writes both `u32` and `f64` as TypeScript `number`, so the
 * drift gate is blind here — and serde is not: sending `0.0` for a `u32` is
 * rejected with *"invalid type: floating point `0.0`, expected u32"*, which is
 * how this was found (spec 040 phase 7, on the real machine).
 *
 * - `tx_count` is `u32` → `Int`.
 * - `last_used_ms` / `first_seen_ms` are `f64` → `Double`. The Rust comment
 *   says why: "no u64 crosses the wire".
 */
@Serializable
data class Contact(
    /** Lowercased address — the canonical key. */
    val address: String,
    /** User-given name. Wins over [resolved_name] for display. */
    val name: String? = null,
    /** Cached identity name (ENS / Basename / passkey). */
    val resolved_name: String? = null,
    val resolved_source: String? = null,
    val kind: ContactKind = ContactKind.Unknown,
    val favorite: Boolean = false,
    val note: String? = null,
    val tx_count: Int = 0,
    val last_used_ms: Double = 0.0,
    val first_seen_ms: Double = 0.0,
    val source: ContactSource = ContactSource.Auto,
)

@Serializable
enum class ContactKind {
    @SerialName("eoa") Eoa,

    @SerialName("account") Account,

    @SerialName("unknown") Unknown,
}

@Serializable
enum class ContactSource {
    /** Saved and named by the person. */
    @SerialName("manual") Manual,

    /** A live suggestion derived from history. */
    @SerialName("auto") Auto,
}

/**
 * A group with its members resolved to contacts, in membership order.
 *
 * A member with no saved contact is synthesised by the core as a minimal `auto`
 * entry, so sending to a group never silently drops a payee.
 */
@Serializable
data class ContactGroupView(
    val id: String,
    val name: String,
    val color: String? = null,
    val members: List<Contact> = emptyList(),
)

/** The trust line for the recipient currently on screen. */
@Serializable
data class ContactRecipientView(
    /** Lowercased. */
    val address: String,
    val saved: Boolean = false,
    /**
     * Saved **and** starred — the only state that earns the green check. A
     * poisoned look-alike address is never a starred contact.
     */
    val verified: Boolean = false,
    val display_name: String? = null,
    val identity: ContactIdentity? = null,
    val kind: ContactKind = ContactKind.Unknown,
    val is_contract: Boolean? = null,
    val first_interaction: Boolean = false,
)

@Serializable
data class ContactIdentity(val name: String, val source: String)

@Serializable
data class ContactImportReport(
    val added: Int = 0,
    val skipped: Int = 0,
    val invalid: Int = 0,
    val groups_created: Int = 0,
)

// -- stored shapes carried by operations -------------------------------------

@Serializable
data class ContactTombstone(val address: String, val dismissed_at_ms: Double)

@Serializable
data class ContactGroup(
    val id: String,
    val name: String,
    val color: String? = null,
    val members: List<String> = emptyList(),
)

@Serializable
data class ContactHistoryTx(
    val kind: ContactTxKind? = null,
    val to: String? = null,
    val to_name: String? = null,
    val timestamp_ms: Double? = null,
)

@Serializable
enum class ContactTxKind {
    @SerialName("send") Send,

    @SerialName("receive") Receive,

    @SerialName("dapp_tx") DappTx,

    @SerialName("sign_message") SignMessage,

    @SerialName("sign_typed_data") SignTypedData,

    @SerialName("connect") Connect,
}

// -- what the screen sends ---------------------------------------------------

@Serializable
data class ContactSaveInput(
    val address: String,
    val name: String? = null,
    val note: String? = null,
    val favorite: Boolean? = null,
    val kind: ContactKind? = null,
    val resolved_name: String? = null,
    val resolved_source: String? = null,
)

@Serializable
data class ContactGroupInput(
    val id: String? = null,
    val name: String,
    val color: String? = null,
    val members: List<String>? = null,
)

@Serializable
data class ContactImportEntry(
    val address: String,
    val name: String? = null,
    val note: String? = null,
    val favorite: Boolean? = null,
)

@Serializable
data class ContactImportGroup(
    val name: String,
    val color: String? = null,
    val members: List<String> = emptyList(),
)

/**
 * `ContactEvent`.
 *
 * `now_ms` is the shell's: the core takes no clock, so the moment a contact was
 * saved, deleted or starred comes from here.
 */
@Serializable
sealed class ContactEvent {
    @Serializable
    @SerialName("account_switched")
    data class AccountSwitched(val my_address: String? = null) : ContactEvent()

    @Serializable
    @SerialName("history_changed")
    data object HistoryChanged : ContactEvent()

    @Serializable
    @SerialName("save")
    data class Save(val input: ContactSaveInput, val now_ms: Double) : ContactEvent()

    @Serializable
    @SerialName("delete")
    data class Delete(val address: String, val now_ms: Double) : ContactEvent()

    @Serializable
    @SerialName("toggle_favorite")
    data class ToggleFavorite(val address: String, val now_ms: Double) : ContactEvent()

    @Serializable
    @SerialName("group_save")
    data class GroupSave(val input: ContactGroupInput) : ContactEvent()

    @Serializable
    @SerialName("group_delete")
    data class GroupDelete(val id: String) : ContactEvent()

    @Serializable
    @SerialName("set_group_members")
    data class SetGroupMembers(val id: String, val members: List<String>) : ContactEvent()

    @Serializable
    @SerialName("import_parsed")
    data class ImportParsed(
        val contacts: List<ContactImportEntry>,
        val groups: List<ContactImportGroup>,
        val now_ms: Double,
    ) : ContactEvent()

    @Serializable
    @SerialName("inspect_recipient")
    data class InspectRecipient(val chain_id: Int, val address: String) : ContactEvent()

    /** A picked file's text; the core parses JSON or CSV and existing wins. */
    @Serializable
    @SerialName("import_file")
    data class ImportFile(
        val content: String,
        val filename: String? = null,
        val into_group: String? = null,
        val now_ms: Double,
    ) : ContactEvent()

    @Serializable
    @SerialName("import_acknowledged")
    data object ImportAcknowledged : ContactEvent()

    @Serializable
    @SerialName("export_requested")
    data class ExportRequested(
        val scope: ContactExportScope,
        val format: ContactFileFormat,
        val exported_at_iso: String,
    ) : ContactEvent()

    @Serializable
    @SerialName("export_taken")
    data object ExportTaken : ContactEvent()

    @Serializable
    @SerialName("add_group_members")
    data class AddGroupMembers(val id: String, val members: List<String>) : ContactEvent()

    @Serializable
    @SerialName("remove_group_member")
    data class RemoveGroupMember(val id: String, val address: String) : ContactEvent()

    @Serializable
    @SerialName("set_contact_groups")
    data class SetContactGroups(val address: String, val group_ids: List<String>) : ContactEvent()
}

// -- what the core asks the shell to do --------------------------------------

@Serializable
sealed class ContactOperation {
    @Serializable
    @SerialName("read_store")
    data object ReadStore : ContactOperation()

    @Serializable
    @SerialName("write_contacts")
    data class WriteContacts(val contacts: List<Contact>) : ContactOperation()

    @Serializable
    @SerialName("write_dismissed")
    data class WriteDismissed(val tombstones: List<ContactTombstone>) : ContactOperation()

    @Serializable
    @SerialName("write_groups")
    data class WriteGroups(val groups: List<ContactGroup>) : ContactOperation()

    @Serializable
    @SerialName("load_send_history")
    data object LoadSendHistory : ContactOperation()

    @Serializable
    @SerialName("resolve_identity")
    data class ResolveIdentity(val address: String) : ContactOperation()

    @Serializable
    @SerialName("classify_recipient")
    data class ClassifyRecipient(val chain_id: Int, val address: String) : ContactOperation()
}

// -- what the shell observed -------------------------------------------------

@Serializable
sealed class ContactShellResult {
    @Serializable
    @SerialName("store_loaded")
    data class StoreLoaded(
        val contacts: List<Contact> = emptyList(),
        val tombstones: List<ContactTombstone> = emptyList(),
        val groups: List<ContactGroup> = emptyList(),
    ) : ContactShellResult()

    @Serializable
    @SerialName("history_loaded")
    data class HistoryLoaded(val txs: List<ContactHistoryTx> = emptyList()) : ContactShellResult()

    @Serializable
    @SerialName("history_failed")
    data object HistoryFailed : ContactShellResult()

    @Serializable
    @SerialName("written")
    data object Written : ContactShellResult()

    @Serializable
    @SerialName("identity_resolved")
    data class IdentityResolved(
        val address: String,
        val identity: ContactIdentity? = null,
    ) : ContactShellResult()

    @Serializable
    @SerialName("recipient_classified")
    data class RecipientClassified(
        val chain_id: Int,
        val address: String,
        /** `null` is **unknown**, not a verdict — the core decides what it means. */
        val code: String? = null,
    ) : ContactShellResult()
}
