/**
 * Contacts view models (spec 018, data-model.md).
 *
 * Display-ready shapes only — no service types, no collation, no ENS, no
 * validation (spec FR-005 / SC-002 / SC-005). A later "real contacts" feature
 * replaces the fixture layer that builds them and nothing else.
 */
import type { UtilityIconId } from '$lib/wallet/icons';
import type { ActivityRowModel, ChainRowModel, WalletHeaderModel } from '$lib/wallet/model';

export type MobileContactsStateId = 'c1' | 'c1s' | 'c1f' | 'c2' | 'c2s' | 'c3' | 'c4' | 'c5' | 'c6';

export type DesktopContactsStateId = 'dc1' | 'dc2' | 'dc3' | 'dc4' | 'dc5' | 'dc6' | 'dc2n';

export const MOBILE_CONTACTS_STATES: MobileContactsStateId[] = [
	'c1',
	'c1s',
	'c1f',
	'c2',
	'c2s',
	'c3',
	'c4',
	'c5',
	'c6'
];

export const DESKTOP_CONTACTS_STATES: DesktopContactsStateId[] = [
	'dc1',
	'dc2',
	'dc3',
	'dc4',
	'dc5',
	'dc6',
	'dc2n'
];

/** One contact as a row renders it. `addressFull` is also the identicon seed. */
export interface ContactModel {
	name: string;
	addressDisplay: string;
	addressFull: string;
	/** Inline SVG from vela-core, seeded by `addressFull` (never lowercased here). */
	identiconSvg: string;
	groups: string[];
}

export interface LetterSectionModel {
	letter: string;
	contacts: ContactModel[];
}

export interface GroupModel {
	/** The core's group id — present on live models, absent in fixtures. */
	id?: string;
	name: string;
	/** Mobile trailing label, e.g. "3 人". */
	countLabel: string;
	/** Desktop rail trailing count, e.g. "3". */
	count: string;
	/** Detail-header label, e.g. "3 位成员". */
	membersLabel: string;
	members: ContactModel[];
}

export interface MenuItemModel {
	icon: UtilityIconId;
	label: string;
	destructive?: boolean;
	/** Hairline below this row (M2 / C6 divider). */
	dividerAfter?: boolean;
}

export type MenuKind = 'sheet' | 'dropdown' | 'context';

export interface MenuModel {
	kind: MenuKind;
	/** Accessible name; sheets render it invisibly (C5/C6 have no visible title). */
	label: string;
	items: MenuItemModel[];
	/** Sheets end with a separate cancel button; anchored menus do not. */
	cancel?: string;
}

export interface ConfirmModel {
	title: string;
	body: string;
	confirm: string;
	cancel: string;
}

export interface SearchModel {
	placeholder: string;
	/** Present when the fixture pre-filters the list (c1f). */
	query?: string;
	/** Desktop header badge. */
	shortcut?: string;
}

export interface ContactActionsModel {
	send: string;
	receive: string;
	qr: string;
}

export interface AddressBlockModel {
	label: string;
	/** Mobile wraps to exactly these lines; desktop joins them. */
	lines: string[];
	full: string;
	copyLabel: string;
}

export interface ContactDetailModel {
	contact: ContactModel;
	chips: string[];
	addChipLabel: string;
	actions: ContactActionsModel;
	address: AddressBlockModel;
	activityTitle: string;
	/** Mobile trailing action (全部 ›). */
	activityAction: string;
	/** Desktop link below the rows (查看全部往来). */
	activityLink: string;
	rows: ActivityRowModel[];
	/** Live only: the reused history empty state when the feed has no row for this contact. */
	emptyActivity?: string;
	editLabel: string;
	deleteLabel: string;
}

export interface GroupDetailModel {
	group: GroupModel;
	addMember: string;
	cta: string;
	ctaCaption: string;
	/** DC4 renders one titled caption line instead of a pinned bar + caption. */
	captionTitled: string;
	menuLabel: string;
}

export interface EmptyCtaModel {
	title: string;
	caption: string;
	primary: string;
	secondary: string;
}

export interface ContactsListModel {
	search: SearchModel;
	groupsTitle: string;
	groupsAction: string;
	groups: GroupModel[];
	contactsTitle: string;
	/** "8 位" — plain trailing text, not an action. */
	contactsCount: string;
	sections: LetterSectionModel[];
	/** Full A–Z + # rail (research.md D4) — letters without a section still render. */
	indexLetters: string[];
	/** c1s pins the swipe-revealed row so the gallery shows it statically. */
	revealed?: { letter: string; index: number };
	swipeActions: { send: string; delete: string };
	/** Search-empty treatment when the pre-filtered fixture matches nothing. */
	noResults?: string;
}

export type ContactsScreenKind = 'list' | 'detail' | 'group' | 'empty';

export interface ContactsHomeModel {
	state: MobileContactsStateId;
	screen: ContactsScreenKind;
	title: string;
	addLabel: string;
	editLabel: string;
	menuLabel: string;
	backLabel: string;
	list?: ContactsListModel;
	detail?: ContactDetailModel;
	group?: GroupDetailModel;
	empty?: EmptyCtaModel;
	/** Fixture-opened bottom sheet: an action menu (c5/c6) or a confirm (c2s). */
	sheet?: MenuModel;
	confirm?: ConfirmModel;
	tabs: { wallet: string; contacts: string; explore: string; settings: string };
}

export type ContactsPanelId = 'none' | 'contact-detail';

export interface GroupRailModel {
	allLabel: string;
	allCount: string;
	allSelected: boolean;
	groupsTitle: string;
	groups: GroupModel[];
	selectedGroup?: string;
	newGroup: string;
	/** Static gallery variant: the row highlighted as a drag drop-target. */
	dropTarget?: string;
}

export interface ContactsDesktopModel {
	state: DesktopContactsStateId;
	sidebar: {
		header: WalletHeaderModel;
		nav: { id: 'wallet' | 'contacts' | 'explore' | 'settings'; label: string; selected: boolean }[];
		networksTitle: string;
		networks: ChainRowModel[];
	};
	title: string;
	search: SearchModel;
	addLabel: string;
	menuLabel: string;
	rail: GroupRailModel;
	/** Present for the all-contacts list views (dc1, dc2, dc5, dc2n). */
	sections: LetterSectionModel[];
	/** Present for the group view (dc4, dc6). */
	group?: GroupDetailModel;
	empty?: EmptyCtaModel;
	/** The third column's body — absent on a live page until a contact is chosen. */
	detail?: ContactDetailModel;
	panelTitle: string;
	initialPanel: ContactsPanelId;
	/** Which list row renders selected (the one the third column shows). */
	selectedContact?: string;
	/** dc2n pins the overlay mode regardless of viewport width. */
	forceOverlay: boolean;
	headerMenu: MenuModel;
	groupMenu: MenuModel;
	contactMenu: MenuModel;
	/** Fixture-opened menu (dc5 header dropdown, dc6 group context menu). */
	openMenu?: 'header' | 'group';
	closeLabel: string;
}
