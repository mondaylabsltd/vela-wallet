package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.contacts.ContactsFixtures
import app.getvela.wallet.feature.contacts.ContactsLive
import app.getvela.wallet.feature.contacts.ContactsScreenState
import app.getvela.wallet.feature.contacts.core.Contact
import app.getvela.wallet.feature.contacts.core.ContactEvent
import app.getvela.wallet.feature.contacts.core.ContactOperation
import app.getvela.wallet.feature.contacts.core.ContactSaveInput
import app.getvela.wallet.feature.contacts.core.ContactShellResult
import app.getvela.wallet.feature.contacts.core.ContactsController
import app.getvela.wallet.feature.contacts.core.ContactsExecutor
import app.getvela.wallet.feature.contacts.core.ContactsView
import java.io.File
import java.util.concurrent.atomic.AtomicInteger
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * P2.5: a contact's note (the web's round 3 — `contacts/forms.ts`,
 * `ContactForm.svelte`, `ContactDetailPanel.svelte`) and `history_changed`
 * (the web's contacts page mount and send open), on the real machine where a
 * machine decides.
 */
class ContactsNoteTest {

    private val scopes = mutableListOf<CoroutineScope>()

    @After
    fun stopEveryMachine() {
        scopes.forEach { it.cancel() }
        scopes.clear()
    }

    private val strings: VelaStrings by lazy {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set")
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
    }

    /** A host whose executor counts the send-history reads it is asked for. */
    private fun host(store: KeyValueStore, historyReads: AtomicInteger = AtomicInteger()): CoreHost<ContactsView> {
        val executor = ContactsExecutor(store)
        val counting: suspend (ContactOperation) -> ContactShellResult = { operation ->
            if (operation is ContactOperation.LoadSendHistory) historyReads.incrementAndGet()
            executor.perform(operation)
        }
        return CoreHost(
            bridge = uniffi.vela_core_uniffi.ContactsCore().asBridge(),
            scope = CoroutineScope(SupervisorJob() + Dispatchers.Default).also { scopes += it },
            initial = ContactsView(),
            serializer = ContactsView.serializer(),
            perform = JsonShell.perform(ContactOperation.serializer(), ContactShellResult.serializer(), counting),
            escapedFailure = JsonShell.escapedFailure(
                ContactOperation.serializer(),
                ContactShellResult.serializer(),
                fallback = ContactShellResult.StoreLoaded(),
                answer = executor::neutralAnswer,
            ),
            onFault = { error -> throw AssertionError("shell fault: $error", error) },
        )
    }

    private fun CoreHost<ContactsView>.settle(predicate: (ContactsView) -> Boolean): ContactsView =
        runBlocking { withTimeout(TIMEOUT_MS) { view.first(predicate) } }

    private fun CoreHost<ContactsView>.send(event: ContactEvent) = dispatch(event, ContactEvent.serializer())

    private fun save(name: String, note: String?) =
        ContactEvent.Save(ContactSaveInput(address = ALICE, name = name, note = note), now_ms = 1_725_000_000_000.0)

    // -- the note on the machine ------------------------------------------------

    @Test
    fun aNoteIsKeptWithItsLinesAndComesBackFromTheStore() {
        val store = FakeStore()
        val host = host(store)
        host.send(ContactEvent.AccountSwitched(ME))
        host.settle { it.loaded }
        host.send(save("Alice", ContactsLive.cleanNote("  Rent, every month\nShared flat  ")))
        val saved = host.settle { it.contacts.any { c -> c.note != null } }.contacts.single()
        assertEquals("Rent, every month\nShared flat", saved.note)

        // A second book over the same store reads it back.
        val again = host(store)
        again.send(ContactEvent.AccountSwitched(ME))
        assertEquals("Rent, every month\nShared flat", again.settle { it.loaded && it.contacts.isNotEmpty() }.contacts.single().note)
    }

    @Test
    fun anAbsentNoteKeepsItAndAnEmptiedOneClearsIt() {
        val host = host(FakeStore())
        host.send(ContactEvent.AccountSwitched(ME))
        host.settle { it.loaded }
        host.send(save("Alice", "Landlord"))
        host.settle { it.contacts.singleOrNull()?.note == "Landlord" }
        // A rename that says nothing about the note (an agent's save_contact without one).
        host.send(save("Alice B", null))
        val renamed = host.settle { it.contacts.singleOrNull()?.name == "Alice B" }.contacts.single()
        assertEquals("Landlord", renamed.note)
        // The form's emptied field: the note goes.
        host.send(save("Alice B", ContactsLive.cleanNote("   ")))
        val cleared = host.settle { it.contacts.singleOrNull()?.note.isNullOrEmpty() }.contacts.single()
        assertTrue(cleared.note.isNullOrEmpty())
    }

    // -- history_changed --------------------------------------------------------

    @Test
    fun historyChangedReReadsTheHistoryAndKeepsTheBook() {
        val reads = AtomicInteger()
        val host = host(FakeStore(), reads)
        host.send(ContactEvent.AccountSwitched(ME))
        host.settle { it.loaded }
        host.send(save("Alice", null))
        host.settle { it.contacts.isNotEmpty() }
        val before = reads.get()
        host.send(ContactEvent.HistoryChanged)
        runBlocking { withTimeout(TIMEOUT_MS) { while (reads.get() == before) delay(10) } }
        val view = host.view.value
        assertTrue("the book is not unloaded", view.loaded)
        assertEquals("Alice", view.contacts.single().name)
    }

    @Test
    fun aVisitToTheBookAlreadyReadIsHistoryChangedAndAnythingElseIsASwitch() {
        assertEquals(ContactEvent.HistoryChanged, ContactsController.visitEvent(openedFor = ME, loaded = true, myAddress = ME.uppercase().replace("0X", "0x")))
        assertEquals(ContactEvent.AccountSwitched(ME), ContactsController.visitEvent(openedFor = ME, loaded = false, myAddress = ME))
        assertEquals(ContactEvent.AccountSwitched(OTHER), ContactsController.visitEvent(openedFor = ME, loaded = true, myAddress = OTHER))
        assertEquals(ContactEvent.AccountSwitched(ME), ContactsController.visitEvent(openedFor = null, loaded = true, myAddress = ME))
        assertEquals(ContactEvent.AccountSwitched(null), ContactsController.visitEvent(openedFor = ME, loaded = true, myAddress = null))
    }

    // -- the form and the detail ------------------------------------------------

    @Test
    fun theLiveFormHasTheNoteFieldAndTheGalleryKeepsTwoFields() {
        val live = ContactsFixtures.contactForm(strings, edit = false, noted = true)
        assertEquals(strings.t(I18nKeys.Contacts.NOTE_LABEL), live.noteLabel)
        assertEquals(strings.t(I18nKeys.Contacts.NOTE_PLACEHOLDER), live.notePlaceholder)
        assertTrue(live.noteLabel != I18nKeys.Contacts.NOTE_LABEL)
        val gallery = ContactsFixtures.contactForm(strings, edit = false)
        assertNull(gallery.noteLabel)
        val typed = ContactsLive.form(live, edit = true, name = "Alice", address = ALICE, note = "Landlord")
        assertEquals("Landlord", typed.note)
    }

    @Test
    fun aNameOfOnlyUnseenCharactersCannotBeSaved() {
        val form = ContactsFixtures.contactForm(strings, edit = false, noted = true)
        assertFalse(ContactsLive.form(form, edit = false, name = "‎‮ ", address = ALICE).saveEnabled)
        assertTrue(ContactsLive.form(form, edit = false, name = "Al‮ice", address = ALICE).saveEnabled)
        assertEquals("Alice", ContactsLive.cleanName(" Al‮ice\n"))
    }

    @Test
    fun cleanNoteKeepsLinesAndTabsAndDropsControlsAndBidi() {
        assertEquals("a\nb\tc", ContactsLive.cleanNote("\u0000a\nb\tc‮\u0007 "))
        assertEquals("", ContactsLive.cleanNote(" ‏ "))
    }

    @Test
    fun theDetailShowsTheNoteOnlyWhenThePersonWroteOne() {
        val fallback = ContactsFixtures.buildMobileState(ContactsScreenState.C2, strings).detail!!
        val book = ContactsView(loaded = true, contacts = listOf(Contact(address = ALICE, name = "Alice", note = "  Landlord\nPays on the 1st ")))
        val noted = ContactsLive.detail(fallback, book.contacts.single(), book, strings = strings).note!!
        assertEquals(strings.t(I18nKeys.Contacts.NOTE_LABEL), noted.label)
        assertEquals("Landlord\nPays on the 1st", noted.text)
        for (none in listOf(null, "", "  ")) {
            val contact = Contact(address = ALICE, name = "Alice", note = none)
            assertNull(ContactsLive.detail(fallback, contact, book, strings = strings).note)
        }
    }

    @Test
    fun theZhFormSaysNoteInChinese() {
        val zh = I18nRuntime { tag -> File(System.getProperty("vela.repo.root"), "assets/i18n/$tag.json").readBytes() }.apply { initialize("zh") }
        assertEquals("备注", ContactsFixtures.contactForm(zh, edit = true, noted = true).noteLabel)
    }

    companion object {
        const val TIMEOUT_MS = 10_000L
        const val ME = "0x1111111111111111111111111111111111111111"
        const val OTHER = "0x2222222222222222222222222222222222222222"
        const val ALICE = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    }
}
