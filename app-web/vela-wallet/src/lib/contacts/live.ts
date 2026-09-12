/**
 * The live contacts builders (spec 024, research D7): `ContactsView` → the
 * same display models the drawn components consume. Siblings of the fixture
 * builders — the galleries keep their canon.
 *
 * One presentation judgement lives here, documented as a render concern:
 * - **Search filtering.** The core has no list-search event (its
 *   `matches_query` serves the recipient picker); filtering the rendered
 *   list by the box's text is display-side narrowing of core-ruled rows.
 *
 * Letter sectioning is NOT here any more (spec 028 US5 addendum): which
 * letter 妈妈 files under is a rule with one right answer, and for two specs
 * this file answered it wrong — `A–Z or #` filed every Chinese name under `#`
 * while the 018 drawing files 阿豪 under A and 妈妈 under M. The core's
 * `ContactsView.sections` now says, for the whole book, by one initial table
 * every shell shares; the narrowing below drops the letters the search empties.
 */

import type { Contact } from '$lib/core/generated/Contact';
import type { ContactGroupView } from '$lib/core/generated/ContactGroupView';
import type { ContactsView } from '$lib/core/generated/ContactsView';
import type { FeedItem } from '$lib/core/generated/FeedItem';
import type { FeedView } from '$lib/core/generated/FeedView';
import { formatDate } from '$lib/services/locale-format';
import { chainName } from '$lib/services/networks';
import { chainLogoURL } from '$lib/services/tokens-model';
import { chainColor } from '$lib/wallet/fixtures';
import { trimBalance } from '$lib/wallet/live';
import { fill } from '$lib/wallet/messages';
import { shortenAddress } from '$lib/wallet/identity';
import type { ActivityRowModel, SidebarModel } from '$lib/wallet/model';
import { contactContextMenu, groupContextMenu, headerDropdown } from './fixtures';
import type { ContactsMessages } from './messages';
import type {
	ContactDetailModel,
	ContactModel,
	ContactsDesktopModel,
	ContactsHomeModel,
	ContactsListModel,
	EmptyCtaModel,
	GroupDetailModel,
	GroupModel,
	LetterSectionModel
} from './model';

/**
 * What the contacts screens read beyond the book itself (spec 028 US5).
 *
 * The feed is the wallet's `activity_feed` view; which of its rows belong to a
 * contact is display-side narrowing (counterparty = this address), the same
 * class of work as the chain filter on the wallet home — the core already
 * ruled on every row's direction, amount and day.
 */
export interface ContactsLiveExtras {
	feed?: FeedView | null;
	/** "全部 ›" was tapped: every row, not the recent few. */
	allActivity?: boolean;
	/** Injected clock for the day labels (tests). */
	now?: number;
}

/** 最近往来 shows this many before "全部 ›" (018 drew two; three fits the column). */
export const RECENT_ACTIVITY_ROWS = 3;

/** The feed's rows whose counterparty is this contact, in feed order (newest first). */
export function contactFeedItems(feed: FeedView | null | undefined, address: string): FeedItem[] {
	if (!feed) return [];
	const target = address.toLowerCase();
	const items: FeedItem[] = [];
	for (const row of feed.rows) {
		if (row.type === 'item' && row.item.counterparty?.toLowerCase() === target)
			items.push(row.item);
	}
	return items;
}

function localMidnight(ms: number): number {
	const d = new Date(ms);
	return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
}

/** "Today" / "Yesterday" from the corpus; older days in the person's date preset. */
function dayLabel(dayStartMs: number, m: ContactsMessages, now: number): string {
	const today = localMidnight(now);
	if (dayStartMs === today) return m.activity.today;
	if (dayStartMs === today - 86_400_000) return m.activity.yesterday;
	return formatDate(dayStartMs);
}

/**
 * One feed item as a contact's 最近往来 row. The wallet's own row builder says
 * "to Alice" in its subtitle; on Alice's page that is noise, so the subtitle
 * is the network and the day instead.
 */
export function contactActivityRow(
	item: FeedItem,
	m: ContactsMessages,
	now = Date.now()
): ActivityRowModel {
	const received = item.direction === 'in';
	return {
		id: item.id,
		kind: received ? 'received' : 'sent',
		title: received ? m.activity.received : m.activity.sent,
		subtitle: `${chainName(item.chain_id)} · ${dayLabel(item.day_start_ms, m, now)}`,
		amount:
			item.value === null
				? String(item.batch?.count ?? '')
				: `${received ? '+' : '-'}${trimBalance(item.value)}`,
		unit: item.symbol,
		positive: received,
		masked: false,
		badgeColor: chainColor(item.chain_id),
		badgeLogoUrl: chainLogoURL(item.chain_id)
	};
}

