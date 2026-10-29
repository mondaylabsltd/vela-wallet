/**
 * The invite a multisig's members are sent (r4D-4, r4D-5): its address, the
 * networks it is used on (the first it was used on first), and what this
 * device knows of its founding members — each one's label and kind — so the
 * device that opens it shows them by name and never guesses a network.
 * Labels only: the device that adds it re-derives the address from the
 * genesis, and who may act is each chain's.
 */
import { shortenAddress } from './display';
import { addLink, type InvitedMember } from './link';
import { names } from './names.svelte';
import type { MultisigAccount } from './store.svelte';

/** What this device knows of each founding member, in the genesis's order. */
export function invitedMembers(account: Pick<MultisigAccount, 'owners'>): InvitedMember[] {
	return account.owners.map((owner) => {
		const label = names.label(owner);
		const kind = names.kindOf(owner);
		return {
			...(label !== shortenAddress(owner) ? { name: label } : {}),
			...(kind ? { kind } : {})
		};
	});
}

export function inviteLink(
	origin: string,
	locale: string,
	account: Pick<MultisigAccount, 'address' | 'owners' | 'chainIds'>
): string {
	return addLink(origin, locale, account.address, {
		chainIds: account.chainIds,
		members: invitedMembers(account)
	});
}
