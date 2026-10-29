import { describe, expect, it } from 'vitest';
import {
	decodeFunctionResult,
	encodeFunctionData,
	getAddress,
	stringToHex,
	type Hex,
	type PublicClient
} from 'viem';
import { CONTRACT_ADDRESS, GENESIS_REGISTRY_ABI } from './chain';
import {
	SAFE_SETUP_ABI,
	decodeName,
	decodeSetup,
	fetchMultisigPage,
	formatGenesis,
	pageSlices,
	registryDeployment
} from './multisig';
import { readChainCount, readMultisigCount } from './multisig-count';
import { GENESIS_REGISTRY_ADDRESS, MULTISIG_CHAINS, readMultisigChain } from './registry-chains';
import { PAGE_HOSTILE_NAMES, PAGE_NEWEST_2, PAGE_RENAMED } from './multisig.fixture';

const A = getAddress('0x1111111111111111111111111111111111111111');
const B = getAddress('0x2222222222222222222222222222222222222222');
const C = getAddress('0x3333333333333333333333333333333333333333');
const ZERO = '0x0000000000000000000000000000000000000000';

function setup(
	owners: `0x${string}`[],
	threshold: bigint,
	extra: Partial<{ to: `0x${string}`; data: Hex }> = {}
) {
	return encodeFunctionData({
		abi: SAFE_SETUP_ABI,
		functionName: 'setup',
		args: [owners, threshold, extra.to ?? ZERO, extra.data ?? '0x', ZERO, ZERO, 0n, ZERO]
	});
}

function decodePage(data: Hex) {
	return decodeFunctionResult({ abi: GENESIS_REGISTRY_ABI, functionName: 'genesisPage', data });
}

describe('initializer decoding', () => {
	it('reads owners and threshold from a Safe setup() call', () => {
		const s = decodeSetup(setup([A, B, C], 2n));
		expect(s.ok).toBe(true);
		if (!s.ok) return;
		expect(s.owners).toEqual([A, B, C]);
		expect(s.threshold).toBe(2n);
		expect(s.problems).toEqual([]);
	});

	it('keeps a setup hook visible', () => {
		const s = decodeSetup(setup([A], 1n, { to: B, data: '0xdeadbeef' }));
		expect(s.ok && s.to).toBe(B);
		expect(s.ok && s.data).toBe('0xdeadbeef');
	});

	it('names what Safe would reject instead of hiding it', () => {
		const s = decodeSetup(setup([A, A], 3n));
		expect(s.ok).toBe(true);
		if (!s.ok) return;
		expect(s.problems).toContain('threshold exceeds the owner count');
		expect(s.problems.some((p) => p.startsWith('duplicate owner'))).toBe(true);
		const zero = decodeSetup(setup([], 0n));
		expect(zero.ok && zero.problems).toEqual(['no owners', 'threshold is 0']);
	});

	it('returns the raw facts for something that is not a setup call', () => {
		const s = decodeSetup('0xa9059cbb0000');
		expect(s).toEqual({
			ok: false,
			reason: 'Not a Safe setup() call',
			selector: '0xa9059cbb',
			byteLength: 6
		});
		expect(decodeSetup('0x12')).toMatchObject({ ok: false, selector: null, byteLength: 1 });
	});

	it('does not throw on a truncated setup call', () => {
		const s = decodeSetup(setup([A, B], 1n).slice(0, 80) as Hex);
		expect(s.ok).toBe(false);
		expect(!s.ok && s.selector).toBe('0xb63e800d');
	});
});

describe('names', () => {
	it('flags bytes that are not UTF-8 and keeps the raw bytes', () => {
		const n = decodeName('0x4261642080ff206f6b');
		expect(n.utf8).toBe(false);
		expect(n.raw).toBe('0x4261642080ff206f6b');
	});

	it('removes bidi overrides and zero-width characters, and says so', () => {
		const n = decodeName(stringToHex('Safe‮gnp.exe​'));
		expect(n.text).toBe('Safegnp.exe');
		expect(n.hadHidden).toBe(true);
		expect(decodeName(stringToHex('金库')).hadHidden).toBe(false);
	});
});

