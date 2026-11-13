/**
 * r4D-5, r8-25: members show by ONE name on every device that keeps a
 * multisig with their labels — the kept label (an invite's words), then the
 * passkey index's, then this device's own account name — and an invite's
 * labels never overwrite what this device holds.
 * The labels are kept by the core's accounts machine (`name_members`), read
 * through the store's facade (`known_members`); the order is the core's
 * (`names.label`, its cases in `multisig_rules/names.rs`).
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

/** The multi-key wallet the index names "Interleave" (§9.1): the one account it answers for. */
const INTERLEAVE = '0x88cCA0EeDbF2C4426110bbFc998F048689266894';
vi.mock('$lib/services/public-key-index', () => ({
	queryWalletName: async () => null,
	lookupWalletName: async (a: string) =>
		a.toLowerCase() === '0x88cca0eedbf2c4426110bbfc998f048689266894' ? { name: 'Interleave' } : null
}));
vi.mock('./display', () => ({
	localName: (a: string) => (a.toLowerCase() === MINE.toLowerCase() ? 'My account' : null),
	memberLabel: (a: string) => a
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
		// The core's short form (EIP-55) until a name answers.
		expect(names.label(BOB)).toBe('0x3C44Cd…4293BC');
		multisig.send({
			type: 'name_members',
			address: SAFE,
			members: { [BOB]: { name: 'Bob', kind: 'wallet' } },
			typed: false
		});
		expect(names.label(BOB)).toBe('Bob');
		expect(names.kindOf(BOB)).toBe('wallet');
		expect(keptMember(BOB.toLowerCase())).toEqual({ name: 'Bob', kind: 'wallet' });
	});

	it("r8-25: the multisig's shared label over this device's own nickname; the kind stays its own", () => {
		multisig.send({
			type: 'name_members',
			address: SAFE,
			members: { [MINE]: { name: 'Dana', kind: 'wallet' } },
			typed: false
		});
		expect(names.label(MINE)).toBe('Dana');
		expect(names.kindOf(MINE)).toBe('passkey');
	});

	it('fills only what is empty: an invite never renames a member this device named', () => {
		multisig.send({
			type: 'name_members',
			address: SAFE,
			members: { [BOB.toLowerCase()]: { name: 'Robert', kind: 'passkey' } },
			typed: false
		});
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
		multisig.send({
			type: 'name_members',
			address: SAFE,
			members: { [BOB]: { name: 'Bob from work', kind: 'passkey' } },
			typed: true
		});
		const account = multisig.find(SAFE);
		expect(account?.members?.[BOB.toLowerCase()]).toEqual({
			name: 'Bob from work',
			kind: 'passkey',
			typed: true
		});
		expect(typedName(account, BOB)).toBe('Bob from work');
		// A link's label later never overwrites it.
		multisig.send({
			type: 'name_members',
			address: SAFE,
			members: { [BOB]: { name: 'Robert', kind: 'wallet' } },
			typed: false
		});
		expect(typedName(multisig.find(SAFE), BOB)).toBe('Bob from work');
		// Kept from a link only: no typed name.
		expect(typedName(multisig.find(SAFE), MINE)).toBeNull();
	});
});

/**
 * Add member's Name field is prefilled from the index (`names.published`):
 * the name the account published for itself — never one this device holds.
 */
describe('the name an account published (Add member)', () => {
	it('is the passkey index’s answer, once it answers', async () => {
		expect(names.published(INTERLEAVE)).toBeNull();
		await vi.waitFor(() => expect(names.published(INTERLEAVE)).toBe('Interleave'));
	});

	it('is never a name only this device holds: its own account’s, or a kept label', async () => {
		// Both are named here (this device's account; a label kept with the multisig)…
		expect(keptMember(MINE).name).toBeTruthy();
		expect(keptMember(BOB).name).toBeTruthy();
		// …and neither published one.
		await new Promise((r) => setTimeout(r, 0));
		expect(names.published(MINE)).toBeNull();
		expect(names.published(BOB)).toBeNull();
	});
});
