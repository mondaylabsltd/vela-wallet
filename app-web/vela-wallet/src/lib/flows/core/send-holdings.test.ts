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
	balance_partial: false,
	unreachable: false,
	notice: null,
	hidden: false,
	refreshing: false,
	last_refreshed_at_ms: 1,
	tokens: [ETH],
	unpriced_tokens: [],
	failed_chain_ids: [],
	rate_limited_chain_ids: [],
	banner_chain_ids: [],
	holdings_loading: false,
	cached_total_usd: null,
	switcher: { open: false, loading: false, balances: [] }
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
		view: () => ({ ...VIEW, ...rounds[at].view }),
		settled: () => rounds[at].settled,
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
