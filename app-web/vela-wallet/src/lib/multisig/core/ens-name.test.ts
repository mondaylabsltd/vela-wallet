/**
 * r4D-5: an ENS name typed as a member is read on Ethereum (registry →
 * resolver → addr) and given back as a checksummed address; a name with no
 * address, an Ethereum that does not answer, and a name with characters this
 * does not normalise are each said as themselves.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

const seams = vi.hoisted(() => ({
	resolver: `0x${'0'.repeat(24)}${'4976fb03c32e5b8cfe2b6ccb31c09ba78ebaba41'}`,
	addr: `0x${'0'.repeat(24)}${'d8da6bf26964af9d7eed9e03e53415d37aa96045'}`,
	fail: false,
	calls: [] as { to: string; chainId: number }[]
}));

vi.mock('$lib/services/ens', () => ({ namehash: () => `0x${'ab'.repeat(32)}` }));
vi.mock('$lib/core/kernels', () => ({
	checksumAddress: (a: string) =>
		a.toLowerCase() === '0xd8da6bf26964af9d7eed9e03e53415d37aa96045'
			? '0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045'
			: a
}));
vi.mock('$lib/services/rpc-pool', () => ({
	poolRpcCall: async (_m: string, params: [{ to: string; data: string }], chainId: number) => {
		seams.calls.push({ to: params[0].to, chainId });
		if (seams.fail) throw new Error('offline');
		return { result: params[0].data.startsWith('0x0178b8bf') ? seams.resolver : seams.addr };
	}
}));

const { looksLikeEns, resolveEns } = await import('./ens-name');
const ZERO = `0x${'0'.repeat(64)}`;

beforeEach(() => {
	seams.fail = false;
	seams.calls = [];
	seams.resolver = `0x${'0'.repeat(24)}4976fb03c32e5b8cfe2b6ccb31c09ba78ebaba41`;
	seams.addr = `0x${'0'.repeat(24)}d8da6bf26964af9d7eed9e03e53415d37aa96045`;
});

describe('an ENS name as a member (r4D-5)', () => {
	it('is told from an address', () => {
		expect(looksLikeEns('vitalik.eth')).toBe(true);
		expect(looksLikeEns(' Team.Alice.ETH ')).toBe(true);
		expect(looksLikeEns('0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045')).toBe(false);
	});

	it('reads its address on Ethereum, checksummed', async () => {
		expect(await resolveEns('Vitalik.eth')).toEqual({
			kind: 'found',
			name: 'vitalik.eth',
			address: '0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045'
		});
		expect(seams.calls.map((c) => c.chainId)).toEqual([1, 1]);
		expect(seams.calls[0].to).toBe('0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e');
	});

	it('says a name with no resolver or no address points nowhere', async () => {
		seams.resolver = ZERO;
		expect((await resolveEns('nobody.eth')).kind).toBe('none');
		seams.resolver = `0x${'0'.repeat(24)}4976fb03c32e5b8cfe2b6ccb31c09ba78ebaba41`;
		seams.addr = ZERO;
		expect((await resolveEns('nobody.eth')).kind).toBe('none');
	});

	it('says Ethereum did not answer, rather than that the name is empty', async () => {
		seams.fail = true;
		expect((await resolveEns('vitalik.eth')).kind).toBe('unavailable');
	});

	it('does not look up a name it cannot normalise (a lookalike)', async () => {
		expect((await resolveEns('vit\u0430lik.eth')).kind).toBe('unsupported');
		expect(seams.calls).toEqual([]);
	});
});
