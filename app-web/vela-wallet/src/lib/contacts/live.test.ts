/**
 * The live contacts builders (spec 024 T031): ContactsView → display models.
 * Core order authoritative; sectioning and search are render concerns.
 */
import { describe, expect, it } from 'vitest';
import type { Contact } from '$lib/core/generated/Contact';
import type { ContactsView } from '$lib/core/generated/ContactsView';
import { resolveContactsMessages } from '$lib/i18n/engine.server';
import { buildContactsDesktopLive, buildContactsLive, displayName, letterSections } from './live';
import type { SidebarModel } from '$lib/wallet/model';

const m = resolveContactsMessages('en');
const identicon = (seed: string) => `<svg data-seed="${seed}"></svg>`;

function contact(partial: Partial<Contact> & { address: string }): Contact {
	return {
		name: null,
		resolved_name: null,
		resolved_source: null,
		kind: 'unknown',
		favorite: false,
		note: null,
		tx_count: 0,
		last_used_ms: 0,
		first_seen_ms: 0,
		source: 'manual',
		...partial
	};
}

const ALICE = contact({ address: '0x' + 'a1'.repeat(20), name: 'Alice', favorite: true });
const ANTON = contact({ address: '0x' + 'a2'.repeat(20), name: 'Anton' });
const BOB = contact({ address: '0x' + 'b1'.repeat(20), name: 'Bob' });
const UNNAMED = contact({ address: '0x' + 'c1'.repeat(20) });

const VIEW: ContactsView = {
	loaded: true,
	// Core order: favourites first, then recency — Anton before Alice would be
	// the core's business; this fixture has Alice (fav) first.
	contacts: [ALICE, ANTON, BOB, UNNAMED],
	// What the core's initial rule says for these four (spec 028 US5 addendum).
	sections: [
		{ letter: 'A', addresses: [ALICE.address, ANTON.address] },
		{ letter: 'B', addresses: [BOB.address] },
		{ letter: '#', addresses: [UNNAMED.address] }
	],
	groups: [{ id: 'g1', name: 'Payroll', color: null, members: [ALICE, BOB] }],
	last_import: null,
	import_failure: null,
	export: null,
	recipient: null
};

describe('displayName', () => {
	it('the given name wins; an unnamed address introduces itself shortened', () => {
		expect(displayName(ALICE)).toBe('Alice');
		expect(displayName(UNNAMED)).toMatch(/^0x[0-9a-f]+…[0-9a-f]+$/i);
	});
});

/**
 * Issue 191 — what an address is called, and how a row nobody named gets a
 * name. The order is the core's (`contact_display_name`); these pin that the
 * shell reads it the same way and offers the naming where the name is missing.
 */
describe('a contact nobody has named (issue 191)', () => {
	const HISTORY_ROW = contact({ address: '0x' + '60'.repeat(20), source: 'auto', tx_count: 1 });

	it('own name → resolved name (registry or name service) → address, in that order', () => {
		const known = { ...HISTORY_ROW, resolved_name: "Bob's Vela", resolved_source: 'passkey' };
		expect(displayName(known)).toBe("Bob's Vela");
		expect(displayName({ ...known, name: 'Bob from the gym' })).toBe('Bob from the gym');
		expect(displayName(HISTORY_ROW)).toMatch(/^0x606060…606060$/);
	});

	it('an empty string is a missing name, as the core reads it', () => {
		expect(displayName({ ...HISTORY_ROW, name: '', resolved_name: 'bob.eth' })).toBe('bob.eth');
		expect(displayName({ ...HISTORY_ROW, name: '', resolved_name: '' })).toMatch(/^0x/);
	});

	it('a history-suggested row offers "Save to contacts" under its name', () => {
		const detail = liveContactDetail(HISTORY_ROW, VIEW, m, identicon);
		expect(detail.nameAction).toBe(m.saveToContacts);
	});

	it('being recognised by the registry is not being named by the person', () => {
		const known = { ...HISTORY_ROW, resolved_name: "Bob's Vela", resolved_source: 'passkey' };
		expect(liveContactDetail(known, VIEW, m, identicon).nameAction).toBe(m.saveToContacts);
	});

	it('a saved contact without a name offers Edit; a named one offers nothing extra', () => {
		expect(liveContactDetail(UNNAMED, VIEW, m, identicon).nameAction).toBe(m.edit);
		expect(liveContactDetail(ALICE, VIEW, m, identicon).nameAction).toBeUndefined();
	});
});

