/**
 * r4a-18: what this device knows of an address a transfer pays, as a send's
 * confirm needs it — the multisigs here and the members of the multisig
 * sending. Facts for the core (`SendRecipientRisk::known_name`/`own`), which
 * decides what the confirm says: one's own account, or a member, is never "a
 * first transfer" to a stranger. Labels only, never authority.
 */
import { sameAddress } from './kernel';
import { multisig, type MultisigAccount } from './store.svelte';

/** A name the person has for `address` among multisigs: a multisig here, or a member of `sending`. */
export function multisigKnownName(
	address: string,
	sending: Pick<MultisigAccount, 'members'> | null
): string | null {
	const here = multisig.accounts.find((a) => sameAddress(a.address, address));
	if (here) return here.label ?? here.genesis.name ?? null;
	return sending?.members?.[address.toLowerCase()]?.name ?? null;
}

/** `address` is a multisig on this device, or one of the members of `sending` (`members`: its members now). */
export function ownOrMember(address: string, members: string[]): boolean {
	return (
		multisig.accounts.some((a) => sameAddress(a.address, address)) ||
		members.some((m) => sameAddress(m, address))
	);
}