describe('real records (Arc Testnet fixture)', () => {
	it('decodes the newest records with setup, name and write order', () => {
		const [total, safes, records] = decodePage(PAGE_NEWEST_2 as Hex);
		expect(Number(total)).toBeGreaterThanOrEqual(32);
		const a = formatGenesis(safes[0], records[0]);
		const b = formatGenesis(safes[1], records[1]);
		expect(a.index).toBe(31);
		expect(a.name.text).toBe('Sandbox');
		expect(a.setup.ok && `${a.setup.threshold} of ${a.setup.owners.length}`).toBe('1 of 1');
		expect(b.index).toBe(30);
		expect(b.name.text).toBe('Résumé Café');
		expect(b.setup.ok && `${b.setup.threshold} of ${b.setup.owners.length}`).toBe('2 of 3');
		expect(b.safe).toBe('0xE8424Cc6E6582f96293249112D9D077269AA2b6E');
		expect(b.selfNamed).toBe(false);
	});

	it('keeps hostile names as text and flags invalid UTF-8', () => {
		const [, safes, records] = decodePage(PAGE_HOSTILE_NAMES as Hex);
		const bad = formatGenesis(safes[0], records[0]);
		const markup = formatGenesis(safes[1], records[1]);
		expect(bad.index).toBe(18);
		expect(bad.name.utf8).toBe(false);
		expect(markup.index).toBe(17);
		expect(markup.name.text).toBe('<img src=x onerror=alert(1)>');
		expect(markup.name.utf8).toBe(true);
	});

	it('marks a name the Safe set itself', () => {
		const [, safes, records] = decodePage(PAGE_RENAMED as Hex);
		const r = formatGenesis(safes[0], records[0]);
		expect(r.index).toBe(1);
		expect(r.renamedAt > 0n).toBe(true);
		expect(r.selfNamed).toBe(true);
		expect(r.name.text.startsWith('Renamed by the Safe')).toBe(true);
	});
});

/** A registry in memory with the contract's genesisPage semantics. */
function fakeRegistry(count: number, opts: { maxLimit?: number; failAll?: boolean } = {}) {
	const calls: [number, number, boolean][] = [];
	const rec = (i: number) => ({
		factory: B,
		singleton: C,
		renamedAt: 0n,
		saltNonce: BigInt(i),
		initializer: setup([A], 1n),
		proxyCreationCodeHash: `0x${'00'.repeat(32)}` as Hex,
		recorder: A,
		recordedAt: 100n + BigInt(i),
		index: i,
		name: stringToHex(`Safe ${i}`)
	});
	const safeAt = (i: number) => getAddress(`0x${(i + 1).toString(16).padStart(40, '0')}`);
	const client = {
		readContract: async ({ args }: { args: [bigint, bigint, boolean] }) => {
			const [o, l, desc] = [Number(args[0]), Number(args[1]), args[2]];
			calls.push([o, l, desc]);
			if (opts.failAll) throw new Error('network down');
			if (opts.maxLimit && l > opts.maxLimit) throw new Error('out of gas');
			const n = Math.max(0, Math.min(l, count - o));
			const idx = Array.from({ length: n }, (_, k) => (desc ? count - 1 - o - k : o + k));
			return [BigInt(count), idx.map(safeAt), idx.map(rec)];
		}
	} as unknown as PublicClient;
	return { client, calls };
}

describe('paging', () => {
	it('splits a page into gas-safe slices at the right offsets', () => {
		expect(pageSlices(1, 100)).toEqual([
			{ offset: 0, limit: 20 },
			{ offset: 20, limit: 20 },
			{ offset: 40, limit: 20 },
			{ offset: 60, limit: 20 },
			{ offset: 80, limit: 20 }
		]);
		expect(pageSlices(3, 50)).toEqual([
			{ offset: 100, limit: 20 },
			{ offset: 120, limit: 20 },
			{ offset: 140, limit: 10 }
		]);
	});

	it('reads newest first, in write order, with the running total', async () => {
		const { client } = fakeRegistry(45);
		const p1 = await fetchMultisigPage(client, 1, 20, true);
		expect(p1.total).toBe(45);
		expect(p1.records.map((r) => r.index)).toEqual(Array.from({ length: 20 }, (_, i) => 44 - i));
		const p3 = await fetchMultisigPage(client, 3, 20, true);
		expect(p3.records.map((r) => r.index)).toEqual([4, 3, 2, 1, 0]);
		const oldest = await fetchMultisigPage(client, 1, 50, false);
		expect(oldest.records.map((r) => r.index)).toEqual(Array.from({ length: 45 }, (_, i) => i));
	});

	it('is empty, not an error, past the end', async () => {
		const { client } = fakeRegistry(5);
		expect(await fetchMultisigPage(client, 2, 20, true)).toEqual({ total: 5, records: [] });
	});

	it('falls back to one record per call when a node refuses the slice', async () => {
		const { client, calls } = fakeRegistry(30, { maxLimit: 1 });
		const p = await fetchMultisigPage(client, 1, 20, true);
		expect(p.records.map((r) => r.index)).toEqual(Array.from({ length: 20 }, (_, i) => 29 - i));
		expect(calls.filter(([, l]) => l === 1)).toHaveLength(20);
	});

	it('throws when the chain cannot be read, so the page can say so', async () => {
		const { client } = fakeRegistry(30, { failAll: true });
		await expect(fetchMultisigPage(client, 1, 20, true)).rejects.toThrow();
	});
});

