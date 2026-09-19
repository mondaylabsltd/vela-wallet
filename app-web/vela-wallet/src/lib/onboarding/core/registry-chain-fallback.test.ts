/**
 * The three layers (064): the index for speed, the chain for truth, Ethereum
 * for survival — through `registry.ts`, with the REAL core and the REAL bytes
 * both chains answered (`__fixtures__/registry-chain.json`).
 *
 * The rules are `vela_core::registry_resolve` and are tested there. What is
 * pinned HERE is that this shell really hands its reads to that walk: that an
 * index answer is PROVED rather than believed, that a forged one is thrown
 * away, and that a listing's unit ids are asked of whoever listed them.
 */
import '$lib/i18n/wasm-init.server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import fixture from './__fixtures__/registry-chain.json';

vi.mock('$lib/core/client', async (original) => ({
	...(await original<typeof import('$lib/core/client')>()),
	loadCore: async () => {}
}));

import { registryChainUnit, registryChainUnitPlan } from '$lib/core/client';
import { _resetUnitSource, queryByPublicKey, queryUnit, RegistryError } from './registry';

const answers = fixture.answers as Record<string, Record<string, string>>;
const chainOf = (url: string) => (url.includes('gnosis') ? 100 : 1);

/** Gnosis unit 10 in the index's shape, built from the CHAIN's own bytes — what an honest index returns. */
function honestUnit() {
	const plan = JSON.parse(registryChainUnitPlan(10)!) as { calls: { data: string }[] };
	const [unit, members] = plan.calls.map((call) => answers['100'][call.data]);
	return JSON.parse(registryChainUnit(10, unit, members)!) as {
		unit: Record<string, unknown>;
		members: { total: number; items: { publicKey: string }[] };
	};
}

/** What the index says for `?publicKey=` and `?unitId=`; `null` = unreachable, a number = that status. */
let indexListing: unknown;
let indexUnit: unknown;
let silentChains: number[];
const asked: string[] = [];

beforeEach(() => {
	_resetUnitSource();
	indexListing = null;
	indexUnit = null;
	silentChains = [];
	asked.length = 0;
	vi.stubGlobal(
		'fetch',
		vi.fn(async (url: string, init?: RequestInit) => {
			if (url.includes('/api/query')) {
				asked.push('index');
				const says = url.includes('publicKey=') ? indexListing : indexUnit;
				if (says === null) throw new TypeError('Failed to fetch');
				if (typeof says === 'number') return new Response('{}', { status: says });
				return new Response(JSON.stringify(says), { status: 200 });
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

describe('an index that answers is proved, not believed', () => {
	beforeEach(() => {
		indexListing = { entry: {}, groups: { total: 1, unitIds: [10] } };
	});

	it('honest: the index, then ONE eth_call to Gnosis — and its answer is what is used', async () => {
		indexUnit = honestUnit();
		await queryByPublicKey(fixture.publicKey);
		asked.length = 0;
		const unit = await queryUnit(10);
		expect(unit.members).toHaveLength(3);
		expect(asked).toEqual(['index', 'chain:100']);
	});

	it("FORGED — one member swapped for another: caught, and the chain's founding set is returned", async () => {
		const forged = honestUnit();
		forged.members.items[2].publicKey = forged.members.items[1].publicKey;
		indexUnit = forged;
		const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
		await queryByPublicKey(fixture.publicKey);
		const unit = await queryUnit(10);
		const honest = honestUnit().members.items.map((m) => m.publicKey);
		expect(unit.members.map((m) => m.public_key_hex)).toEqual(honest);
		expect(new Set(honest).size).toBe(3);
		expect(warn).toHaveBeenCalledWith(expect.stringContaining('did not match the chain'));
		warn.mockRestore();
	});

	it("no chain reachable: the person still signs in on the index's word", async () => {
		indexUnit = honestUnit();
		silentChains = [100, 1];
		await queryByPublicKey(fixture.publicKey);
		expect((await queryUnit(10)).members).toHaveLength(3);
	});

	it('Gnosis silent: the Ethereum backup proves the same answer — no id translation', async () => {
		indexUnit = honestUnit();
		silentChains = [100];
		await queryByPublicKey(fixture.publicKey);
		asked.length = 0;
		expect((await queryUnit(10)).members).toHaveLength(3);
		// (Gnosis has more than one public endpoint; each is tried before it counts as silent.)
		expect([...new Set(asked)]).toEqual(['index', 'chain:100', 'chain:1']);
	});
});

describe('an index that does not answer', () => {
	it('unreachable: the key and its unit are read from the contract on Gnosis', async () => {
		expect(await queryByPublicKey(fixture.publicKey)).toEqual({
			registered: true,
			unitIds: [12, 10, 8]
		});
		const unit = await queryUnit(10);
		expect(unit.members).toHaveLength(3);
		expect(unit.members[0].public_key_hex).toBe(fixture.publicKey);
		expect(asked).not.toContain('chain:1');
	});

	it('a 5xx, a refusal, and "no such wallet" are all checked with the contract first', async () => {
		for (const says of [502, 400, { entry: null, groups: { total: 0, unitIds: [] } }]) {
			_resetUnitSource();
			indexListing = says;
			expect((await queryByPublicKey(fixture.publicKey)).unitIds, String(says)).toEqual([
				12, 10, 8
			]);
		}
	});

	it("Gnosis silent too: Ethereum's backup answers, under ITS unit ids, and only it is asked about them", async () => {
		silentChains = [100];
		expect((await queryByPublicKey(fixture.publicKey)).unitIds).toEqual([0]);
		asked.length = 0;
		const unit = await queryUnit(0);
		expect(unit.members).toHaveLength(3);
		expect(unit.members[0].public_key_hex).toBe(fixture.publicKey);
		expect(new Set(asked)).toEqual(new Set(['chain:1']));
	});

	it("nobody at all: the INDEX's own failure is what is reported", async () => {
		silentChains = [100, 1];
		const gone = await queryByPublicKey(fixture.publicKey).catch((e: unknown) => e);
		expect(gone).toBeInstanceOf(RegistryError);
		expect((gone as RegistryError).network).toBe(true);

		indexListing = 400;
		const refused = await queryByPublicKey(fixture.publicKey).catch((e: unknown) => e);
		expect((refused as RegistryError).network).toBe(false);
		expect((refused as RegistryError).message).toContain('400');
	});
});
