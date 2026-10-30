/**
 * r4D-5: members show by name on every device that keeps a multisig with
 * their labels — after this device's own account names, before the passkey
 * index's — and an invite's labels never overwrite what this device holds.
 * The labels are kept by the core's accounts machine (`name_members`), read
 * through the store's facade.
 */
import '$lib/i18n/wasm-init.server';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';
import {
	startAccounts,
	stopAccounts,
	type AccountsMachineForTests
} from '../shell/accounts.testing';

const BOB = '0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC';
const MINE = '0x70997970C51812dc3A010C7d01b50e0d17dc79C8';
const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';

vi.mock('$lib/services/public-key-index', () => ({
	queryWalletName: async () => null,
	lookupWalletName: async () => null
}));
vi.mock('./display', () => ({
	localName: (a: string) => (a.toLowerCase() === MINE.toLowerCase() ? 'My account' : null),
	shortenAddress: (a: string) => `${a.slice(0, 8)}…`
}));

const { multisig } = await import('./store.svelte');
const { names, keptMember } = await import('./names.svelte');

const row = {
	address: SAFE,
	genesis: { factory: '0x01', singleton: '0x02', saltNonce: '0x1', initializer: '0x' },
	owners: [MINE, BOB],
	threshold: 2,
	recorded: true,
	chainIds: [100],
	addedMs: 1
};

let machine: AccountsMachineForTests;
beforeAll(async () => {
	({ machine } = await startAccounts({ seed: { 'vela.multisigAccounts': JSON.stringify([row]) } }));
});
afterAll(() => stopAccounts(machine));

describe("members' names (r4D-5)", () => {
	it('shows a member by the label kept with a multisig here, with the kind', () => {
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

/**
 * r5reg-16: a name the person types in Settings → Add member is their word
 * now — it replaces what was kept (a link's label), is marked as typed, and
 * is what the member-add review and its title show.
 */
describe('a name typed when a member is added (r5reg-16)', () => {
	it('replaces what was kept, and is marked as typed', async () => {
		const { typedName } = await import('./typed-name');
		multisig.nameMembers(
			SAFE,
			{ [BOB]: { name: 'Bob from work', kind: 'passkey' } },
			{ typed: true }
		);
		const account = multisig.find(SAFE);
		expect(account?.members?.[BOB.toLowerCase()]).toEqual({
			name: 'Bob from work',
			kind: 'passkey',
			typed: true
		});
		expect(typedName(account, BOB)).toBe('Bob from work');
		// A link's label later never overwrites it.
		multisig.nameMembers(SAFE, { [BOB]: { name: 'Robert', kind: 'wallet' } });
		expect(typedName(multisig.find(SAFE), BOB)).toBe('Bob from work');
		// Kept from a link only: no typed name.
		expect(typedName(multisig.find(SAFE), MINE)).toBeNull();
	});
});
