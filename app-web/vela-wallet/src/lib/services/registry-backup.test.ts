/**
 * The Ethereum backup check (spec 062 §5a) — the REAL core, fed the REAL
 * bytes the Gnosis registry answered for wallet 0x88cC…6894
 * (`__fixtures__/registry-backup.gnosis.json`). What is pinned here is the
 * web/core seam: which chain each question goes to, that a bare `0x` reaches
 * the core as an answer, and that silence is never a verdict.
 */
import '$lib/i18n/wasm-init.server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import gnosis from './__fixtures__/registry-backup.gnosis.json';

vi.mock('$lib/core/client', async (original) => ({
	...(await original<typeof import('$lib/core/client')>()),
	loadCore: async () => {}
}));
vi.mock('$lib/services/endpoints', () => ({ getPasskeyIndexURL: () => 'https://index.test' }));
const fetched: string[] = [];
vi.mock('$lib/services/net', () => ({
	NET_TIMEOUTS: { keyIndexRead: 1 },
	fetchWithTimeout: vi.fn(async (url: string) => {
		fetched.push(url);
		return { ok: false, status: 500, text: async () => '' };
	})
}));

const REGISTRY = '0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9';
const SELECTOR = {
	version: '0xffa1ad74',
	groups: '0xcc7aae8e',
	unit: '0x1655bfbe',
	mirrored: '0xa451c8ce',
	payload: '0x19178efe'
};
const word = (n: number) => n.toString(16).padStart(64, '0');
/** `(false, 0, Unit{})` — what a registry that lacks the group answers. */
const NOT_MIRRORED =
	'0x' + word(0) + word(0) + word(0x60) + word(0xc0) + word(0xe0) + word(0x100) + word(0).repeat(6);

/** What each chain says. `null` = the pool reports an error. */
let ethereum: { version: string | null; mirrored: string | null };
let silentOnGnosis: string | null;
const asked: string[] = [];

vi.mock('$lib/services/rpc-pool', () => ({
	poolRpcCall: vi.fn(
		async (_method: string, params: [{ to: string; data: string }], chainId: number) => {
			const { to, data } = params[0];
			expect(to).toBe(REGISTRY);
			const selector = data.slice(0, 10);
			asked.push(`${chainId}:${selector}`);
			const answer = (result: string | null) =>
				result === null
					? { jsonrpc: '2.0', id: 1, error: { code: -32000, message: 'down' } }
					: { jsonrpc: '2.0', id: 1, result };
			if (chainId === 1) {
				if (selector === SELECTOR.version) return answer(ethereum.version);
				if (selector === SELECTOR.mirrored) return answer(ethereum.mirrored);
			}
			if (chainId === 100 && selector !== silentOnGnosis) {
				if (selector === SELECTOR.groups) return answer(gnosis.groups);
				if (selector === SELECTOR.payload) return answer(gnosis.payload10);
				if (selector === SELECTOR.unit)
					return answer(data.endsWith(word(12)) ? gnosis.unit12 : gnosis.unit10);
			}
			return answer(null);
		}
	)
}));

import { checkEthereumBackup } from './registry-backup';

beforeEach(() => {
	ethereum = { version: gnosis.version, mirrored: NOT_MIRRORED };
	silentOnGnosis = null;
	asked.length = 0;
	fetched.length = 0;
});

describe('checkEthereumBackup', () => {
	it('today: the registry is not on Ethereum — unavailable, and nothing else is asked', async () => {
		ethereum.version = '0x';
		expect(await checkEthereumBackup(gnosis.address, gnosis.foundingPublicKey)).toEqual({
			state: 'unavailable',
			call: null,
			unitId: null
		});
		expect(asked).toEqual([`1:${SELECTOR.version}`]);
	});

	it('not backed up: five questions, the right chain for each, and the REAL Gnosis bytes offered', async () => {
		const check = await checkEthereumBackup(gnosis.address, gnosis.foundingPublicKey);
		expect(asked).toEqual([
			`1:${SELECTOR.version}`,
			`100:${SELECTOR.groups}`,
			`100:${SELECTOR.unit}`, // unit 12 — the same key's OTHER wallet
			`100:${SELECTOR.unit}`, // unit 10 — this one
			`1:${SELECTOR.mirrored}`,
			`100:${SELECTOR.payload}`
		]);
		expect(check.state).toBe('not_backed_up');
		expect(check.unitId).toBe(10);
		expect(check.call).toMatchObject({ chain_id: 1, to: REGISTRY, value: '0' });
		// register(...) — 4,324 bytes, exactly what Gnosis holds for this wallet.
		expect(check.call?.data.slice(0, 10)).toBe('0xcd438f9b');
		expect((check.call!.data.length - 2) / 2).toBe(4324);
		expect(gnosis.payload10).toContain(check.call!.data.slice(2));
	});

	it('backed up: Ethereum holds the group, and the payload is never fetched', async () => {
		ethereum.mirrored = gnosis.mirroredTrue;
		const check = await checkEthereumBackup(gnosis.address, gnosis.foundingPublicKey);
		expect(check).toEqual({ state: 'backed_up', call: null, unitId: 10 });
		expect(asked).not.toContain(`100:${SELECTOR.payload}`);
	});

	it('another wallet of the same key is not this one: not registered', async () => {
		const other = '0x' + '11'.repeat(20);
		expect((await checkEthereumBackup(other, gnosis.foundingPublicKey)).state).toBe(
			'not_registered'
		);
	});

	it('silence on either chain is could-not-check — never "backed up", never a button', async () => {
		ethereum.version = null;
		expect((await checkEthereumBackup(gnosis.address, gnosis.foundingPublicKey)).state).toBe(
			'could_not_check'
		);
		ethereum.version = gnosis.version;
		for (const selector of [SELECTOR.groups, SELECTOR.unit, SELECTOR.payload]) {
			silentOnGnosis = selector;
			const check = await checkEthereumBackup(gnosis.address, gnosis.foundingPublicKey);
			expect(check, selector).toEqual({ state: 'could_not_check', call: null, unitId: null });
		}
		ethereum.mirrored = null;
		silentOnGnosis = null;
		expect((await checkEthereumBackup(gnosis.address, gnosis.foundingPublicKey)).state).toBe(
			'could_not_check'
		);
	});

	it('server-free: the index service is never contacted', async () => {
		await checkEthereumBackup(gnosis.address, gnosis.foundingPublicKey);
		expect(fetched).toEqual([]);
	});
});
