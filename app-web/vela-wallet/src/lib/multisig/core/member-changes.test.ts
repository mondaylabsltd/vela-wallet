/**
 * A member change already waiting on a network refuses another there (the
 * web's proposer reads it until the proposals machine proposes). The plan per
 * deployed network, the rows and the changes waiting are the core's members
 * machine's: Rust, `tests/app_multisig_members.rs`
 * (`each_networks_change_is_built_against_that_networks_own_list`,
 * `a_second_member_change_is_refused_while_one_waits_on_that_network`,
 * `nothing_is_offered_where_it_is_not_deployed`,
 * `every_deployed_networks_members_once_the_first_networks_order_first`,
 * `the_changes_waiting_read_as_one_change_per_action`).
 */
import { describe, expect, it } from 'vitest';
import type { Action } from './kernel';
import { changesMembers, memberStateOf } from './member-changes';

const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const A = '0x1111111111111111111111111111111111111111';
const B = '0x2222222222222222222222222222222222222222';
const D = '0x4444444444444444444444444444444444444444';

const waiting = (action: Action, status: 'pending' | 'done' = 'pending') => ({
	status,
	review: { safeTxHash: '0x', nonce: '3', action, refusal: null }
});
const members = { owners: [A, B], threshold: 2 };

describe('a member change waiting on a network', () => {
	it('is one alone or inside a batch; a transfer, or one done, is not', () => {
		const add = { kind: 'addOwner', owner: D, threshold: '2' } as const;
		expect(memberStateOf({ members, items: [waiting(add)] })).toEqual({
			owners: [A, B],
			threshold: 2,
			changeWaiting: true
		});
		expect(
			changesMembers({ kind: 'batch', calls: [{ to: SAFE, value: '0', data: '0x', action: add }] })
		).toBe(true);
		expect(memberStateOf({ members, items: [waiting(add, 'done')] }).changeWaiting).toBe(false);
		expect(
			memberStateOf({
				members,
				items: [waiting({ kind: 'transfer', token: null, to: D, amount: '1' })]
			}).changeWaiting
		).toBe(false);
	});
});
