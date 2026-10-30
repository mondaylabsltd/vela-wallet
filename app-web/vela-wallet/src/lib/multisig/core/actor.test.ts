/**
 * r5reg-5: a device holding two members of a multisig never picks who
 * proposes silently — the person's pick on the review, else the member used
 * here last, else the first; and r4a-18's facts for a send's confirm: a
 * multisig here, or a member of the multisig sending, is the person's own.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

const SAFE = '0x00000000000000000000000000000000000000cc';
const ALICE = { address: '0x00000000000000000000000000000000000000a1', name: 'Alice' };
const CAROL = { address: '0x00000000000000000000000000000000000000c1', name: 'Carol' };

const kept = vi.hoisted(() => new Map<string, string>());
vi.stubGlobal('localStorage', {
	getItem: (k: string) => kept.get(k) ?? null,
	setItem: (k: string, v: string) => void kept.set(k, v),
	removeItem: (k: string) => void kept.delete(k)
});
vi.mock('./store.svelte', () => ({
	multisig: { accounts: [{ address: SAFE, label: 'R5 Team', genesis: {} }] }
}));

const { defaultActor, lastActor, proposerOf } = await import('./actor');
const { multisigKnownName, ownOrMember } = await import('./known');

beforeEach(() => kept.clear());

describe('who proposes (r5reg-5)', () => {
	it('the person’s pick on the review', () => {
		expect(proposerOf([ALICE, CAROL], CAROL.address.toUpperCase().replace('0X', '0x'), SAFE)).toBe(
			CAROL
		);
	});

	it('else the member used here last, else the first', () => {
		expect(proposerOf([ALICE, CAROL], null, SAFE)).toBe(ALICE);
		lastActor.note(SAFE, CAROL.address);
		expect(proposerOf([ALICE, CAROL], null, SAFE)).toBe(CAROL);
		expect(defaultActor([ALICE, CAROL], SAFE)).toBe(CAROL);
		// Remembered per multisig.
		expect(defaultActor([ALICE, CAROL], '0x00000000000000000000000000000000000000dd')).toBe(ALICE);
	});

	it('a pick that is not a member here is never used', () => {
		expect(proposerOf([ALICE], CAROL.address, SAFE)).toBe(ALICE);
		expect(proposerOf([], null, SAFE)).toBeUndefined();
	});
});

describe('a send’s confirm: whose an address is (r4a-18)', () => {
	const MEMBER = '0x00000000000000000000000000000000000000b0';
	it('a multisig here is the person’s own, named as it is here', () => {
		expect(ownOrMember(SAFE, [])).toBe(true);
		expect(multisigKnownName(SAFE, null)).toBe('R5 Team');
	});

	it('a member of the multisig sending is not a stranger; named as kept', () => {
		expect(ownOrMember(MEMBER, [MEMBER])).toBe(true);
		expect(multisigKnownName(MEMBER, { members: { [MEMBER]: { name: 'Bob' } } })).toBe('Bob');
	});

	it('anybody else is', () => {
		expect(ownOrMember('0x00000000000000000000000000000000000000ee', [MEMBER])).toBe(false);
		expect(multisigKnownName('0x00000000000000000000000000000000000000ee', null)).toBeNull();
	});
});
