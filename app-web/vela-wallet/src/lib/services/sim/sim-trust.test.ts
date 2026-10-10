/**
 * PR 3 — an unverified token is a direction, never a figure.
 *
 * Android's signing sheet printed "Unverified token
 * +5,000,000,000,000,000,000,000.00" for a balance change: the simulation's
 * raw delta, which is whatever the site being signed for chose to emit from a
 * contract it controls. The core's judgment no longer carries that number
 * (`TrustSimJudgment::Erc20Unverified { token, direction }`), and neither
 * does the change this shell makes of it — nor the blob a record would store.
 *
 * The judgments are the REAL `token_trust` core's, through the web's own
 * resident session and executor; only the network is stood in for.
 */
import { beforeAll, describe, expect, it, vi } from 'vitest';

const kv = new Map<string, string>();
vi.mock('$lib/services/storage', () => ({
	getItem: vi.fn(async (key: string) => kv.get(key) ?? null),
	setItem: vi.fn(async (key: string, value: string) => void kv.set(key, value)),
	removeItem: vi.fn(async (key: string) => void kv.delete(key))
}));
// The chain's registry names no token of ours, and the account holds none:
// whatever arrives is a stranger's.
vi.mock('$lib/services/chain-tokens', () => ({
	fetchChainTokens: async () => ({ stables: [], wrappedNativeToken: null })
}));
vi.mock('$lib/services/wallet-api', () => ({
	getCachedHeldTokens: () => [],
	clearTokenCache: () => {}
}));
/** Tokens whose `symbol()` answers — as any contract's can, with any word. */
const named = vi.hoisted(() => new Map<string, { symbol: string; decimals: number }>());
vi.mock('$lib/services/token-metadata', () => ({
	resolveTokenMetadata: async (_chainId: number, addrs: string[]) =>
		new Map(
			addrs.flatMap((addr) => {
				const meta = named.get(addr.toLowerCase());
				return meta ? [[addr.toLowerCase(), meta] as const] : [];
			})
		)
}));
// The resident's own exports, real — and watchable, for the one judgment a
// simulation of whole numbers cannot produce.
vi.mock('$lib/wallet/core/token-trust-resident', { spy: true });

import { judgeSimDeltas } from '$lib/wallet/core/token-trust-resident';
import { enrichDeltas } from './sim-trust';
import { deserializeAssetSim, serializeAssetSim, type AssetSimResult } from './tx-simulation';

const ME = '0x14fb1f4e2b9c7a5d8e3f6a1b4c7d9e2f5a8b1d1e';
const LURE_TOKEN = '0x' + 'ba'.repeat(20);
const UNNAMED = '0x' + 'cd'.repeat(20);
const IDLE = '0x' + 'ef'.repeat(20);
/** The figure the site chose: 5,000 of an 18-decimal token. */
const LURE = 5_000n * 10n ** 18n;
const OUTFLOW = 7_000n * 10n ** 18n;

beforeAll(async () => {
	await import('$lib/i18n/wasm-init.server');
	// It answers "USDC", 18 decimals. Anybody's contract can.
	named.set(LURE_TOKEN, { symbol: 'USDC', decimals: 18 });
});

