/**
 * A member typed by address, checked where it is added — not three steps
 * later, where a list the core cannot make a multisig of would only leave
 * Create disabled with no reason given.
 */
import { sameAddress, validOwner } from './kernel';

/** Why `typed` cannot join `owners`, or `null` when it can. */
export type MemberProblem = 'invalidAddress' | 'notMember' | 'duplicate';

export function memberProblem(typed: string, owners: string[]): MemberProblem | null {
	const value = typed.trim();
	if (!/^0x[0-9a-fA-F]{40}$/.test(value)) return 'invalidAddress';
	// The zero address and Safe's owner-list sentinel (`0x…01`): `setup` refuses both.
	if (!validOwner(value)) return 'notMember';
	if (owners.some((o) => sameAddress(o, value))) return 'duplicate';
	return null;
}