/** The full rail, always — letters without a section still render (018 D4). */
const INDEX_LETTERS = [...'ABCDEFGHIJKLMNOPQRSTUVWXYZ', '#'];

/** The avatar producer. `name` matters only in the initials style (spec 028). */
type Identicon = (seed: string, name?: string) => string;

/** What the person calls this contact — their name wins over a resolved one,
 *  and an unnamed address introduces itself as its short form. */
export function displayName(contact: Contact): string {
	return contact.name ?? contact.resolved_name ?? shortenAddress(contact.address);
}

function toContactModel(contact: Contact, view: ContactsView, identicon: Identicon): ContactModel {
	return {
		name: displayName(contact),
		addressDisplay: shortenAddress(contact.address),
		addressFull: contact.address,
		identiconSvg: identicon(contact.address, displayName(contact)),
		groups: view.groups
			.filter((g) => g.members.some((mb) => mb.address === contact.address))
			.map((g) => g.name)
	};
}

/**
 * The core's A–Z directory, narrowed by the search box: each section keeps
 * the rows that match, in the core's order, and a letter the search empties
 * is dropped (an empty letter header is the shell's to avoid).
 */
export function letterSections(
	view: ContactsView,
	identicon: Identicon,
	query: string
): LetterSectionModel[] {
	const q = query.trim().toLowerCase();
	const matches = (c: Contact) =>
		q === '' ||
		displayName(c).toLowerCase().includes(q) ||
		(c.resolved_name ?? '').toLowerCase().includes(q) ||
		c.address.includes(q);
	const byAddress = new Map(view.contacts.map((c) => [c.address, c]));
	const out: LetterSectionModel[] = [];
	for (const section of view.sections) {
		const contacts: ContactModel[] = [];
		for (const address of section.addresses) {
			const contact = byAddress.get(address);
			if (contact !== undefined && matches(contact))
				contacts.push(toContactModel(contact, view, identicon));
		}
		if (contacts.length > 0) out.push({ letter: section.letter, contacts });
	}
	return out;
}

function toGroupModel(
	group: ContactGroupView,
	view: ContactsView,
	m: ContactsMessages,
	identicon: Identicon
): GroupModel {
	return {
		id: group.id,
		name: group.name,
		countLabel: fill(m.groupMembers, { count: group.members.length }),
		count: String(group.members.length),
		membersLabel: fill(m.membersCount, { count: group.members.length }),
		members: group.members.map((member) => toContactModel(member, view, identicon))
	};
}

/**
 * The group screen. The CTA's caption says what the button will do — and,
 * when the group is empty and the button is disabled, WHY (the founder's
 * ask: a dead button with no words is a button that "does nothing").
 */
function liveGroupDetail(
	group: ContactGroupView,
	view: ContactsView,
	m: ContactsMessages,
	identicon: Identicon
): GroupDetailModel {
	const count = group.members.length;
	const empty = count === 0;
	return {
		group: toGroupModel(group, view, m, identicon),
		addMember: m.addMember,
		cta: m.batchSend,
		ctaCaption: empty ? m.batchSendNeedsMembers : fill(m.batchSendHint, { count }),
		captionTitled: empty ? m.batchSendNeedsMembers : fill(m.batchSendHintTitled, { count }),
		menuLabel: m.manage
	};
}

function emptyModel(m: ContactsMessages): EmptyCtaModel {
	return {
		title: m.empty,
		caption: m.emptyHint,
		primary: m.addContact,
		secondary: m.importFile
	};
}

