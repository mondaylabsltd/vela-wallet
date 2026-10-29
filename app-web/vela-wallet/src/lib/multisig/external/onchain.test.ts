/**
 * r4C-16: an owner's on-chain approval (`approveHash`), read from the chain:
 * a yes is kept, a no asked again only after a while.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

const rpc = vi.hoisted(() => ({
	calls: [] as { to: string; data: string }[],
	approved: new Set<string>(),
	down: false
}));
vi.mock('$lib/services/rpc-pool', () => ({
	poolRpcCall: async (_method: string, params: [{ to: string; data: string }]) => {
		rpc.calls.push(params[0]);
		if (rpc.down) return { error: { message: 'down' } };
		const owner = `0x${params[0].data.slice(10 + 24, 10 + 64)}`;
		return { result: `0x${(rpc.approved.has(owner) ? 1 : 0).toString(16).padStart(64, '0')}` };
	}
}));

const { approveHashCall, approvedHashesCall, onChainApprovals, RECHECK_MS, resetOnChainForTests } =
	await import('./onchain');

const SAFE = '0x0cb9aef9713c4a2ea2995fca68d7355ce5e27d83';
const HASH = `0x${'ab'.repeat(32)}`;
const A = '0x00000000000000000000000000000000000000aa';
const B = '0x00000000000000000000000000000000000000bb';

beforeEach(() => {
	rpc.calls = [];
	rpc.approved.clear();
	rpc.down = false;
	resetOnChainForTests();
});

describe('approvals on chain', () => {
	it('the calldata: approvedHashes(owner, hash) and approveHash(hash)', () => {
		expect(approvedHashesCall(A, HASH)).toBe(`0x7d832974${'0'.repeat(62)}aa${'ab'.repeat(32)}`);
		expect(approveHashCall(HASH)).toBe(`0xd4d9bdcd${'ab'.repeat(32)}`);
	});

	it('names the owners whose approvedHashes is set; a yes is kept, a no asked again later', async () => {
		rpc.approved.add(A);
		expect(await onChainApprovals(100, SAFE, HASH, [A, B], 1_000)).toEqual([A]);
		expect(rpc.calls).toHaveLength(2);
		// Soon after: nothing asked.
		expect(await onChainApprovals(100, SAFE, HASH, [A, B], 2_000)).toEqual([A]);
		expect(rpc.calls).toHaveLength(2);
		// B approves; after the recheck window only B is asked again.
		rpc.approved.add(B);
		expect(await onChainApprovals(100, SAFE, HASH, [A, B], 1_000 + RECHECK_MS + 1)).toEqual([A, B]);
		expect(rpc.calls).toHaveLength(3);
	});

	it('a chain that does not answer approves nobody', async () => {
		rpc.down = true;
		expect(await onChainApprovals(100, SAFE, HASH, [A])).toEqual([]);
	});
});
