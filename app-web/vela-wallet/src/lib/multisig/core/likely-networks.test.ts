/**
 * r4D-4, r4a-14: the network to pick, the likely ones first — what a link
 * named, the networks used here, where funds are — and a first network for
 * a new multisig only when something says which: never Sepolia by accident.
 */
import { describe, expect, it, vi } from 'vitest';

vi.mock('./store.svelte', () => ({
	multisig: {
		accounts: [
			{ chainIds: [100, 1], addedMs: 1 },
			{ chainIds: [5042002], addedMs: 5 }
		]
	}
}));
vi.mock('$lib/services/networks', () => ({
	networkRank: (id: number) => [1, 100, 11155111, 5042002, 8453].indexOf(id)
}));

const { firstNetworkDefault, heldChains, lastUsedChains, likelyNetworks } =
	await import('./likely-networks');

const net = (chainId: number) => ({ chainId, displayName: String(chainId) });
const ALL = [1, 100, 11155111, 5042002, 8453].map(net);

describe('likely networks', () => {
	it('puts what the link named first, then the networks used here, then funds, then the wallet order', () => {
		const order = likelyNetworks(ALL, { named: [8453], used: [5042002], held: [100] }).map(
			(n) => n.chainId
		);
		expect(order).toEqual([8453, 5042002, 100, 1, 11155111]);
	});

	it('reads the multisigs here newest first, each by its first network', () => {
		expect(lastUsedChains()).toEqual([5042002, 100]);
	});

	it('reads where funds are, the most valuable first, spam and empties aside', () => {
		expect(
			heldChains([
				{ chain_id: 1, balance: '0', price_usd: 3000 },
				{ chain_id: 100, balance: '2', price_usd: 1 },
				{ chain_id: 8453, balance: '1', price_usd: 3000 },
				{ chain_id: 5042002, balance: '9', price_usd: null },
				{ chain_id: 11155111, balance: '5', price_usd: 1, spam: true }
			])
		).toEqual([8453, 100, 5042002]);
	});

	it('defaults a first network only when something says which', () => {
		expect(firstNetworkDefault(ALL, { used: [], held: [] })).toBeNull();
		expect(firstNetworkDefault(ALL, { used: [5042002], held: [100] })).toBe(5042002);
		expect(firstNetworkDefault(ALL, { used: [999], held: [100] })).toBe(100);
	});
});