export function liveContactDetail(
	contact: Contact,
	view: ContactsView,
	m: ContactsMessages,
	identicon: Identicon,
	extras: ContactsLiveExtras = {}
): ContactDetailModel {
	const model = toContactModel(contact, view, identicon);
	// The address block wraps to two even halves on the phone (018 canon).
	const half = Math.ceil(contact.address.length / 2);
	const items = contactFeedItems(extras.feed, contact.address);
	const shown = extras.allActivity ? items : items.slice(0, RECENT_ACTIVITY_ROWS);
	return {
		contact: model,
		chips: model.groups,
		addChipLabel: m.moveGroup,
		actions: { send: m.send, receive: m.receive, qr: m.actionQr },
		address: {
			label: m.addressLabel,
			lines: [contact.address.slice(0, half), contact.address.slice(half)],
			full: contact.address,
			copyLabel: m.copyAddress
		},
		activityTitle: m.recentActivity,
		activityAction: m.activity.all,
		activityLink: m.viewAllActivity,
		rows: shown.map((item) => contactActivityRow(item, m, extras.now)),
		emptyActivity: items.length === 0 ? m.noActivity : undefined,
		editLabel: m.edit,
		deleteLabel: m.deleteContact
	};
}

/** What the LIVE route renders — screen choice is the route's render state. */
export interface ContactsUiState {
	screen: 'list' | 'detail' | 'group' | 'empty';
	query: string;
	selectedAddress?: string;
	selectedGroupId?: string;
	/** The detail's 最近往来 expanded past the recent few. */
	allActivity?: boolean;
}

export function buildContactsLive(
	view: ContactsView,
	m: ContactsMessages,
	identicon: Identicon,
	ui: ContactsUiState,
	extras: ContactsLiveExtras = {}
): ContactsHomeModel {
	const base = {
		state: 'c1' as const,
		title: m.title,
		addLabel: m.addContact,
		editLabel: m.edit,
		menuLabel: m.manage,
		backLabel: m.cancel,
		tabs: {
			wallet: m.shell.navWallet,
			contacts: m.shell.navContacts,
			explore: m.shell.navExplore,
			settings: m.shell.navSettings
		}
	};

	if (
		ui.screen === 'empty' ||
		(ui.screen === 'list' && view.loaded && view.contacts.length === 0)
	) {
		return { ...base, screen: 'empty', empty: emptyModel(m) };
	}

	if (ui.screen === 'detail' && ui.selectedAddress !== undefined) {
		const contact = view.contacts.find((c) => c.address === ui.selectedAddress);
		if (contact !== undefined) {
			const detail = liveContactDetail(contact, view, m, identicon, {
				...extras,
				allActivity: ui.allActivity
			});
			return { ...base, screen: 'detail', detail };
		}
	}

	if (ui.screen === 'group' && ui.selectedGroupId !== undefined) {
		const group = view.groups.find((g) => g.id === ui.selectedGroupId);
		if (group !== undefined) {
			return { ...base, screen: 'group', group: liveGroupDetail(group, view, m, identicon) };
		}
	}

	const sections = letterSections(view, identicon, ui.query);
	const list: ContactsListModel = {
		search: { placeholder: m.searchPlaceholder, query: ui.query === '' ? undefined : ui.query },
		groupsTitle: m.sectionGroups,
		groupsAction: m.groupNew,
		groups: view.groups.map((g) => toGroupModel(g, view, m, identicon)),
		contactsTitle: m.sectionContacts,
		contactsCount: fill(m.countPeople, { count: view.contacts.length }),
		sections,
		indexLetters: INDEX_LETTERS,
		swipeActions: { send: m.send, delete: m.delete },
		noResults:
			ui.query !== '' && sections.length === 0 ? fill(m.noResults, { query: ui.query }) : undefined
	};
	return { ...base, screen: 'list', list };
}

/**
 * The desktop shape of the same book (spec 018's DC boards, live).
 *
 * One `ContactsUiState` renders two ways. What the phone pushes as a screen,
 * the desktop keeps beside the list: a chosen contact is the third column, a
 * chosen group is the rail's highlight with its members where the A–Z list
 * was. `sidebar` is the app sidebar the route already carries, identity and
 * network rows filled in by the page.
 */
