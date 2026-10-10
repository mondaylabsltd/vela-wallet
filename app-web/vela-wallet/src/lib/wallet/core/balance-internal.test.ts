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
import { resolveSettingsMessages, resolveWalletMessages } from '$lib/i18n/engine.server';
import { liveBalance, withLiveWallet } from '$lib/wallet/live';
import {
	liveBalanceDetail,
	liveUnreachable,
	pickRescueMessages,
	switcherBalances
} from '$lib/settings/live';
import { buildMobileState } from '$lib/wallet/fixtures';

const ADDRESS = '0x14fb1fb21751e29f7ec48dc450017552e3d1ea5c';
const USD = { code: 'USD', rate: 1, committed: true, pending: null };
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

const settled = (
	failed: number[],
	internal: number[],
	registry: number[] = []
): BalanceShellResult => ({
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
	registry_chain_ids: registry,
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

	it('every chain failing inside the app, nothing known: no settled $0.00 — the skeleton and the reason', () => {
		// The integration's follow-up: every chain asked failed and nothing is
		// cached — "nothing known", as a fetch that threw, never "$0.00" and
		// "Deposit your first asset" under Vela's own error.
		const view = homeAfter(() => ({ ...settled([1, 100], [1, 100]), tokens: [] }));
		expect(view.unreachable).toBe(true);
		// PR 2 polish: the core gives no figure for a round that read nothing.
		expect(view.display_total_usd).toBeNull();
		expect(view.internal_key).toBe('componentsUi.gas.reasonInternal');
		const model = liveBalance(view, USD, m);
		expect(model.state).toBe('loading');
		expect(model.integer).toBeUndefined();
		expect(model.status?.text).toBe(m.assets.internal['componentsUi.gas.reasonInternal']);
		// Never "Deposit your first asset" under it: a look that reached
		// nothing is no look.
		const home = withLiveWallet(
			buildMobileState('h1', m, () => ''),
			{
				balance: view,
				currency: USD,
				m
			} as Parameters<typeof withLiveWallet>[1]
		);
		expect(home.assetsSection.mode).toBe('loading');
		// …and the same with every chain simply down: the network's sentence.
		const down = homeAfter(() => ({ ...settled([1, 100], []), tokens: [] }));
		expect(down.unreachable).toBe(true);
		expect(down.display_total_usd).toBeNull();
		expect(down.internal_key).toBeNull();
		expect(liveBalance(down, USD, m)).toMatchObject({
			state: 'loading',
			status: { text: m.balance.unreachable }
		});
	});

	it('a whole fetch that threw inside the app, with nothing known, says so over the skeleton', () => {
		const view = homeAfter(() => ({
			type: 'fetch_errored',
			address: ADDRESS,
			pull: false,
			internal: true
		}));
		expect(view.internal_key).toBe('componentsUi.gas.reasonInternal');
		expect(view.unreachable).toBe(true);
		expect(view.display_total_usd).toBeNull();
		const model = liveBalance(view, USD, m);
		expect(model.status?.text).toBe(m.assets.internal['componentsUi.gas.reasonInternal']);
		expect(model.status?.text).not.toBe(m.balance.unreachable);
		expect(model.state).toBe('loading');
		expect(model.integer).toBeUndefined();
	});
});

/**
 * PR 2 polish: a balance that read nothing has no figure
 * (`display_total_usd` is null while `unreachable`). The account switcher
 * stood the live total in for the active row, and a 0 there read as "$0.00"
 * on an account nobody had read.
 */
describe('the switcher over a home that read nothing (PR 2 polish)', () => {
	it('the active row is not given a figure nobody read', () => {
		const view = homeAfter(() => ({ ...settled([1, 100], []), tokens: [] }));
		expect(view.unreachable).toBe(true);
		const balances = switcherBalances(view, ADDRESS);
		expect(balances.has(ADDRESS.toLowerCase())).toBe(false);
	});

	it('a read that answered still stands in for the active row', () => {
		const view = homeAfter(() => settled([], []));
		expect(view.display_total_usd).toBe(10);
		expect(switcherBalances(view, ADDRESS).get(ADDRESS.toLowerCase())).toBe(10);
	});
});

/**
 * PR 3 note 4: Tempo's token list (its registry document) away read "Can't
 * reach Tempo" on home, and the list under it offered "Fix" — the RPC editor —
 * for a network whose RPC nobody had asked. The executor now says which failed
 * chains failed for that reason (`registry_chain_ids`); the REAL core words
 * the line and decides the fix, and the builders draw both.
 */
describe('a token list that cannot be loaded is not a network out of reach (PR 3 note 4)', () => {
	const TEMPO = 4217;
	const rescue = pickRescueMessages(resolveSettingsMessages('en'));

	it('home says the token list, by name — never "Can’t reach Tempo"', () => {
		const view = homeAfter(() => ({
			...settled([TEMPO], [], [TEMPO]),
			read_chain_ids: [100, TEMPO]
		}));
		expect(view.unreachable_key).toBe('assets.tokenListUnreachable');
		expect(view.unreachable_networks).toMatchObject([
			{ chain_id: TEMPO, cause: 'token_list', rpc_fixable: false }
		]);
		const status = liveBalance(view, USD, m).status;
		expect(status).toEqual({ kind: 'warning', text: "Can't load Tempo's token list right now" });
		expect(status?.text).not.toMatch(/reach/i);
	});

	it('its row in the list offers no "Fix": there is no endpoint to repair', () => {
		const view = homeAfter(() => ({
			...settled([TEMPO], [], [TEMPO]),
			read_chain_ids: [100, TEMPO]
		}));
		const panel = liveUnreachable(view, USD, rescue);
		expect(panel.title).toBe("Can't load Tempo's token list right now");
		expect(panel.rows).toHaveLength(1);
		expect(panel.rows[0]).toMatchObject({ name: 'Tempo', chainId: TEMPO });
		expect(panel.rows[0].action).toBeUndefined();
		expect('action' in panel.rows[0]).toBe(false);
	});

	it('a network that did not answer keeps its "Fix", beside one that has none', () => {
		const view = homeAfter(() => ({
			...settled([1, TEMPO], [], [TEMPO]),
			read_chain_ids: [1, 100, TEMPO]
		}));
		// Several: the counted line, unchanged.
		expect(view.unreachable_key).toBe('assets.unreachableMany');
		const panel = liveUnreachable(view, USD, rescue);
		expect(panel.title).toBe("Can't reach 2 networks right now");
		const byName = Object.fromEntries(panel.rows.map((row) => [row.name, row.action]));
		expect(byName).toEqual({ Ethereum: rescue.rescue.rpcFix, Tempo: undefined });
	});

	it('the breakdown never says "RPC unavailable" of it', () => {
		const view = homeAfter(() => ({
			...settled([1, TEMPO], [], [TEMPO]),
			read_chain_ids: [1, 100, TEMPO]
		}));
		const detail = liveBalanceDetail(view, USD, rescue, m.balance.unpriced);
		const status = Object.fromEntries(detail.pending.map((row) => [row.name, row.status]));
		expect(status).toEqual({
			Ethereum: rescue.balanceDetail.statusFailed,
			Tempo: "Can't load Tempo's token list right now"
		});
	});

	it('a chain named in the set that did not fail is not in the list at all', () => {
		// Only a chain that failed can have failed for this reason (the core's rule).
		const view = homeAfter(() => ({ ...settled([], [], [TEMPO]), read_chain_ids: [100, TEMPO] }));
		expect(view.unreachable_networks).toEqual([]);
		expect(view.unreachable_key).toBeNull();
	});
});
