/**
 * The live wallet-home builders (spec 025 T125): BalanceView + currency pair
 * → the drawn models. Presentation only — the numbers are the core's.
 */
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { formatRelativeTime } from '$lib/core/kernels';
import type { BalanceView } from '$lib/core/generated/BalanceView';
import type { CurrencyView } from '$lib/core/generated/CurrencyView';
import type { UnreachableNetwork } from '$lib/core/generated/UnreachableNetwork';
import { resolveWalletMessages } from '$lib/i18n/engine.server';
import type { Locale } from '$lib/i18n/locales';
import { fill } from './messages';
import { buildMobileState } from './fixtures';
import {
	agoText,
	liveAssetRow,
	liveBalance,
	moneyParts,
	tokenAmountText,
	trimBalance,
	withLiveWallet,
	withLiveWalletDesktop
} from './live';
import { buildDesktopState } from './fixtures';
import type { FeedView } from '$lib/core/generated/FeedView';
import { resolveWalletFlowMessages } from '$lib/i18n/engine.server';
import { buildFlowState } from '$lib/flows/fixtures';
import { withLiveFlow } from '$lib/flows/live';

const m = resolveWalletMessages('en');

/** The core's relative-time conformance vectors (issue 462). */
const RELATIVE_TIME = JSON.parse(
	readFileSync('../../rust/crates/vela-core/tests/vectors/relative-time.json', 'utf8')
) as {
	cases: {
		name: string;
		input: {
			lng: string;
			ts_seconds: number;
			now_ms: number;
			utc_offset_minutes: number;
			date_format: string;
		};
		expect: { value: string };
	}[];
};
const USD: CurrencyView = { code: 'USD', rate: 1, committed: true };
const EUR: CurrencyView = { code: 'EUR', rate: 0.5, committed: true };
const UNPRICED_JPY: CurrencyView = { code: 'JPY', rate: null, committed: true };

const PRISTINE: BalanceView = {
	address: null,
	display_total_usd: null,
	balance_unknown: true,
	balance_partial: false,
	unreachable: false,
	notice: null,
	hidden: false,
	refreshing: false,
	last_refreshed_at_ms: null,
	tokens: [],
	unpriced_tokens: [],
	failed_chain_ids: [],
	rate_limited_chain_ids: [],
	unreachable_networks: [],
	unreachable_key: null,
	internal_chain_ids: [],
	internal_key: null,
	holdings_loading: false,
	cached_total_usd: null,
	switcher: { open: false, loading: false, balances: [], hidden: false }
};

/** A network the core lists as unreachable, not read yet (spec 092). */
function unreachableRow(chainId: number): UnreachableNetwork {
	return {
		chain_id: chainId,
		last_known: 'not_read',
		last_seen_usd: null,
		line_key: 'assets.notReadYet'
	};
}

const ETH = {
	chain_id: 1,
	symbol: 'ETH',
	name: 'Ether',
	balance: '1.500000000000000000',
	decimals: 18,
	token_address: null,
	price_usd: 3000,
	spam: false
};

describe('moneyParts', () => {
	it('groups digits and splits the cents', () => {
		expect(moneyParts(4500, USD)).toMatchObject({ code: 'USD', integer: '$4,500', decimals: '00' });
		expect(moneyParts(1383.28, USD)).toMatchObject({ integer: '$1,383', decimals: '28' });
	});
	it('converts at the committed rate', () => {
		expect(moneyParts(4500, EUR)).toMatchObject({ code: 'EUR', integer: '€2,250' });
	});
	it('a null rate shows the USD figure, never a defaulted 1 under a ¥ (024 rule)', () => {
		expect(moneyParts(4500, UNPRICED_JPY)).toMatchObject({ code: 'USD', integer: '$4,500' });
	});
});

describe('trimBalance', () => {
	it('trims the tail without touching the number', () => {
		expect(trimBalance('1.500000000000000000')).toBe('1.5');
		expect(trimBalance('0')).toBe('0');
		expect(trimBalance('12.3456789')).toBe('12.345678');
	});
});

