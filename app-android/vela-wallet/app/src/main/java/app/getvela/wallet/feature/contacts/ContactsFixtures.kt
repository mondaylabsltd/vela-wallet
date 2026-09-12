package app.getvela.wallet.feature.contacts

import androidx.compose.ui.graphics.Color
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.wallet.ActivityKind
import app.getvela.wallet.feature.wallet.ActivityRowModel
import app.getvela.wallet.feature.wallet.TabsModel
import app.getvela.wallet.feature.wallet.WalletFixtures

/**
 * Canonical contacts fixtures (spec 018, data-model.md — the single canon all
 * four platforms port; Android sibling of the web's `src/lib/contacts/
 * fixtures.ts`). Names, addresses, amounts and dates are DATA, verbatim across
 * locales (FR-012); every label resolves through the corpus keys in
 * contracts/i18n-keys.md.
 *
 * Only Alice's full address appears in a mock (C2); the other eight are the
 * canon's pinned inventions whose first/last four hex chars match the mock's
 * truncated display (research D7) — identicon artwork therefore will not
 * pixel-match those mock renders, but all four clients share these seeds so
 * cross-platform parity (SC-003) holds. Seeds go to the identicon exactly as
 * written; nothing lowercases them (FR-006).
 */
object ContactsFixtures {

    // --- Canon ----------------------------------------------------------------

    private data class ContactFixture(
        val name: String,
        val addressDisplay: String,
        val addressFull: String,
        val sectionKey: String,
    )

    private val ALICE = ContactFixture(
        name = "Alice",
        addressDisplay = "0x9F3c…21aE",
        addressFull = "0x9F3cA71b04E82f5C55d9B21aE00734F8Dd8021aE",
        sectionKey = "A",
    )

    private val A_HAO = ContactFixture(
        name = "阿豪",
        addressDisplay = "0x77Bd…4F02",
        addressFull = "0x77Bd59A302cC93D23dB0d0BA6a45C6830EF74F02",
        sectionKey = "A",
    )

    private val BARTHOLOMEW = ContactFixture(
        name = "Bartholomew Vanderbilt-Konstantinopoulos.eth",
        addressDisplay = "0x31c9…E77a",
        addressFull = "0x31c9A100517d2436E9E1350D383A7d0aAeC1E77a",
        sectionKey = "B",
    )

    private val BOB = ContactFixture(
        name = "Bob · 泵泵",
        addressDisplay = "0x44Aa…9C21",
        addressFull = "0x44AaF19cE84f22101b5D6cbA918B92DcA5f19C21",
        sectionKey = "B",
    )

    private val CHARLIE = ContactFixture(
        name = "Charlie",
        addressDisplay = "0x5eF0…3a9C",
        addressFull = "0x5eF0FF25a1A24E5cCb2a6D939B87F5DAb2003a9C",
        sectionKey = "C",
    )

    private val DAO_TREASURY = ContactFixture(
        name = "DAO 金库",
        addressDisplay = "0xF00d…C0de",
        addressFull = "0xF00dBaBe8712004343cD00926Ab004D6C042C0de",
        sectionKey = "D",
    )

    private val HOLD_ON = ContactFixture(
        name = "hold on",
        addressDisplay = "0xCafe…F00d",
        addressFull = "0xCafe9078B1c2A04d33Ff21B0BC934eB8A812F00d",
        sectionKey = "H",
    )

    private val MAMA = ContactFixture(
        name = "妈妈",
        addressDisplay = "0x88Ce…12aB",
        addressFull = "0x88Ce02FdB0e50D9C21e33c0F9B58a3E38f7612aB",
        sectionKey = "M",
    )

    /** Group-only member — the recorded C1-vs-DC1 mock inconsistency. */
    private val COUSIN = ContactFixture(
        name = "表弟",
        addressDisplay = "0xA1c3…88dD",
        addressFull = "0xA1c3D3d7085B90AF14E5d21C86e6dB49F30a88dD",
        sectionKey = "B",
    )

