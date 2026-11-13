/**
 * r4a-1: every member change reads in full, alone or inside a batch — who it
 * adds, removes or replaces, the rule before and after on its network, and
 * the caution — from the core's own reading of Safe's owner-management
 * functions, against the network's members now.
 */
import { describe, expect, it } from 'vitest';
import { resolveMultisigMessages } from '$lib/i18n/engine.server';
import { actionTitle, changeLines, shortenAddress } from './display';
import { callsTx, readMemberChanges, reviewTx, type Action } from './kernel';

const m = resolveMultisigMessages('en');
const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const A = '0x1111111111111111111111111111111111111111';
const B = '0x2222222222222222222222222222222222222222';
const C = '0x3333333333333333333333333333333333333333';
const D = '0x4444444444444444444444444444444444444444';
const SENTINEL = '0x0000000000000000000000000000000000000001';
const members = { owners: [A, B, C], threshold: 2 };
const who = (address: string) =>
	address === D ? `Dana (${shortenAddress(D)})` : shortenAddress(address);
/** r12-8: the names a title gives (the core's `names.change` words them). */
const label = (address: string) => (address === D ? 'Dana' : null);

const word = (hex: string) => hex.replace(/^0x/, '').toLowerCase().padStart(64, '0');
const num = (n: number) => n.toString(16).padStart(64, '0');
const ADD = (owner: string, t: number) => `0x0d582f13${word(owner)}${num(t)}`;
const REMOVE = (prev: string, owner: string, t: number) =>
	`0xf8dc5dd9${word(prev)}${word(owner)}${num(t)}`;
const SWAP = (prev: string, old: string, next: string) =>
	`0xe318b52b${word(prev)}${word(old)}${word(next)}`;
const THRESHOLD = (t: number) => `0x694e80c3${num(t)}`;

/** What the core reads calls into the multisig itself as (one, or a MultiSend batch). */
function action(...datas: string[]): Action {
	const tx = callsTx(
		datas.map((data) => ({ to: SAFE, value: '0', data })),
		'0x0'
	);
	const read = reviewTx(5042002, SAFE, tx).action;
	if (!read) throw new Error('not signed');
	return read;
}

function read(...datas: string[]) {
	const reading = readMemberChanges(action(...datas), members);
	if (!reading) throw new Error('no member change');
	return changeLines(m, reading, 'Arc Testnet', who);
}

describe('a member change, alone', () => {
	it('adding one names who, the rule before and after, and the network', async () => {
		const lines = read(ADD(D, 2));
		expect(lines.changes).toEqual([`Add Dana (${shortenAddress(D)})`]);
		expect(lines.rule).toBe('Approvals: 2 of 3 → 2 of 4');
		expect(lines.caution).toBe(
			'This changes who controls this multisig on Arc Testnet. Approve only if every member expects it.'
		);
		expect(lines.doesNotFit).toBeNull();
		// The Home row's title names the member, never only "Add a member".
		expect(await actionTitle(m, 5042002, action(ADD(D, 2)), 0n, label)).toBe(
			`Add Dana (${shortenAddress(D)})`
		);
	});

	it('removing one', async () => {
		const lines = read(REMOVE(A, B, 1));
		expect(lines.changes).toEqual([`Remove ${shortenAddress(B)}`]);
		expect(lines.rule).toBe('Approvals: 2 of 3 → 1 of 2');
		expect(await actionTitle(m, 5042002, action(REMOVE(A, B, 1)), 0n, label)).toBe(
			`Remove ${shortenAddress(B)}`
		);
	});

	it('replacing one', async () => {
		const lines = read(SWAP(SENTINEL, A, D));
		expect(lines.changes).toEqual([
			`Replace ${shortenAddress(A)} with Dana (${shortenAddress(D)})`
		]);
		expect(lines.rule).toBe('Approvals: 2 of 3 → 2 of 3');
		expect(await actionTitle(m, 5042002, action(SWAP(SENTINEL, A, D)), 0n, label)).toBe(
			`Replace ${shortenAddress(A)} with Dana (${shortenAddress(D)})`
		);
	});

	it('changing the threshold', () => {
		const lines = read(THRESHOLD(3));
		expect(lines.changes).toEqual(['Change the approvals required']);
		expect(lines.rule).toBe('Approvals: 2 of 3 → 3 of 3');
	});

	it('one the Safe would refuse says executing it would fail', () => {
		expect(read(ADD(A, 2)).doesNotFit).toBe(
			'This doesn’t fit the members it has on Arc Testnet now: executing it would fail.'
		);
		expect(read(THRESHOLD(4)).doesNotFit).not.toBeNull();
	});
});

describe('member changes inside a batch', () => {
	it('reads each one, numbered by its call, against what the ones before left', () => {
		const lines = read(ADD(D, 2), REMOVE(D, A, 3), THRESHOLD(2), SWAP(SENTINEL, B, A));
		expect(lines.changes).toEqual([
			`#1 · Add Dana (${shortenAddress(D)})`,
			`#2 · Remove ${shortenAddress(A)}`,
			'#3 · Change the approvals required',
			`#4 · Replace ${shortenAddress(B)} with ${shortenAddress(A)}`
		]);
		expect(lines.rule).toBe('Approvals: 2 of 3 → 2 of 3');
		expect(lines.doesNotFit).toBeNull();
	});

	it('keeps only the member changes of a batch that also pays', () => {
		const tx = callsTx(
			[
				{ to: B, value: '0x5', data: '0x' },
				{ to: SAFE, value: '0', data: ADD(D, 3) }
			],
			'0x0'
		);
		const mixed = reviewTx(5042002, SAFE, tx).action!;
		const lines = changeLines(m, readMemberChanges(mixed, members)!, 'Arc Testnet', who);
		expect(lines.changes).toEqual([`#2 · Add Dana (${shortenAddress(D)})`]);
		expect(lines.rule).toBe('Approvals: 2 of 3 → 3 of 4');
	});

	it('a transaction that changes no member reads none', () => {
		const transfer = reviewTx(
			5042002,
			SAFE,
			callsTx([{ to: B, value: '0x5', data: '0x' }], '0x0')
		).action!;
		expect(readMemberChanges(transfer, members)).toBeNull();
	});
});
