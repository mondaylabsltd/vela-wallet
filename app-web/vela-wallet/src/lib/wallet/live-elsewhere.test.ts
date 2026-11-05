/**
 * Round 13 on the web home:
 *
 * - r13-1: a multisig used only on Arc Testnet with Mainnet in front read
 *   "$0.00 · Live · listening for payments", "No activity yet", "Deposit
 *   your first asset". The core says where it lives (`lives_elsewhere`); the
 *   home then claims nothing of the space in front — the other space's
 *   heading in the figure's place, no status line, no refresh, no empty
 *   state — as the native homes draw it.
 * - r13-7: while a new multisig's history is first read, Activity is being
 *   read, never "No activity yet"; a history that could not be read says so.
 */
import { describe, expect, it } from 'vitest';
import type { BalanceView } from '$lib/core/generated/BalanceView';
import type { CurrencyView } from '$lib/core/generated/CurrencyView';
import type { FeedView } from '$lib/core/generated/FeedView';
import { resolveWalletMessages } from '$lib/i18n/engine.server';
import { buildDesktopState, buildMobileState } from './fixtures';
import { withLiveWallet, withLiveWalletDesktop } from './live';

const m = resolveWalletMessages('en');
const IDENTICON = (seed: string) => `<svg data-seed="${seed}"></svg>`;
const USD: CurrencyView = { code: 'USD', rate: 1, committed: true, pending: null };
const MS = '0x777E00000000000000000000000000000000875a';

/** What the balance machine said of a multisig read on Mainnet: zero, Live. */
const LIVE_ZERO: BalanceView = {
	address: MS,
	display_total_usd: 0,
	balance_unknown: false,
	unpriced_only: false,
	balance_partial: false,
	unreachable: false,
	notice: null,
	hidden: false,
	refreshing: false,
	last_refreshed_at_ms: 1,
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
	live_key: 'home.liveIndicator',
	empty_key: 'assets.emptyTitle',
	holdings_loading: false,
	cached_total_usd: 0,
	switcher: { open: false, loading: false, balances: [], hidden: false }
};

const EMPTY_FEED = {
	rows: [],
	home_rows: [],
	hidden: false,
	home_empty_key: 'home.emptyNoActivity'
} as unknown as FeedView;

function home(extra: object) {
	return withLiveWallet(buildMobileState('h1s', m, IDENTICON), {
		balance: LIVE_ZERO,
		currency: USD,
		m,
		feed: EMPTY_FEED,
		...extra
	});
}

describe('r13-1: a multisig of the other space claims nothing of the space in front', () => {
	it('without the core’s word (the bug): a confident zero, Live, and both empty states', () => {
		const model = home({});
		expect(model.balance.state).not.toBe('no-value');
		expect(model.activitySection.mode).toBe('empty');
		expect(model.assetsSection.mode).toBe('empty');
	});

	it('with it: the heading in the figure’s place, no status, no empty state', () => {
		for (const model of [
			home({ elsewhere: { heading: 'Testnet space' } }),
			withLiveWalletDesktop(buildDesktopState('d1', m, IDENTICON), {
				balance: LIVE_ZERO,
				currency: USD,
				m,
				feed: EMPTY_FEED,
				elsewhere: { heading: 'Testnet space' }
			})
		]) {
			expect(model.balance.state).toBe('no-value');
			expect(model.balance.noValueText).toBe('Testnet space');
			expect(model.balance.status).toBeUndefined();
			expect(model.balance.liveText).toBeUndefined();
			expect(model.balance.refresh).toBeUndefined();
			expect(model.balance.integer).toBeUndefined();
			expect(model.activitySection.mode).toBe('rows');
			// 'rows' over nothing draws no empty state ("No activity yet",
			// "Deposit your first asset").
			expect(model.assetsSection.mode).toBe('rows');
		}
	});
});

describe('r13-7: a new multisig’s first history read', () => {
	it('being read: Activity is being read, never "No activity yet"', () => {
		const model = home({ multisigHistory: { stand: 'reading', unreadText: 'x' } });
		expect(model.activitySection.mode).toBe('loading');
	});

	it('read, nothing there: "No activity yet"', () => {
		const model = home({ multisigHistory: { stand: 'read', unreadText: 'x' } });
		expect(model.activitySection.mode).toBe('empty');
		expect(model.activitySection.empty?.title).toBe(m.activity.emptyTitle);
	});

	it('not read: said as that, never "No activity yet"', () => {
		const model = home({
			multisigHistory: { stand: 'unread', unreadText: 'Couldn’t read this multisig’s history' }
		});
		expect(model.activitySection.mode).toBe('empty');
		expect(model.activitySection.empty?.title).toBe('Couldn’t read this multisig’s history');
	});
});