    /** The 8-contact roster, in mock order. */
    private val ROSTER = listOf(
        ALICE, A_HAO, BARTHOLOMEW, BOB, CHARLIE, DAO_TREASURY, HOLD_ON, MAMA,
    )

    /** Identicon-board seeds: the 9 canon addresses + the placeholder probe. */
    val IDENTICON_BOARD_SEEDS: List<String> =
        (ROSTER + COUSIN).map { it.addressFull } + ""

    /** Groups: name + member count (家人's roster is the only one mocked). */
    private data class GroupFixture(val name: String, val count: Int)

    private val GROUPS = listOf(
        GroupFixture("家人", 3),
        GroupFixture("工作", 5),
        GroupFixture("交易所", 2),
    )

    private val FAMILY_MEMBERS = listOf(MAMA, COUSIN, ALICE)

    const val TOTAL_CONTACTS = 8

    /** Search-active fixture (c1f): the query and the pre-filtered result. */
    const val SEARCH_QUERY = "Ali"

    private val SEARCH_RESULTS = listOf(ALICE)

    /** Component-board search-empty probe (data-model §Component boards). */
    const val NO_RESULTS_QUERY = "zzz"

    /** c1s pins 阿豪 (flattened row #1) as the swipe-revealed row. */
    const val REVEALED_INDEX = 1

    /**
     * Full A–Z + # rail — rendered whole regardless of which sections exist
     * (data-model: letters without a section jump to the nearest one).
     */
    val INDEX_LETTERS: List<String> = ('A'..'Z').map { it.toString() } + "#"

    /** Alice's 地址 block wraps to exactly these two mono lines on mobile. */
    private val ALICE_ADDRESS_LINES = listOf(
        "0x9F3cA71b04E82f5C55d9",
        "B21aE00734F8Dd8021aE",
    )

    private data class ActivityFixture(
        val kind: ActivityKind,
        /** Verbatim mock subtitle (date · chain) — data, not a template. */
        val subtitle: String,
        val amount: String,
        val unit: String,
        val positive: Boolean,
        val badgeColor: Color,
    )

    /** Alice's 最近往来 — the received row mirrors the 015 wallet fixture. */
    private val ALICE_ACTIVITY = listOf(
        ActivityFixture(
            kind = ActivityKind.Received,
            subtitle = "昨天 20:15 · Ethereum",
            amount = "+50",
            unit = "USDC",
            positive = true,
            badgeColor = WalletFixtures.ChainColors.ethereum,
        ),
        ActivityFixture(
            kind = ActivityKind.Sent,
            subtitle = "8 月 5 日 · Arbitrum",
            amount = "−0.2",
            unit = "ETH",
            positive = false,
            badgeColor = WalletFixtures.ChainColors.arbitrum,
        ),
    )

    // --- Assembly -------------------------------------------------------------

    private fun ContactFixture.toModel(): ContactModel = ContactModel(
        name = name,
        addressDisplay = addressDisplay,
        addressFull = addressFull,
        sectionKey = sectionKey,
    )

    /** Group the roster under its pre-computed letter keys (order preserved). */
    private fun sectionsOf(contacts: List<ContactFixture>): List<ContactSectionModel> {
        val sections = mutableListOf<ContactSectionModel>()
        for (c in contacts) {
            val last = sections.lastOrNull()
            if (last != null && last.letter == c.sectionKey) {
                sections[sections.lastIndex] = last.copy(contacts = last.contacts + c.toModel())
            } else {
                sections += ContactSectionModel(c.sectionKey, listOf(c.toModel()))
            }
        }
        return sections
    }

    private fun activityRow(strings: VelaStrings, f: ActivityFixture): ActivityRowModel =
        ActivityRowModel(
            kind = f.kind,
            title = when (f.kind) {
                ActivityKind.Received -> strings.t(I18nKeys.Contacts.LABEL_RECEIVED)
                else -> strings.t(I18nKeys.Contacts.LABEL_SENT)
            },
            subtitle = f.subtitle,
            amount = f.amount,
            unit = f.unit,
            positive = f.positive,
            masked = false,
            badgeColor = f.badgeColor,
        )