export function buildContactsDesktopLive(
	view: ContactsView,
	m: ContactsMessages,
	identicon: Identicon,
	ui: ContactsUiState,
	sidebar: SidebarModel,
	extras: ContactsLiveExtras = {}
): ContactsDesktopModel {
	const selectedGroup =
		ui.screen === 'group' && ui.selectedGroupId !== undefined
			? view.groups.find((g) => g.id === ui.selectedGroupId)
			: undefined;
	const selectedContact =
		ui.screen === 'detail' && ui.selectedAddress !== undefined
			? view.contacts.find((c) => c.address === ui.selectedAddress)
			: undefined;
	const empty = view.loaded && view.contacts.length === 0;

	const group: GroupDetailModel | undefined =
		selectedGroup === undefined ? undefined : liveGroupDetail(selectedGroup, view, m, identicon);

	return {
		state: 'dc1',
		sidebar,
		title: m.title,
		search: {
			placeholder: m.searchPlaceholder,
			query: ui.query === '' ? undefined : ui.query
			// No ⌘F badge live (founder, 2026-09-05): nothing listens for the
			// chord, and a key cap that promises one is a control that lies.
		},
		addLabel: m.addContact,
		menuLabel: m.manage,
		rail: {
			allLabel: m.allContacts,
			allCount: String(view.contacts.length),
			allSelected: selectedGroup === undefined,
			groupsTitle: m.sectionGroups,
			groups: view.groups.map((g) => toGroupModel(g, view, m, identicon)),
			selectedGroup: selectedGroup?.name,
			newGroup: m.groupNew
		},
		sections:
			selectedGroup === undefined && !empty ? letterSections(view, identicon, ui.query) : [],
		group,
		empty: empty ? emptyModel(m) : undefined,
		detail:
			selectedContact === undefined
				? undefined
				: liveContactDetail(selectedContact, view, m, identicon, {
						...extras,
						allActivity: ui.allActivity
					}),
		panelTitle: m.sectionContacts,
		initialPanel: selectedContact === undefined ? 'none' : 'contact-detail',
		selectedContact: selectedContact === undefined ? undefined : displayName(selectedContact),
		forceOverlay: false,
		headerMenu: headerDropdown(m),
		groupMenu: groupContextMenu(m),
		contactMenu: contactContextMenu(m),
		closeLabel: m.shell.close
	};
}

// ---------------------------------------------------------------------------
// The pickers and the report (spec 028 US5) — small models the route hands
// to `PickList` and to the report sheet. Data only; every rule they answer
// to (union, normalisation, existing-wins) is the core's.
// ---------------------------------------------------------------------------

export interface PickRowModel {
	/** What the route dispatches: an address for a contact, an id for a group. */
	id: string;
	name: string;
	detail?: string;
	identiconSvg?: string;
	checked: boolean;
}

export interface PickListModel {
	title: string;
	searchPlaceholder: string;
	rows: PickRowModel[];
	save: string;
	/** Shown when there is nothing to pick from. */
	empty: string;
}

/** 添加成员: every contact in the book, the group's current members ticked. */
export function memberPickModel(
	view: ContactsView,
	groupId: string,
	m: ContactsMessages,
	identicon: Identicon
): PickListModel {
	const group = view.groups.find((g) => g.id === groupId);
	const members = new Set(group?.members.map((member) => member.address) ?? []);
	return {
		title: m.addMember,
		searchPlaceholder: m.searchPlaceholder,
		save: m.save,
		empty: m.groupNoContacts,
		rows: view.contacts.map((contact) => ({
			id: contact.address,
			name: displayName(contact),
			detail: shortenAddress(contact.address),
			identiconSvg: identicon(contact.address, displayName(contact)),
			checked: members.has(contact.address)
		}))
	};
}

/** 移入分组: every group, the ones holding this contact ticked. */
export function groupPickModel(
	view: ContactsView,
	address: string,
	m: ContactsMessages
): PickListModel {
	return {
		title: m.moveGroup,
		searchPlaceholder: m.searchPlaceholder,
		save: m.save,
		empty: m.groupNoContacts,
		rows: view.groups.map((group) => ({
			id: group.id,
			name: group.name,
			detail: fill(m.groupMembers, { count: group.members.length }),
			checked: group.members.some((member) => member.address === address)
		}))
	};
}

/**
 * The import's outcome in the corpus's words: the core's counts, or its
 * refusal. `undefined` while nothing is pending.
 */
export function importReport(
	view: ContactsView,
	m: ContactsMessages
): { title: string; body: string } | undefined {
	if (view.import_failure !== null) return { title: m.importFailTitle, body: m.importFailBody };
	const report = view.last_import;
	if (report === null) return undefined;
	let body = fill(m.importDoneBody, { added: report.added, skipped: report.skipped });
	if (report.invalid > 0) body += ` ${fill(m.importDoneInvalid, { invalid: report.invalid })}`;
	return { title: m.importDoneTitle, body };
}
