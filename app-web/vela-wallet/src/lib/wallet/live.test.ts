/**
 * The live wallet-home builders (spec 025 T125): BalanceView + currency pair
 * → the drawn models. Presentation only — the numbers are the core's.
 */
import { describe, expect, it } from 'vitest';
import type { BalanceView } from '$lib/core/generated/BalanceView';
import type { CurrencyView } from '$lib/core/generated/CurrencyView';
import type { UnreachableNetwork } from '$lib/core/generated/UnreachableNetwork';
import { resolveWalletMessages } from '$lib/i18n/engine.server';
import { buildMobileState } from './fixtures';
import {
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
	holdings_loading: false,
	cached_total_usd: null,
	switcher: { open: false, loading: false, balances: [] }
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
