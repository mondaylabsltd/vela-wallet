/**
 * r4D-18 (round 5): a multisig's poll reads its nonce, and its members only
 * when the nonce moved (or a minute on, for a module's change); a deployed
 * Safe's code is read once per device, not once per page.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const OWNER = '0x1111111111111111111111111111111111111111';
const word = (n: bigint | number) => BigInt(n).toString(16).padStart(64, '0');

const chain = vi.hoisted(() => ({ nonce: 3n, asked: [] as string[] }));
vi.mock('$lib/services/rpc-pool', () => ({
	poolRpcCall: async (method: string, params: [{ data?: string } | string]) => {
		if (method === 'eth_getCode') {
			chain.asked.push('code');
			return { result: '0x6080' };
		}
		const data = (params[0] as { data: string }).data;
		const name =
			{ '0xaffed0e0': 'nonce', '0xa0e67e2b': 'owners', '0xe75235b8': 'threshold' }[data] ?? data;
		chain.asked.push(name);
		if (name === 'nonce') return { result: `0x${word(chain.nonce)}` };
		if (name === 'threshold') return { result: `0x${word(1)}` };
		return { result: `0x${word(32)}${word(1)}${OWNER.slice(2).padStart(64, '0')}` };
	}
}));

const store = new Map<string, string>();
vi.stubGlobal('localStorage', {
	getItem: (k: string) => store.get(k) ?? null,
	setItem: (k: string, v: string) => void store.set(k, v),
	removeItem: (k: string) => void store.delete(k)
});

const { readMembers, resetChainReadsForTests, MEMBERS_FRESH_MS } = await import('./chain');

let clock = 1_000_000;
beforeEach(() => {
	chain.asked = [];
	chain.nonce = 3n;
	clock = 1_000_000;
	resetChainReadsForTests(() => clock);
});

describe('a multisig’s members, read on its poll', () => {
	it('once; then only its nonce while the nonce stands', async () => {
		const first = await readMembers(100, SAFE, {} as never);
		expect(first).toMatchObject({ deployed: true, threshold: 1, nonce: 3n });
		expect(first.owners.map((o) => o.toLowerCase())).toEqual([OWNER]);
		expect(chain.asked.sort()).toEqual(['code', 'nonce', 'owners', 'threshold']);
		chain.asked = [];
		clock += 12_000;
		expect(await readMembers(100, SAFE, {} as never)).toEqual(first);
		expect(chain.asked, 'the code is known, the nonce stood').toEqual(['nonce']);
	});

	it('again when the nonce moved, and a minute on regardless', async () => {
		await readMembers(100, SAFE, {} as never);
		chain.asked = [];
		chain.nonce = 4n;
		expect((await readMembers(100, SAFE, {} as never)).nonce).toBe(4n);
		expect(chain.asked.sort()).toEqual(['nonce', 'owners', 'threshold']);
		chain.asked = [];
		clock += MEMBERS_FRESH_MS;
		await readMembers(100, SAFE, {} as never);
		expect(chain.asked.sort()).toEqual(['nonce', 'owners', 'threshold']);
	});

	it('a deployed Safe’s code is kept on the device: the next page does not read it', async () => {
		await readMembers(100, SAFE, {} as never);
		vi.resetModules();
		const again = await import('./chain');
		chain.asked = [];
		await again.readMembers(100, SAFE, {} as never);
		expect(chain.asked).not.toContain('code');
	});
});
