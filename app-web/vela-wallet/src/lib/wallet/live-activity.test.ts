/**
 * The live activity builders (spec 025 Phase 4): the core's grouped rows →
 * the drawn ActivityGroupModel; the shell only words and formats.
 */
import { describe, expect, it } from 'vitest';
import type { FeedDapp } from '$lib/core/generated/FeedDapp';
import type { FeedItem } from '$lib/core/generated/FeedItem';
import type { FeedView } from '$lib/core/generated/FeedView';
import { resolveWalletMessages } from '$lib/i18n/engine.server';
import { preferences } from '$lib/services/preferences.svelte';
import { dayLabel, liveActivityGroups, liveActivityRow } from './live';

const m = resolveWalletMessages('en');
const DAY = 86_400_000;

function item(partial: Partial<FeedItem> & { id: string }): FeedItem {
	return {
		direction: 'in',
		counterparty: '0x' + 'b1'.repeat(20),
		alias: null,
		value: '1.5',
		symbol: 'ETH',
		decimals: 18,
		usd_value: 4500,
		chain_id: 1,
		timestamp: 1_700_000_000,
		day_start_ms: 0,
		tx_hash: '0xabc',
		batch: null,
		kind: (partial.direction ?? 'in') === 'in' ? 'receive' : 'send',
		status: 'confirmed',
		site: null,
		counterparty_role: 'recipient',
		...partial
	};
}

describe('dayLabel', () => {
	it('today and yesterday from the corpus; older days in the date preset', () => {
		const now = new Date(2026, 8, 3, 12).getTime();
		const today = new Date(2026, 8, 3).getTime();
		expect(dayLabel(today, m, now)).toBe(m.activity.today);
		expect(dayLabel(today - DAY, m, now)).toBe(m.activity.yesterday);
		// The preset, not the platform: the same history groups the same way on
		// every machine the person opens this wallet on (spec 028 D47).
		preferences.setDateFormat('iso');
		expect(dayLabel(today - 3 * DAY, m, now)).toBe('2026-08-31');
		preferences.setDateFormat('dmy_dot');
		expect(dayLabel(today - 3 * DAY, m, now)).toBe('31.08.2026');
		preferences.resetForTests();
	});
});

