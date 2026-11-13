/**
 * The invite a multisig's members are sent (r4D-4, r4D-5): its address, the
 * networks it is used on (the first it was used on first), and what this
 * device knows of its members — each one's address, label and kind, the
 * founding ones and those added since in Settings — so the device that opens
 * it shows them by name, tells its own account apart (r5reg-24), and never
 * guesses a network. Labels only: the device that adds it re-derives the
 * address from the genesis, keeps a label only for an address that is a
 * member, keeps its own names over the link's (the core's accounts machine,
 * `keep_invited`), and who may act is each chain's.
 */
import { shortenAddress } from './display';
import { sameAddress } from './kernel';
import { addLink, type InvitedMember } from './link';
import { names } from './names.svelte';
import { membersAnywhere } from './proposals.svelte';
import { multisig, type MultisigAccount } from './store.svelte';

/**
 * What this device knows of each member, by address: the founding members in
 * the genesis's order, then those that are members now on a network it is
 * deployed on (added since — their names typed in Settings travel, r4D-5).
 */
export function invitedMembers(
	account: Pick<MultisigAccount, 'address' | 'owners'>,
	now: string[] = membersAnywhere(account)
): InvitedMember[] {
	const all = [...account.owners];
	for (const member of now) if (!all.some((a) => sameAddress(a, member))) all.push(member);
	return all.map((owner) => {
		const label = names.label(owner);
		const kind = names.kindOf(owner);
		return {
			address: owner,
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
	// r9-12: the members as the accounts machine names them (`invite_members`:
	// every name known, the same on every platform); a multisig it does not
	// list (a draft just created) by the same rule here.
	const row = multisig.view?.accounts.find((r) =>
		sameAddress((r.account as MultisigAccount).address, account.address)
	);
	const members = (row?.invite_members as InvitedMember[] | undefined) ?? invitedMembers(account);
	return addLink(origin, locale, account.address, {
		chainIds: account.chainIds,
		members
	});
}
