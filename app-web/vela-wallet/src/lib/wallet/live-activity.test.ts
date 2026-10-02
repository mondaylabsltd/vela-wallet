/**
 * The live activity builders (spec 025 Phase 4): the core's grouped rows →
 * the drawn ActivityGroupModel; the shell only words and formats.
 */
import { describe, expect, it } from 'vitest';
import type { FeedDapp } from '$lib/core/generated/FeedDapp';
import type { FeedItem } from '$lib/core/generated/FeedItem';
import type { FeedView } from '$lib/core/generated/FeedView';
import { resolveWalletMessages } from '$lib/i18n/engine.server';
import { chainName } from '$lib/services/networks';
import { preferences } from '$lib/services/preferences.svelte';
import { feedItemsThroughCore } from './core/feed-through-core';
import { dappActivityRecords, feedDapp } from './dapp-activity-fixtures';
import { dayLabel, liveActivityGroups, liveActivityRow } from './live';

const m = resolveWalletMessages('en');
const zh = resolveWalletMessages('zh');
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
		subtitle: [],
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
		const row = liveActivityRow(
			item({
				id: 'a',
				alias: 'Alice',
				subtitle: [{ type: 'from', address: '0x' + 'b1'.repeat(20), name: 'Alice' }]
			}),
			m,
			false
		);
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
		const row = liveActivityRow(
			item({
				id: 'b',
				direction: 'out',
				subtitle: [{ type: 'to', address: '0x' + 'b1'.repeat(20), name: null }]
			}),
			m,
			false
		);
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
				symbol: '',
				subtitle: [{ type: 'site', site: 'app.uniswap.org' }]
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
				site: '127.0.0.1:8137',
				subtitle: [
					{ type: 'status', status: 'pending' },
					{ type: 'site', site: '127.0.0.1:8137' }
				]
			}),
			m,
			false
		);
		expect(pending.subtitle).toBe(`${m.activity.pending} · 127.0.0.1:8137`);
		const failed = liveActivityRow(
			item({
				id: 'g',
				direction: 'out',
				kind: 'send',
				status: 'failed',
				alias: 'Bob',
				subtitle: [
					{ type: 'status', status: 'failed' },
					{ type: 'to', address: '0x' + 'b1'.repeat(20), name: 'Bob' }
				]
			}),
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
				site: 'app.uniswap.org',
				subtitle: [
					{ type: 'status', status: 'unknown' },
					{ type: 'site', site: 'app.uniswap.org' }
				]
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
			item({
				id: LOCAL,
				direction: 'out',
				kind: 'dapp_tx',
				status: 'pending',
				site: 'a.example',
				subtitle: [
					{ type: 'status', status: 'pending' },
					{ type: 'site', site: 'a.example' }
				]
			}),
			m,
			false
		);
		expect(row.id).toBe(LOCAL);
		expect(row.subtitle.startsWith(m.activity.pending)).toBe(true);
	});

	// 083 H2, spec 093: a dApp row is titled by what it did — the core's
	// headline verb in the reader's words, the descriptor's own text when the
	// wallet has none — and where; with no figure when it moved nothing.
	it("a dApp's row is titled by its verb and place, both the core's", () => {
		const dapp = (id: string, partial: Partial<FeedDapp>) =>
			item({
				id,
				direction: 'out',
				kind: 'dapp_tx',
				value: null,
				symbol: '',
				decimals: null,
				usd_value: 0,
				dapp: feedDapp(partial),
				subtitle: [{ type: 'network', chain_id: 1 }]
			});
		const blind = liveActivityRow(dapp('a', { intent_term: 'intentContractCall' }), m, false);
		expect(blind).toMatchObject({
			kind: 'dapp',
			title: m.activity.intents.intentContractCall,
			subtitle: 'Ethereum',
			amount: '',
			unit: ''
		});
		// No figure, nothing to mask: "••••" would claim one (083 H2 review).
		expect(liveActivityRow(dapp('a', {}), m, true)).toMatchObject({ amount: '', masked: false });
		const swap = liveActivityRow(
			dapp('b', { intent: 'Swap', intent_term: 'intentSwap', place: 'Uniswap' }),
			m,
			false
		);
		expect(swap.title).toBe('Swap on Uniswap');
		expect(
			liveActivityRow(dapp('b', { intent_term: 'intentSwap', place: 'Uniswap' }), zh, false).title
		).toBe('在 Uniswap 兑换');
		// A descriptor's word with no translation is still better than none.
		const odd = liveActivityRow(dapp('c', { intent: 'Frobnicate', place: 'a.example' }), m, false);
		expect(odd.title).toBe('Frobnicate on a.example');
		// The signatures' own verbs (spec 093).
		expect(liveActivityRow(dapp('d', { intent_term: 'permitIntent' }), m, false).title).toBe(
			m.activity.intents.permitIntent
		);
		expect(m.activity.intents.permitIntent).toBe('Spending permit');
	});

	it('the second line is the core’s parts, worded and joined', () => {
		const row = liveActivityRow(
			item({
				id: 'l',
				direction: 'out',
				kind: 'send',
				subtitle: [
					{ type: 'status', status: 'pending' },
					{ type: 'to', address: '0x' + 'cd'.repeat(20), name: null },
					{ type: 'site', site: 'app.uniswap.org' },
					{ type: 'network', chain_id: 42161 }
				]
			}),
			m,
			false
		);
		expect(row.subtitle).toBe(
			`${m.activity.pending} · To 0xcdcdcd…cdcdcd · app.uniswap.org · ${chainName(42161)}`
		);
		// The shell adds nothing the core did not send: no line, no subtitle.
		expect(liveActivityRow(item({ id: 'n', subtitle: [] }), m, false).subtitle).toBe('');
	});

	// Spec 093: a contact's rows carry the day instead of headers — worded as
	// the date headers are: today, yesterday, else the person's date preset.
	it('the day part reads as the date headers do', () => {
		const now = new Date(2026, 8, 3, 12).getTime();
		const today = new Date(2026, 8, 3).getTime();
		const row = (day: number) =>
			liveActivityRow(
				item({
					id: 'd',
					subtitle: [
						{ type: 'network', chain_id: 1 },
						{ type: 'day', day_start_ms: day }
					]
				}),
				m,
				false,
				now
			).subtitle;
		expect(row(today)).toBe(`${chainName(1)} · ${m.activity.today}`);
		expect(row(today - DAY)).toBe(`${chainName(1)} · ${m.activity.yesterday}`);
		preferences.setDateFormat('iso');
		expect(row(today - 3 * DAY)).toBe(`${chainName(1)} · 2026-08-31`);
		expect(row(today - 3 * DAY)).toBe(`${chainName(1)} · ${dayLabel(today - 3 * DAY, m, now)}`);
		preferences.resetForTests();
	});

	it('an allowance is the figure: unlimited in red and never masked, a cap like money', () => {
		const grant = (id: string, value: string | null, unlimited: boolean) =>
			item({
				id,
				direction: 'out',
				kind: 'sign_typed_data',
				value: null,
				symbol: '',
				dapp: feedDapp({
					action: 'permit',
					off_chain: true,
					intent_term: 'permitIntent',
					allowance: { symbol: 'USDC', value, decimals: value ? 6 : null, unlimited, token: null }
				})
			});
		expect(liveActivityRow(grant('u', null, true), m, false)).toMatchObject({
			kind: 'dapp',
			amount: m.activity.unlimited,
			unit: 'USDC',
			danger: true,
			masked: false
		});
		// A risk to see, not a balance: privacy does not hide it.
		expect(liveActivityRow(grant('u', null, true), m, true).amount).toBe(m.activity.unlimited);
		const capped = liveActivityRow(grant('c', '100', false), m, false);
		expect(capped).toMatchObject({ amount: '100', unit: 'USDC', masked: false });
		expect(capped.danger).toBeUndefined();
		expect(liveActivityRow(grant('c', '100', false), m, true)).toMatchObject({
			amount: '••••',
			masked: true
		});
	});

	it('a simulated figure says "≈", and a swap’s coin back sits beside it', () => {
		const swap = item({
			id: 's',
			direction: 'out',
			kind: 'dapp_tx',
			value: '100',
			symbol: 'USDC',
			dapp: feedDapp({
				estimated: true,
				received: {
					direction: 'in',
					verified: true,
					symbol: 'ETH',
					value: '0.03',
					decimals: 18
				}
			})
		});
		expect(liveActivityRow(swap, m, false)).toMatchObject({
			amount: '≈ \u2212100',
			unit: 'USDC',
			received: { amount: '≈ +0.03', unit: 'ETH' }
		});
		// The chain's own receipt folded in: exact, no "≈".
		const landed = {
			...swap,
			dapp: feedDapp({ received: { ...swap.dapp!.received!, exact: true } })
		};
		expect(liveActivityRow(landed, m, false).received?.amount).toBe('+0.03');
		expect(liveActivityRow(swap, m, true)).toMatchObject({
			amount: '••••',
			received: { amount: '••••', unit: 'ETH' }
		});
	});

	// Spec 093: the three fixture records, read and worded by the REAL core.
	it('through the core: a swap, a permit and a sign-in, each said in plain words', async () => {
		const items = await feedItemsThroughCore(
			dappActivityRecords(ACCOUNT, NOW_S),
			ACCOUNT,
			NOW_S * 1000 + 1000
		);
		const rows = items.map((it) => liveActivityRow(it, m, false));
		expect(rows.map((row) => [row.kind, row.title, row.subtitle])).toEqual([
			['dapp', 'Sign in on app.uniswap.org', 'Ethereum'],
			['dapp', 'Spending permit on Uniswap', 'app.uniswap.org · Ethereum'],
			['dapp', 'Swap on Uniswap', 'app.uniswap.org · Ethereum']
		]);
		expect(rows[0]).toMatchObject({ amount: '', unit: '' });
		expect(rows[1]).toMatchObject({ amount: m.activity.unlimited, unit: 'USDC', danger: true });
		expect(rows[2]).toMatchObject({
			amount: '≈ \u2212100',
			unit: 'USDC',
			received: { amount: '≈ +0.03', unit: 'ETH' }
		});
		const zhRows = items.map((it) => liveActivityRow(it, zh, false).title);
		expect(zhRows).toEqual(['在 app.uniswap.org 登录', '在 Uniswap 授权签名', '在 Uniswap 兑换']);
	}, 30_000);
});

const ACCOUNT = '0xD400866e00B055B20752a826CD5C89b811de130b';
const NOW_S = 1_790_000_000;

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
			home_empty_key: 'home.emptyNoActivity',
			contact_rows: []
		};
		const groups = liveActivityGroups(view, m, false);
		expect(groups.map((g) => [g.label, g.rows.length])).toEqual([
			[m.activity.today, 2],
			[m.activity.yesterday, 1]
		]);
	});
});
