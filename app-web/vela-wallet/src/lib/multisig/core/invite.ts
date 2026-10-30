/**
 * The invite a multisig's members are sent (r4D-4, r4D-5): its address, the
 * networks it is used on (the first it was used on first), and what this
 * device knows of its members — each one's address, label and kind, the
 * founding ones and those added since in Settings — so the device that opens
 * it shows them by name, tells its own account apart (r5reg-24), and never
 * guesses a network. Labels only: the device that adds it re-derives the
 * address from the genesis, keeps a label only for an address that is a
 * member, keeps its own names over the link's, and who may act is each
 * chain's.
 */
import { shortenAddress } from './display';
import { sameAddress } from './kernel';
import { addLink, type AddLink, type InvitedMember } from './link';
import { names } from './names.svelte';
import { membersAnywhere } from './proposals.svelte';
import { multisig, type MemberInfo, type MultisigAccount } from './store.svelte';

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
	return addLink(origin, locale, account.address, {
		chainIds: account.chainIds,
		members: invitedMembers(account)
	});
}

/**
 * Keep what an invite says of `address`'s members with it on this device
 * (r4D-5): a member named by address is kept only if it is one — a founding
 * member, or one of the members now on a network the invite names (`readOwners`)
 * — so a label never lands on an address that is not a member; one named by
 * the genesis's order (a link from before r6) only when the counts match.
 * This device's own names stay (`nameMembers`). Labels only.
 */
export async function keepInvited(
	address: string,
	link: AddLink | undefined,
	readOwners: (chainId: number, account: MultisigAccount) => Promise<string[]>
): Promise<void> {
	const account = multisig.find(address);
	if (!account || !link?.members?.length) return;
	const addressed = link.members.filter((m): m is InvitedMember & { address: string } =>
		Boolean(m.address)
	);
	if (addressed.length === 0) {
		if (link.members.length !== account.owners.length) return;
		multisig.nameMembers(
			account.address,
			Object.fromEntries(account.owners.map((owner, i) => [owner, info(link.members?.[i])]))
		);
		return;
	}
	const founding = addressed.filter((m) => account.owners.some((o) => sameAddress(o, m.address)));
	keep(account.address, founding);
	const others = addressed.filter((m) => !founding.includes(m));
	if (others.length === 0) return;
	const chainIds = link.chainIds.filter((id) => account.chainIds.includes(id));
	const now = (
		await Promise.all(chainIds.map((id) => readOwners(id, account).catch(() => [] as string[])))
	).flat();
	keep(
		account.address,
		others.filter((m) => now.some((o) => sameAddress(o, m.address)))
	);
}

function info(member: InvitedMember | undefined): MemberInfo {
	return {
		...(member?.name ? { name: member.name } : {}),
		...(member?.kind ? { kind: member.kind } : {})
	};
}

function keep(address: string, members: (InvitedMember & { address: string })[]): void {
	if (members.length === 0) return;
	multisig.nameMembers(address, Object.fromEntries(members.map((m) => [m.address, info(m)])));
}