describe('letterSections', () => {
	it('groups by initial, keeps core order inside a letter, 0x names go to #', () => {
		const sections = letterSections(VIEW, identicon, '');
		expect(sections.map((s) => s.letter)).toEqual(['A', 'B', '#']);
		expect(sections[0].contacts.map((c) => c.name)).toEqual(['Alice', 'Anton']);
	});

	it('search narrows by name and address, case-insensitive', () => {
		expect(letterSections(VIEW, identicon, 'ant')[0].contacts[0].name).toBe('Anton');
		expect(letterSections(VIEW, identicon, 'b1b1')[0].contacts[0].name).toBe('Bob');
		expect(letterSections(VIEW, identicon, 'zzz')).toEqual([]);
	});
});

describe('buildContactsLive', () => {
	it('an empty loaded book renders the invitation', () => {
		const model = buildContactsLive({ ...VIEW, contacts: [], groups: [] }, m, identicon, {
			screen: 'list',
			query: ''
		});
		expect(model.screen).toBe('empty');
		expect(model.empty?.primary).toBe(m.addContact);
	});

	// Founder, 2026-09-26: entering 通讯录 flashed 分组 / 新建分组 and "0 位",
	// then an empty book became the empty state. Before `loaded`: chrome only.
	it('an unread book is chrome only — no groups head, no count', () => {
		const model = buildContactsLive(
			{ ...VIEW, loaded: false, contacts: [], groups: [], sections: [] },
			m,
			identicon,
			{ screen: 'list', query: '' }
		);
		expect(model.screen).toBe('list');
		expect(model.list?.pending).toBe(true);
		expect(model.list?.search.placeholder).toBe(m.searchPlaceholder);
	});

	// …and the empty book keeps the search field Android and iOS have.
	it('an empty book still carries the search field', () => {
		const model = buildContactsLive({ ...VIEW, contacts: [], groups: [] }, m, identicon, {
			screen: 'list',
			query: ''
		});
		expect(model.screen).toBe('empty');
		expect(model.search?.placeholder).toBe(m.searchPlaceholder);
	});

	it('the list carries groups (with core ids), counts, and the full A–Z rail', () => {
		const model = buildContactsLive(VIEW, m, identicon, { screen: 'list', query: '' });
		expect(model.screen).toBe('list');
		expect(model.list?.groups[0]).toMatchObject({ id: 'g1', name: 'Payroll', count: '2' });
		expect(model.list?.indexLetters).toHaveLength(27);
		expect(model.list?.sections.map((s) => s.letter)).toEqual(['A', 'B', '#']);
	});

	it('detail shows the selected contact with its group chips', () => {
		const model = buildContactsLive(VIEW, m, identicon, {
			screen: 'detail',
			query: '',
			selectedAddress: ALICE.address
		});
		expect(model.screen).toBe('detail');
		expect(model.detail?.contact.name).toBe('Alice');
		expect(model.detail?.chips).toEqual(['Payroll']);
		expect(model.detail?.address.full).toBe(ALICE.address);
	});

	it('a vanished selection falls back to the list, never a blank screen', () => {
		const model = buildContactsLive(VIEW, m, identicon, {
			screen: 'detail',
			query: '',
			selectedAddress: '0xdead'
		});
		expect(model.screen).toBe('list');
	});

	it('the group screen resolves members through the core view', () => {
		const model = buildContactsLive(VIEW, m, identicon, {
			screen: 'group',
			query: '',
			selectedGroupId: 'g1'
		});
		expect(model.screen).toBe('group');
		expect(model.group?.group.members.map((c) => c.name)).toEqual(['Alice', 'Bob']);
	});
});

