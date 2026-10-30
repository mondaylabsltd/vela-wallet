/**
 * r5reg-16: the name the person typed for a member on this device (Settings
 * → Add member) — what that member's change and review show, over a local
 * account's name or a link's label. Labels only, never authority.
 */
import type { MultisigAccount } from './store.svelte';

/** The name the person typed for `member` of `account` on this device, if any. */
export function typedName(
	account: Pick<MultisigAccount, 'members'> | undefined,
	member: string
): string | null {
	const info = account?.members?.[member.toLowerCase()];
	return info?.typed && info.name ? info.name : null;
}
