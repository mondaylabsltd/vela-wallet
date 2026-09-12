package app.getvela.wallet.feature.contacts

import androidx.compose.runtime.Immutable
import app.getvela.wallet.feature.wallet.ActivityRowModel
import app.getvela.wallet.feature.wallet.TabsModel

/**
 * Contacts view models (spec 018, data-model.md — the Android port of the
 * canonical shapes; web's `src/lib/contacts/model.ts` is the sibling).
 *
 * Display-ready only (FR-005 / SC-005): pre-grouped sections, pre-formatted
 * counts, resolved labels. Nothing here fetches, sorts by collation, resolves
 * ENS or validates an address — a later "real contacts" feature swaps the
 * fixture layer that builds these and touches no component.
 */

/** Mobile gallery inventory (data-model.md §Screen states). */
enum class ContactsScreenState { C1, C1S, C1F, C2, C2S, C3, C4, C5, C6, C7, C8, C9 }

@Immutable
data class ContactModel(
    val name: String,
    /** Middle-truncated address shown under the name (mock verbatim). */
    val addressDisplay: String,
    /** Full address — the identicon seed; never lowercased at a call site. */
    val addressFull: String,
    /** Pre-computed letter-section key (fixtures group, components don't). */
    val sectionKey: String,
)

@Immutable
data class ContactSectionModel(val letter: String, val contacts: List<ContactModel>)

/** Mobile 分组 row: glyph tile, name, `N 人` + chevron. */
@Immutable
data class GroupRowModel(val name: String, val countLabel: String)

/** Search field state: idle (empty query) or filtering (query + clear). */
@Immutable
data class SearchFieldModel(val placeholder: String, val query: String = "") {
    val filtering: Boolean get() = query.isNotEmpty()
}

/** Which glyph an action row/menu item draws (models stay UI-type free). */
enum class ContactsIcon { AddContact, Import, Export, Edit, Delete, Send, Receive, Qr, MoveGroup }

@Immutable
data class MenuItemModel(
    /** Action-sink id — logged, never routed (spec Assumptions). */
    val id: String,
    val icon: ContactsIcon,
    val label: String,
    val destructive: Boolean = false,
    /** Renders the hairline above this row (C6 / M2 divider position). */
    val dividerBefore: Boolean = false,
)

/** C5 / C6 bottom-sheet menus: icon+label rows plus a separate 取消 button. */
@Immutable
data class ActionMenuModel(val items: List<MenuItemModel>, val cancel: String)

/** The delete-confirmation variant of the same sheet (c2s). */
@Immutable
data class DeleteConfirmModel(
    val title: String,
    val body: String,
    val confirm: String,
    val cancel: String,
)

/** One 转账 / 收款 / 二维码 action on the detail screen. */
@Immutable
data class ContactActionModel(val icon: ContactsIcon, val label: String)

/** Group membership pills on the detail screen (`家人` + the `+ 分组` chip). */
@Immutable
data class GroupChipsModel(val groups: List<String>, val addLabel: String)

@Immutable
data class AddressBlockModel(
    val label: String,
    /** Pre-split display lines — mobile wraps to exactly two (data-model). */
    val lines: List<String>,
    val copyLabel: String,
)

@Immutable
data class RecentActivityModel(
    val title: String,
    val action: String,
    val rows: List<ActivityRowModel>,
    /** Shown instead of rows when a contact has no history (spec edge case). */
    val empty: ContactsEmptyModel? = null,
)

/** EmptyState copy plus the optional CTA pair (C3 / search-empty). */
@Immutable
data class ContactsEmptyModel(
    val title: String,
    val caption: String,
    val primaryCta: String? = null,
    val secondaryCta: String? = null,
)

@Immutable
data class ContactDetailModel(
    val contact: ContactModel,
    val chips: GroupChipsModel,
    val actions: List<ContactActionModel>,
    val address: AddressBlockModel,
    val activity: RecentActivityModel,
    /** Spec 045 US5/US7: the favourite control and the core's inspection of this address. */
    val favourite: FavouriteControlModel? = null,
    val inspection: ContactInspectionModel? = null,
    /** Accessible name for the header pencil. */
    val editLabel: String,
    val deleteLabel: String,
)

@Immutable
data class GroupDetailModel(
    val name: String,
    val membersLabel: String,
    val members: List<ContactModel>,
    val addMemberLabel: String,
    val batchSendLabel: String,
    val batchSendHint: String,
    /** Accessible name for the header ⋯ button. */
    val manageLabel: String,
)

/** The favourite star (spec 045 US5): on or off, with 018's word for the section it files under. */
@Immutable
data class FavouriteControlModel(val on: Boolean, val label: String)

/** One sentence from the core after an import (spec 045 US6): the report, or why the file was refused. */
@Immutable
data class ContactNoticeModel(val title: String, val body: String, val close: String)

/** What the core says about the address on open (spec 045 US7): contract or wallet, and first time or not. */
@Immutable
data class ContactInspectionModel(val tag: String? = null, val firstTime: String? = null)

/**
 * The add/edit contact form (spec 045 US5, drawn from 018's vocabulary): two
 * fields, one error line, Save and Cancel. The core validates; `error` is its
 * word, `saveEnabled` its gate.
 */
@Immutable
data class ContactFormModel(
    val title: String,
    val nameLabel: String,
    val namePlaceholder: String,
    val addressLabel: String,
    val addressPlaceholder: String,
    val name: String,
    val address: String,
    val error: String? = null,
    val save: String,
    val cancel: String,
    val saveEnabled: Boolean,
    /** Editing an existing contact: the address is the contact's identity and stays put. */
    val addressLocked: Boolean = false,
    /** The core's word for a bad address, shown on the error line once something was typed. */
    val invalidAddress: String = "",
)

/**
 * One mobile contacts screen state — everything the assembly needs. Exactly one
 * of [detail] / [groupDetail] is non-null on the detail states; `null` on both
 * means the list screen (C1 family / C3).
 */
@Immutable
data class ContactsHomeModel(
    val state: ContactsScreenState,
    val title: String,
    /** Accessible name for the header person-add button. */
    val addContactLabel: String,
    /** Accessible name for the detail screens' back chevron. */
    val backLabel: String,
    val search: SearchFieldModel,
    val groupsSectionTitle: String,
    val groupsAction: String,
    val groups: List<GroupRowModel>,
    val contactsSectionTitle: String,
    val totalLabel: String,
    val sections: List<ContactSectionModel>,
    val indexLetters: List<String>,
    val tabs: TabsModel,
    val empty: ContactsEmptyModel? = null,
    val detail: ContactDetailModel? = null,
    val groupDetail: GroupDetailModel? = null,
    val menu: ActionMenuModel? = null,
    val deleteConfirm: DeleteConfirmModel? = null,
    val form: ContactFormModel? = null,
    val notice: ContactNoticeModel? = null,
    /**
     * Index of the contact whose swipe actions are revealed, counted across the
     * flattened section list (c1s pins it so the gallery renders the state with
     * no gesture); null = nothing revealed.
     */
    val revealedIndex: Int? = null,
    val swipeSendLabel: String = "",
    val swipeDeleteLabel: String = "",
    /** 1 or 1.35 — multiplies the font scale via LocalDensity (FR-011). */
    val textScale: Float = 1f,
)
