/**
 * Changing the members, network by network, asked of the real core: one
 * action is one proposal per network the multisig is deployed on, each built
 * against THAT network's members (its own list: the previous owner comes
 * from it), refused there with its reason when it cannot be made, and a
 * network it is not deployed on is only said, never offered. The proposals
 * of one action read as one change with each network's progress.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import type { ChainMembers } from './chain';
import { reviewTx, type Action } from './kernel';
import {
	changeKey,
	changesMembers,
	deployedMembers,
	memberChangeGroups,
	memberStateOf,
	planMemberChange,
	progressLine
} from './member-changes';

const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const A = '0x1111111111111111111111111111111111111111';
const B = '0x2222222222222222222222222222222222222222';
const C = '0x3333333333333333333333333333333333333333';
const D = '0x4444444444444444444444444444444444444444';
const ARC = 5042002;
const BASE = 8453;
const GNOSIS = 100;

const members = (owners: string[], threshold: number, deployed = true): ChainMembers => ({
	deployed,
	owners,
	threshold,
	nonce: 3n
});

const waiting = (action: Action) => ({
	status: 'pending' as const,
	review: { safeTxHash: '0x', nonce: '3', action, refusal: null }
});

/** Arc 2 of 3 (A, B, C); Base already changed: 2 of 3 (D, A, B); Gnosis not deployed. */
const chains = [
	{ chainId: ARC, members: members([A, B, C], 2), items: [] },
	{ chainId: BASE, members: members([D, A, B], 2), items: [] },
	{ chainId: GNOSIS, members: members([A, B, C], 2, false), items: [] }
];

describe('one action, one proposal per deployed network', () => {
	it("builds each network's change against that network's own list", () => {
		const plan = planMemberChange(SAFE, chains, { kind: 'remove', owner: B });
		expect(plan.ready).toEqual([ARC, BASE]);
		expect(plan.undeployed).toEqual([GNOSIS]);
		const [arc, base] = plan.networks;
		if (arc.verdict.kind !== 'ready' || base.verdict.kind !== 'ready') throw new Error('refused');
		// removeOwner(prevOwner, owner, threshold): B follows A on Arc …
		expect(arc.verdict.tx.data.toLowerCase()).toBe(
			'0xf8dc5dd9' +
				A.slice(2).padStart(64, '0') +
				B.slice(2).padStart(64, '0') +
				'2'.padStart(64, '0')
		);
		// … and on Base too, but A follows D there: the previous owner is each list's.
		expect(base.verdict.tx.data.toLowerCase().slice(0, 74)).toBe(
			'0xf8dc5dd9' + A.slice(2).padStart(64, '0')
		);
		expect(arc.verdict.before.owners).toHaveLength(3);
		expect(arc.verdict.after.owners).toHaveLength(2);
		// The review every member reads it with says what it is.
		expect(reviewTx(ARC, SAFE, arc.verdict.tx).action).toMatchObject({ kind: 'removeOwner' });
	});

	it('says why a network refuses, in its own terms, and proposes only where it can', () => {
		// C is a member on Arc, not on Base (replaced there already).
		const plan = planMemberChange(SAFE, chains, { kind: 'replace', oldOwner: C, newOwner: D });
		expect(plan.ready).toEqual([ARC]);
		expect(plan.networks[1].verdict).toMatchObject({
			kind: 'refused',
			refusal: { kind: 'notMember' }
		});
		expect(
			planMemberChange(SAFE, chains, { kind: 'add', owner: SAFE }).networks.map(
				(n) => n.verdict.kind === 'refused' && n.verdict.refusal.kind
			)
		).toEqual(['theMultisig', 'theMultisig']);
		expect(
			planMemberChange(SAFE, chains, { kind: 'threshold', threshold: 4 }).networks[0].verdict
		).toMatchObject({ refusal: { kind: 'thresholdTooHigh', threshold: 4, owners: 3 } });
		expect(
			planMemberChange(SAFE, chains, {
				kind: 'add',
				owner: '0x0000000000000000000000000000000000000000'
			}).ready
		).toEqual([]);
	});

	it('refuses a second member change while one waits on that network', () => {
		const busy = [
			{ ...chains[0], items: [waiting({ kind: 'changeThreshold', threshold: '3' })] },
			chains[1]
		];
		expect(memberStateOf(busy[0]).changeWaiting).toBe(true);
		const plan = planMemberChange(SAFE, busy, { kind: 'add', owner: C });
		expect(plan.networks[0].verdict).toMatchObject({ refusal: { kind: 'changeWaiting' } });
		// Base has nothing waiting, and C is not a member there.
		expect(plan.ready).toEqual([BASE]);
		// A transfer waiting is no member change.
		expect(
			memberStateOf({
				members: chains[0].members,
				items: [waiting({ kind: 'transfer', token: null, to: D, amount: '1' })]
			}).changeWaiting
		).toBe(false);
	});

	it('offers nothing where it is not deployed (a change there would deploy it)', () => {
		const plan = planMemberChange(SAFE, [chains[2]], { kind: 'threshold', threshold: 3 });
		expect(plan.networks).toEqual([]);
		expect(plan.ready).toEqual([]);
		expect(plan.undeployed).toEqual([GNOSIS]);
	});

	it('lists every deployed network’s members once, the first network’s order first', () => {
		expect(deployedMembers(chains)).toEqual([A, B, C, D]);
	});
});

describe('the changes waiting, as one change per action', () => {
	it('groups the networks of one change and reads their progress', () => {
		const item = (chainId: number, action: Action, approved: number) => ({
			chainId,
			safeTxHash: `0x${chainId}`,
			approved,
			threshold: 2,
			status: 'pending' as const,
			review: { safeTxHash: `0x${chainId}`, nonce: '3', action, refusal: null }
		});
		const add = { kind: 'addOwner', owner: D, threshold: '2' } as const;
		const groups = memberChangeGroups([
			item(ARC, add, 1),
			item(BASE, { ...add, owner: D.toUpperCase().replace('0X', '0x') }, 0),
			item(ARC, { kind: 'transfer', token: null, to: D, amount: '5' }, 0),
			{ ...item(GNOSIS, add, 2), status: 'done' as const }
		]);
		expect(groups).toHaveLength(1);
		expect(progressLine(groups[0], (id) => (id === ARC ? 'Arc' : 'Base'))).toBe(
			'Arc 1/2 · Base 0/2'
		);
		// The networks in the wallet's order, whichever was proposed first.
		const ordered = memberChangeGroups([item(BASE, add, 0), item(ARC, add, 1)], [ARC, BASE]);
		expect(ordered[0].parts.map((p) => p.chainId)).toEqual([ARC, BASE]);
		expect(changeKey({ kind: 'changeThreshold', threshold: '3' })).toBe('threshold:3');
		expect(changeKey({ kind: 'transfer', token: null, to: D, amount: '1' })).toBeNull();
		expect(
			changesMembers({ kind: 'batch', calls: [{ to: SAFE, value: '0', data: '0x', action: add }] })
		).toBe(true);
	});
});
