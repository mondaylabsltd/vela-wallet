/**
 * The live contacts builders (spec 024 T031): ContactsView → display models.
 * Core order authoritative; sectioning and search are render concerns.
 */
import { describe, expect, it } from 'vitest';
import type { Contact } from '$lib/core/generated/Contact';
import type { ContactsView } from '$lib/core/generated/ContactsView';
import { resolveContactsMessages } from '$lib/i18n/engine.server';
import { buildContactsLive, displayName, letterSections } from './live';

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

// ---------------------------------------------------------------------------
// 028 US5 — 最近往来 rows, the pickers, the import report
// ---------------------------------------------------------------------------

import type { FeedItem } from '$lib/core/generated/FeedItem';
import type { FeedView } from '$lib/core/generated/FeedView';
import {
	contactActivityRow,
	contactFeedItems,
	groupPickModel,
	importReport,
	liveContactDetail,
	memberPickModel,
	RECENT_ACTIVITY_ROWS
} from './live';

const DAY = 86_400_000;
const NOW = new Date(2026, 8, 5, 12, 0, 0).getTime();
const TODAY = new Date(2026, 8, 5).getTime();

function item(partial: Partial<FeedItem> & { id: string }): FeedItem {
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
		day_start_ms: TODAY,
		tx_hash: null,
		batch: null,
		...partial
	};
}

const FEED: FeedView = {
	rows: [
		{ type: 'header', id: 'day-1', day_start_ms: TODAY, timestamp: Math.floor(NOW / 1000) },
		{ type: 'item', item: item({ id: 't1' }) },
		{ type: 'item', item: item({ id: 't2', direction: 'in', value: '20', symbol: 'USDC' }) },
		// A different counterparty, and one spelled in checksum case.
		{ type: 'item', item: item({ id: 't3', counterparty: BOB.address }) },
		{
			type: 'item',
			item: item({
				id: 't4',
				counterparty: ALICE.address.toUpperCase().replace('0X', '0x'),
				day_start_ms: TODAY - DAY
			})
		},
		{ type: 'item', item: item({ id: 't5', day_start_ms: TODAY - 3 * DAY }) }
	],
	transactions: [],
	new_item_id: null,
	toast: null
};

describe('contactFeedItems', () => {
	it('keeps only this counterparty, case-insensitively, in feed order', () => {
		expect(contactFeedItems(FEED, ALICE.address).map((i) => i.id)).toEqual([
			't1',
			't2',
			't4',
			't5'
		]);
		expect(contactFeedItems(FEED, BOB.address).map((i) => i.id)).toEqual(['t3']);
		expect(contactFeedItems(null, ALICE.address)).toEqual([]);
	});
});

describe('contactActivityRow', () => {
	it('signs the amount by direction and says the network and the day, not the person', () => {
		const sent = contactActivityRow(item({ id: 't1' }), m, NOW);
		expect(sent.kind).toBe('sent');
		expect(sent.amount).toBe('-1.5');
		expect(sent.unit).toBe('xDAI');
		expect(sent.positive).toBe(false);
		expect(sent.subtitle).toContain(m.activity.today);
		expect(sent.subtitle).not.toContain('Alice');

		const received = contactActivityRow(
			item({ id: 't2', direction: 'in', value: '20', day_start_ms: TODAY - DAY }),
			m,
			NOW
		);
		expect(received.kind).toBe('received');
		expect(received.amount).toBe('+20');
		expect(received.subtitle).toContain(m.activity.yesterday);
	});
});

describe('liveContactDetail with a feed', () => {
	it('shows the recent few, all on request, and the empty state when there is nothing', () => {
		const recent = liveContactDetail(ALICE, VIEW, m, identicon, { feed: FEED, now: NOW });
		expect(recent.rows).toHaveLength(RECENT_ACTIVITY_ROWS);
		expect(recent.rows.map((r) => r.id)).toEqual(['t1', 't2', 't4']);
		expect(recent.emptyActivity).toBeUndefined();

		const all = liveContactDetail(ALICE, VIEW, m, identicon, {
			feed: FEED,
			allActivity: true,
			now: NOW
		});
		expect(all.rows).toHaveLength(4);

		const none = liveContactDetail(UNNAMED, VIEW, m, identicon, { feed: FEED, now: NOW });
		expect(none.rows).toEqual([]);
		expect(none.emptyActivity).toBe(m.noActivity);
	});
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