    private fun tabs(strings: VelaStrings): TabsModel = TabsModel(
        wallet = strings.t(I18nKeys.Contacts.NAV_WALLET),
        contacts = strings.t(I18nKeys.Contacts.NAV_CONTACTS),
        explore = strings.t(I18nKeys.Contacts.NAV_EXPLORE),
        settings = strings.t(I18nKeys.Contacts.NAV_SETTINGS),
    )

    private fun groupRows(strings: VelaStrings): List<GroupRowModel> = GROUPS.map {
        GroupRowModel(
            name = it.name,
            countLabel = strings.t(
                I18nKeys.Contacts.GROUP_MEMBERS,
                mapOf("count" to it.count.toString()),
            ),
        )
    }

    /** C3 / DC3 empty state: title, caption and the two CTAs. */
    fun emptyState(strings: VelaStrings): ContactsEmptyModel = ContactsEmptyModel(
        title = strings.t(I18nKeys.Contacts.EMPTY),
        caption = strings.t(I18nKeys.Contacts.EMPTY_HINT),
        primaryCta = strings.t(I18nKeys.Contacts.ADD_CONTACT),
        secondaryCta = strings.t(I18nKeys.Contacts.IMPORT_FILE),
    )

    /** Search-empty treatment (not mocked; reuses EmptyState per spec edge case). */
    fun searchEmptyState(strings: VelaStrings): ContactsEmptyModel = ContactsEmptyModel(
        title = strings.t(
            I18nKeys.Contacts.NO_RESULTS,
            mapOf("query" to NO_RESULTS_QUERY),
        ),
        caption = strings.t(I18nKeys.Contacts.EMPTY_HINT),
    )

    /** C5 add / import / export sheet. */
    fun addMenu(strings: VelaStrings): ActionMenuModel = ActionMenuModel(
        items = listOf(
            MenuItemModel(
                id = "contacts.addTitle",
                icon = ContactsIcon.AddContact,
                label = strings.t(I18nKeys.Contacts.ADD_TITLE),
            ),
            MenuItemModel(
                id = "contacts.importFile",
                icon = ContactsIcon.Import,
                label = strings.t(I18nKeys.Contacts.IMPORT_FILE),
            ),
            MenuItemModel(
                id = "contacts.exportTitle",
                icon = ContactsIcon.Export,
                label = strings.t(I18nKeys.Contacts.EXPORT_TITLE),
            ),
        ),
        cancel = strings.t(I18nKeys.Contacts.CANCEL),
    )

    /** C6 group-actions sheet (divider + destructive 删除分组). */
    fun groupMenu(strings: VelaStrings): ActionMenuModel = ActionMenuModel(
        items = listOf(
            MenuItemModel(
                id = "contacts.groupEdit",
                icon = ContactsIcon.Edit,
                label = strings.t(I18nKeys.Contacts.GROUP_EDIT),
            ),
            MenuItemModel(
                id = "contacts.importGroup",
                icon = ContactsIcon.Import,
                label = strings.t(I18nKeys.Contacts.IMPORT_GROUP),
            ),
            MenuItemModel(
                id = "contacts.exportGroup",
                icon = ContactsIcon.Export,
                label = strings.t(I18nKeys.Contacts.EXPORT_GROUP),
            ),
            MenuItemModel(
                id = "contacts.groupDelete",
                icon = ContactsIcon.Delete,
                label = strings.t(I18nKeys.Contacts.GROUP_DELETE),
                destructive = true,
                dividerBefore = true,
            ),
        ),
        cancel = strings.t(I18nKeys.Contacts.CANCEL),
    )

    /** c2s destructive confirmation (names the contact being removed). */
    fun deleteConfirm(strings: VelaStrings, name: String = ALICE.name): DeleteConfirmModel =
        DeleteConfirmModel(
            title = strings.t(I18nKeys.Contacts.DELETE_TITLE),
            body = strings.t(I18nKeys.Contacts.DELETE_BODY, mapOf("name" to name)),
            confirm = strings.t(I18nKeys.Contacts.DELETE),
            cancel = strings.t(I18nKeys.Contacts.CANCEL),
        )