describe('a judged simulation holds no figure for an unverified token (PR 3)', () => {
	it('an inflow of 5000000000000000000000 is "in" and nothing else; an outflow nobody could name is "out"', async () => {
		const changes = await enrichDeltas(
			[
				{ kind: 'native', delta: -30_000_000_000_000_000n },
				{ kind: 'erc20', token: LURE_TOKEN, delta: LURE },
				{ kind: 'erc20', token: UNNAMED, delta: -OUTFLOW }
			],
			100,
			ME
		);
		// The whole of each change: a direction, the caution — and no `delta`.
		expect(changes).toEqual([
			{ kind: 'native', delta: -30_000_000_000_000_000n, symbol: 'xDAI', decimals: 18 },
			{ kind: 'erc20', token: LURE_TOKEN, direction: 'in', unverified: true },
			{ kind: 'erc20', token: UNNAMED, direction: 'out', unverified: true }
		]);
		expect(changes[1]).not.toHaveProperty('delta');
		expect(changes[2]).not.toHaveProperty('delta');
		// Nor the name the contract gave itself: a stranger's "USDC" is not one.
		expect(changes[1]).not.toHaveProperty('symbol');

		// What a record would keep of it: no digit run of either figure.
		const blob = JSON.stringify(serializeAssetSim({ ok: true, changes, engine: 'rpc' }));
		expect(blob).not.toContain('5000');
		expect(blob).not.toContain('7000');
		// The native coin's own figure is still there, to the unit.
		expect(blob).toContain('"-30000000000000000"');
	}, 30_000);

	it('an unverified token that did not move is no row at all', async () => {
		const changes = await enrichDeltas(
			[
				{ kind: 'erc20', token: IDLE, delta: 0n },
				{ kind: 'erc20', token: LURE_TOKEN, delta: LURE }
			],
			100,
			ME
		);
		expect(changes).toEqual([
			{ kind: 'erc20', token: LURE_TOKEN, direction: 'in', unverified: true }
		]);
	}, 30_000);

	it('a figure the core could not read is a row with no sign and no figure', async () => {
		vi.mocked(judgeSimDeltas).mockResolvedValueOnce([
			{ type: 'erc20_unverified', token: LURE_TOKEN, direction: 'unreadable' },
			{ type: 'erc20_unverified', token: null, direction: 'still' }
		]);
		const changes = await enrichDeltas(
			[{ kind: 'erc20', token: LURE_TOKEN, delta: LURE }],
			100,
			ME
		);
		expect(changes).toEqual([
			{ kind: 'erc20', token: LURE_TOKEN, direction: 'unreadable', unverified: true }
		]);
	});
});

describe('the stored blob reads both shapes of an unverified change (PR 3)', () => {
	it('a blob stored before keeps its line as a direction — the figure does not come back', () => {
		const older = {
			ok: true,
			engine: 'rpc' as const,
			changes: [
				{ kind: 'native' as const, delta: '-30000000000000000', symbol: 'xDAI', decimals: 18 },
				{ kind: 'erc20' as const, token: LURE_TOKEN, delta: LURE.toString(), unverified: true },
				{ kind: 'erc20' as const, token: UNNAMED, delta: `-${OUTFLOW}`, unverified: true },
				{ kind: 'erc20' as const, token: IDLE, delta: '0', unverified: true },
				{ kind: 'erc20' as const, token: IDLE, delta: 'lots', unverified: true }
			]
		};
		const read = deserializeAssetSim(older);
		expect(read.changes).toEqual([
			{ kind: 'native', delta: -30_000_000_000_000_000n, symbol: 'xDAI', decimals: 18 },
			{ kind: 'erc20', token: LURE_TOKEN, direction: 'in', unverified: true },
			{ kind: 'erc20', token: UNNAMED, direction: 'out', unverified: true },
			{ kind: 'erc20', token: IDLE, direction: 'still', unverified: true },
			{ kind: 'erc20', token: IDLE, direction: 'unreadable', unverified: true }
		]);
		// Written back, the figures are gone for good.
		const again = JSON.stringify(serializeAssetSim(read));
		expect(again).not.toContain('5000');
		expect(again).not.toContain('7000');
	});

	it('a blob stored since round-trips as it is', () => {
		const sim: AssetSimResult = {
			ok: true,
			engine: 'rpc',
			changes: [
				{ kind: 'native', delta: 5n, symbol: 'xDAI', decimals: 18 },
				{ kind: 'erc20', token: LURE_TOKEN, direction: 'in', unverified: true }
			]
		};
		expect(deserializeAssetSim(JSON.parse(JSON.stringify(serializeAssetSim(sim)))).changes).toEqual(
			sim.changes
		);
	});
});
