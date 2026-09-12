package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.contacts.ContactsFixtures
import app.getvela.wallet.feature.contacts.ContactsLive
import app.getvela.wallet.feature.contacts.ContactsScreenState
import app.getvela.wallet.feature.contacts.core.Contact
import app.getvela.wallet.feature.contacts.core.ContactGroupView
import app.getvela.wallet.feature.contacts.core.ContactsView
import app.getvela.wallet.feature.wallet.core.FeedView
import app.getvela.wallet.feature.wallet.core.FeedRow
import app.getvela.wallet.feature.wallet.core.FeedItem
import app.getvela.wallet.feature.wallet.core.FeedDirection
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
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }
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
    fun theDetailPageInventsNoTransactions() {
        // The bug this pins was found on a device, not here: the builder kept
        // the fallback's activity rows, and the C2 fixture has two — so a
        // contact saved a minute ago was shown "+50 USDC received yesterday".
        // Handed the POPULATED fixture on purpose, because that is what the
        // call site did wrong.
        val populated = ContactsFixtures.buildMobileState(ContactsScreenState.C2, strings).detail!!
        assertTrue("the fixture really does carry rows", populated.activity.rows.isNotEmpty())

        val view = book(contact(ALICE, "Alice"))
        val detail = ContactsLive.detail(populated, view.contacts[0], view)
        assertTrue(
            "a live contact has no history until spec 041 gives it one",
            detail.activity.rows.isEmpty(),
        )
        // And no borrowed empty copy either: the no-activity fixture fills that
        // block with the CONTACTS-list empty text, which under 最近往来 tells a
        // person they have no contacts on a page showing one of them.
        assertNull(
            "the activity block must not borrow the contact-list empty copy",
            detail.activity.empty,
        )
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

    // -- the detail page's activity block (spec 041 phase 7) -----------------

    private fun feedItem(counterparty: String, value: String) = FeedRow.Item(
        FeedItem(
            id = "tx-$counterparty-$value",
            direction = FeedDirection.Out,
            counterparty = counterparty,
            value = value,
            symbol = "USDC",
            decimals = 6,
            chain_id = 137,
            timestamp = System.currentTimeMillis() / 1000.0,
            day_start_ms = midnightToday(),
        ),
    )

    private fun midnightToday(): Double = java.util.Calendar.getInstance().apply {
        set(java.util.Calendar.HOUR_OF_DAY, 0)
        set(java.util.Calendar.MINUTE, 0)
        set(java.util.Calendar.SECOND, 0)
        set(java.util.Calendar.MILLISECOND, 0)
    }.timeInMillis.toDouble()

    private fun feedWith(vararg rows: FeedRow) = FeedView(
        rows = listOf(
            FeedRow.Header("day-today", midnightToday(), System.currentTimeMillis() / 1000.0),
        ) + rows.toList(),
    )

    /**
     * The device bug, still pinned.
     *
     * The C2 fixture carries two transactions. A freshly saved contact that
     * inherited them was shown "+50 USDC received yesterday" for a payment that
     * never happened — the kind of wrong answer that looks completely ordinary.
     */
    @Test
    fun `a contact with no history shows no transactions`() {
        val contact = contact("0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", "Alice")

        val detail = ContactsLive.detail(
            fallback = ContactsFixtures.buildMobileState(ContactsScreenState.C2, strings).detail!!,
            contact = contact,
            view = ContactsView(contacts = listOf(contact)),
            feed = FeedView(),
            strings = strings,
        )

        assertEquals(emptyList<Any>(), detail.activity.rows)
        // And no empty-state block either: `contacts.empty` reads as
        // "还没有联系人 / 添加常用地址" under 最近往来, which is nonsense on a
        // page that is showing a contact.
        assertNull(detail.activity.empty)
    }

    /** Payments to THIS person, and only to this person. */
    @Test
    fun `the activity block shows this contact's own transactions`() {
        val alice = contact("0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", "Alice")
        val feed = feedWith(
            feedItem("0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", "50"),
            feedItem("0xBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB", "70"),
            feedItem("0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", "20"),
        )

        val detail = ContactsLive.detail(
            fallback = ContactsFixtures.buildMobileState(ContactsScreenState.C2, strings).detail!!,
            contact = alice,
            view = ContactsView(contacts = listOf(alice)),
            feed = feed,
            strings = strings,
        )

        assertEquals(2, detail.activity.rows.size)
        assertEquals(listOf("−50", "−20"), detail.activity.rows.map { it.amount })
    }

    /** An address is an identity; a name is a label two contacts can share. */
    @Test
    fun `matching is by address, not by name`() {
        val alice = contact("0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", "Alice")
        // Same capitalisation difference a real address book produces.
        val feed = feedWith(feedItem("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "50"))

        val detail = ContactsLive.detail(
            fallback = ContactsFixtures.buildMobileState(ContactsScreenState.C2, strings).detail!!,
            contact = alice,
            view = ContactsView(contacts = listOf(alice)),
            feed = feed,
            strings = strings,
        )

        assertEquals(1, detail.activity.rows.size)
    }
}