    /** C2 / DC2 contact detail (Alice). */
    fun contactDetail(strings: VelaStrings): ContactDetailModel = ContactDetailModel(
        contact = ALICE.toModel(),
        chips = GroupChipsModel(
            groups = listOf(GROUPS[0].name),
            addLabel = strings.t(I18nKeys.Contacts.SECTION_GROUPS),
        ),
        actions = listOf(
            ContactActionModel(ContactsIcon.Send, strings.t(I18nKeys.Contacts.ACTION_SEND)),
            ContactActionModel(ContactsIcon.Receive, strings.t(I18nKeys.Contacts.ACTION_RECEIVE)),
            ContactActionModel(ContactsIcon.Qr, strings.t(I18nKeys.Contacts.ACTION_QR)),
        ),
        address = AddressBlockModel(
            label = strings.t(I18nKeys.Contacts.ADDRESS_LABEL),
            lines = ALICE_ADDRESS_LINES,
            copyLabel = strings.t(I18nKeys.Contacts.COPY_ADDRESS),
        ),
        activity = RecentActivityModel(
            title = strings.t(I18nKeys.Contacts.RECENT_ACTIVITY),
            action = strings.t(I18nKeys.Contacts.FILTER_ALL),
            rows = ALICE_ACTIVITY.map { activityRow(strings, it) },
        ),
        editLabel = strings.t(I18nKeys.Contacts.EDIT),
        deleteLabel = strings.t(I18nKeys.Contacts.DELETE_CONTACT),
    )

    /** C7 / C8 — the add form (empty, Save gated) and the edit form (filled). */
    fun contactForm(strings: VelaStrings, edit: Boolean): ContactFormModel = ContactFormModel(
        title = strings.t(if (edit) I18nKeys.Contacts.EDIT_TITLE else I18nKeys.Contacts.ADD_TITLE),
        nameLabel = strings.t(I18nKeys.Contacts.NAME_LABEL),
        namePlaceholder = strings.t(I18nKeys.Contacts.NAME_PLACEHOLDER),
        addressLabel = strings.t(I18nKeys.Contacts.ADDRESS_LABEL),
        addressPlaceholder = strings.t(I18nKeys.Contacts.ADDRESS_PLACEHOLDER),
        name = if (edit) ALICE.name else "",
        address = if (edit) ALICE.addressFull else "",
        error = null,
        save = strings.t(I18nKeys.Contacts.SAVE),
        cancel = strings.t(I18nKeys.Contacts.CANCEL),
        saveEnabled = edit,
        addressLocked = edit,
        invalidAddress = strings.t(I18nKeys.Contacts.INVALID_ADDRESS),
    )

    /** C9 — the detail with the star on and the core's inspection drawn. */
    fun contactDetailInspected(strings: VelaStrings): ContactDetailModel = contactDetail(strings).copy(
        favourite = FavouriteControlModel(on = true, label = strings.t(I18nKeys.Contacts.SECTION_FAVORITES)),
        inspection = ContactInspectionModel(
            tag = strings.t(I18nKeys.Contacts.WALLET_TAG),
            firstTime = strings.t(I18nKeys.Contacts.FIRST_TIME_TAG),
        ),
    )

    /**
     * Contact-with-no-activity variant (spec edge case; not mocked) — the
     * 最近往来 section falls back to the reused empty treatment.
     */
    fun contactDetailNoActivity(strings: VelaStrings): ContactDetailModel =
        contactDetail(strings).let { detail ->
            detail.copy(
                activity = detail.activity.copy(
                    rows = emptyList(),
                    empty = ContactsEmptyModel(
                        title = strings.t(I18nKeys.Contacts.EMPTY),
                        caption = strings.t(I18nKeys.Contacts.EMPTY_HINT),
                    ),
                ),
            )
        }

