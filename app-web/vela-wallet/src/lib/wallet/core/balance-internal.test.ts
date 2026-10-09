/**
 * PR 2 note 11 (issue 483): with a fault inside the app, home said "Can't
 * reach Ethereum" — the balance machine's words for a network out of reach,
 * whatever had failed. The shell now tells the machine which failed reads
 * never left the app (`internal_chain_ids`, `FetchErrored.internal`), and home
 * draws the core's `internal_key` where the unreachable line goes.
 *
 * The REAL `balance_dashboard` core, answered by hand the way the executor
 * answers it; the drawn line through the real builder and the real corpus.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import { BalanceDashboardCore } from '$lib/core/client';
import type { BalanceView } from '$lib/core/generated/BalanceView';
import type { BalanceShellResult } from '$lib/core/generated/BalanceShellResult';
import { resolveWalletMessages } from '$lib/i18n/engine.server';
import { liveBalance } from '$lib/wallet/live';

const ADDRESS = '0x14fb1fb21751e29f7ec48dc450017552e3d1ea5c';
const USD = { code: 'USD', rate: 1, committed: true };
const m = resolveWalletMessages('en');

type Out = { view: BalanceView; effects: { id: number; operation: { type: string } }[] };

/** Boot the machine for an account and settle its first fetch with `settle`. */
function homeAfter(settle: (fetch: { id: number }) => BalanceShellResult): BalanceView {
	const core = new BalanceDashboardCore();
	let out = JSON.parse(
		core.dispatch(JSON.stringify({ type: 'account_changed', address: ADDRESS }))
	) as Out;
	let view = out.view;
	// Answer everything until the fetch is asked; the caches answer empty.
	for (let round = 0; round < 6; round += 1) {
		const fetch = out.effects.find((effect) => effect.operation.type === 'fetch_tokens');
		if (fetch) {
			out = JSON.parse(core.resolve_effect(BigInt(fetch.id), JSON.stringify(settle(fetch)))) as Out;
			view = out.view;
			break;
		}
		const next = out.effects[0];
		if (!next) break;
		const answer: BalanceShellResult =
			next.operation.type === 'read_balance_cache'
				? { type: 'cached_total_loaded', address: ADDRESS, usd: null }
				: next.operation.type === 'read_balance_cache_many'
					? { type: 'cached_balances_loaded', balances: [] }
					: { type: 'balance_cache_written' };
		out = JSON.parse(core.resolve_effect(BigInt(next.id), JSON.stringify(answer))) as Out;
		view = out.view;
	}
	core.free();
	return view;
}

const settled = (failed: number[], internal: number[]): BalanceShellResult => ({
	type: 'fetch_settled',
	address: ADDRESS,
	pull: false,
	tokens: [
		{
			chain_id: 100,
			symbol: 'XDAI',
			name: 'xDAI',
			balance: '10',
			decimals: 18,
			token_address: null,
			price_usd: 1,
			spam: false
		}
	],
	failed_chain_ids: failed,
	rate_limited_chain_ids: [],
	read_chain_ids: [1, 100],
	internal_chain_ids: internal,
	now_ms: 1_000
});

describe('home with a fault inside the app (PR 2 note 11)', () => {
	it('never reads "Can’t reach Ethereum" — it says what failed was Vela', () => {
		const view = homeAfter(() => settled([1], [1]));
		expect(view.internal_chain_ids).toEqual([1]);
		expect(view.internal_key).toBe('componentsUi.gas.reasonInternal');
		expect(view.unreachable_networks).toEqual([]);
		const status = liveBalance(view, USD, m).status;
		expect(status).toEqual({
			kind: 'warning',
			text: m.assets.internal['componentsUi.gas.reasonInternal']
		});
		expect(status?.text).not.toContain('Ethereum');
	});

	it('a chain that is really down is still named — the two are told apart', () => {
		const view = homeAfter(() => settled([1], []));
		expect(view.internal_key).toBeNull();
		expect(liveBalance(view, USD, m).status?.text).toBe("Can't reach Ethereum right now");
	});

	it('a whole fetch that threw inside the app, with nothing known, says so over the skeleton', () => {
		const view = homeAfter(() => ({
			type: 'fetch_errored',
			address: ADDRESS,
			pull: false,
			internal: true
		}));
		expect(view.internal_key).toBe('componentsUi.gas.reasonInternal');
		const model = liveBalance(view, USD, m);
		expect(model.status?.text).toBe(m.assets.internal['componentsUi.gas.reasonInternal']);
		expect(model.status?.text).not.toBe(m.balance.unreachable);
	});
});
