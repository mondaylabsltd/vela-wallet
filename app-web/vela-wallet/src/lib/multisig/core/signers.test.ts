/**
 * r4-9, r4a-9: how a member signs decides what the sheet offers them — an
 * outside wallet only for a plain account, never for a Vela (smart) account:
 * not before the passkey index answers, not after it approved with its
 * passkey, not for one this device keeps as a Vela account.
 */
import { describe, expect, it, vi } from 'vitest';
import type { ApprovalRow } from './proposals.svelte';

const HERE = '0x00000000000000000000000000000000000000aa';
const DEPLOYED = '0x00000000000000000000000000000000000000d1';
const INDEXED = '0x00000000000000000000000000000000000000e1';
const PLAIN = '0x00000000000000000000000000000000000000f1';
const UNREAD = '0x00000000000000000000000000000000000000f2';
/** Carol: a Vela account added by address, not deployed there, the index holding none. */
const CAROL = '0x00000000000000000000000000000000000000c1';
const KEPT_VELA = '0x00000000000000000000000000000000000000c2';
const KEPT_WALLET = '0x00000000000000000000000000000000000000c3';
const SIGNED_WALLET = '0x00000000000000000000000000000000000000c4';
/** r7: deployed on network 2 only, kept here as an outside wallet. */
const KEPT_WALLET_DEPLOYED = '0x00000000000000000000000000000000000000c5';
/** r7: no code on network 1; network 3 never answers. */
const UNREACHABLE = '0x00000000000000000000000000000000000000c6';
/** r7: no code anywhere, the index answered none. */
const FRESH = '0x00000000000000000000000000000000000000c7';
/** r7: another Safe as a member — code on network 1, no index record. */
const OTHER_SAFE = '0x00000000000000000000000000000000000000c8';
/** r7: kept here as a Vela account, with code on network 1. */
const KEPT_VELA_DEPLOYED = '0x00000000000000000000000000000000000000c9';
/** r7: kept here as a smart contract account (an invite's `c`), no code read. */
const KEPT_CONTRACT = '0x00000000000000000000000000000000000000ca';

const index = vi.hoisted(() => ({ answered: new Set<string>() }));

vi.mock('./owners', () => ({
	localOwners: (owners: string[]) =>
		owners.filter((o) => o === HERE).map((address) => ({ address }))
}));
vi.mock('./names.svelte', () => ({
	names: {
		isVela: (a: string) => a === INDEXED,
		answered: (a: string) => index.answered.has(a)
	},
	keptMember: (a: string) =>
		a === KEPT_VELA || a === KEPT_VELA_DEPLOYED
			? { kind: 'passkey' }
			: a === KEPT_WALLET || a === KEPT_WALLET_DEPLOYED
				? { kind: 'wallet' }
				: a === KEPT_CONTRACT
					? { kind: 'contract' }
					: {}
}));
vi.mock('./chain', () => ({
	hasContractCode: async (chain: number, a: string) => {
		if (a === UNREACHABLE && chain === 3) throw new Error('down');
		if (a === KEPT_CONTRACT) return new Promise<boolean>(() => {});
		return (
			a === DEPLOYED ||
			(a === KEPT_WALLET_DEPLOYED && chain === 2) ||
			((a === OTHER_SAFE || a === KEPT_VELA_DEPLOYED) && chain === 1)
		);
	}
}));

vi.mock('$lib/services/public-key-index', () => ({
	lookupWalletName: async (a: string) =>
		a === INDEXED ? { name: 'Vela', publicKey: '0x04' } : null
}));

const { signers, plainAccounts, approvedWith } = await import('./signers.svelte');

const passkey = (owner: string, counts = true): ApprovalRow => ({
	owner,
	counts,
	createdMs: 0,
	approval: {
		owner,
		ownerGenesis: {} as never,
		signer: '0x',
		authenticatorData: '0x',
		clientDataFields: '',
		r: '0x',
		s: '0x'
	}
});
const ecdsa = (owner: string): ApprovalRow => ({
	owner,
	counts: true,
	createdMs: 0,
	approval: { owner, signature: '0x' }
});

