// The retry-once rule (desktop's, now all four shells'): `FetchTokens`
// answered from the asset list must not report a round that reached nothing
// — a proxy blip at launch — as "could not load tokens". It asks the balance
// machine for ONE forced re-read and answers from the NEXT round, whatever it
// holds; only a second round that reaches nothing is the failure.
import { describe, expect, it, vi } from 'vitest';
import type { BalanceToken } from '$lib/core/generated/BalanceToken';
import type { BalanceView } from '$lib/core/generated/BalanceView';
import type { SendToken } from '$lib/core/generated/SendToken';
import { answerHoldings, readHoldings, type HoldingsSource } from './send-holdings';

const ACCOUNT = '0x' + 'aa'.repeat(20);
const ETH: BalanceToken = {
	chain_id: 1,
	symbol: 'ETH',
	name: 'Ether',
	balance: '0.043968123456789012',
	decimals: 18,
	token_address: null,
	price_usd: 2700,
	spam: false
};
const VIEW: BalanceView = {
	address: ACCOUNT,
	display_total_usd: null,
	balance_unknown: false,
	unpriced_only: false,
	balance_partial: false,
	unreachable: false,
	notice: null,
	hidden: false,
	refreshing: false,
	last_refreshed_at_ms: 1,
	updated_at_ms: 1,
	networks_read: true,
	unread_chain_ids: [],
	tokens: [ETH],
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
	cached_total_usd: null,
	switcher: { open: false, loading: false, balances: [], hidden: false }
};
/** A launch-time blip: every chain failed, nothing was known before. */
const BLIP: Partial<BalanceView> = { tokens: [], failed_chain_ids: [1, 56, 8453] };

const toSend = (token: BalanceToken): SendToken => ({
	network: 'eth-mainnet',
	chain_id: token.chain_id,
	symbol: token.symbol,
	balance: token.balance,
	decimals: token.decimals,
	token_address: token.token_address,
	price_usd: token.price_usd,
	logo_urls: [],
	spam: token.spam
});

/**
 * A balance machine whose rounds are scripted: `rounds[0]` is what it holds
 * when asked, and each re-read lands the next one.
 */
function machine(rounds: { view: Partial<BalanceView>; settled: boolean }[]) {
	let at = 0;
	const reread = vi.fn(async () => {
		at = Math.min(at + 1, rounds.length - 1);
	});
	const source: HoldingsSource = {
		// "Settled": the core's word that the account's first read has ended.
		view: () => ({ ...VIEW, networks_read: rounds[at].settled, ...rounds[at].view }),
		reread,
		toSend
	};
	return { source, reread };
}

describe('readHoldings — what the balance machine can say', () => {
	it('a settled round with holdings is the answer', () => {
		const { source } = machine([{ view: {}, settled: true }]);
		expect(readHoldings(ACCOUNT, source)).toEqual({ kind: 'tokens', tokens: [toSend(ETH)] });
	});

	it('an empty wallet every chain answered for is an answer, not a failure', () => {
		const { source } = machine([{ view: { tokens: [] }, settled: true }]);
		expect(readHoldings(ACCOUNT, source)).toEqual({ kind: 'tokens', tokens: [] });
	});

	it('another account, or no round settled yet, walks the chains as before', () => {
		expect(
			readHoldings(
				ACCOUNT,
				machine([{ view: { address: '0x' + 'bb'.repeat(20) }, settled: true }]).source
			)
		).toEqual({ kind: 'walk' });
		expect(readHoldings(ACCOUNT, machine([{ view: {}, settled: false }]).source)).toEqual({
			kind: 'walk'
		});
	});

	it('a round that reached nothing — or nothing readable at all — is "nothing"', () => {
		expect(readHoldings(ACCOUNT, machine([{ view: BLIP, settled: true }]).source)).toEqual({
			kind: 'nothing'
		});
		expect(
			readHoldings(
				ACCOUNT,
				machine([{ view: { tokens: [], unreachable: true }, settled: false }]).source
			)
		).toEqual({ kind: 'nothing' });
	});
});

describe('answerHoldings — a round that reached nothing is read once more', () => {
	it('answers at once, with no re-read, when the round reached something', async () => {
		const { source, reread } = machine([{ view: {}, settled: true }]);
		expect(await answerHoldings(ACCOUNT, source)).toEqual({
			kind: 'tokens',
			tokens: [toSend(ETH)]
		});
		expect(reread).not.toHaveBeenCalled();
	});

	it('a blip round asks ONE forced re-read and answers from the next round', async () => {
		const { source, reread } = machine([
			{ view: BLIP, settled: true },
			{ view: {}, settled: true }
		]);
		expect(await answerHoldings(ACCOUNT, source)).toEqual({
			kind: 'tokens',
			tokens: [toSend(ETH)]
		});
		expect(reread).toHaveBeenCalledExactlyOnceWith(ACCOUNT);
	});

	it('a first fetch that read nothing at all is re-read too', async () => {
		const { source, reread } = machine([
			{ view: { tokens: [], unreachable: true }, settled: false },
			{ view: {}, settled: true }
		]);
		expect(await answerHoldings(ACCOUNT, source)).toEqual({
			kind: 'tokens',
			tokens: [toSend(ETH)]
		});
		expect(reread).toHaveBeenCalledOnce();
	});

	it('the next round answers WHATEVER it holds — an honestly empty wallet included', async () => {
		const { source } = machine([
			{ view: BLIP, settled: true },
			{ view: { tokens: [], failed_chain_ids: [] }, settled: true }
		]);
		expect(await answerHoldings(ACCOUNT, source)).toEqual({ kind: 'tokens', tokens: [] });
	});

	it('only a SECOND round that reaches nothing is the failure — and there is no third', async () => {
		const { source, reread } = machine([
			{ view: BLIP, settled: true },
			{ view: BLIP, settled: true },
			{ view: {}, settled: true }
		]);
		expect(await answerHoldings(ACCOUNT, source)).toEqual({ kind: 'unreadable' });
		expect(reread).toHaveBeenCalledOnce();
	});

	it('an account switch during the re-read walks for the account asked about', async () => {
		const { source } = machine([
			{ view: BLIP, settled: true },
			{ view: { address: '0x' + 'bb'.repeat(20) }, settled: false }
		]);
		expect(await answerHoldings(ACCOUNT, source)).toEqual({ kind: 'walk' });
	});
});

