package app.getvela.wallet.feature.contacts.core

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
class ContactsController(context: Context, scope: CoroutineScope) {

    private val executor = ContactsExecutor(VelaStore(context))

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

    fun setGroupMembers(id: String, members: List<String>) =
        dispatch(ContactEvent.SetGroupMembers(id, members.map { it.lowercase() }))

    private fun dispatch(event: ContactEvent) = host.dispatch(event, ContactEvent.serializer())

    /** The clock is the shell's — the core takes none. */
    private fun now(): Double = System.currentTimeMillis().toDouble()
}
