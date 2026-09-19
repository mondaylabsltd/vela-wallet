/**
 * Signing in when the index is gone (spec 062): the two registry reads fall
 * back to the contract — Gnosis, then Ethereum — and the REAL core turns the
 * REAL bytes both chains answered (`__fixtures__/registry-chain.json`) into the
 * index's own shapes, so every guard in `registry.ts` runs unchanged.
 */
import '$lib/i18n/wasm-init.server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import fixture from './__fixtures__/registry-chain.json';

vi.mock('$lib/core/client', async (original) => ({
	...(await original<typeof import('$lib/core/client')>()),
	loadCore: async () => {}
}));

import { _resetUnitSource, queryByPublicKey, queryUnit, RegistryError } from './registry';

const answers = fixture.answers as Record<string, Record<string, string>>;
/** How the index behaves, and which chains are reachable. */
let index: 'unreachable' | 'failing' | 'refusing' | { units: number[] };
let silentChains: number[];
const chainOf = (url: string) => (url.includes('gnosis') ? 100 : 1);
const asked: string[] = [];

beforeEach(() => {
	_resetUnitSource();
	index = 'unreachable';
	silentChains = [];
	asked.length = 0;
	vi.stubGlobal(
		'fetch',
		vi.fn(async (url: string, init?: RequestInit) => {
			if (url.includes('/api/query')) {
				asked.push('index');
				if (index === 'unreachable') throw new TypeError('Failed to fetch');
				if (index === 'failing') return new Response('bad gateway', { status: 502 });
				if (index === 'refusing') return new Response('{}', { status: 400 });
				return Response.json({ entry: {}, groups: { total: 1, unitIds: index.units } });
			}
			const chain = chainOf(url);
			asked.push(`chain:${chain}`);
			if (silentChains.includes(chain)) throw new TypeError('Failed to fetch');
			const call = JSON.parse(String(init?.body)).params[0] as { data: string };
			return Response.json({ jsonrpc: '2.0', id: 1, result: answers[String(chain)][call.data] });
		})
	);
});
afterEach(() => vi.unstubAllGlobals());

describe('the registry reads, with the index gone', () => {
	it('an unreachable index: the key and its unit are read from the contract on Gnosis', async () => {
		expect(await queryByPublicKey(fixture.publicKey)).toEqual({
			registered: true,
			unitIds: [12, 10, 8]
		});
		const unit = await queryUnit(10);
		expect(unit.members).toHaveLength(3);
		expect(unit.members[0].public_key_hex).toBe(fixture.publicKey);
		expect(unit.members.every((m) => m.credential_id !== '')).toBe(true);
		// The metadata is the wallet's own: address and key names.
		expect(Buffer.from(unit.metadataHex, 'hex').toString()).toContain(
			'0x88cCA0EeDbF2C4426110bbFc998F048689266894'
		);
		// Gnosis listed the units, so Gnosis — not the index — is asked about them.
		expect(asked.filter((a) => a === 'index')).toHaveLength(1);
	});

	it('Gnosis silent too: a backed-up wallet signs in from Ethereum, where it is unit 0', async () => {
		silentChains = [100];
		expect(await queryByPublicKey(fixture.publicKey)).toEqual({ registered: true, unitIds: [0] });
		const unit = await queryUnit(0);
		expect(unit.members).toHaveLength(3);
		expect(asked).toContain('chain:1');

		// The same three founding keys, in the same founding order, as Gnosis holds.
		_resetUnitSource();
		silentChains = [];
		await queryByPublicKey(fixture.publicKey);
		const home = await queryUnit(10);
		expect(unit.members.map((m) => m.public_key_hex)).toEqual(
			home.members.map((m) => m.public_key_hex)
		);
		expect(unit.metadataHex).toBe(home.metadataHex);
	});

	it('a 5xx is the index failing; a 4xx is the index ANSWERING — no fallback, no second opinion', async () => {
		index = 'failing';
		expect((await queryByPublicKey(fixture.publicKey)).unitIds).toEqual([12, 10, 8]);
		_resetUnitSource();
		index = 'refusing';
		await expect(queryByPublicKey(fixture.publicKey)).rejects.toBeInstanceOf(RegistryError);
		expect(asked.filter((a) => a.startsWith('chain:'))).toHaveLength(2); // only the 5xx run read chains
	});

	it("the index listed the unit and then went away: its ids are Gnosis's", async () => {
		index = { units: [10] };
		await queryByPublicKey(fixture.publicKey);
		index = 'unreachable';
		expect((await queryUnit(10)).members).toHaveLength(3);
	});

	it('nobody answers: the ORIGINAL failure is what the person is told', async () => {
		silentChains = [100, 1];
		await expect(queryByPublicKey(fixture.publicKey)).rejects.toMatchObject({ network: true });
	});
});
