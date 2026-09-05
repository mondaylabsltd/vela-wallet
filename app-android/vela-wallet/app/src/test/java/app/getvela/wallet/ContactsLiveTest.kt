package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.contacts.ContactsFixtures
import app.getvela.wallet.feature.contacts.ContactsLive
import app.getvela.wallet.feature.contacts.ContactsScreenState
import app.getvela.wallet.feature.contacts.core.Contact
import app.getvela.wallet.feature.contacts.core.ContactGroupView
import app.getvela.wallet.feature.contacts.core.ContactsView
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The live contacts builders.
 *
 * The letter-sectioning and search cases are **ported from the web sibling**
 * (`app-web/vela-wallet/src/lib/contacts/live.test.ts`) rather than re-derived.
 * Two clients that section the same address book differently is a bug nobody
 * notices until somebody uses both, and no compiler can see it.
 */
class ContactsLiveTest {

    private val strings: VelaStrings by lazy {
        val root = System.getProperty("vela.repo.root")
            ?: error("vela.repo.root not set — run via Gradle (testOptions wires it)")
        I18nRuntime { tag -> File(root, "public/i18n/$tag.json").readBytes() }
            .apply { initialize("en") }
    }

    private fun labels() = ContactsFixtures.buildMobileState(ContactsScreenState.C1, strings)

    /**
     * C3 is the empty book — the only fixture state carrying the "nothing saved
     * yet" copy, because C1 is a populated list and has no empty state to
     * borrow. Reading it from the wrong state is how a new person gets a blank
     * screen instead of a welcome.
     */
    private fun emptyState() =
        ContactsFixtures.buildMobileState(ContactsScreenState.C3, strings).empty

    private fun contact(address: String, name: String? = null, resolved: String? = null) =
        Contact(address = address, name = name, resolved_name = resolved)

    private fun book(vararg contacts: Contact, groups: List<ContactGroupView> = emptyList()) =
        ContactsView(loaded = true, contacts = contacts.toList(), groups = groups)

    // -- what a person is called ---------------------------------------------

    @Test
    fun aNameWinsOverAResolvedOne() {
        assertEquals("Alice", ContactsLive.displayName(contact(ALICE, "Alice", "alice.eth")))
    }

    @Test
    fun aResolvedNameWinsOverAnAddress() {
        assertEquals("alice.eth", ContactsLive.displayName(contact(ALICE, null, "alice.eth")))
    }

    @Test
    fun anUnnamedAddressIntroducesItselfShort() {
        // Not as a blank row, and not as forty characters that wrap.
        assertEquals("0xaaaa…aaaa", ContactsLive.displayName(contact(ALICE)))
    }

    // -- sectioning -----------------------------------------------------------

    @Test
    fun contactsAreGroupedByInitial() {
        val sections = ContactsLive.letterSections(
            book(contact(ALICE, "Alice"), contact(BOB, "Bob"), contact(CAROL, "Anna")),
            query = "",
        )
        assertEquals(listOf("A", "B"), sections.map { it.letter })
        assertEquals(listOf("Alice", "Anna"), sections[0].contacts.map { it.name })
    }

    @Test
    fun theCoresOrderSurvivesInsideALetter() {
        // The core sorts favourites first, then most recent. Whatever order it
        // hands over is the order inside each letter — sectioning regroups, it
        // does not re-sort.
        val sections = ContactsLive.letterSections(
            book(contact(BOB, "Zoe"), contact(ALICE, "Zach")),
            query = "",
        )
        assertEquals(listOf("Zoe", "Zach"), sections.single().contacts.map { it.name })
    }

    @Test
    fun theHashBucketSortsLast() {
        val sections = ContactsLive.letterSections(
            book(contact(ALICE, "字节"), contact(BOB, "Bob"), contact(CAROL, "1inch")),
            query = "",
        )
        assertEquals("#", sections.last().letter)
        assertEquals(2, sections.last().contacts.size)
    }

