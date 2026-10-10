/**
 * PR 2 polish: a chain with no native coin of its own (Tempo) is read from
 * its registry document alone — the stablecoins it lists are what it holds.
 * When that document could not be read (offline, a timeout, 5xx — never a
 * 404, which is the server saying there is none), the balance read built no
 * calls and answered `[]`: "answered, holds nothing", and Tempo's dollars left
 * the total. It now fails like a chain that did not answer: its last holdings
 * carried, and it is in the round's failed chains.
 *
 * Only the wire (`poolRpcCall`), the registry document and the price seams
 * are mocked; the read plan is the core's and the orchestration is the real
 * code.
 */
import '$lib/i18n/wasm-init.server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Call3 } from './abi';
import type { ChainTokensRead } from './chain-tokens';

const { TEMPO, BASE, USD_TEMPO, wire, registry } = vi.hoisted(() => ({
	TEMPO: 4217,
	BASE: 8453,
	USD_TEMPO: '0x20c000000000000000000000b9537d11c60e8b50',
	/** Each encoded multicall, by the token `encAggregate3` handed back. */
	wire: new Map<string, { target: string; allowFailure: boolean; callData: string }[]>(),
	/** What each chain's registry document read comes to this round. */
	registry: new Map<number, 'doc' | 'absent' | 'unread'>()
}));

vi.mock('./rpc-pool', async (importOriginal) => ({
	// The pool's own error class stays the real one: it is what tells a read
	// that never left the app from one the network failed.
	...(await importOriginal<typeof import('./rpc-pool')>()),
	poolRpcCall: async (_method: string, params: unknown[]) => ({
		result: (params[0] as { data: string }).data
	}),
	getFailedRpcChains: () => new Set<number>(),
	getRateLimitedChains: () => new Set<number>()
}));

/** One raw unit of everything: a balance of 1 (6 decimals), and 6 as `decimals()`. */
vi.mock('./abi', async (importOriginal) => {
	const actual = await importOriginal<typeof import('./abi')>();
	const word = (n: bigint) => '0x' + n.toString(16).padStart(64, '0');
	return {
		...actual,
		encAggregate3: (calls: Call3[]) => {
			const token = '0x' + (wire.size + 1).toString(16).padStart(8, '0');
			wire.set(token, calls);
			return token;
		},
		decAggregate3: (hex: string) =>
			(wire.get(hex) ?? []).map((call) =>
				call.callData === actual.encDecimals()
					? { success: true, data: word(6n) }
					: { success: true, data: word(1_000_000n) }
			)
	};
});

vi.mock('./chain-tokens', async (importOriginal) => {
	const actual = await importOriginal<typeof import('./chain-tokens')>();
	return {
		...actual,
		readChainTokens: async (chainId: number): Promise<ChainTokensRead> => {
			const outcome = registry.get(chainId) ?? 'doc';
			if (outcome === 'absent') return { kind: 'absent' };
			if (outcome === 'unread') return { kind: 'unread', cause: new Error('HTTP 503') };
			return {
				kind: 'doc',
				data: {
					chainId,
					nativeCurrency: { name: 'Coin', symbol: chainId === TEMPO ? 'USD' : 'ETH', decimals: 6 },
					stables:
						chainId === TEMPO ? [{ symbol: 'USDC', type: 'native', contract: USD_TEMPO }] : [],
					wrappedNativeToken: null,
					dex: null
				}
			};
		}
	};
});

vi.mock('./records', () => ({ loadCustomTokens: async () => [] }));

vi.mock('./price-service', async (importOriginal) => {
	const actual = await importOriginal<typeof import('./price-service')>();
	return { ...actual, fetchChainlinkPrices: async () => ({}) };
});

vi.mock('./native-price', () => ({
	bestNativeDexPrice: () => null,
	chooseNativePrice: () => ({ price: null, source: 'none' }),
	peggedNativeUsd: () => null
}));

vi.mock('./networks', async (importOriginal) => {
	const actual = await importOriginal<typeof import('./networks')>();
	const only = actual.getAllNetworksSync().filter((n) => n.chainId === TEMPO || n.chainId === BASE);
	return { ...actual, getAllNetworksSync: () => only };
});