describe('tokenAmountText — the one token-amount formatter (spec 078)', () => {
	// The core's own vectors for `send::max_figure`
	// (`a_max_reads_on_the_balance_lines_ladder`): the figure Max writes and
	// the balance it came from must agree digit for digit.
	it.each([
		['0.043790209243313861', '0.04379'],
		['0.0439686', '0.043969'],
		['1.22456789123456789', '1.2246'],
		['1234.567', '1234.57'],
		['2', '2'],
		['0', '0'],
		['0.9999996', '1'],
		['999.99996', '1000'],
		['0.0000001234', '0.00000012'],
		['5.000000', '5']
	])('%s reads %s', (exact, shown) => {
		expect(tokenAmountText(exact)).toBe(shown);
	});

	it('rounds half up where trimBalance truncated', () => {
		expect(tokenAmountText('0.0437909')).toBe('0.043791');
		expect(trimBalance('0.0437909')).toBe('0.04379');
	});

	it('rounds DOWN when asked — a ceiling typed back has to fit', () => {
		expect(tokenAmountText('0.0409086', 'down')).toBe('0.040908');
		expect(tokenAmountText('0.9999996', 'down')).toBe('0.999999');
		expect(tokenAmountText('0.0409086')).toBe('0.040909');
	});

	it('passes through what is not a plain decimal rather than inventing digits', () => {
		expect(tokenAmountText('')).toBe('');
		expect(tokenAmountText('-1.5')).toBe('-1.5');
		expect(tokenAmountText('1e-7')).toBe('1e-7');
	});

	it('writes the preset’s decimal mark', async () => {
		const { preferences } = await import('$lib/services/preferences.svelte');
		preferences.setNumberFormat('dot_comma');
		try {
			expect(tokenAmountText('0.043968123456789012')).toBe('0,043968');
		} finally {
			preferences.setNumberFormat('comma_dot');
		}
	});
});

describe('liveBalance', () => {
	it('the pristine view is a skeleton — never a fake $0', () => {
		expect(liveBalance(PRISTINE, USD, m).state).toBe('loading');
	});
	it('hidden withholds the figure with the mask', () => {
		const model = liveBalance({ ...PRISTINE, hidden: true, display_total_usd: 4500 }, USD, m);
		expect(model.state).toBe('hidden');
		expect(model.integer).toBe('••••••');
		expect(model.decimals).toBeUndefined();
	});
	it('a live zero with nothing held is the zero-live state', () => {
		const model = liveBalance(
			{ ...PRISTINE, balance_unknown: false, display_total_usd: 0 },
			USD,
			m
		);
		expect(model.state).toBe('zero-live');
		expect(model.liveText).toBe(m.balance.liveIndicator);
	});
	it('a cached total paints first, marked as refreshing', () => {
		const model = liveBalance({ ...PRISTINE, cached_total_usd: 1383.28 }, USD, m);
		expect(model).toMatchObject({
			state: 'normal',
			integer: '$1,383',
			decimals: '28',
			status: { kind: 'refreshing', text: m.balance.stale }
		});
	});
	it('a partial zero is NOT live-zero: an unreachable chain names itself (T152 finding)', () => {
		const partial = {
			...PRISTINE,
			balance_unknown: false,
			balance_partial: true,
			display_total_usd: 0,
			failed_chain_ids: [100],
			unreachable_networks: [unreachableRow(100)],
			unreachable_key: 'assets.unreachableOne'
		};
		const model = liveBalance(partial, USD, m);
		expect(model.state).toBe('normal');
		expect(model.liveText).toBeUndefined();
		// Spec 092 (F08): no "RPC" on the home — a fact, calmly.
		expect(model.status).toEqual({ kind: 'warning', text: "Can't reach Gnosis right now" });
		// Several unreachable chains: the count of every one of them.
		expect(
			liveBalance(
				{
					...partial,
					failed_chain_ids: [1, 100],
					unreachable_networks: [unreachableRow(1), unreachableRow(100)],
					unreachable_key: 'assets.unreachableMany'
				},
				USD,
				m
			).status?.text
		).toBe("Can't reach 2 networks right now");
	});
	it('a rate-limited chain is not nagged about: failed but not a banner → still-updating', () => {
		const model = liveBalance(
			{
				...PRISTINE,
				balance_unknown: false,
				balance_partial: true,
				display_total_usd: 10,
				tokens: [ETH],
				failed_chain_ids: [56],
				rate_limited_chain_ids: [56],
				unreachable_networks: [],
				unreachable_key: null,
				notice: 'still_updating'
			},
			USD,
			m
		);
		expect(model.status).toEqual({ kind: 'refreshing', text: m.balance.stale });
	});
	it('an unpriced notice is a warning; still-updating is refreshing', () => {
		const live = { ...PRISTINE, balance_unknown: false, display_total_usd: 10, tokens: [ETH] };
		expect(liveBalance({ ...live, notice: 'unpriced' }, USD, m).status).toEqual({
			kind: 'warning',
			text: m.balance.unpriced
		});
		expect(liveBalance({ ...live, notice: 'still_updating' }, USD, m).status?.kind).toBe(
			'refreshing'
		);
	});
});

