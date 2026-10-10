/**
 * Issue 469: the wallet home's Activity is the newest three, and History is
 * every row.
 *
 * The home used to draw the whole feed, so a wallet with a week of history
 * pushed its Assets off the first screen. The cut is the CORE's
 * (`FeedView.home_rows`, `HOME_ACTIVITY_ITEMS` = 3) — these records go through
 * the real `activity_feed` machine (wasm), so what is asserted is the web's
 * wiring of the core's own lists: the home reads `home_rows`, History reads
 * `rows`, and nothing in the shell counts to three.
 */
import { describe, expect, it } from 'vitest';
import type { BalanceView } from '$lib/core/generated/BalanceView';
import type { CurrencyView } from '$lib/core/generated/CurrencyView';
import type { FeedView } from '$lib/core/generated/FeedView';
import { buildFlowState } from '$lib/flows/fixtures';
import { withLiveFlow } from '$lib/flows/live';
import { resolveWalletFlowMessages, resolveWalletMessages } from '$lib/i18n/engine.server';
import type { LocalTransaction } from '$lib/services/transactions-model';
import { feedViewThroughCore } from './core/feed-through-core';
import { buildDesktopState, buildMobileState } from './fixtures';
import { narrowedFeed, withLiveWallet, withLiveWalletDesktop } from './live';
import { findFeedItem } from './live-detail';

const m = resolveWalletMessages('en');
const fm = resolveWalletFlowMessages('en');
const IDENTICON = (seed: string) => `<svg data-seed="${seed}"></svg>`;
const USD: CurrencyView = { code: 'USD', rate: 1, committed: true, pending: null };

const ME = '0xD400866e00B055B20752a826CD5C89b811de130b';
const BOB = '0x' + 'b0'.repeat(20);
const DAY_S = 86_400;
/** Noon, so "today" and "yesterday" do not depend on when the suite runs. */
const NOW_S = Math.floor(new Date(new Date().setHours(12, 0, 0, 0)).getTime() / 1000);

function received(id: string, at: number, chainId = 1): LocalTransaction {
	return {
		id,
		userOpHash: '',
		txHash: '0x' + id.padStart(64, '0'),
		from: BOB,
		to: ME,
		value: '1',
		symbol: 'ETH',
		decimals: 18,
		chainId,
		timestamp: at,
		status: 'confirmed',
		type: 'receive'
	};
}

/** Six receipts over three days, newest first: two today, two yesterday, two before. */
const RECORDS: LocalTransaction[] = [
	received('1', NOW_S - 60),
	received('2', NOW_S - 120, 8453),
	received('3', NOW_S - DAY_S),
	received('4', NOW_S - DAY_S - 60, 8453),
	received('5', NOW_S - 2 * DAY_S, 8453),
	received('6', NOW_S - 2 * DAY_S - 60, 8453)
];

const BALANCE: BalanceView = {
	address: ME,
	display_total_usd: 0,
	balance_unknown: false,
	balance_partial: false,
	unreachable: false,
	notice: null,
	hidden: false,
	refreshing: false,
	last_refreshed_at_ms: 0,
	tokens: [],
	unpriced_tokens: [],
	failed_chain_ids: [],
	rate_limited_chain_ids: [],
	unreachable_networks: [],
	unreachable_key: null,
	internal_chain_ids: [],
	internal_key: null,
	holdings_loading: false,
	cached_total_usd: 0,
	switcher: { open: false, loading: false, balances: [], hidden: false }
};

const ids = (groups: { rows: { id?: string }[] }[]) =>
	groups.flatMap((g) => g.rows.map((r) => r.id));

function home(feed: FeedView, chainFilter?: number) {
	return withLiveWallet(buildMobileState('h1s', m, IDENTICON), {
		balance: BALANCE,
		currency: USD,
		m,
		feed,
		chainFilter
	});
}

function wide(feed: FeedView, chainFilter?: number) {
	return withLiveWalletDesktop(buildDesktopState('d1', m, IDENTICON), {
		balance: BALANCE,
		currency: USD,
		m,
		feed,
		chainFilter
	});
}

