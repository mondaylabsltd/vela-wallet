/**
 * Canonical contacts fixtures (spec 018, data-model.md — the single canon all
 * four platforms port). Names, addresses and amounts are verbatim data; every
 * label resolves through `ContactsMessages`. Pure data + assembly: no fetching,
 * no collation, no filtering, no formatting rules (FR-005).
 *
 * Identicon seeds are the FULL addresses below, handed to the platform's
 * normalize route untouched — never lowercased at the call site (spec 003).
 */
import { fill } from '$lib/wallet/messages';
import { CHAIN_COLORS, IDENTITY, chainRowsFor } from '$lib/wallet/fixtures';
import type { ActivityRowModel } from '$lib/wallet/model';
import type { ContactsMessages } from './messages';
import type {
	ContactDetailModel,
	ContactModel,
	ContactsDesktopModel,
	ContactsHomeModel,
	ContactsListModel,
	DesktopContactsStateId,
	GroupDetailModel,
	GroupModel,
	GroupRailModel,
	LetterSectionModel,
	MenuModel,
	MobileContactsStateId
} from './model';

type Identicon = (seed: string) => string;

// --- Canon ----------------------------------------------------------------

interface ContactFixture {
	name: string;
	addressDisplay: string;
	addressFull: string;
	section: string;
	groups: string[];
}

/**
 * The 8 位 roster. Only Alice's full address appears in a mock (C2); the other
 * seven are pinned inventions whose first/last four hex chars match the mock's
 * truncated display (research.md D7) — identicon artwork therefore differs
 * from the mock renders for those seven, but is identical across platforms.
 */
export const CONTACTS: ContactFixture[] = [
	{
		name: 'Alice',
		addressDisplay: '0x9F3c…21aE',
		addressFull: '0x9F3cA71b04E82f5C55d9B21aE00734F8Dd8021aE',
		section: 'A',
		groups: ['家人']
	},
	{
		name: '阿豪',
		addressDisplay: '0x77Bd…4F02',
		addressFull: '0x77Bd59A302cC93D23dB0d0BA6a45C6830EF74F02',
		section: 'A',
		groups: []
	},
	{
		name: 'Bartholomew Vanderbilt-Konstantinopoulos.eth',
		addressDisplay: '0x31c9…E77a',
		addressFull: '0x31c9A100517d2436E9E1350D383A7d0aAeC1E77a',
		section: 'B',
		groups: []
	},
	{
		name: 'Bob · 泵泵',
		addressDisplay: '0x44Aa…9C21',
		addressFull: '0x44AaF19cE84f22101b5D6cbA918B92DcA5f19C21',
		section: 'B',
		groups: []
	},
	{
		name: 'Charlie',
		addressDisplay: '0x5eF0…3a9C',
		addressFull: '0x5eF0FF25a1A24E5cCb2a6D939B87F5DAb2003a9C',
		section: 'C',
		groups: []
	},
	{
		name: 'DAO 金库',
		addressDisplay: '0xF00d…C0de',
		addressFull: '0xF00dBaBe8712004343cD00926Ab004D6C042C0de',
		section: 'D',
		groups: []
	},
	{
		name: 'hold on',
		addressDisplay: '0xCafe…F00d',
		addressFull: '0xCafe9078B1c2A04d33Ff21B0BC934eB8A812F00d',
		section: 'H',
		groups: []
	},
	{
		name: '妈妈',
		addressDisplay: '0x88Ce…12aB',
		addressFull: '0x88Ce02FdB0e50D9C21e33c0F9B58a3E38f7612aB',
		section: 'M',
		groups: ['家人']
	}
];

/** Exists only inside the 家人 group (the recorded DC1 mock inconsistency). */
export const GROUP_ONLY_MEMBER: ContactFixture = {
	name: '表弟',
	addressDisplay: '0xA1c3…88dD',
	addressFull: '0xA1c3D3d7085B90AF14E5d21C86e6dB49F30a88dD',
	section: 'B',
	groups: ['家人']
};

/** Letter sections present in the canon, in order. */
export const SECTION_LETTERS = ['A', 'B', 'C', 'D', 'H', 'M'] as const;

/** The rail renders the full alphabet regardless of which sections exist (D4). */
export const INDEX_LETTERS: string[] = [...'ABCDEFGHIJKLMNOPQRSTUVWXYZ'.split(''), '#'];