describe('the hero refresh control (issue 462)', () => {
	const NOW = Date.UTC(2026, 9, 8, 12, 0, 0);
	const read = {
		...PRISTINE,
		balance_unknown: false,
		display_total_usd: 4500,
		tokens: [ETH],
		last_refreshed_at_ms: NOW - 120_000
	};
	const at = (ms: number) => agoText(NOW - ms, NOW, m.balance.ago, 'en');

	it('ages in the core’s compact words, on the core’s thresholds', () => {
		expect(at(0)).toBe('now');
		expect(at(44_000)).toBe('now');
		expect(at(45_000)).toBe('1m');
		expect(at(89_000)).toBe('1m');
		expect(at(90_000)).toBe('2m');
		expect(at(3_599_000)).toBe('60m');
		expect(at(3_600_000)).toBe('1h');
		expect(at(86_399_000)).toBe('24h');
		// A read stamped ahead of this clock is "now", never a negative age.
		expect(at(-5_000)).toBe('now');
		// Past a day the core names the weekday, in the page's language…
		expect(at(86_400_000)).toMatch(/^(Mon|Tue|Wed|Thu|Fri|Sat|Sun)$/);
		expect(agoText(NOW - 2 * 86_400_000, NOW, m.balance.ago, 'zh')).toMatch(/^周[一二三四五六日]$/);
		// …and past a week writes the date, in the person's preset.
		expect(at(8 * 86_400_000)).toMatch(/\d/);
		// The corpus's own words, in another language.
		const zh = resolveWalletMessages('zh');
		expect(agoText(NOW - 120_000, NOW, zh.balance.ago, 'zh')).toBe('2分钟前');
		expect(fill(zh.balance.lastUpdated, { ago: zh.balance.ago.now })).not.toContain('{{');
	});

	it('is the core’s answer for every vector, in the words each page ships', () => {
		// tests/vectors/relative-time.json, the suite every app replays: the
		// page's prerendered time.* words and its locale, handed to the core.
		expect(RELATIVE_TIME.cases.length).toBeGreaterThan(100);
		for (const c of RELATIVE_TIME.cases) {
			const words = resolveWalletMessages(c.input.lng as Locale).balance.ago;
			const got = formatRelativeTime(
				c.input.ts_seconds * 1000,
				c.input.now_ms,
				c.input.utc_offset_minutes,
				c.input.date_format,
				c.input.lng,
				words
			);
			expect(got, c.name).toBe(c.expect.value);
		}
	});

	it('says when the figure was read, and turns while the read the person asked for is out', () => {
		const rest = liveBalance(read, USD, m, { now: NOW, held: false, language: 'en' });
		expect(rest.refresh).toEqual({
			updated: fill(m.balance.lastUpdated, { ago: '2m' }),
			updating: m.balance.updating,
			a11yIdle: m.balance.refreshBalance,
			spinning: false
		});
		expect(rest.refresh?.updated).toBe('Updated 2m');
		expect(m.balance.refreshBalance).toBe('Refresh balance');
		expect(m.balance.updating).toBe('Updating…');
		// The core's flag turns it…
		expect(
			liveBalance({ ...read, refreshing: true }, USD, m, { now: NOW, held: false, language: 'en' })
				.refresh
		).toMatchObject({ spinning: true });
		// …and so does a press's 650 ms hold, after the core has already answered.
		expect(
			liveBalance(read, USD, m, { now: NOW, held: true, language: 'en' }).refresh?.spinning
		).toBe(true);
	});

	it('a refresh the person asked for adds no status line above the control', () => {
		const model = liveBalance({ ...read, refreshing: true }, USD, m, {
			now: NOW,
			held: false,
			language: 'en'
		});
		expect(model.status).toBeUndefined();
		// A figure that really is not final keeps its line: the cache, still updating.
		const cached = { ...PRISTINE, cached_total_usd: 1383.28, refreshing: true };
		expect(liveBalance(cached, USD, m).status).toEqual({
			kind: 'refreshing',
			text: m.balance.stale
		});
		expect(
			liveBalance({ ...read, refreshing: true, notice: 'still_updating' }, USD, m).status?.kind
		).toBe('refreshing');
	});

	it('is drawn under a skeleton and a hidden figure too, and nowhere it was not asked for', () => {
		const input = { now: NOW, held: false, language: 'en' };
		const loading = liveBalance({ ...PRISTINE, refreshing: true }, USD, m, input);
		expect(loading.state).toBe('loading');
		expect(loading.refresh).toMatchObject({ updated: undefined, spinning: true });
		const hidden = liveBalance({ ...read, hidden: true }, USD, m, input);
		expect(hidden.state).toBe('hidden');
		expect(hidden.refresh?.updated).toBe('Updated 2m');
		expect(liveBalance(read, USD, m).refresh).toBeUndefined();
	});

	it('reaches the home and the wide layout through the live inputs', () => {
		const inputs = {
			balance: read,
			currency: USD,
			m,
			refresh: { now: NOW, held: false, language: 'en' }
		};
		const home = withLiveWallet(
			buildMobileState('h1', m, () => ''),
			inputs
		);
		expect(home.balance.refresh?.updated).toBe('Updated 2m');
		const wide = withLiveWalletDesktop(
			buildDesktopState('d1', m, () => ''),
			inputs
		);
		expect(wide.balance.refresh?.updated).toBe('Updated 2m');
	});
});