    /** C4 group detail (家人). */
    fun groupDetail(strings: VelaStrings): GroupDetailModel = GroupDetailModel(
        name = GROUPS[0].name,
        membersLabel = strings.t(
            I18nKeys.Contacts.MEMBERS_COUNT,
            mapOf("count" to GROUPS[0].count.toString()),
        ),
        members = FAMILY_MEMBERS.map { it.toModel() },
        addMemberLabel = strings.t(I18nKeys.Contacts.ADD_MEMBER),
        batchSendLabel = strings.t(I18nKeys.Contacts.BATCH_SEND),
        batchSendHint = strings.t(
            I18nKeys.Contacts.BATCH_SEND_HINT,
            mapOf("count" to GROUPS[0].count.toString()),
        ),
        manageLabel = strings.t(I18nKeys.Contacts.MANAGE),
    )

    /** Empty-group variant (spec edge case; not mocked). */
    fun emptyGroupDetail(strings: VelaStrings): GroupDetailModel =
        groupDetail(strings).copy(
            members = emptyList(),
            membersLabel = strings.t(I18nKeys.Contacts.MEMBERS_COUNT, mapOf("count" to "0")),
            batchSendHint = strings.t(I18nKeys.Contacts.BATCH_SEND_HINT, mapOf("count" to "0")),
        )

    /** Assemble the mobile contacts view model for one C-state. */
    fun buildMobileState(state: ContactsScreenState, strings: VelaStrings): ContactsHomeModel {
        val searching = state == ContactsScreenState.C1F
        val empty = state == ContactsScreenState.C3
        // Pre-filtered, never computed: the c1f variant ships its own roster
        // (spec Assumptions — search filtering is fixture-side).
        val roster = when {
            searching -> SEARCH_RESULTS
            empty -> emptyList()
            else -> ROSTER
        }

        val base = ContactsHomeModel(
            state = state,
            title = strings.t(I18nKeys.Contacts.TITLE),
            addContactLabel = strings.t(I18nKeys.Contacts.ADD_CONTACT),
            // The corpus' only generic 返回; recorded as an extra reused key
            // beyond contracts/i18n-keys.md's table (results note).
            backLabel = strings.t(I18nKeys.Flow.BACK),
            search = SearchFieldModel(
                placeholder = strings.t(I18nKeys.Contacts.SEARCH_PLACEHOLDER),
                query = if (searching) SEARCH_QUERY else "",
            ),
            groupsSectionTitle = strings.t(I18nKeys.Contacts.SECTION_GROUPS),
            groupsAction = strings.t(I18nKeys.Contacts.GROUP_NEW),
            groups = if (empty || searching) emptyList() else groupRows(strings),
            contactsSectionTitle = strings.t(I18nKeys.Contacts.SECTION_CONTACTS),
            totalLabel = strings.t(
                I18nKeys.Contacts.COUNT_PEOPLE,
                mapOf("count" to roster.size.toString()),
            ),
            sections = sectionsOf(roster),
            indexLetters = INDEX_LETTERS,
            tabs = tabs(strings),
            swipeSendLabel = strings.t(I18nKeys.Contacts.ACTION_SEND),
            swipeDeleteLabel = strings.t(I18nKeys.Contacts.DELETE),
        )

        return when (state) {
            ContactsScreenState.C1, ContactsScreenState.C1F -> base

            ContactsScreenState.C1S -> base.copy(revealedIndex = REVEALED_INDEX)

            ContactsScreenState.C2 -> base.copy(detail = contactDetail(strings))
            ContactsScreenState.C7 -> base.copy(form = contactForm(strings, edit = false))
            ContactsScreenState.C8 -> base.copy(detail = contactDetail(strings), form = contactForm(strings, edit = true))
            ContactsScreenState.C9 -> base.copy(detail = contactDetailInspected(strings))

            ContactsScreenState.C2S -> base.copy(
                detail = contactDetail(strings),
                deleteConfirm = deleteConfirm(strings),
            )

            ContactsScreenState.C3 -> base.copy(empty = emptyState(strings))

            ContactsScreenState.C4 -> base.copy(groupDetail = groupDetail(strings))

            ContactsScreenState.C5 -> base.copy(menu = addMenu(strings))

            ContactsScreenState.C6 -> base.copy(
                groupDetail = groupDetail(strings),
                menu = groupMenu(strings),
            )
        }
    }
}
