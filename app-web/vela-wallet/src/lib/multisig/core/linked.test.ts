/**
 * ms-21: a proposal link on a device with none of the multisig's members
 * never adds or shows the multisig — its transaction is read for a preview,
 * and the store is left as it was. A member's device opens it as before.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const HASH = `0x${'ab'.repeat(32)}`;
const ME = '0x00000000000000000000000000000000000000aa';
const OTHER = '0x00000000000000000000000000000000000000bb';
const NEW_MEMBER = '0x00000000000000000000000000000000000000cc';

const fake = vi.hoisted(() => ({
	stored: null as null | { address: string; owners: string[] },
	found: null as null | { ok: boolean; account?: unknown; reason?: string },
	chainOwners: [] as string[],
	chainFails: false,
	local: [] as string[],
	reads: 0,
	writes: [] as string[]
}));

vi.mock('./store.svelte', () => ({
	multisig: {
		find: () => fake.stored ?? undefined,
		add: () => void fake.writes.push('add'),
		activate: () => void fake.writes.push('activate'),
		markShared: () => void fake.writes.push('markShared'),
		// The accounts machine's lookup (`find`), which keeps nothing.
		lookUp: async () => fake.found
	}
}));
vi.mock('./owners', () => ({
	localOwners: (owners: string[]) =>
		owners.filter((o) => fake.local.includes(o)).map((address) => ({ address }))
}));
vi.mock('./proposals.svelte', () => ({
	readChain: async () => {
		fake.reads += 1;
		if (fake.chainFails) throw new Error('down');
		return {
			chainId: 100,
			members: { owners: fake.chainOwners, threshold: 2, nonce: 0n },
			items: [{ safeTxHash: HASH }],
			mailbox: true,
			proposals: []
		};
	}
}));

const { LINKED_POLL_MS, LinkedProposals, openProposalLink } = await import('./linked.svelte');

const LINK = { chainId: 100, multisig: SAFE, safeTxHash: HASH };
const account = (owners: string[]) => ({ address: SAFE, owners, genesis: {}, chainIds: [100] });

beforeEach(() => {
	fake.stored = null;
	fake.found = { ok: true, account: account([ME, OTHER]) };
	fake.chainOwners = [ME, OTHER];
	fake.chainFails = false;
	fake.local = [];
	fake.reads = 0;
	fake.writes = [];
});

describe('a proposal link opened here (ms-21)', () => {
	it('no member here: a preview of the transaction, and nothing added or switched', async () => {
		const opening = await openProposalLink(LINK);
		expect(opening.kind).toBe('preview');
		if (opening.kind !== 'preview') return;
		expect(opening.added).toBe(false);
		expect(opening.source.chain(100)?.items).toEqual([{ safeTxHash: HASH }]);
		expect(opening.source.loading).toBe(false);
		expect(fake.writes).toEqual([]);
	});

	it('kept here before, still no member: a preview that says it is kept', async () => {
		fake.stored = account([OTHER]);
		const opening = await openProposalLink(LINK);
		expect(opening).toMatchObject({ kind: 'preview', added: true });
		expect(fake.writes).toEqual([]);
	});

	it('a member here: opened as before (the caller keeps and shows it)', async () => {
		fake.local = [ME];
		const opening = await openProposalLink(LINK);
		expect(opening).toMatchObject({ kind: 'member', added: false });
		// Deciding wrote nothing: keeping it is the caller's, after.
		expect(fake.writes).toEqual([]);
	});

	it('a member here, kept already: no chain read is waited for', async () => {
		fake.local = [ME];
		fake.stored = account([ME, OTHER]);
		const opening = await openProposalLink(LINK);
		expect(opening).toMatchObject({ kind: 'member', added: true });
		expect(fake.reads).toBe(0);
	});

	it('a member added on chain since its creation counts', async () => {
		fake.local = [NEW_MEMBER];
		fake.chainOwners = [ME, NEW_MEMBER];
		expect((await openProposalLink(LINK)).kind).toBe('member');
	});

	it('the chain cannot be read: decided by its founding members, still adding nothing', async () => {
		fake.chainFails = true;
		const opening = await openProposalLink(LINK);
		expect(opening.kind).toBe('preview');
		if (opening.kind === 'preview') expect(opening.source.stale).toBe(true);
		expect(fake.writes).toEqual([]);
	});

	it('not found: said as that', async () => {
		fake.found = { ok: false, reason: 'unavailable' };
		expect(await openProposalLink(LINK)).toEqual({ kind: 'failed', reason: 'unavailable' });
	});

	it('a genesis the core refuses, or a high-risk multisig: nothing read, nothing kept', async () => {
		for (const reason of ['refused', 'highRisk', 'unverified']) {
			fake.found = { ok: false, reason };
			expect(await openProposalLink(LINK)).toEqual({ kind: 'failed', reason });
		}
		expect(fake.reads).toBe(0);
		expect(fake.writes).toEqual([]);
	});
});

/**
 * r4b-4: a non-member's read-only preview is read again while it is shown,
 * as a member's sheet is — an execution elsewhere is seen there too — and a
 * network that does not answer is said as that (r4b-3).
 */
describe('a linked transaction, kept fresh while shown (r4b-4)', () => {
	it('is read again on the poll and when the tab comes back, and stops when closed', async () => {
		vi.useFakeTimers();
		const listeners = new Map<string, () => void>();
		const target = {
			addEventListener: (type: string, fn: () => void) => void listeners.set(type, fn),
			removeEventListener: (type: string) => void listeners.delete(type)
		};
		vi.stubGlobal('window', target);
		vi.stubGlobal('document', { ...target, visibilityState: 'visible' });
		try {
			const source = new LinkedProposals(account([OTHER]) as never, 100);
			const stop = source.watch();
			fake.reads = 0;
			await vi.advanceTimersByTimeAsync(LINKED_POLL_MS + 1);
			expect(fake.reads).toBe(1);
			listeners.get('visibilitychange')?.();
			await vi.advanceTimersByTimeAsync(0);
			expect(fake.reads).toBe(2);
			stop();
			expect(listeners.size).toBe(0);
			await vi.advanceTimersByTimeAsync(LINKED_POLL_MS * 3);
			expect(fake.reads).toBe(2);
		} finally {
			vi.useRealTimers();
			vi.unstubAllGlobals();
		}
	});

	it('a network that does not answer is named, the last read kept', async () => {
		const source = new LinkedProposals(account([OTHER]) as never, 100);
		await source.refresh();
		expect(source.chain(100)).toBeDefined();
		fake.chainFails = true;
		await source.refresh();
		expect(source.unreadable).toEqual([100]);
		expect(source.chain(100)).toBeDefined();
		fake.chainFails = false;
		await source.refresh();
		expect(source.unreadable).toEqual([]);
	});
});
