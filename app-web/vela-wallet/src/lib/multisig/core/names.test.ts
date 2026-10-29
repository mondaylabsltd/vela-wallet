/**
 * r4D-5: members show by name on every device that keeps a multisig with
 * their labels — after this device's own account names, before the passkey
 * index's — and an invite's labels never overwrite what this device holds.
 */
import { describe, expect, it, vi } from 'vitest';

const BOB = '0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC';
const MINE = '0x70997970C51812dc3A010C7d01b50e0d17dc79C8';
const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';

const store = vi.hoisted(() => ({ written: [] as string[] }));
vi.stubGlobal('localStorage', {
	getItem: () => null,
	setItem: (_k: string, v: string) => void store.written.push(v),
	removeItem: () => {}
});
vi.stubGlobal('window', { addEventListener: () => {} });
vi.mock('$lib/services/public-key-index', () => ({ queryWalletName: async () => null }));
vi.mock('./display', () => ({
	localName: (a: string) => (a.toLowerCase() === MINE.toLowerCase() ? 'My account' : null),
	shortenAddress: (a: string) => `${a.slice(0, 8)}…`
}));
vi.mock('./kernel', () => ({
	sameAddress: (a: string | null, b: string | null) =>
		!!a && !!b && a.toLowerCase() === b.toLowerCase(),
	genesisAddress: () => SAFE,
	genesisMembers: () => ({ owners: [MINE, BOB], threshold: 2 })
}));

const { multisig } = await import('./store.svelte');
const { names, keptMember } = await import('./names.svelte');

describe("members' names (r4D-5)", () => {
	it('shows a member by the label kept with a multisig here, with the kind', () => {
		multisig.add(SAFE, {} as never, true, [100]);
		expect(names.label(BOB)).toBe('0x3C44Cd…');
		multisig.nameMembers(SAFE, { [BOB]: { name: 'Bob', kind: 'wallet' } });
		expect(names.label(BOB)).toBe('Bob');
		expect(names.kindOf(BOB)).toBe('wallet');
		expect(keptMember(BOB.toLowerCase())).toEqual({ name: 'Bob', kind: 'wallet' });
	});

	it("keeps this device's own name for its accounts over any label", () => {
		multisig.nameMembers(SAFE, { [MINE]: { name: 'Not me', kind: 'wallet' } });
		expect(names.label(MINE)).toBe('My account');
		expect(names.kindOf(MINE)).toBe('passkey');
	});

	it('fills only what is empty: an invite never renames a member this device named', () => {
		multisig.nameMembers(SAFE, { [BOB.toLowerCase()]: { name: 'Robert', kind: 'passkey' } });
		expect(names.label(BOB)).toBe('Bob');
		expect(names.kindOf(BOB)).toBe('wallet');
	});
});