import { clearTokenCache, fetchTokens } from './wallet-api';
import { tokenChainId, type APIToken } from './tokens-model';

const ADDRESS = '0x2C1c940000000000000000000000000000347c23';

beforeEach(() => {
	clearTokenCache();
	wire.clear();
	registry.clear();
	vi.spyOn(console, 'log').mockImplementation(() => {});
});

const onTempo = (tokens: APIToken[]) => tokens.filter((t) => tokenChainId(t) === TEMPO);

/** One round, forced, with the chains it could not read. */
async function round(): Promise<{
	tokens: APIToken[];
	failed: number[];
	internal: number[];
	registry: number[];
}> {
	let failed: number[] = [];
	let internal: number[] = [];
	let registry: number[] = [];
	const tokens = await fetchTokens(ADDRESS, {
		forceRefresh: true,
		onFailedChains: (ids, inside, tokenList) => {
			failed = ids;
			internal = inside;
			registry = tokenList;
		}
	});
	return { tokens, failed, internal, registry };
}

describe('Tempo, with its registry document unread (PR 2 polish)', () => {
	it('is a chain that did not answer: its holdings carried, in the failed chains — never $0.00', async () => {
		// The document is there: Tempo's USDC is read.
		const first = await round();
		expect(onTempo(first.tokens).map((t) => t.symbol)).toEqual(['USDC']);
		expect(first.failed).toEqual([]);

		// The document's server is down: Tempo was not read.
		registry.set(TEMPO, 'unread');
		const second = await round();
		expect(second.failed).toContain(TEMPO);
		// The network's, not Vela's own fault.
		expect(second.internal).not.toContain(TEMPO);
		expect(onTempo(second.tokens).map((t) => t.symbol)).toEqual(['USDC']);
	});

	it('is not read inside the app — the failure is the network’s', async () => {
		registry.set(TEMPO, 'unread');
		const { failed, internal } = await round();
		expect(failed).toEqual([TEMPO]);
		expect(internal).toEqual([]);
	});

	it('a 404 (`absent`) is still an answer: no document, nothing read, nothing failed', async () => {
		registry.set(TEMPO, 'absent');
		const { tokens, failed } = await round();
		expect(failed).toEqual([]);
		expect(onTempo(tokens)).toEqual([]);
	});

	it('a chain with a coin of its own keeps the native-only read when its document is unread', async () => {
		registry.set(BASE, 'unread');
		const { tokens, failed, registry: tokenList } = await round();
		expect(failed).not.toContain(BASE);
		// It was read: nothing of it is a token-list failure.
		expect(tokenList).toEqual([]);
		expect(tokens.filter((t) => tokenChainId(t) === BASE).map((t) => t.symbol)).toEqual(['ETH']);
	});
});

/**
 * PR 3 note 4: Tempo with its registry document away read "Can't reach
 * Tempo", and its row offered an RPC fix — for a chain whose RPC was never
 * asked. The round now says which failed chains failed for THAT reason
 * (`registry_chain_ids`), so the core can word the line and withhold the fix.
 */
describe('the round says which chains were not read for want of their token list (PR 3 note 4)', () => {
	it('Tempo, document unread: failed AND in the token-list set', async () => {
		registry.set(TEMPO, 'unread');
		const { failed, internal, registry: tokenList } = await round();
		expect(failed).toEqual([TEMPO]);
		expect(tokenList).toEqual([TEMPO]);
		expect(internal).toEqual([]);
	});

	it('a round with nothing failed names none', async () => {
		const { failed, registry: tokenList } = await round();
		expect(failed).toEqual([]);
		expect(tokenList).toEqual([]);
	});

	it('a fetch that joins the round in flight is told the same set', async () => {
		registry.set(TEMPO, 'unread');
		const first = round();
		let joined: number[] = [];
		const second = fetchTokens(ADDRESS, {
			onFailedChains: (_ids, _inside, tokenList) => (joined = tokenList)
		});
		await Promise.all([first, second]);
		expect(joined).toEqual([TEMPO]);
	});
});