describe('liveAssetRow', () => {
	it('prices the row at the committed currency, trims the balance, colours the chain', () => {
		const row = liveAssetRow(ETH, EUR, m, false);
		expect(row).toMatchObject({
			ticker: 'ETH',
			chain: 'Ethereum',
			balance: '1.5',
			fiat: { kind: 'value', text: '€2,250.00' },
			masked: false
		});
		expect(row.badgeColor).toMatch(/^#/);
	});
	it('no price is said, not guessed; hidden masks both figures', () => {
		expect(liveAssetRow({ ...ETH, price_usd: null }, USD, m, false).fiat).toEqual({
			kind: 'no-price',
			text: m.balance.noPrice
		});
		const hidden = liveAssetRow(ETH, USD, m, true);
		expect(hidden.fiat).toEqual({ kind: 'masked' });
		expect(hidden.balance).toBe('••••');
	});
});

describe('withLiveWallet', () => {
	const base = buildMobileState('h1', m, (seed) => `<svg data-seed="${seed}"></svg>`);

	it('replaces the fixture money with the core view and leaves activity honestly loading', () => {
		const model = withLiveWallet(base, {
			balance: { ...PRISTINE, balance_unknown: false, display_total_usd: 4500, tokens: [ETH] },
			currency: USD,
			m
		});
		expect(model.balance.integer).toBe('$4,500');
		expect(model.assetRows.map((r) => r.ticker)).toEqual(['ETH']);
		expect(model.assetsSection.mode).toBe('rows');
		expect(model.activitySection.mode).toBe('loading');
		expect(model.activityGroups).toEqual([]);
		// The fixture's staged figures are gone.
		expect(JSON.stringify(model)).not.toContain('$1,383');
	});

	it('nothing held after the core has looked is the empty state, not a skeleton', () => {
		const model = withLiveWallet(base, {
			balance: { ...PRISTINE, balance_unknown: false, display_total_usd: 0 },
			currency: USD,
			m
		});
		expect(model.assetsSection.mode).toBe('empty');
	});

	it('a first load streams the list under a skeleton hero (issue 188)', () => {
		// Nothing cached, nothing settled, one chain answered: the core keeps
		// the figure withheld while the holdings already list.
		const model = withLiveWallet(base, {
			balance: { ...PRISTINE, tokens: [ETH] },
			currency: USD,
			m
		});
		expect(model.balance.state).toBe('loading');
		expect(model.balance.integer).toBeUndefined();
		expect(model.assetsSection.mode).toBe('rows');
		expect(model.assetRows.map((r) => r.ticker)).toEqual(['ETH']);
	});
});

describe('liveBalance — the decimal mark is the preset’s (spec 028 Phase 9, T480)', () => {
	it('carries the preset’s decimal mark beside the grouped integer', async () => {
		const { preferences } = await import('$lib/services/preferences.svelte');
		preferences.setNumberFormat('dot_comma');
		try {
			const model = liveBalance({ ...PRISTINE, display_total_usd: 1575.55 }, USD, m);
			expect(model.integer).toBe('$1.575');
			expect(model.decimals).toBe('55');
			expect(model.decimalMark).toBe(',');
		} finally {
			preferences.setNumberFormat('comma_dot');
		}
		const model = liveBalance({ ...PRISTINE, display_total_usd: 1575.55 }, USD, m);
		expect(model.integer).toBe('$1,575');
		expect(model.decimalMark).toBe('.');
	});
});

/**
 * Spec 082 RG5, RB13 (L-D7, G1): which empty line the home and the history
 * say is the core's (`home_empty_key`, `history_empty_key`), and the wide
 * layout draws the section's mode the way the narrow one does.
 */
describe('empty activity, chosen by the core', () => {
	const IDENT = (seed: string) => `<svg data-seed="${seed}"></svg>`;
	const EMPTY_FEED: FeedView = {
		rows: [],
		transactions: [],
		new_item_id: null,
		toast: null,
		history_empty_key: 'history.emptyTitle',
		home_empty_key: 'home.emptyNoActivity',
		hidden: false,
		contact_rows: []
	};
	const FILTERED: FeedView = {
		...EMPTY_FEED,
		history_empty_key: 'history.emptyFilter',
		home_empty_key: 'home.emptyNoActivityNetwork'
	};
	const LOOKED = { ...PRISTINE, balance_unknown: false, display_total_usd: 0 };

	it('all networks → "no activity"; filtered → "none on this network"', () => {
		const base = buildMobileState('h1', m, IDENT);
		const all = withLiveWallet(base, { balance: LOOKED, currency: USD, m, feed: EMPTY_FEED });
		expect(all.activitySection.mode).toBe('empty');
		expect(all.activitySection.empty?.title).toBe(m.activity.emptyTitle);
		const one = withLiveWallet(base, {
			balance: LOOKED,
			currency: USD,
			m,
			feed: FILTERED,
			chainFilter: 100
		});
		expect(one.activitySection.empty?.title).toBe(m.activity.emptyTitleNetwork);
		expect(one.activitySection.empty?.title).not.toBe(m.activity.emptyTitle);
	});

	it('the wide layout: a skeleton while loading, then the empty state with the narrow copy', () => {
		const base = buildDesktopState('d1', m, IDENT);
		const loading = withLiveWalletDesktop(base, { balance: PRISTINE, currency: USD, m });
		expect(loading.activitySection.mode).toBe('loading');
		const empty = withLiveWalletDesktop(base, {
			balance: LOOKED,
			currency: USD,
			m,
			feed: EMPTY_FEED
		});
		expect(empty.activitySection.mode).toBe('empty');
		expect(empty.activitySection.empty).toEqual({
			title: m.activity.emptyTitle,
			caption: m.activity.emptyCaption
		});
		expect(empty.assetsSection.mode).toBe('empty');
		expect(empty.assetsSection.empty).toEqual({
			title: m.assets.emptyTitle,
			caption: m.assets.emptyCaption
		});
	});

	it('the history says the core’s key: emptyTitle, or emptyFilter under a filter', () => {
		const fm = resolveWalletFlowMessages('en');
		const history = buildFlowState('a1', fm, IDENT);
		const live = (feed: FeedView) =>
			withLiveFlow(history, {
				balance: LOOKED,
				currency: USD,
				m,
				emptyCopy: undefined,
				feed,
				fm
			});
		const all = live(EMPTY_FEED);
		const filtered = live(FILTERED);
		if (all.base.kind !== 'history' || filtered.base.kind !== 'history') {
			throw new Error('expected the history');
		}
		expect(all.base.model.mode).toBe('empty');
		expect(all.base.model.emptyText).toBe(fm['history.emptyTitle']);
		expect(filtered.base.model.emptyText).toBe(fm['history.emptyFilter']);
	});
});