describe('who an outside wallet page may sign for (r4C-10)', () => {
	it('only plain accounts: never this device’s, a deployed smart account, or a Vela account', async () => {
		expect(await plainAccounts(1, [HERE, DEPLOYED, INDEXED, PLAIN])).toEqual([PLAIN]);
	});

	it('never one that approved with a passkey, or this device keeps as a Vela account (r4a-9)', async () => {
		const items = [{ approvals: [passkey(CAROL)] }];
		expect(await plainAccounts(1, [CAROL, KEPT_VELA, PLAIN], items)).toEqual([PLAIN]);
	});
});

describe('how a member signs', () => {
	it('this device’s account signs here', () => {
		expect(signers.kindOf(1, HERE)).toBe('here');
	});

	it('the passkey index knows it: a Vela account, in Vela', () => {
		expect(signers.kindOf(1, INDEXED)).toBe('vela');
	});

	it('code at its address there and no index record: a smart contract account, never an outside wallet', async () => {
		expect(signers.kindOf(1, DEPLOYED), 'nothing offered on a guess').toBeNull();
		// r7: code alone is no Vela account — not before the index answers…
		await new Promise((r) => setTimeout(r, 0));
		expect(signers.kindOf(1, DEPLOYED)).toBeNull();
		// …and once it answered none, a smart contract account.
		index.answered.add(DEPLOYED);
		expect(signers.kindOf(1, DEPLOYED)).toBe('contract');
	});

	it('no code, the index not answered yet: still unknown — no outside wallet offered', async () => {
		signers.kindOf(1, UNREAD);
		await new Promise((r) => setTimeout(r, 0));
		expect(signers.kindOf(1, UNREAD)).toBeNull();
	});
});

/**
 * r4a-9 / r4b-10 / r4D-6: Carol, a Vela member added by address — not set
 * up on that network, the index holding no name for her — was offered
 * "Approve with another wallet" before the index answered and after she
 * approved with her passkey.
 */
describe('a Vela member added by address (r4a-9)', () => {
	it('approved anything with her passkey: a Vela account, on every proposal', async () => {
		index.answered.add(CAROL);
		const items = [{ approvals: [passkey(CAROL)] }, { approvals: [] }];
		expect(signers.kindOf(1, CAROL, items)).toBe('vela');
		// Before the code read answers too.
		expect(signers.kindOf(2, CAROL, items)).toBe('vela');
	});

	it('only a counted approval is evidence', () => {
		expect(approvedWith([{ approvals: [passkey(CAROL, false)] }], CAROL)).toBeNull();
		expect(approvedWith([{ approvals: [passkey(CAROL)] }], CAROL)).toBe('passkey');
		expect(approvedWith([{ approvals: [ecdsa(SIGNED_WALLET)] }], SIGNED_WALLET)).toBe('wallet');
	});

	it('kept here as a Vela account (typed when added, or the invite’s word): a Vela account', () => {
		expect(signers.kindOf(1, KEPT_VELA)).toBe('vela');
	});

	it('nothing says, no code, the index holds none: unsure — the Vela way first, never the outside one alone', async () => {
		signers.kindOf(1, CAROL);
		await vi.waitFor(() => expect(signers.kindOf(1, CAROL)).toBe('unsure'));
	});

	it('a wallet’s own signature, or kept as an outside wallet, with no code: a plain wallet', async () => {
		index.answered.add(SIGNED_WALLET);
		const items = [{ approvals: [ecdsa(SIGNED_WALLET)] }];
		signers.kindOf(1, SIGNED_WALLET, items);
		await vi.waitFor(() => expect(signers.kindOf(1, SIGNED_WALLET, items)).toBe('wallet'));
		signers.kindOf(1, KEPT_WALLET);
		await vi.waitFor(() => expect(signers.kindOf(1, KEPT_WALLET)).toBe('wallet'));
	});

	it('no code, the index answered none, nothing kept: unsure, as before it was "wallet"', async () => {
		index.answered.add(PLAIN);
		signers.kindOf(1, PLAIN);
		await vi.waitFor(() => expect(signers.kindOf(1, PLAIN)).toBe('unsure'));
	});
});

