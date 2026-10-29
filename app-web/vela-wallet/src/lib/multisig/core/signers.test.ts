/**
 * r4-9: how a member signs decides what the sheet offers them — an outside
 * wallet only for a plain account, never for a Vela (smart) account.
 */
import { describe, expect, it, vi } from 'vitest';

const HERE = '0x00000000000000000000000000000000000000aa';
const DEPLOYED = '0x00000000000000000000000000000000000000d1';
const INDEXED = '0x00000000000000000000000000000000000000e1';
const PLAIN = '0x00000000000000000000000000000000000000f1';
const UNREAD = '0x00000000000000000000000000000000000000f2';

const index = vi.hoisted(() => ({ answered: new Set<string>() }));

vi.mock('./owners', () => ({
	localOwners: (owners: string[]) =>
		owners.filter((o) => o === HERE).map((address) => ({ address }))
}));
vi.mock('./names.svelte', () => ({
	names: {
		isVela: (a: string) => a === INDEXED,
		answered: (a: string) => index.answered.has(a)
	}
}));
vi.mock('./chain', () => ({
	hasCode: async (_chain: number, a: string) => a === DEPLOYED
}));

vi.mock('$lib/services/public-key-index', () => ({
	queryWalletName: async (a: string) => (a === INDEXED ? { name: 'Vela', publicKey: '0x04' } : null)
}));

const { signers, plainAccounts } = await import('./signers.svelte');

describe('who an outside wallet page may sign for (r4C-10)', () => {
	it('only plain accounts: never this device’s, a deployed smart account, or a Vela account', async () => {
		expect(await plainAccounts(1, [HERE, DEPLOYED, INDEXED, PLAIN])).toEqual([PLAIN]);
	});
});

describe('how a member signs', () => {
	it('this device’s account signs here', () => {
		expect(signers.kindOf(1, HERE)).toBe('here');
	});

	it('the passkey index knows it: a Vela account, in Vela', () => {
		expect(signers.kindOf(1, INDEXED)).toBe('vela');
	});

	it('code at its address there: a smart account, never an outside wallet', async () => {
		expect(signers.kindOf(1, DEPLOYED), 'nothing offered on a guess').toBeNull();
		await vi.waitFor(() => expect(signers.kindOf(1, DEPLOYED)).toBe('vela'));
	});

	it('no code, and the index said it is no Vela account: a plain wallet', async () => {
		index.answered.add(PLAIN);
		signers.kindOf(1, PLAIN);
		await vi.waitFor(() => expect(signers.kindOf(1, PLAIN)).toBe('wallet'));
	});

	it('no code, the index not answered yet: still unknown', async () => {
		signers.kindOf(1, UNREAD);
		await new Promise((r) => setTimeout(r, 0));
		expect(signers.kindOf(1, UNREAD)).toBeNull();
	});
});
