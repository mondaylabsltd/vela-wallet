package app.getvela.wallet.feature.contacts

import app.getvela.wallet.feature.contacts.core.Contact
import app.getvela.wallet.feature.contacts.core.ContactGroupView
import app.getvela.wallet.feature.contacts.core.ContactsView

/**
 * The live contacts builders: `ContactsView` → the display models the drawn
 * components already consume.
 *
 * The sibling of [ContactsFixtures]; the gallery keeps its canon, this renders
 * one person's book.
 *
 * **Two presentation judgements live here, and they are judgements rather than
 * rules** — the same two the web sibling documents at
 * `app-web/vela-wallet/src/lib/contacts/live.ts:5-13`:
 *
 * - **Letter sectioning.** The core's order (favourites first, then most
 *   recent) is authoritative; the list PAGE presents an A–Z directory, so
 *   contacts are grouped by initial and keep the core's relative order inside
 *   each letter. Same class of work as date-grouping a feed.
 * - **Search filtering.** The core has no list-search event — its
 *   `matches_query` serves the recipient picker — so narrowing the rendered
 *   list by the box's text is display-side filtering of core-ruled rows.
 *
 * Both are ported case-for-case from the web builder rather than re-derived,
 * because two clients that section the same book differently is a bug nobody
 * would notice until a person used both.
 */
object ContactsLive {

    /** The rail is always the full alphabet; letters with no section still draw. */
    private val INDEX_LETTERS: List<String> = ('A'..'Z').map { it.toString() } + "#"

    /**
     * What the person calls this contact.
     *
     * Their own name wins over a resolved one, and an unnamed address
     * introduces itself by its short form rather than as an empty row.
     */
    fun displayName(contact: Contact): String =
        contact.name ?: contact.resolved_name ?: shortenAddress(contact.address)

    /** `0x1234…cdef` — the same shape the address block and rows already show. */
    fun shortenAddress(address: String): String =
        if (address.length <= 12) address else "${address.take(6)}…${address.takeLast(4)}"

    private fun sectionLetter(name: String): String {
        val first = name.trim().firstOrNull()?.uppercaseChar() ?: '#'
        return if (first in 'A'..'Z') first.toString() else "#"
    }

    private fun matches(contact: Contact, query: String): Boolean {
        if (query.isEmpty()) return true
        val needle = query.trim().lowercase()
        return displayName(contact).lowercase().contains(needle) ||
            (contact.resolved_name ?: "").lowercase().contains(needle) ||
            contact.address.contains(needle)
    }

    private fun toContactModel(contact: Contact): ContactModel = ContactModel(
        name = displayName(contact),
        addressDisplay = shortenAddress(contact.address),
        // Never lowercased at a call site: this is the identicon seed AND the
        // identity every action on this row acts upon.
        addressFull = contact.address,
        sectionKey = sectionLetter(displayName(contact)),
    )

    /** Group by initial; the core's relative order survives inside each letter. */
    fun letterSections(view: ContactsView, query: String): List<ContactSectionModel> {
        val byLetter = LinkedHashMap<String, MutableList<ContactModel>>()
        for (contact in view.contacts) {
            if (!matches(contact, query)) continue
            val model = toContactModel(contact)
            byLetter.getOrPut(model.sectionKey) { mutableListOf() }.add(model)
        }
        return byLetter.entries
            .sortedWith(
                compareBy(
                    // `#` last, always: it is the bucket for everything that is
                    // not a letter, and a directory that opens with it reads as
                    // broken.
                    { if (it.key == "#") 1 else 0 },
                    { it.key },
                ),
            )
            .map { ContactSectionModel(letter = it.key, contacts = it.value) }
    }

