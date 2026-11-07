/**
 * Round 16 leftover 5: Activity says which networks the incoming scan could
 * not read — "Activity can't be read on {network}" — the core's line
 * (`FeedView.unread_key` over `unread_chains`), on the narrow home and the
 * wide one: over the rows when there are rows, in the empty state's place
 * when there are none; never "No activity yet" and never a skeleton for ever
 * while a network is unread.
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
import { withLiveWallet, withLiveWalletDesktop } from './live';

const m = resolveWalletMessages('en');
const fm = resolveWalletFlowMessages('en');
const IDENTICON = (seed: string) => `<svg data-seed="${seed}"></svg>`;
const USD: CurrencyView = { code: 'USD', rate: 1, committed: true, pending: null };
const ME = '0xD400866e00B055B20752a826CD5C89b811de130b';
const ARC = 5042002;

const BALANCE: BalanceView = {
	address: ME,
	display_total_usd: 0,
	balance_unknown: false,
	unpriced_only: false,
	balance_partial: false,
	unreachable: false,
	notice: null,
	hidden: false,
	refreshing: false,
	last_refreshed_at_ms: 0,
	updated_at_ms: 0,
	networks_read: true,
	unread_chain_ids: [],
	tokens: [],
	unpriced_tokens: [],
	failed_chain_ids: [],
	rate_limited_chain_ids: [],
	unreachable_networks: [],
	fallback_chain_ids: [],
	fallback_status_key: '',
	unreachable_key: null,
	internal_chain_ids: [],
	internal_key: null,
	checking_key: null,
	live_key: null,
	empty_key: null,
	holdings_loading: false,
	cached_total_usd: 0,
	switcher: { open: false, loading: false, balances: [], hidden: false }
};

const EMPTY: FeedView = {
	rows: [],
	home_rows: [],
	transactions: [],
	new_item_id: null,
	toast: null,
	hidden: false,
	history_empty_key: 'history.emptyTitle',
	home_empty_key: 'home.emptyNoActivity',
	contact_rows: [],
	account: ME.toLowerCase(),
	settled: false,
	unread_chains: [ARC],
	unread_key: 'home.activityUnreadOne',
	unread_network: null
};

function home(feed: FeedView) {
	return withLiveWallet(buildMobileState('h1s', m, IDENTICON), {
		balance: BALANCE,
		currency: USD,
		m,
		feed
	});
}

function wide(feed: FeedView) {
	return withLiveWalletDesktop(buildDesktopState('d1', m, IDENTICON), {
		balance: BALANCE,
		currency: USD,
		m,
		feed
	});
}

function history(feed: FeedView) {
	const live = withLiveFlow(buildFlowState('a1', fm, IDENTICON), {
		balance: BALANCE,
		currency: USD,
		m,
		emptyCopy: undefined,
		feed,
		fm
	});
	if (live.base.kind !== 'history') throw new Error('a1 is the history');
	return live.base.model;
}

describe('Activity names the networks the scan could not read (r16 leftover 5)', () => {
	it('History says it too: in the empty line’s place, never "no transactions yet"', () => {
		const model = history(EMPTY);
		expect(model.mode).toBe('empty');
		expect(model.emptyText).toBe("Activity can't be read on Arc Testnet right now.");
		expect(model.note).toBeUndefined();
	});

	it('with nothing to show: the line, in the empty state’s place — never "No activity yet"', () => {
		for (const model of [home(EMPTY), wide(EMPTY)]) {
			expect(model.activitySection.mode).toBe('empty');
			expect(model.activitySection.empty?.title).toBe(
				"Activity can't be read on Arc Testnet right now."
			);
			expect(model.activitySection.empty?.title).not.toBe(m.activity.emptyTitle);
		}
	});

	it('several networks are counted', () => {
		const feed = { ...EMPTY, unread_chains: [1, ARC], unread_key: 'home.activityUnreadMany' };
		expect(home(feed).activitySection.empty?.title).toBe(
			"Activity can't be read on 2 networks right now."
		);
	});

	it('over rows: the rows, and the line above them', async () => {
		const record: LocalTransaction = {
			id: 'r1',
			userOpHash: '',
			txHash: '0x' + '1'.padStart(64, '0'),
			from: '0x' + 'b0'.repeat(20),
			to: ME,
			value: '1',
			symbol: 'ETH',
			decimals: 18,
			chainId: 1,
			timestamp: Math.floor(Date.now() / 1000) - 60,
			status: 'confirmed',
			type: 'receive'
		};
		const read = await feedViewThroughCore([record], ME, Date.now());
		const feed = { ...read, unread_chains: [ARC], unread_key: 'home.activityUnreadOne' };
		for (const model of [home(feed), wide(feed)]) {
			expect(model.activitySection.mode).toBe('rows');
			expect(model.activitySection.note).toBe("Activity can't be read on Arc Testnet right now.");
		}
	});

	it('every network read: nothing said, and the empty claim is the core’s', () => {
		const feed = { ...EMPTY, settled: true, unread_chains: [], unread_key: null };
		const model = home(feed);
		expect(model.activitySection.note).toBeUndefined();
		expect(model.activitySection.mode).toBe('empty');
		expect(model.activitySection.empty?.title).toBe(m.activity.emptyTitle);
	});
});

describe('a network is said by its name, never its chain id (r17-4)', () => {
	it('the core names it: that name', () => {
		const feed = { ...EMPTY, unread_chains: [ARC], unread_network: 'Arc Testnet' };
		expect(home(feed).activitySection.empty?.title).toBe(
			"Activity can't be read on Arc Testnet right now."
		);
	});

	it('a network nobody here can name is counted, never said by its number', () => {
		const feed = { ...EMPTY, unread_chains: [777777], unread_network: null };
		const title = home(feed).activitySection.empty?.title ?? '';
		expect(title).not.toContain('777777');
		expect(title).toBe("Activity can't be read on 1 networks right now.");
	});
});
