/**
 * Which balances a chain's read covers is the CORE's (spec 082 RE9, G24):
 * `balance_dashboard::read_plan`, which every client now reads. The iPhone
 * read only the native coin and custom tokens, so USDC on Base was missing
 * from its total; the web had its own list, including the spec 038 Celo rule
 * (CELO is an ERC-20 at the address the chain data calls its wrapped native,
 * so listing "WCELO" beside CELO counted one holding twice). Both rules are
 * the core's now, and these pin what the web's multicall asks for.
 */
import '$lib/i18n/wasm-init.server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Call3 } from './abi';

const { BASE, CELO, USDC_BASE, WETH_BASE, GOLD_TOKEN, CUSTOM, asked, wire } = vi.hoisted(() => ({
	BASE: 8453,
	CELO: 42220,
	USDC_BASE: '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913',
	WETH_BASE: '0x4200000000000000000000000000000000000006',
	GOLD_TOKEN: '0x471EcE3750Da237f93B8E339c536989b8978a438',
	CUSTOM: '0x' + 'cc'.repeat(20),
	/** Every multicall's calls, per chain, in the order they were asked. */
	asked: new Map<number, { target: string; allowFailure: boolean; callData: string }[]>(),
	/** Each encoded multicall, by the token `encAggregate3` handed back. */
	wire: new Map<string, { target: string; allowFailure: boolean; callData: string }[]>()
}));

vi.mock('./rpc-pool', () => ({
	// The multicall's data is the token below; the chain it was sent on is
	// what files its calls — reads of several chains run at once.
	poolRpcCall: async (_method: string, params: unknown[], chainId: number) => {
		const data = (params[0] as { data: string }).data;
		asked.set(chainId, wire.get(data) ?? []);
		return { result: data };
	},
	getFailedRpcChains: () => new Set<number>(),
	getRateLimitedChains: () => new Set<number>()
}));

vi.mock('./abi', async (importOriginal) => {
	const actual = await importOriginal<typeof import('./abi')>();
	return {
		...actual,
		encAggregate3: (calls: Call3[]) => {
			const token = '0x' + (wire.size + 1).toString(16).padStart(8, '0');
			wire.set(token, calls);
			return token;
		},
		decAggregate3: (hex: string) =>
			(wire.get(hex) ?? []).map(() => ({ success: true, data: '0x' + '0'.repeat(64) }))
	};
});

vi.mock('./chain-tokens', async (importOriginal) => {
	const actual = await importOriginal<typeof import('./chain-tokens')>();
	return {
		...actual,
		// The balance read asks for the document's outcome (PR 2 polish):
		// here it is always there.
		readChainTokens: async (chainId: number) => ({
			kind: 'doc' as const,
			data: {
				chainId,
				nativeCurrency: { name: 'Coin', symbol: chainId === CELO ? 'CELO' : 'ETH', decimals: 18 },
				stables: chainId === BASE ? [{ symbol: 'USDC', contract: USDC_BASE, type: 'native' }] : [],
				wrappedNativeToken: chainId === BASE ? WETH_BASE : chainId === CELO ? GOLD_TOKEN : null,
				dex: null
			}
		})
	};
});

vi.mock('./records', () => ({
	loadCustomTokens: async () => [
		{ chainId: BASE, contractAddress: CUSTOM, symbol: 'MINE', name: 'Mine', decimals: 9 },
		// The person also added USDC by hand: it is read once, as the registry's.
		{
			chainId: BASE,
			contractAddress: USDC_BASE.toLowerCase(),
			symbol: 'USDC',
			name: 'USD Coin',
			decimals: 6
		}
	]
}));

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
	const only = actual.getAllNetworksSync().filter((n) => n.chainId === BASE || n.chainId === CELO);
	return { ...actual, getAllNetworksSync: () => only };
});

import { clearTokenCache, fetchTokens } from './wallet-api';
import { encBalanceOf, encGetEthBalance, MULTICALL3 } from './abi';

const ADDRESS = '0x2C1c940000000000000000000000000000347c23';

beforeEach(() => {
	clearTokenCache();
	asked.clear();
	wire.clear();
	vi.spyOn(console, 'log').mockImplementation(() => {});
});

/** The balance reads of one chain's multicall, as `target:kind`. */
function balanceReads(chainId: number): string[] {
	return (asked.get(chainId) ?? []).flatMap((call) => {
		if (call.target === MULTICALL3 && call.callData === encGetEthBalance(ADDRESS)) {
			return ['native'];
		}
		if (call.callData === encBalanceOf(ADDRESS)) return [call.target.toLowerCase()];
		return [];
	});
}

describe('the balance read plan is the core’s (spec 082 RE9)', () => {
	it('Base reads USDC, in the order native → stables → wrapped → the person’s tokens', async () => {
		await fetchTokens(ADDRESS);
		expect(balanceReads(BASE)).toEqual([
			'native',
			USDC_BASE.toLowerCase(),
			WETH_BASE.toLowerCase(),
			CUSTOM
		]);
	});

	it('a contract is read once, however many lists name it', async () => {
		await fetchTokens(ADDRESS);
		const reads = balanceReads(BASE);
		expect(reads.filter((r) => r === USDC_BASE.toLowerCase())).toHaveLength(1);
	});

	it("Celo's GoldToken is the native coin, so it is not read a second time as a wrapped one", async () => {
		await fetchTokens(ADDRESS);
		expect(balanceReads(CELO)).toEqual(['native']);
	});
});