    /**
     * The list screen, from the person's own book.
     *
     * [fallback] supplies the labels — titles, the empty-state copy, the tab
     * bar — which are content rather than data, and which the fixture builder
     * has already resolved through the i18n engine.
     */
    fun home(
        fallback: ContactsHomeModel,
        view: ContactsView,
        query: String = "",
        /**
         * The empty-state copy, which lives in a DIFFERENT fixture state — C3,
         * the empty book — because the C1 fixture that supplies every other
         * label is a populated list and carries none. A device with nothing
         * saved is the first thing a new person sees, so getting this from the
         * wrong state means showing them a blank screen.
         */
        emptyState: ContactsEmptyModel? = fallback.empty,
    ): ContactsHomeModel {
        if (!view.loaded) return fallback.copy(sections = emptyList(), groups = emptyList())
        val sections = letterSections(view, query)
        val total = sections.sumOf { it.contacts.size }
        return fallback.copy(
            search = fallback.search.copy(query = query),
            groups = view.groups.map { group ->
                GroupRowModel(
                    name = group.name,
                    countLabel = fallback.groups.firstOrNull()?.countLabel.orEmpty()
                        .replaceFirst(Regex("\\d+"), group.members.size.toString()),
                )
            },
            sections = sections,
            indexLetters = INDEX_LETTERS,
            totalLabel = fallback.totalLabel.replaceFirst(Regex("\\d+"), total.toString()),
            // The empty state is a real state, not an absence of one: a device
            // with nothing saved must say so rather than showing a bare screen.
            empty = if (total == 0) emptyState else null,
            detail = null,
            groupDetail = null,
        )
    }

    /**
     * One contact's detail page.
     *
     * **The identity travels with the model.** Every action on this page takes
     * its target from [ContactDetailModel.contact], never from a separately
     * held index — the desktop sibling shipped a page that displayed contact A
     * while its delete acted on contact B, and it survived three review passes
     * because both halves looked right in isolation.
     */
    fun detail(fallback: ContactDetailModel, contact: Contact, view: ContactsView): ContactDetailModel {
        val model = toContactModel(contact)
        return fallback.copy(
            contact = model,
            chips = fallback.chips.copy(
                groups = view.groups
                    .filter { group -> group.members.any { it.address == contact.address } }
                    .map { it.name },
            ),
            // Two lines, split at the halfway point — the mobile block wraps to
            // exactly two and the fixture's own lines are pre-split the same
            // way. The FULL address, never the shortened one: this is the value
            // the copy button puts on the clipboard.
            address = fallback.address.copy(lines = splitAddress(contact.address)),
            // **Cleared, not inherited.** // live in 041
            //
            // The recent-activity block needs the local transaction store spec
            // 041 brings. An earlier version of this line left the fallback's
            // rows alone, on the assumption that the fallback carried none —
            // and the C2 fixture carries two, so a freshly saved contact was
            // shown "+50 USDC received yesterday" for a transaction that never
            // happened. Found on a device, in the one place a wrong answer
            // looks completely ordinary.
            //
            // The empty BLOCK is cleared too, and that is not tidiness.
            // `contactDetailNoActivity` fills it with `contacts.empty` —
            // "还没有联系人 / 添加常用地址…" — which reads as nonsense under
            // 最近往来 on a page that is showing a contact. The fixture could
            // reuse it because it was never on a live screen; here it is on
            // every one. There is no i18n key for "no transactions with this
            // person yet", and inventing product copy is not this feature's
            // call, so the section keeps its heading and stands empty until
            // spec 041 gives it both a history and a sentence.
            activity = fallback.activity.copy(rows = emptyList(), empty = null),
        )
    }

    /** The address as the detail block draws it: two lines, halfway. */
    private fun splitAddress(address: String): List<String> {
        if (address.length <= 2) return listOf(address)
        val half = (address.length + 1) / 2
        return listOf(address.take(half), address.drop(half))
    }

    /** One group's page: its own members, in the core's membership order. */
    fun groupDetail(fallback: GroupDetailModel, group: ContactGroupView): GroupDetailModel =
        fallback.copy(
            name = group.name,
            members = group.members.map(::toContactModel),
            membersLabel = fallback.membersLabel
                .replaceFirst(Regex("\\d+"), group.members.size.toString()),
        )
}