/**
 * r22-7: Arc refused. The page opens with Arc's rows restored from the last
 * visit — the home draws the USDC balance under "Can't reach Arc Testnet" —
 * and the round ends with Arc failed. Since r9/r10-1 the core does not move
 * the "Updated" stamp for a round in which every network the account lives
 * on failed, and the resident took that stamp as "a round has settled": the
 * picker walked the chains instead, read nothing on Arc, and said "No tokens
 * with balance" beside a home showing the balance. The core says when the
 * first read has ended (`networks_read`), whatever it reached: the picker
 * answers from the same rows the home draws — the last-known ones.
 */
describe('r22-7: a network that does not answer keeps its last-known rows in the picker', () => {
	const ARC = 5042002;
	const USDC: BalanceToken = {
		chain_id: ARC,
		symbol: 'USDC',
		name: 'USD Coin',
		balance: '25',
		decimals: 6,
		token_address: '0x3600000000000000000000000000000000000000',
		price_usd: 1,
		spam: false
	};
	type Out = { view: BalanceView; effects: { id: number; operation: { type: string } }[] };

	/** The real `balance_dashboard`: Arc's rows restored, then a round in which Arc refused. */
	async function arcRefused(): Promise<{ view: BalanceView; stamp: number | null }> {
		await import('$lib/i18n/wasm-init.server');
		const { BalanceDashboardCore } = await import('$lib/core/client');
		const core = new BalanceDashboardCore();
		let out = JSON.parse(
			core.dispatch(JSON.stringify({ type: 'account_changed', address: ACCOUNT }))
		) as Out;
		let fetchId: number | null = null;
		for (let round = 0; round < 8; round += 1) {
			const fetch = out.effects.find((effect) => effect.operation.type === 'fetch_tokens');
			if (fetch) fetchId = fetch.id;
			const next = out.effects.find((effect) => effect.operation.type !== 'fetch_tokens');
			if (!next) break;
			const answer =
				next.operation.type === 'read_balance_cache'
					? { type: 'cached_total_loaded', address: ACCOUNT, usd: 25 }
					: next.operation.type === 'read_balance_cache_many'
						? { type: 'cached_balances_loaded', balances: [] }
						: { type: 'balance_cache_written' };
			out = JSON.parse(core.resolve_effect(BigInt(next.id), JSON.stringify(answer))) as Out;
		}
		if (fetchId === null) throw new Error('the core asked for no fetch');
		core.dispatch(JSON.stringify({ type: 'lives_on', address: ACCOUNT, chain_ids: [ARC] }));
		const restored = JSON.parse(
			core.dispatch(JSON.stringify({ type: 'holdings_restored', address: ACCOUNT, tokens: [USDC] }))
		) as Out;
		const settled = JSON.parse(
			core.resolve_effect(
				BigInt(fetchId),
				JSON.stringify({
					type: 'fetch_settled',
					address: ACCOUNT,
					pull: false,
					tokens: [],
					failed_chain_ids: [ARC],
					rate_limited_chain_ids: [],
					read_chain_ids: [ARC],
					internal_chain_ids: [],
					registry_chain_ids: [],
					own_node_chain_ids: [],
					fallback_chain_ids: [],
					now_ms: 1_000
				})
			)
		) as Out;
		return { view: settled.view, stamp: restored.view.last_refreshed_at_ms };
	}

	it('the round ended with Arc refused: Send reads the rows the home shows, not a walk', async () => {
		const { view, stamp } = await arcRefused();
		// The home: the last-known USDC, and Arc named as not reachable.
		expect(view.tokens.map((t) => t.symbol)).toEqual(['USDC']);
		expect(view.unreachable_networks.map((n) => n.chain_id)).toEqual([ARC]);
		// The stamp did not move (r10-1) — and the first read has ended.
		expect(view.last_refreshed_at_ms).toBe(stamp);
		expect(view.networks_read).toBe(true);
		// The resident took "the stamp moved" for "a round settled": here it
		// did not, and the picker walked the chains. The core's word decides.
		expect(readHoldings(ACCOUNT, { view: () => view, toSend })).toEqual({
			kind: 'tokens',
			tokens: [toSend(USDC)]
		});
	});
});
