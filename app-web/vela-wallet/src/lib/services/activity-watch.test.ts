/**
 * r4a-17: an idle home picks up a native deposit on a network the person
 * added (Arc Testnet) within its normal 10 s tick — it was left out of the
 * watch for being past the space's first six networks, and the testnet
 * space's scan asked mainnets the token API fell back to (r4D-18).
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

const seen = vi.hoisted(() => ({ watched: [] as number[][], scanned: [] as number[][] }));
vi.mock('./wallet-api', () => ({ fetchTokens: async () => [] }));
vi.mock('./native-watch', () => ({
	watchNativeBalances: async (_a: string, chains: number[]) => void seen.watched.push(chains)
}));
vi.mock('./incoming-transfers', () => ({
	fetchIncomingTransfers: async (_a: string, chains: number[]) => {
		seen.scanned.push(chains);
		return [];
	}
}));
/** The testnet space: six built in, and Arc the person added. */
const SPACE = [11155111, 84532, 421614, 11155420, 10200, 80002, 5042002];
vi.mock('./networks', () => ({
	getCustomChainIdsSync: () => [5042002],
	inSpace: (id: number) => SPACE.includes(id),
	nativeSymbol: () => 'ETH',
	spaceNetworksSync: () => SPACE.map((chainId) => ({ chainId }))
}));
vi.mock('./records', () => ({ mergeTransactions: async () => 0 }));
vi.mock('./token-metadata', () => ({ resolveTokenMetadata: async () => new Map() }));
vi.mock('$lib/flows/marks', () => ({ tokenLogoURLs: () => [] }));

const { syncReceivedTransfers } = await import('./activity');

beforeEach(() => {
	seen.watched = [];
	seen.scanned = [];
});

describe('the networks an idle home watches (r4a-17)', () => {
	it('a network the person added is watched, and the testnet space scans its own', async () => {
		await syncReceivedTransfers('0x00000000000000000000000000000000000000aa');
		expect(seen.watched[0]).toContain(5042002);
		expect(seen.scanned[0]).toContain(5042002);
		// No mainnet asked in the testnet space.
		expect(seen.scanned[0].some((id) => [1, 56, 137, 42161, 8453, 100].includes(id))).toBe(false);
	});
});
