/**
 * r4a-17: an idle home picks up a native deposit on a network the person
 * added (Arc Testnet) within its normal 10 s tick — it was left out of the
 * watch for being past the space's first six networks, and the testnet
 * space's scan asked mainnets the token API fell back to (r4D-18).
 */
import { readFileSync } from 'node:fs';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { initSync } from '../../../../../rust/pkg-web/vela_core.js';
import { WASM_URL } from '../../../../../rust/pkg-web/vela_core_wasm_url.js';

const seen = vi.hoisted(() => ({
	watched: [] as number[][],
	scanned: [] as number[][],
	facts: [] as unknown[]
}));
vi.mock('./wallet-api', () => ({ fetchTokens: async () => [] }));
vi.mock('./native-watch', () => ({
	watchNativeBalances: async (_a: string, chains: number[]) => void seen.watched.push(chains)
}));
vi.mock('./incoming-transfers', () => ({
	fetchIncomingTransfers: async (_a: string, facts: unknown, chains: number[]) => {
		seen.facts.push(facts);
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

beforeAll(() => {
	initSync({ module: readFileSync(`../../assets/wasm${WASM_URL}`) });
});

beforeEach(() => {
	seen.watched = [];
	seen.scanned = [];
	seen.facts = [];
});

afterEach(() => vi.unstubAllGlobals());

describe('the networks an idle home watches (r4a-17)', () => {
	it('a network the person added is watched, and the testnet space scans its own', async () => {
		await syncReceivedTransfers('0x00000000000000000000000000000000000000aa');
		expect(seen.watched[0]).toContain(5042002);
		expect(seen.scanned[0]).toContain(5042002);
		// No mainnet asked in the testnet space.
		expect(seen.scanned[0].some((id) => [1, 56, 137, 42161, 8453, 100].includes(id))).toBe(false);
	});

	// r9-17: the networks are the core's, from facts — a multisig this
	// browser added hands the networks it lives on, so its first receipt on
	// Arc is looked for though it holds nothing there yet (the apps missed
	// "Received +0.6" for three rounds).
	it('a multisig holding nothing scans the network it lives on', async () => {
		const MULTISIG = '0x710311C8d71cc405c5b7782be1367eafa3d78937';
		vi.stubGlobal('localStorage', {
			getItem: (key: string) =>
				key === 'vela.multisigAccounts'
					? JSON.stringify([{ address: MULTISIG.toLowerCase(), chainIds: [5042002] }])
					: null
		});
		await syncReceivedTransfers(MULTISIG);
		expect(seen.scanned[0]).toEqual([5042002]);
		expect(seen.facts[0]).toEqual({
			held_chain_ids: [],
			account_networks: [{ chain_id: 5042002, deploy_block: null }],
			space_chain_ids: SPACE,
			added_chain_ids: [5042002]
		});
	});
});