describe('liveActivityRow', () => {
	it('a receipt is positive, titled and attributed from the corpus', () => {
		const row = liveActivityRow(item({ id: 'a', alias: 'Alice' }), m, false);
		expect(row).toMatchObject({
			kind: 'received',
			title: m.activity.received,
			amount: '+1.5',
			unit: 'ETH',
			positive: true,
			masked: false
		});
		expect(row.subtitle).toContain('Alice');
	});
	it('a send is negative and names the recipient by short address when unaliased', () => {
		const row = liveActivityRow(item({ id: 'b', direction: 'out' }), m, false);
		expect(row.kind).toBe('sent');
		// U+2212, as the detail and the signing sheet write it (spec 082 G19).
		expect(row.amount).toBe('\u22121.5');
		expect(row.subtitle).toMatch(/0xb1b1/i);
	});
	it('privacy masks the amount', () => {
		expect(liveActivityRow(item({ id: 'c' }), m, true)).toMatchObject({
			amount: '••••',
			masked: true
		});
	});
	it('a mixed batch row shows its count, not an invented sum', () => {
		const row = liveActivityRow(
			item({
				id: 'd',
				direction: 'out',
				value: null,
				symbol: '',
				counterparty: null,
				batch: {
					kind: 'multi_select',
					count: 3,
					total_usd: 10,
					transfers: [],
					ids: [],
					from: '0x1',
					chain_id: 1,
					timestamp: 1,
					status: 'confirmed',
					tx_hash: '',
					user_op_hash: '',
					symbol: null,
					logo_urls: null,
					to: null,
					to_name: null
				}
			}),
			m,
			false
		);
		expect(row.amount).toBe('3');
	});

	/** Spec 082 RG1–RG4: what the row is and where it stands are the core's. */
	it('a dApp transaction is titled as one and names its site — never guessed from a direction', () => {
		const row = liveActivityRow(
			item({
				id: 'e',
				direction: 'out',
				kind: 'dapp_tx',
				status: 'confirmed',
				site: 'app.uniswap.org',
				value: null,
				symbol: ''
			}),
			m,
			false
		);
		expect(row).toMatchObject({
			kind: 'dapp',
			title: m.activity.dapp,
			subtitle: 'app.uniswap.org'
		});
		expect(row.amount).toBe('');
	});

	it('a row the tracker has not closed says so first: Pending · <site>, Failed · <site>', () => {
		const pending = liveActivityRow(
			item({
				id: 'f',
				direction: 'out',
				kind: 'dapp_tx',
				status: 'pending',
				site: '127.0.0.1:8137'
			}),
			m,
			false
		);
		expect(pending.subtitle).toBe(`${m.activity.pending} · 127.0.0.1:8137`);
		const failed = liveActivityRow(
			item({ id: 'g', direction: 'out', kind: 'send', status: 'failed', alias: 'Bob' }),
			m,
			false
		);
		expect(failed.subtitle).toBe(
			`${m.activity.failed} · ${m.activity.toName.replace('{{name}}', 'Bob')}`
		);
	});

	// 087 F04: the core says `unknown` for a pending record nothing will
	// settle (no op hash past its grace, or past the tracker's 24 h).
	it('a record nothing will settle says Unknown first — never Pending for ever, never Failed', () => {
		const row = liveActivityRow(
			item({
				id: 'dapp-1790500000796-tx',
				direction: 'out',
				kind: 'dapp_tx',
				status: 'unknown',
				site: 'app.uniswap.org'
			}),
			m,
			false
		);
		expect(m.activity.unknown).toBe('Unknown');
		expect(row.subtitle).toBe(`${m.activity.unknown} · app.uniswap.org`);
	});

	it('a may-have-been-sent op is a Pending dApp row under its local hash (RG4)', () => {
		const LOCAL = '0x' + 'ab'.repeat(32);
		const row = liveActivityRow(
			item({ id: LOCAL, direction: 'out', kind: 'dapp_tx', status: 'pending', site: 'a.example' }),
			m,
			false
		);
		expect(row.id).toBe(LOCAL);
		expect(row.subtitle.startsWith(m.activity.pending)).toBe(true);
	});

	// 083 H2: a dApp's transaction is titled by what it did — the sheet's
	// words, "Contract interaction" when nobody decoded it — and labelled
	// with the site that asked; with no figure when it moved no coin.
	it("a dApp's call is titled by its intent and labelled with its site", () => {
		const dapp = (
			id: string,
			site: string | null,
			intent: string | null,
			term: FeedDapp['intent_term']
		) =>
			item({
				id,
				direction: 'out',
				value: null,
				symbol: '',
				decimals: null,
				usd_value: 0,
				dapp: { site, intent, intent_term: term }
			});
		const blind = liveActivityRow(dapp('a', 'app.uniswap.org', null, null), m, false);
		expect(blind).toMatchObject({
			kind: 'sent',
			title: m.activity.contractCall,
			subtitle: 'app.uniswap.org',
			amount: '',
			unit: ''
		});
		// No figure, nothing to mask: "••••" would claim one (083 H2 review).
		expect(liveActivityRow(dapp('a', 'app.uniswap.org', null, null), m, true)).toMatchObject({
			amount: '',
			masked: false
		});
		const swap = liveActivityRow(dapp('b', 'app.uniswap.org', 'Swap', 'intentSwap'), m, false);
		expect(swap.title).toBe(m.activity.intents.intentSwap);
		expect(swap.title).not.toBe('');
		// A descriptor's word with no translation is still better than none,
		// and without a site the row says who it went to, like a send.
		const odd = liveActivityRow(dapp('c', null, 'Frobnicate', null), m, false);
		expect(odd.title).toBe('Frobnicate');
		expect(odd.subtitle).toMatch(/0xb1b1/i);
	});
	it("a dApp's native send reads as a send of its amount", () => {
		const row = liveActivityRow(
			item({
				id: 'e',
				direction: 'out',
				value: '0.01',
				symbol: 'xDAI',
				dapp: { site: '127.0.0.1', intent: 'Send', intent_term: 'intentSend' }
			}),
			m,
			false
		);
		expect(row).toMatchObject({
			title: m.activity.intents.intentSend,
			subtitle: '127.0.0.1',
			amount: '\u22120.01',
			unit: 'xDAI'
		});
	});
});

describe('liveActivityGroups', () => {
	it('headers open groups, items fill them, in the order the core emitted', () => {
		const today = new Date(new Date().setHours(0, 0, 0, 0)).getTime();
		const view: FeedView = {
			rows: [
				{ type: 'header', id: `day-${today}`, day_start_ms: today, timestamp: today / 1000 },
				{ type: 'item', item: item({ id: 'x' }) },
				{ type: 'item', item: item({ id: 'y', direction: 'out' }) },
				{ type: 'header', id: `day-${today - DAY}`, day_start_ms: today - DAY, timestamp: 1 },
				{ type: 'item', item: item({ id: 'z' }) }
			],
			transactions: [],
			new_item_id: null,
			toast: null,
			history_empty_key: 'history.emptyTitle',
			home_empty_key: 'home.emptyNoActivity'
		};
		const groups = liveActivityGroups(view, m, false);
		expect(groups.map((g) => [g.label, g.rows.length])).toEqual([
			[m.activity.today, 2],
			[m.activity.yesterday, 1]
		]);
	});
});