describe('buildContactsDesktopLive — the rail before the book is read', () => {
	const sidebar = {} as SidebarModel;
	const ui = { screen: 'list' as const, query: '' };

	// The same rule as the phone list and iOS: the rail said 全部联系人 "0" and
	// then jumped to the real count, with 分组 and its rows appearing under it.
	it('draws the 全部联系人 row with no count, and no groups or 新建分组', () => {
		const rail = buildContactsDesktopLive(
			{ ...VIEW, loaded: false, contacts: [], groups: [], sections: [] },
			m,
			identicon,
			ui,
			sidebar
		).rail;
		expect(rail.pending).toBe(true);
		expect(rail.allLabel).toBe(m.allContacts);
		expect(rail.allCount).toBe('');
		expect(rail.groups).toEqual([]);
	});

	it('draws the count and the groups once it is', () => {
		const desktop = buildContactsDesktopLive(VIEW, m, identicon, ui, sidebar);
		expect(desktop.rail.pending).toBe(false);
		expect(desktop.rail.allCount).toBe('4');
		expect(desktop.rail.groups.map((g) => g.name)).toEqual(['Payroll']);
		// An empty book is a real "0" — it has been read.
		const empty = buildContactsDesktopLive(
			{ ...VIEW, contacts: [], groups: [], sections: [] },
			m,
			identicon,
			ui,
			sidebar
		);
		expect(empty.rail.allCount).toBe('0');
		expect(empty.empty).toBeDefined();
	});
});

// ---------------------------------------------------------------------------
// 028 US5 — 最近往来 rows, the pickers, the import report
// ---------------------------------------------------------------------------

import type { FeedItem } from '$lib/core/generated/FeedItem';
import type { FeedView } from '$lib/core/generated/FeedView';
import { resolveWalletMessages } from '$lib/i18n/engine.server';
import { chainName } from '$lib/services/networks';
import type { LocalTransaction } from '$lib/services/transactions-model';
import { feedViewThroughCore } from '$lib/wallet/core/feed-through-core';
import { dappTitle } from '$lib/wallet/live';
import {
	groupPickModel,
	importReport,
	liveContactDetail,
	memberPickModel,
	RECENT_ACTIVITY_ROWS
} from './live';

const DAY = 86_400_000;
const NOW = new Date(2026, 8, 5, 12, 0, 0).getTime();
const TODAY = new Date(2026, 8, 5).getTime();
const wm = resolveWalletMessages('en');
const rowMessages = { activity: wm.activity };

/** A contact row as the core hands it: its second line the network and the day. */
function item(partial: Partial<FeedItem> & { id: string }): FeedItem {
	const day_start_ms = partial.day_start_ms ?? TODAY;
	return {
		direction: 'out',
		counterparty: ALICE.address,
		alias: null,
		value: '1.5',
		symbol: 'xDAI',
		decimals: 18,
		usd_value: 1.5,
		chain_id: 100,
		timestamp: Math.floor(NOW / 1000),
		day_start_ms,
		tx_hash: null,
		batch: null,
		kind: partial.direction === 'in' ? 'receive' : 'send',
		status: 'confirmed',
		site: null,
		counterparty_role: 'recipient',
		subtitle: [
			{ type: 'network', chain_id: 100 },
			{ type: 'day', day_start_ms }
		],
		...partial
	};
}

/** Alice's page is open: the core's `contact_rows` are hers, newest first. */
const FEED: FeedView = {
	rows: [],
	transactions: [],
	new_item_id: null,
	toast: null,
	history_empty_key: 'history.emptyTitle',
	home_empty_key: 'home.emptyNoActivity',
	contact_rows: [
		item({ id: 't1' }),
		item({ id: 't2', direction: 'in', value: '20', symbol: 'USDC' }),
		item({ id: 't4', day_start_ms: TODAY - DAY }),
		item({ id: 't5', day_start_ms: TODAY - 3 * DAY })
	]
};

/**
 * Spec 093: a contact's 最近往来 is the core's rows for that contact
 * (`contact_rows`), drawn by Activity's own row builder — the shell keeps no
 * counterparty filter and no "Sent"/"Received" of its own.
 */
