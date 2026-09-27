package app.getvela.wallet.feature.contacts

import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.contacts.core.Contact
import app.getvela.wallet.feature.contacts.core.ContactGroupView
import app.getvela.wallet.feature.contacts.core.ContactsView
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.wallet.core.FeedRow
import app.getvela.wallet.feature.wallet.core.FeedView

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
        if (!view.loaded) {
            return fallback.copy(
                search = fallback.search.copy(query = query),
                sections = emptyList(),
                groups = emptyList(),
                empty = null,
                pending = true,
            )
        }
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
    fun detail(
        fallback: ContactDetailModel,
        contact: Contact,
        view: ContactsView,
        feed: FeedView = FeedView(),
        strings: VelaStrings? = null,
    ): ContactDetailModel {
        val model = toContactModel(contact)
        // Only the rows whose counterparty IS this person. Matched on the
        // address rather than the name: a name is a label somebody typed and
        // two contacts can share one, while the address is the identity the
        // payment actually went to.
        val withThisPerson = if (strings == null) {
            emptyList()
        } else {
            WalletLive.activity(
                FeedView(
                    rows = feed.rows.filter { row ->
                        row !is FeedRow.Item ||
                            row.item.counterparty.equals(contact.address, ignoreCase = true)
                    },
                ),
                strings,
            ).flatMap { it.rows }
        }
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
            // **This person's transactions, or nothing.**
            //
            // An earlier version left the fallback's rows alone, on the
            // assumption that the fallback carried none — and the C2 fixture
            // carries two, so a freshly saved contact was shown "+50 USDC
            // received yesterday" for a payment that never happened. Found on a
            // device, in the one place a wrong answer looks completely
            // ordinary.
            //
            // The empty BLOCK stays cleared, and that is not tidiness.
            // `contactDetailNoActivity` fills it with `contacts.empty` —
            // "还没有联系人 / 添加常用地址…" — which reads as nonsense under
            // 最近往来 on a page that is showing a contact. There is still no
            // i18n key for "no transactions with this person yet", and
            // inventing product copy is not this feature's call, so a contact
            // with no history keeps its heading and stands empty.
            activity = fallback.activity.copy(rows = withThisPerson, empty = null),
            // Spec 045 US5/US7: the star is the contact's flag; the inspection
            // is the core's `recipient` when it is about THIS address — a
            // contract-or-wallet verdict only once judged (`is_contract` null =
            // not judged, never "wallet"), first-time from its history rule.
            favourite = strings?.let { FavouriteControlModel(on = contact.favorite, label = it.t(I18nKeys.Contacts.SECTION_FAVORITES)) },
            inspection = strings?.let { s ->
                view.recipient?.takeIf { it.address.equals(contact.address, ignoreCase = true) }?.let { seen ->
                    ContactInspectionModel(
                        tag = when (seen.is_contract) {
                            true -> s.t(I18nKeys.Contacts.CONTRACT_TAG)
                            false -> s.t(I18nKeys.Contacts.WALLET_TAG)
                            null -> null
                        },
                        firstTime = if (seen.first_interaction) s.t(I18nKeys.Contacts.FIRST_TIME_TAG) else null,
                    )
                }
            },
        )
    }

    /** The address as the detail block draws it: two lines, halfway. */
    private fun splitAddress(address: String): List<String> {
        if (address.length <= 2) return listOf(address)
        val half = (address.length + 1) / 2
        return listOf(address.take(half), address.drop(half))
    }

    /** One group's page: its own members, in the core's membership order. */
    /**
     * The add/edit form (spec 045 US5): the web's rule verbatim — a valid
     * 0x address, a non-empty name; the address error shows once something
     * was typed there. Editing keeps the address (it is the contact's identity).
     */
    fun form(fallback: ContactFormModel, edit: Boolean, name: String, address: String): ContactFormModel {
        val trimmed = address.trim()
        val valid = ADDRESS.matches(trimmed)
        return fallback.copy(
            name = name,
            address = address,
            error = if (trimmed.isNotEmpty() && !valid) fallback.error ?: fallback.invalidWord() else null,
            saveEnabled = valid && name.isNotBlank(),
            addressLocked = edit,
        )
    }

    private fun ContactFormModel.invalidWord(): String = invalidAddress

    /** After an import (spec 045 US6): the core's report, or its refusal — one sheet, one sentence, one Close. */
    fun importNotice(view: ContactsView, strings: VelaStrings, close: String): ContactNoticeModel? {
        view.import_failure?.let {
            return ContactNoticeModel(
                title = strings.t(I18nKeys.Contacts.IMPORT_FAIL_TITLE),
                body = strings.t(I18nKeys.Contacts.IMPORT_FAIL_BODY),
                close = close,
            )
        }
        val report = view.last_import ?: return null
        val body = strings.t(I18nKeys.Contacts.IMPORT_DONE_BODY, mapOf("added" to report.added.toString(), "skipped" to report.skipped.toString()))
        val invalid = if (report.invalid > 0) " " + strings.t(I18nKeys.Contacts.IMPORT_DONE_INVALID, mapOf("invalid" to report.invalid.toString())) else ""
        return ContactNoticeModel(title = strings.t(I18nKeys.Contacts.IMPORT_DONE_TITLE), body = body + invalid, close = close)
    }

    private val ADDRESS = Regex("^0x[0-9a-fA-F]{40}$")

    fun groupDetail(fallback: GroupDetailModel, group: ContactGroupView): GroupDetailModel =
        fallback.copy(
            name = group.name,
            members = group.members.map(::toContactModel),
            membersLabel = fallback.membersLabel
                .replaceFirst(Regex("\\d+"), group.members.size.toString()),
        )
}