    @Test
    fun theRailIsAlwaysTheWholeAlphabet() {
        // Letters with no section still draw — a rail that changes width as
        // contacts are added is a rail that jumps under a person's thumb.
        val model = ContactsLive.home(labels(), book(contact(ALICE, "Alice")))
        assertEquals(27, model.indexLetters.size)
        assertEquals("A", model.indexLetters.first())
        assertEquals("#", model.indexLetters.last())
    }

    // -- search ---------------------------------------------------------------

    @Test
    fun searchMatchesNameResolvedNameAndAddress() {
        val view = book(contact(ALICE, "Alice", "alice.eth"), contact(BOB, "Bob"))
        assertEquals(1, ContactsLive.letterSections(view, "ali").sumOf { it.contacts.size })
        assertEquals(1, ContactsLive.letterSections(view, "ALICE.ETH").sumOf { it.contacts.size })
        assertEquals(1, ContactsLive.letterSections(view, "0xbbbb").sumOf { it.contacts.size })
        assertEquals(2, ContactsLive.letterSections(view, "").sumOf { it.contacts.size })
    }

    @Test
    fun aSearchWithNoMatchesShowsTheEmptyState() {
        val model = ContactsLive.home(labels(), book(contact(ALICE, "Alice")), "zzz", emptyState())
        assertTrue(model.sections.isEmpty())
        assertNotNull("an empty result is a state, not a blank screen", model.empty)
    }

    // -- the list -------------------------------------------------------------

    @Test
    fun anEmptyBookShowsTheEmptyStateAndNoStrangers() {
        val model = ContactsLive.home(labels(), ContactsView(loaded = true), "", emptyState())
        assertTrue(model.sections.isEmpty())
        assertNotNull(model.empty)
    }

    @Test
    fun anUnloadedBookShowsNobodyRatherThanFixtures() {
        // The C1 fixture has six people in it. Before storage answers, a
        // signed-in person must see none of them.
        val model = ContactsLive.home(labels(), ContactsView(loaded = false))
        assertTrue(model.sections.isEmpty())
        assertTrue(model.groups.isEmpty())
    }

    // -- the detail page ------------------------------------------------------

    @Test
    fun theDetailPageShowsTheContactItWasGiven() {
        // The bug this guards is the desktop sibling's: a page that displayed
        // contact A while its actions operated on contact B. Both halves look
        // right in isolation; only the pairing is wrong.
        val fallback = ContactsFixtures.buildMobileState(ContactsScreenState.C2, strings).detail!!
        val view = book(contact(ALICE, "Alice"), contact(BOB, "Bob"))
        val detail = ContactsLive.detail(fallback, view.contacts[1], view)

        assertEquals("Bob", detail.contact.name)
        assertEquals(BOB, detail.contact.addressFull)
        // The address block carries the FULL address, split for two lines — it
        // is what the copy button puts on the clipboard.
        assertEquals(BOB, detail.address.lines.joinToString(""))
    }

    @Test
    fun theDetailPageListsTheGroupsThisContactIsIn() {
        val group = ContactGroupView(id = "g1", name = "Team", members = listOf(contact(BOB, "Bob")))
        val fallback = ContactsFixtures.buildMobileState(ContactsScreenState.C2, strings).detail!!
        val view = book(contact(ALICE, "Alice"), contact(BOB, "Bob"), groups = listOf(group))

        assertEquals(listOf("Team"), ContactsLive.detail(fallback, view.contacts[1], view).chips.groups)
        assertTrue(ContactsLive.detail(fallback, view.contacts[0], view).chips.groups.isEmpty())
    }

    @Test
    fun theListModelCarriesNoDetail() {
        // A list state with a detail attached renders the detail. The two are
        // one model, so the builder must clear what it is not showing.
        val model = ContactsLive.home(labels(), book(contact(ALICE, "Alice")))
        assertNull(model.detail)
        assertNull(model.groupDetail)
    }

    private companion object {
        const val ALICE = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        const val BOB = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        const val CAROL = "0xcccccccccccccccccccccccccccccccccccccccc"
    }
}