describe('liveContactDetail with a feed', () => {
	const extras = { feed: FEED, contactAddress: ALICE.address, rowMessages, now: NOW };

	it('shows the recent few, all on request, and the empty state when there is nothing', () => {
		const recent = liveContactDetail(ALICE, VIEW, m, identicon, extras);
		expect(recent.rows).toHaveLength(RECENT_ACTIVITY_ROWS);
		expect(recent.rows.map((r) => r.id)).toEqual(['t1', 't2', 't4']);
		expect(recent.emptyActivity).toBeUndefined();

		const all = liveContactDetail(ALICE, VIEW, m, identicon, { ...extras, allActivity: true });
		expect(all.rows).toHaveLength(4);
	});

	it('draws the rows as Activity does, the second line the network and the day', () => {
		const rows = liveContactDetail(ALICE, VIEW, m, identicon, {
			...extras,
			allActivity: true
		}).rows;
		expect(rows[0]).toMatchObject({
			kind: 'sent',
			title: wm.activity.sent,
			amount: '\u22121.5',
			unit: 'xDAI',
			subtitle: `${chainName(100)} · ${wm.activity.today}`
		});
		expect(rows[1]).toMatchObject({ kind: 'received', amount: '+20', positive: true });
		expect(rows[2].subtitle).toBe(`${chainName(100)} · ${wm.activity.yesterday}`);
		// Older days read in the person's date preset, as the headers do.
		expect(rows[3].subtitle).toMatch(new RegExp(`^${chainName(100)} · \\S`));
		expect(rows[3].subtitle).not.toContain(wm.activity.yesterday);
		// Privacy masks a contact's figures as it masks Activity's.
		expect(
			liveContactDetail(ALICE, VIEW, m, identicon, { ...extras, hidden: true }).rows[0]
		).toMatchObject({ amount: '••••', masked: true });
	});

	it("another contact's rows never stand under this one", () => {
		// The core was told about Alice; Bob's page draws nothing of hers.
		const bob = liveContactDetail(BOB, VIEW, m, identicon, extras);
		expect(bob.rows).toEqual([]);
		expect(bob.emptyActivity).toBe(m.noActivity);
		const none = liveContactDetail(UNNAMED, VIEW, m, identicon, {
			...extras,
			contactAddress: null
		});
		expect(none.rows).toEqual([]);
		expect(none.emptyActivity).toBe(m.noActivity);
	});

	// Through the REAL core: a dApp's token transfer to Alice reads as the
	// verb it was, beside her plain send; Bob's receipt is not on her page.
	it("through the core: a dApp's transfer to a contact reads its verb, not 'Sent'", async () => {
		const ME = '0xD400866e00B055B20752a826CD5C89b811de130b';
		const USDC = '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48';
		const at = Math.floor(NOW / 1000);
		const transfer =
			'0xa9059cbb' +
			ALICE.address.slice(2).padStart(64, '0') +
			(5_000_000).toString(16).padStart(64, '0');
		const records: LocalTransaction[] = [
			{
				id: `dapp-${at * 1000}-tx`,
				userOpHash: '0x' + 'e3'.repeat(32),
				txHash: '0x' + 'f3'.repeat(32),
				from: ME,
				to: USDC,
				value: '0x0',
				symbol: 'ETH',
				decimals: 18,
				chainId: 1,
				timestamp: at,
				status: 'confirmed',
				type: 'dapp_tx',
				dappOrigin: 'https://pay.example',
				dappUrl: 'https://pay.example',
				intent: 'Transfer',
				signedRequest: { method: 'eth_sendTransaction', params: [{ to: USDC, data: transfer }] },
				dappSummary: { action: 'call', calls: 1, contract: USDC }
			},
			{
				id: '0x' + 'f4'.repeat(32),
				userOpHash: '0x' + 'e4'.repeat(32),
				txHash: '0x' + 'f4'.repeat(32),
				from: ME,
				to: ALICE.address,
				value: '0.2',
				symbol: 'ETH',
				decimals: 18,
				chainId: 1,
				timestamp: at - 86_400,
				status: 'confirmed',
				type: 'send'
			},
			{
				id: '1-0xbob-0',
				userOpHash: '',
				txHash: '0x' + 'f5'.repeat(32),
				from: BOB.address,
				to: ME,
				value: '3',
				symbol: 'ETH',
				decimals: 18,
				chainId: 1,
				timestamp: at - 60,
				status: 'confirmed',
				type: 'receive'
			}
		];
		const view = await feedViewThroughCore(records, ME, NOW, [
			{ type: 'contact_filter_changed', address: ALICE.address }
		]);
		expect(view.contact_rows.map((row) => row.id)).toEqual([records[0].id, records[1].id]);
		const rows = liveContactDetail(ALICE, VIEW, m, identicon, {
			feed: view,
			contactAddress: ALICE.address,
			rowMessages,
			now: NOW
		}).rows;
		const [dapp, send] = rows;
		expect(dapp.kind).toBe('dapp');
		expect(dapp.title).toBe(dappTitle(view.contact_rows[0].dapp!, wm));
		expect(dapp.title).not.toBe(wm.activity.sent);
		expect(dapp.title).toContain(wm.activity.intents.intentTransfer);
		expect(dapp.subtitle).toBe(`${chainName(1)} · ${wm.activity.today}`);
		expect(send).toMatchObject({ title: wm.activity.sent, amount: '\u22120.2' });
		expect(send.subtitle).toBe(`${chainName(1)} · ${wm.activity.yesterday}`);
		// The page closed: the core hands nobody's rows.
		const closed = await feedViewThroughCore(records, ME, NOW, [
			{ type: 'contact_filter_changed', address: ALICE.address },
			{ type: 'contact_filter_changed', address: null }
		]);
		expect(closed.contact_rows).toEqual([]);
	}, 30_000);
});