function history(feed: FeedView, chainFilter?: number) {
	const live = withLiveFlow(buildFlowState('a1', fm, IDENTICON), {
		balance: BALANCE,
		currency: USD,
		m,
		emptyCopy: undefined,
		feed,
		fm,
		chainFilter
	});
	if (live.base.kind !== 'history') throw new Error('a1 is the history');
	return live.base.model;
}

describe('the home’s Activity is the newest three; History is every row (issue 469)', () => {
	it('the core cuts the home’s list, and both layouts draw exactly it', async () => {
		const feed = await feedViewThroughCore(RECORDS, ME, NOW_S * 1000);
		// The core's own lists: six in `rows`, the newest three in `home_rows`.
		expect(feed.rows.filter((r) => r.type === 'item')).toHaveLength(6);
		expect(feed.home_rows.filter((r) => r.type === 'item')).toHaveLength(3);

		const phone = home(feed);
		const desktop = wide(feed);
		expect(ids(phone.activityGroups)).toEqual(['1', '2', '3']);
		expect(ids(desktop.activityGroups)).toEqual(['1', '2', '3']);
		// Two day headers — today's two and yesterday's one — and never a
		// header with nothing under it.
		expect(phone.activityGroups.map((g) => [g.label, g.rows.length])).toEqual([
			[m.activity.today, 2],
			[m.activity.yesterday, 1]
		]);
		expect(phone.activitySection.mode).toBe('rows');
		// "All" stays: it is the way to the other three.
		expect(phone.activitySection.action).toBe(m.sections.all);
		expect(desktop.activitySection.action).toBe(m.sections.all);
	});

	it('History draws every row, and each home row is one of them', async () => {
		const feed = await feedViewThroughCore(RECORDS, ME, NOW_S * 1000);
		const all = history(feed);
		expect(ids(all.groups)).toEqual(['1', '2', '3', '4', '5', '6']);
		expect(all.groups).toHaveLength(3);
		// A tap on a home row names its id, and the detail is looked up in
		// `rows` — so the row on the home opens the transaction History opens.
		for (const id of ids(home(feed).activityGroups)) {
			expect(findFeedItem(feed, id ?? null)?.id).toBe(id);
		}
	});

	it('under a network filter the core cuts again: the newest three ON that network', async () => {
		const feed = await feedViewThroughCore(RECORDS, ME, NOW_S * 1000, [
			{ type: 'chain_filter_changed', chain_id: 8453 }
		]);
		expect(ids(home(feed, 8453).activityGroups)).toEqual(['2', '4', '5']);
		expect(ids(wide(feed, 8453).activityGroups)).toEqual(['2', '4', '5']);
		expect(ids(history(feed, 8453).groups)).toEqual(['2', '4', '5', '6']);
	});

	it('the shell’s own narrowing only drops other networks’ rows from both lists — it never counts', async () => {
		// The frame before the core has heard the filter: the lists are still
		// the unfiltered ones, and the shell narrows what it holds.
		const feed = await feedViewThroughCore(RECORDS, ME, NOW_S * 1000);
		const narrowed = narrowedFeed(feed, 8453);
		const items = (rows: FeedView['rows']) =>
			rows.flatMap((r) => (r.type === 'item' ? [r.item.id] : []));
		expect(items(narrowed.rows)).toEqual(['2', '4', '5', '6']);
		// Of the home's three (1, 2, 3) only 2 is on Base, and yesterday's
		// header — left with nothing under it — goes too.
		expect(items(narrowed.home_rows)).toEqual(['2']);
		expect(narrowed.home_rows.filter((r) => r.type === 'header')).toHaveLength(1);
		expect(ids(home(feed, 8453).activityGroups)).toEqual(['2']);
		// No filter is no work: the very same view.
		expect(narrowedFeed(feed, null)).toBe(feed);
	});

	it('a feed of three or fewer is drawn whole on the home', async () => {
		const feed = await feedViewThroughCore(RECORDS.slice(0, 2), ME, NOW_S * 1000);
		expect(ids(home(feed).activityGroups)).toEqual(['1', '2']);
		expect(ids(history(feed).groups)).toEqual(['1', '2']);
	});
});