/** Exported for spec 021: SD2e's contact picker lists the same groups. */
export const GROUPS: { name: string; count: number; members: ContactFixture[] }[] = [
	{
		name: '家人',
		count: 3,
		members: [CONTACTS[7], GROUP_ONLY_MEMBER, CONTACTS[0]]
	},
	{ name: '工作', count: 5, members: [] },
	{ name: '交易所', count: 2, members: [] }
];

/** Identicon board seeds: the 8+1 canon addresses plus the invalid placeholder. */
export const CONTACT_BOARD_SEEDS: string[] = [
	...CONTACTS.map((c) => c.addressFull),
	GROUP_ONLY_MEMBER.addressFull,
	''
];

export const MOBILE_STATES: MobileContactsStateId[] = [
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

export const DESKTOP_STATES: DesktopContactsStateId[] = [
	'dc1',
	'dc2',
	'dc3',
	'dc4',
	'dc5',
	'dc6',
	'dc2n'
];

interface ContactActivityFixture {
	kind: 'received' | 'sent';
	/** Localized "yesterday" + this clock, or the verbatim `day` when absent. */
	clock?: string;
	day?: string;
	chain: string;
	amount: string;
	unit: string;
	positive: boolean;
	badgeColor: string;
}

/** Alice's 最近往来 — the received row mirrors the 015 wallet fixture (D7). */
const ALICE_ACTIVITY: ContactActivityFixture[] = [
	{
		kind: 'received',
		clock: '20:15',
		chain: 'Ethereum',
		amount: '+50',
		unit: 'USDC',
		positive: true,
		badgeColor: CHAIN_COLORS.ethereum
	},
	{
		kind: 'sent',
		day: '8 月 5 日',
		chain: 'Arbitrum',
		amount: '−0.2',
		unit: 'ETH',
		positive: false,
		badgeColor: CHAIN_COLORS.arbitrum
	}
];

/** Alice's mono address block wraps to exactly these two lines on mobile (C2). */
const ALICE_ADDRESS_LINES = ['0x9F3cA71b04E82f5C55d9', 'B21aE00734F8Dd8021aE'];

// --- Assembly -------------------------------------------------------------

function contact(f: ContactFixture, identicon: Identicon): ContactModel {
	return {
		name: f.name,
		addressDisplay: f.addressDisplay,
		addressFull: f.addressFull,
		identiconSvg: identicon(f.addressFull),
		groups: [...f.groups]
	};
}

function sections(list: ContactFixture[], identicon: Identicon): LetterSectionModel[] {
	const out: LetterSectionModel[] = [];
	for (const f of list) {
		const last = out.at(-1);
		const row = contact(f, identicon);
		if (last !== undefined && last.letter === f.section) last.contacts.push(row);
		else out.push({ letter: f.section, contacts: [row] });
	}
	return out;
}

function groups(m: ContactsMessages, identicon: Identicon): GroupModel[] {
	return GROUPS.map((g) => ({
		name: g.name,
		countLabel: fill(m.groupMembers, { count: g.count }),
		count: String(g.count),
		membersLabel: fill(m.membersCount, { count: g.count }),
		members: g.members.map((f) => contact(f, identicon))
	}));
}

function aliceActivity(m: ContactsMessages): ActivityRowModel[] {
	return ALICE_ACTIVITY.map((f) => ({
		kind: f.kind,
		title: f.kind === 'received' ? m.activity.received : m.activity.sent,
		subtitle:
			f.clock === undefined
				? `${f.day ?? ''} · ${f.chain}`
				: `${m.activity.yesterday} ${f.clock} · ${f.chain}`,
		amount: f.amount,
		unit: f.unit,
		positive: f.positive,
		masked: false,
		badgeColor: f.badgeColor
	}));
}

function aliceDetail(m: ContactsMessages, identicon: Identicon): ContactDetailModel {
	return {
		contact: contact(CONTACTS[0], identicon),
		chips: [...CONTACTS[0].groups],
		addChipLabel: m.sectionGroups,
		actions: { send: m.send, receive: m.receive, qr: m.actionQr },
		address: {
			label: m.addressLabel,
			lines: [...ALICE_ADDRESS_LINES],
			full: CONTACTS[0].addressFull,
			copyLabel: m.copyAddress
		},
		activityTitle: m.recentActivity,
		activityAction: m.activity.all,
		activityLink: m.viewAllActivity,
		rows: aliceActivity(m),
		editLabel: m.edit,
		deleteLabel: m.deleteContact
	};
}

function familyDetail(m: ContactsMessages, identicon: Identicon): GroupDetailModel {
	const family = groups(m, identicon)[0];
	return {
		group: family,
		addMember: m.addMember,
		cta: m.batchSend,
		ctaCaption: fill(m.batchSendHint, { count: family.members.length }),
		captionTitled: fill(m.batchSendHintTitled, { count: family.members.length }),
		menuLabel: m.sectionGroups
	};
}

function listModel(
	m: ContactsMessages,
	identicon: Identicon,
	opts: { query?: string; revealed?: { letter: string; index: number } } = {}
): ContactsListModel {
	// The "search active" variant ships pre-filtered (FR-005: no filtering here).
	const matched = opts.query === 'Ali' ? [CONTACTS[0]] : CONTACTS;
	return {
		search: { placeholder: m.searchPlaceholder, query: opts.query },
		groupsTitle: m.sectionGroups,
		groupsAction: m.manage,
		groups: groups(m, identicon),
		contactsTitle: m.sectionContacts,
		contactsCount: fill(m.countPeople, { count: matched.length }),
		sections: sections(matched, identicon),
		indexLetters: [...INDEX_LETTERS],
		revealed: opts.revealed,
		swipeActions: { send: m.send, delete: m.delete },
		noResults: matched.length === 0 ? fill(m.noResults, { query: opts.query ?? '' }) : undefined
	};
}

// --- Menus (data-model.md §Menus) -----------------------------------------

export function addMenu(m: ContactsMessages): MenuModel {
	return {
		kind: 'sheet',
		label: m.addContact,
		items: [
			{ icon: 'user-round-plus', label: m.addTitle },
			{ icon: 'download', label: m.importFile },
			{ icon: 'upload', label: m.exportTitle }
		],
		cancel: m.cancel
	};
}

export function groupMenuMobile(m: ContactsMessages): MenuModel {
	return {
		kind: 'sheet',
		label: m.sectionGroups,
		items: [
			{ icon: 'pencil', label: m.groupEdit },
			{ icon: 'download', label: m.importGroup },
			{ icon: 'upload', label: m.exportGroup, dividerAfter: true },
			{ icon: 'trash-2', label: m.groupDelete, destructive: true }
		],
		cancel: m.cancel
	};
}

export function headerDropdown(m: ContactsMessages): MenuModel {
	return {
		kind: 'dropdown',
		label: m.title,
		items: [
			{ icon: 'download', label: m.importAll },
			{ icon: 'upload', label: m.exportAll }
		]
	};
}

export function groupContextMenu(m: ContactsMessages): MenuModel {
	return {
		kind: 'context',
		label: m.sectionGroups,
		items: [
			{ icon: 'pencil', label: m.groupRename },
			{ icon: 'download', label: m.importGroup },
			{ icon: 'upload', label: m.exportGroup, dividerAfter: true },
			{ icon: 'trash-2', label: m.groupDelete, destructive: true }
		]
	};
}

export function contactContextMenu(m: ContactsMessages): MenuModel {
	return {
		kind: 'context',
		label: m.sectionContacts,
		items: [
			{ icon: 'arrow-up-right', label: m.send },
			{ icon: 'arrow-down-left', label: m.receive },
			{ icon: 'copy', label: m.copyAddress },
			{ icon: 'pencil', label: m.edit },
			{ icon: 'users-round', label: m.moveGroup, dividerAfter: true },
			{ icon: 'trash-2', label: m.delete, destructive: true }
		]
	};
}

// --- Screen builders ------------------------------------------------------

/** Assemble the mobile contacts view model for one C-state. */
export function buildMobileState(
	state: MobileContactsStateId,
	m: ContactsMessages,
	identicon: Identicon
): ContactsHomeModel {
	const base = {
		state,
		title: m.title,
		addLabel: m.addContact,
		editLabel: m.edit,
		menuLabel: m.manage,
		backLabel: m.shell.close,
		tabs: {
			wallet: m.shell.navWallet,
			contacts: m.shell.navContacts,
			explore: m.shell.navExplore,
			settings: m.shell.navSettings
		}
	};

	switch (state) {
		case 'c1':
			return { ...base, screen: 'list', list: listModel(m, identicon) };
		case 'c1s':
			// 阿豪 is the second row of section A (research.md D5: fixture-pinned).
			return {
				...base,
				screen: 'list',
				list: listModel(m, identicon, { revealed: { letter: 'A', index: 1 } })
			};
		case 'c1f':
			return { ...base, screen: 'list', list: listModel(m, identicon, { query: 'Ali' }) };
		case 'c2':
			return { ...base, screen: 'detail', detail: aliceDetail(m, identicon) };
		case 'c2s':
			return {
				...base,
				screen: 'detail',
				detail: aliceDetail(m, identicon),
				confirm: {
					title: m.deleteTitle,
					body: fill(m.deleteBody, { name: CONTACTS[0].name }),
					confirm: m.delete,
					cancel: m.cancel
				}
			};
		case 'c3':
			return {
				...base,
				screen: 'empty',
				list: { ...listModel(m, identicon), groups: [], sections: [] },
				empty: {
					title: m.empty,
					caption: m.emptyHint,
					primary: m.addContact,
					secondary: m.importFile
				}
			};
		case 'c4':
			return { ...base, screen: 'group', group: familyDetail(m, identicon) };
		case 'c5':
			return { ...base, screen: 'list', list: listModel(m, identicon), sheet: addMenu(m) };
		case 'c6':
			return {
				...base,
				screen: 'group',
				group: familyDetail(m, identicon),
				sheet: groupMenuMobile(m)
			};
	}
}

function rail(
	m: ContactsMessages,
	identicon: Identicon,
	opts: { empty?: boolean; selectedGroup?: string; dropTarget?: string } = {}
): GroupRailModel {
	return {
		allLabel: m.allContacts,
		allCount: opts.empty === true ? '0' : String(CONTACTS.length),
		allSelected: opts.selectedGroup === undefined,
		groupsTitle: m.sectionGroups,
		groups: opts.empty === true ? [] : groups(m, identicon),
		selectedGroup: opts.selectedGroup,
		newGroup: m.groupNew,
		dropTarget: opts.dropTarget
	};
}

/** Assemble the desktop contacts view model for one DC-state. */
export function buildDesktopState(
	state: DesktopContactsStateId,
	m: ContactsMessages,
	identicon: Identicon
): ContactsDesktopModel {
	const groupView = state === 'dc4' || state === 'dc6';
	const empty = state === 'dc3';
	const detailOpen = state === 'dc2' || state === 'dc2n';

	return {
		state,
		sidebar: {
			header: {
				name: IDENTITY.name,
				addressDisplay: IDENTITY.addressDisplay,
				addressFull: IDENTITY.addressFull,
				identiconSvg: identicon(IDENTITY.addressFull)
			},
			nav: [
				{ id: 'wallet', label: m.shell.navWallet, selected: false },
				{ id: 'contacts', label: m.shell.navContacts, selected: true },
				{ id: 'explore', label: m.shell.navExplore, selected: false },
				{ id: 'settings', label: m.shell.navSettings, selected: false }
			],
			networksTitle: m.shell.networksTitle,
			networks: chainRowsFor(m.shell.allNetworks)
		},
		title: m.title,
		search: { placeholder: m.searchPlaceholder, shortcut: '⌘F' },
		addLabel: m.addContact,
		menuLabel: m.manage,
		rail: rail(m, identicon, {
			empty,
			selectedGroup: groupView ? '家人' : undefined
		}),
		sections: groupView || empty ? [] : sections(CONTACTS, identicon),
		group: groupView ? familyDetail(m, identicon) : undefined,
		empty: empty
			? { title: m.empty, caption: m.emptyHint, primary: m.addContact, secondary: m.importFile }
			: undefined,
		detail: aliceDetail(m, identicon),
		panelTitle: m.sectionContacts,
		initialPanel: detailOpen ? 'contact-detail' : 'none',
		selectedContact: detailOpen ? CONTACTS[0].name : undefined,
		forceOverlay: state === 'dc2n',
		headerMenu: headerDropdown(m),
		groupMenu: groupContextMenu(m),
		contactMenu: contactContextMenu(m),
		openMenu: state === 'dc5' ? 'header' : state === 'dc6' ? 'group' : undefined,
		closeLabel: m.shell.close
	};
}