describe('not deployed', () => {
	const viaCode = (result: unknown) => ({ request: async () => result }) as unknown as PublicClient;

	it('is "not deployed" only on a well-formed empty code answer', async () => {
		expect(await registryDeployment(viaCode('0x'))).toBe('not-deployed');
		expect(await registryDeployment(viaCode('0x6080604052'))).toBe('deployed');
		// A node that answers an error inside `result` is a failed read, not "no contract".
		await expect(
			registryDeployment(viaCode({ code: 503, message: 'unavailable' }))
		).rejects.toThrow();
		await expect(registryDeployment(viaCode(undefined))).rejects.toThrow();
	});

	it('asks about the contract it is given (the passkey registry on Ethereum, too)', async () => {
		const asked: unknown[] = [];
		const client = {
			request: async ({ params }: { params: unknown[] }) => (asked.push(params[0]), '0x')
		} as unknown as PublicClient;
		expect(await registryDeployment(client, CONTRACT_ADDRESS)).toBe('not-deployed');
		expect(await registryDeployment(client)).toBe('not-deployed');
		expect(asked).toEqual([CONTRACT_ADDRESS, GENESIS_REGISTRY_ADDRESS]);
	});

	function fakeFetch(perUrl: Record<string, { code?: unknown; total?: unknown } | 'down'>) {
		return (async (url: string, init: { body: string }) => {
			const cfg = perUrl[url];
			if (!cfg || cfg === 'down') throw new Error('unreachable');
			const { method } = JSON.parse(init.body);
			const result = method === 'eth_getCode' ? cfg.code : cfg.total;
			return { json: async () => ({ jsonrpc: '2.0', id: 1, result }) };
		}) as unknown as typeof fetch;
	}

	const word = (n: number) => `0x${n.toString(16).padStart(64, '0')}`;
	const chains = [
		{ ...MULTISIG_CHAINS[0], rpcs: ['g1', 'g2'] },
		{ ...MULTISIG_CHAINS[1], rpcs: ['e1'] }
	];

	it('counts a chain with the registry, says "not deployed" for one without', async () => {
		const f = fakeFetch({ g1: 'down', g2: { code: '0x60', total: word(7) }, e1: { code: '0x' } });
		expect(await readChainCount(chains[0], f)).toEqual({ state: 'deployed', count: 7 });
		expect(await readChainCount(chains[1], f)).toEqual({ state: 'not-deployed' });
	});

	it('homepage: not deployed anywhere in production', async () => {
		const f = fakeFetch({ g1: { code: '0x' }, e1: { code: '0x' } });
		expect(await readMultisigCount(f, chains)).toEqual({ state: 'not-deployed' });
	});

	it('homepage: never adds two chains (a Safe can be on both); one failed chain = unknown', async () => {
		const both = fakeFetch({
			g1: { code: '0x60', total: word(12) },
			e1: { code: '0x60', total: word(3) }
		});
		expect(await readMultisigCount(both, chains)).toEqual({
			state: 'deployed',
			count: 12,
			chains: ['Gnosis', 'Ethereum']
		});
		const half = fakeFetch({ g1: { code: '0x60', total: word(12) }, e1: 'down' });
		expect(await readMultisigCount(half, chains)).toBeNull();
	});

	it('offers production chains only (owner, 2026-10-29: no testnet on the site)', () => {
		expect(MULTISIG_CHAINS.map((c) => c.key)).toEqual(['gnosis', 'ethereum']);
		expect(MULTISIG_CHAINS.some((c) => c.testnet)).toBe(false);
	});

	it('reads ?chain= and falls back to the production default', () => {
		// The site offers no testnet: an old ?chain=arc-testnet link opens production.
		expect(readMultisigChain('arc-testnet')).toBe('gnosis');
		expect(readMultisigChain('ethereum')).toBe('ethereum');
		expect(readMultisigChain('<script>')).toBe('gnosis');
		expect(readMultisigChain(null)).toBe('gnosis');
	});
});