describe('the pickers', () => {
	it("memberPickModel lists the whole book with the group's members ticked", () => {
		const model = memberPickModel(VIEW, 'g1', m, identicon);
		expect(model.title).toBe(m.addMember);
		expect(model.rows.map((r) => [r.id, r.checked])).toEqual([
			[ALICE.address, true],
			[ANTON.address, false],
			[BOB.address, true],
			[UNNAMED.address, false]
		]);
		expect(model.rows[3].name).toMatch(/^0x/);
	});

	it('groupPickModel lists the groups holding this contact ticked', () => {
		expect(groupPickModel(VIEW, ANTON.address, m).rows).toEqual([
			{ id: 'g1', name: 'Payroll', detail: expect.stringContaining('2'), checked: false }
		]);
		expect(groupPickModel(VIEW, BOB.address, m).rows[0].checked).toBe(true);
	});
});

describe('importReport', () => {
	it('is nothing while nothing is pending, the counts after an import, the refusal after a bad file', () => {
		expect(importReport(VIEW, m)).toBeUndefined();
		const done = importReport(
			{ ...VIEW, last_import: { added: 2, skipped: 1, invalid: 0, groups_created: 1 } },
			m
		);
		expect(done?.title).toBe(m.importDoneTitle);
		expect(done?.body).toContain('2');
		expect(done?.body).not.toContain(m.importDoneInvalid.slice(0, 6));
		const invalid = importReport(
			{ ...VIEW, last_import: { added: 0, skipped: 0, invalid: 3, groups_created: 0 } },
			m
		);
		expect(invalid?.body).toContain('3');
		const refused = importReport({ ...VIEW, import_failure: { type: 'no_address_column' } }, m);
		expect(refused).toEqual({ title: m.importFailTitle, body: m.importFailBody });
		// Issue 333: a file in a legacy encoding is told how to save it.
		const encoding = importReport({ ...VIEW, import_failure: { type: 'unsupported_encoding' } }, m);
		expect(encoding).toEqual({ title: m.importFailTitle, body: m.importFailEncoding });
		expect(m.importFailEncoding).toContain('UTF-8');
	});
});

describe('the group screen says why its button is dead', () => {
	it('an empty group captions the CTA with the reason; a filled one with the count', () => {
		const emptyGroup = { id: 'g2', name: 'Nobody', color: null, members: [] };
		const view: ContactsView = { ...VIEW, groups: [...VIEW.groups, emptyGroup] };
		const empty = buildContactsLive(view, m, identicon, {
			screen: 'group',
			query: '',
			selectedGroupId: 'g2'
		});
		expect(empty.group?.ctaCaption).toBe(m.batchSendNeedsMembers);
		expect(empty.group?.captionTitled).toBe(m.batchSendNeedsMembers);
		const filled = buildContactsLive(view, m, identicon, {
			screen: 'group',
			query: '',
			selectedGroupId: 'g1'
		});
		expect(filled.group?.ctaCaption).toContain('2');
		expect(filled.group?.ctaCaption).not.toBe(m.batchSendNeedsMembers);
	});
});