/**
 * r7: a member's kind is declared, then verified. The add screens pre-fill
 * it from what the wallet can tell (`evidence`), and evidence beats the
 * label wherever it is read (`kindOf`).
 */
describe('what the wallet can tell of a member being added (r7)', () => {
	it('an account here, or one the passkey index holds: a Vela account', () => {
		expect(signers.evidence([1], HERE)).toBe('vela');
		expect(signers.evidence([1], INDEXED)).toBe('vela');
	});

	it('another Safe as a member — code on one of the networks, no index record: a smart contract account', async () => {
		signers.evidence([1, 2], OTHER_SAFE);
		await new Promise((r) => setTimeout(r, 0));
		// Code alone never says "Vela account"; the index not answered, it asks.
		expect(signers.evidence([1, 2], OTHER_SAFE)).toBeNull();
		index.answered.add(OTHER_SAFE);
		expect(signers.evidence([1, 2], OTHER_SAFE)).toBe('contract');
		// On the proposal sheet too: never offered an outside wallet, nor the Vela link.
		expect(signers.kindOf(2, OTHER_SAFE, [], [1, 2])).toBe('contract');
		expect(await plainAccounts(1, [OTHER_SAFE])).toEqual([]);
	});

	it('no code anywhere and the index holds none: likely an outside wallet', async () => {
		expect(signers.evidence([1, 2], FRESH), 'read first, never guessed').toBeNull();
		index.answered.add(FRESH);
		await vi.waitFor(() => expect(signers.evidence([1, 2], FRESH)).toBe('wallet'));
	});

	it('a network that does not answer, or an index that has not: it can’t tell — asked', async () => {
		index.answered.add(UNREACHABLE);
		signers.evidence([1, 3], UNREACHABLE);
		await new Promise((r) => setTimeout(r, 0));
		expect(signers.evidence([1, 3], UNREACHABLE)).toBeNull();
		// No networks to look on is no answer either.
		expect(signers.evidence([], FRESH)).toBeNull();
	});
});

describe('evidence beats the label (r7)', () => {
	it('kept as an outside wallet, but code on another of its networks: a smart contract account', async () => {
		index.answered.add(KEPT_WALLET_DEPLOYED);
		signers.kindOf(1, KEPT_WALLET_DEPLOYED, [], [1, 2]);
		await vi.waitFor(() =>
			expect(signers.kindOf(1, KEPT_WALLET_DEPLOYED, [], [1, 2])).toBe('contract')
		);
		// Without its other networks, the label is all there is.
		expect(signers.kindOf(1, KEPT_WALLET_DEPLOYED)).toBe('wallet');
	});

	it('kept as a Vela account with code: stands until the index answers none, then a smart contract account', async () => {
		signers.kindOf(1, KEPT_VELA_DEPLOYED);
		await vi.waitFor(() => expect(signers.kindOf(1, KEPT_VELA_DEPLOYED)).toBe('vela'));
		index.answered.add(KEPT_VELA_DEPLOYED);
		expect(signers.kindOf(1, KEPT_VELA_DEPLOYED)).toBe('contract');
	});

	it('kept as a smart contract account (an invite’s `c`): one, before any code is read', () => {
		expect(signers.kindOf(1, KEPT_CONTRACT)).toBe('contract');
	});

	it('kept as an outside wallet, but it approved with its passkey: a Vela account', () => {
		expect(signers.kindOf(1, KEPT_WALLET, [{ approvals: [passkey(KEPT_WALLET)] }])).toBe('vela');
	});
});
