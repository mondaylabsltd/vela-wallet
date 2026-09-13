package app.getvela.wallet.feature.contacts.core

import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.coroutines.launch
import kotlinx.coroutines.flow.first
import app.getvela.wallet.feature.documents.DocumentPorts
import android.content.Context
import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.data.VelaStore
import app.getvela.wallet.core.diagnostics.VelaLog
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.StateFlow
import uniffi.vela_core_uniffi.ContactsCore

/**
 * The address book, app-resident.
 *
 * Held by the composition root and created on first use, like the settings
 * machines: the book is read by more than the contacts tab (a recipient picker
 * asks it who an address belongs to), and re-reading storage on every screen
 * entry would flash an empty list at somebody who has fifty contacts.
 */
class ContactsController(
    context: Context,
    private val scope: CoroutineScope,
    /** A name for an address, from the passkey index. `null` = nobody knows one. */
    registryName: suspend (String) -> String? = { null },
    /** `eth_getCode` on one chain, for the contract badge. `null` = unchecked. */
    code: suspend (Int, String) -> String? = { _, _ -> null },
    /** Spec 045 US6: the platform's documents — the picker for an import, the share sheet for an export. */
    private val documents: () -> DocumentPorts? = { null },
    /** Spec 043 T048: the whole waterfall; `null` keeps the index-only seam. */
    identity: (suspend (String) -> ContactIdentity?)? = null,
) {

    private val executor = ContactsExecutor(
        store = VelaStore(context),
        registryName = registryName,
        code = code,
        identity = identity,
    )

    private val host = CoreHost(
        bridge = ContactsCore().asBridge(),
        scope = scope,
        initial = ContactsView(),
        serializer = ContactsView.serializer(),
        perform = JsonShell.perform(
            ContactOperation.serializer(),
            ContactShellResult.serializer(),
            executor::perform,
        ),
        escapedFailure = JsonShell.escapedFailure(
            ContactOperation.serializer(),
            ContactShellResult.serializer(),
            fallback = ContactShellResult.StoreLoaded(),
            answer = executor::neutralAnswer,
        ),
        onFault = { error -> VelaLog.failure("contacts.fault", "core fault", error) },
    )

    /** The unified book: saved ⊕ history-derived, tombstone-suppressed. */
    val view: StateFlow<ContactsView> = host.view

    /**
     * Read the book for this account.
     *
     * The address matters: the core keys history-derived entries by whose
     * wallet they belong to, and a book read for nobody is a book with no
     * history half. Safe to call on every entry — a second read supersedes the
     * first rather than stacking.
     */
    fun open(myAddress: String?) =
        dispatch(ContactEvent.AccountSwitched(myAddress?.lowercase()))

    /** Save a new contact, or edit one that exists. */
    fun save(input: ContactSaveInput) = dispatch(ContactEvent.Save(input, now()))

    /**
     * Remove one contact.
     *
     * "Remove" is not always "delete": the core tombstones a history-derived
     * entry so it stays gone without inventing a saved record to delete, and
     * deletes a manually saved one. Which of those happens is its decision, and
     * the caller passes an address either way.
     */
    fun delete(address: String) = dispatch(ContactEvent.Delete(address.lowercase(), now()))

    fun toggleFavourite(address: String) =
        dispatch(ContactEvent.ToggleFavorite(address.lowercase(), now()))

    fun saveGroup(input: ContactGroupInput) = dispatch(ContactEvent.GroupSave(input))

    fun deleteGroup(id: String) = dispatch(ContactEvent.GroupDelete(id))

    // -- Spec 045 US5/US6/US7 -------------------------------------------------

    /** Opening a contact asks the core about the address on one chain: contract or wallet, first time or not. */
    fun inspect(chainId: Int, address: String) = dispatch(ContactEvent.InspectRecipient(chainId, address.lowercase()))

    fun addGroupMembers(id: String, members: List<String>) =
        dispatch(ContactEvent.AddGroupMembers(id, members.map { it.lowercase() }))

    fun removeGroupMember(id: String, address: String) =
        dispatch(ContactEvent.RemoveGroupMember(id, address.lowercase()))

    fun setContactGroups(address: String, groupIds: List<String>) =
        dispatch(ContactEvent.SetContactGroups(address.lowercase(), groupIds))

    /**
     * The book travels out through the share sheet: the core writes the file
     * (`ExportRequested` → the view's `export`), the shell hands it over, then
     * tells the core it was taken.
     */
    fun exportBook(scope: ContactExportScope = ContactExportScope.All, format: ContactFileFormat = ContactFileFormat.Json) {
        this.scope.launch {
            dispatch(ContactEvent.ExportRequested(scope, format, isoNow()))
            val file = withTimeoutOrNull(EXPORT_TIMEOUT_MS) { view.first { it.export != null } }?.export
            if (file == null) {
                VelaLog.event("contacts.export", "no file from the core")
                return@launch
            }
            val handed = runCatching { documents()?.share(file.filename, file.mime, file.content.toByteArray()) ?: false }
                .onFailure { VelaLog.failure("contacts.export", "share sheet failed", it) }
                .getOrDefault(false)
            VelaLog.event("contacts.export", if (handed) "handed to the share sheet" else "not handed", "contacts" to file.contacts)
            dispatch(ContactEvent.ExportTaken)
        }
    }

    /**
     * The book travels in through the picker: the file's text goes to the
     * core, which parses JSON or CSV (existing wins) and answers with a
     * report or a refusal in the view; the shell shows it, then acknowledges.
     */
    fun importBook(intoGroup: String? = null) {
        scope.launch {
            val picked = runCatching { documents()?.pick(IMPORT_MIMES) }
                .onFailure { VelaLog.failure("contacts.import", "picker failed", it) }
                .getOrNull() ?: return@launch
            dispatch(ContactEvent.ImportFile(content = picked.bytes.decodeToString(), filename = picked.name, into_group = intoGroup, now_ms = now()))
        }
    }

    fun acknowledgeImport() = dispatch(ContactEvent.ImportAcknowledged)

    private fun isoNow(): String = java.time.Instant.ofEpochMilli(now().toLong()).toString()

    fun setGroupMembers(id: String, members: List<String>) =
        dispatch(ContactEvent.SetGroupMembers(id, members.map { it.lowercase() }))

    private fun dispatch(event: ContactEvent) = host.dispatch(event, ContactEvent.serializer())

    /** The clock is the shell's — the core takes none. */
    private fun now(): Double = System.currentTimeMillis().toDouble()

    companion object {
        const val EXPORT_TIMEOUT_MS = 10_000L
        val IMPORT_MIMES = listOf("application/json", "text/csv", "text/comma-separated-values", "text/plain")
    }
}
