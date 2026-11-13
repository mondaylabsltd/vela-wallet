/**
 * An unstable network, end to end through the REAL `balance_dashboard` core
 * and the real builders (exploratory round, fix-balance):
 *
 * - home-1: the testnet space, Arc down, the page reloaded — the asset is
 *   kept (restored rows), the line names Arc, never "Deposit your first
 *   asset" or a bare "Updated now".
 * - ms-18 / home-14: an RPC answering zeros for a holding: not "$0.00 ·
 *   Live · listening" on one read.
 * - home-7: a switch of space never reports the other space's outage.
 *
 * The core is answered by hand, the way the executor answers it.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import { BalanceDashboardCore } from '$lib/core/client';
import type { BalanceView } from '$lib/core/generated/BalanceView';
import type { BalanceShellResult } from '$lib/core/generated/BalanceShellResult';
import type { BalanceToken } from '$lib/core/generated/BalanceToken';
import { resolveWalletMessages } from '$lib/i18n/engine.server';
import { buildMobileState } from '$lib/wallet/fixtures';
import { liveBalance, withLiveWallet } from '$lib/wallet/live';

const ADDRESS = '0x4afb65cbcccb9b6f64fd0962c5a55f86cae5bdc5';
const USD = { code: 'USD', rate: 1, committed: true, pending: null };
const m = resolveWalletMessages('en');
const NO_VALUE = 'Test assets · no real value';
const ARC = 5_042_002;
const SEPOLIA = 11_155_111;

type Effect = { id: number; operation: { type: string } };
type Out = { view: BalanceView; effects: Effect[] };
type Settled = Extract<BalanceShellResult, { type: 'fetch_settled' }>;

const USDC: BalanceToken = {
	chain_id: ARC,
	symbol: 'USDC',
	name: 'USDC',
	balance: '0.25',
	decimals: 18,
	token_address: null,
	price_usd: 1,
	spam: false
};
const XDAI: BalanceToken = { ...USDC, chain_id: 100, symbol: 'xDAI', balance: '10' };

/** The machine, driven: every cache answered `cached`, fetches answered by hand. */
function machine(cached: number | null) {
	const core = new BalanceDashboardCore();
	let fetches: number[] = [];
	let view: BalanceView;
	const take = (json: string) => {
		const out = JSON.parse(json) as Out;
		view = out.view;
		for (const effect of out.effects) {
			const type = effect.operation.type;
			if (type === 'fetch_tokens') {
				fetches.push(effect.id);
				continue;
			}
			const answer: BalanceShellResult | null =
				type === 'read_balance_cache'
					? { type: 'cached_total_loaded', address: ADDRESS, usd: cached }
					: type === 'write_balance_cache'
						? { type: 'balance_cache_written' }
						: null; // timers stay unanswered
			if (answer) take(core.resolve_effect(BigInt(effect.id), JSON.stringify(answer)));
		}
	};
	return {
		dispatch(event: object): BalanceView {
			take(core.dispatch(JSON.stringify(event)));
			return view;
		},
		/** Answer every fetch that is out with this round. */
		settle(round: Partial<Settled>): BalanceView {
			const out = fetches;
			fetches = [];
			for (const id of out) {
				const answer: Settled = {
					type: 'fetch_settled',
					address: ADDRESS,
					pull: false,
					tokens: [],
					failed_chain_ids: [],
					rate_limited_chain_ids: [],
					read_chain_ids: [],
					internal_chain_ids: [],
					registry_chain_ids: [],
					own_node_chain_ids: [],
					fallback_chain_ids: [],
					now_ms: 1_000,
					...round
				};
				take(core.resolve_effect(BigInt(id), JSON.stringify(answer)));
			}
			return view;
		}
	};
}

const home = buildMobileState('h1', m, (seed) => `<svg data-seed="${seed}"></svg>`);

describe('an unstable network, through the real core (fix-balance)', () => {
	it('home-1: Arc down at reload in the testnet space keeps the asset and names Arc', () => {
		const wallet = machine(null);
		wallet.dispatch({ type: 'account_changed', address: ADDRESS });
		const restored = wallet.dispatch({
			type: 'holdings_restored',
			address: ADDRESS,
			tokens: [USDC]
		});
		// The list opens on the last-known row, not a skeleton (home-3).
		expect(restored.tokens.map((t) => t.symbol)).toEqual(['USDC']);
		const view = wallet.settle({ failed_chain_ids: [ARC], read_chain_ids: [ARC, SEPOLIA] });
		const model = withLiveWallet(home, { balance: view, currency: USD, m, noValue: NO_VALUE });
		expect(model.assetsSection.mode).toBe('rows');
		expect(model.assetRows.map((r) => [r.ticker, r.stale])).toEqual([['USDC', true]]);
		expect(model.balance.state).toBe('no-value');
		expect(model.balance.status).toEqual({
			kind: 'warning',
			text: "Can't reach Arc Testnet right now",
			unread: true
		});
		expect(model.assetRows.length).toBe(1);
	});

	it('ms-18 / home-14: one read of zeros for a held 10 xDAI is not "$0.00 · Live"', () => {
		const wallet = machine(10);
		wallet.dispatch({ type: 'account_changed', address: ADDRESS });
		wallet.settle({ tokens: [XDAI], read_chain_ids: [100] });
		wallet.dispatch({ type: 'refresh_requested', force: true, pull: false });
		const doubted = wallet.settle({ tokens: [], read_chain_ids: [100] });
		const hero = liveBalance(doubted, USD, m);
		expect(hero.state).toBe('normal');
		expect(hero.integer).toBe('$10');
		expect(hero.liveText).toBeUndefined();
		const model = withLiveWallet(home, { balance: doubted, currency: USD, m });
		expect(model.assetsSection.mode).toBe('rows');
		expect(model.assetRows.length).toBe(1);
	});

	it('home-7: a switch of space forgets the other space’s outage at once', () => {
		const wallet = machine(null);
		wallet.dispatch({ type: 'account_changed', address: ADDRESS });
		const testnet = wallet.settle({ failed_chain_ids: [ARC], read_chain_ids: [ARC, SEPOLIA] });
		expect(testnet.unreachable_networks.map((n) => n.chain_id)).toEqual([ARC]);
		const mainnet = wallet.dispatch({ type: 'space_changed' });
		expect(mainnet.unreachable_networks).toEqual([]);
		expect(liveBalance(mainnet, USD, m).status).toBeUndefined();
		expect(liveBalance(mainnet, USD, m).checkingText).toBe('Checking…');
	});
});
