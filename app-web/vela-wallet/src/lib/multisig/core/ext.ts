/**
 * A multisig as the extension's dApp side sees an account: its address and
 * its name. dApps connect to the account the wallet shows, multisig or not;
 * a multisig carries no keys of its own — its transactions are proposed and
 * approved (`DappRequestHost`).
 */
import type { Account } from '$lib/core/generated/Account';
import { multisig, type MultisigAccount } from './store.svelte';

/** A multisig's account id starts with this (the extension tells them apart by it). */
export const MULTISIG_ACCOUNT_PREFIX = 'multisig:';

export function multisigAsAccount(a: MultisigAccount): Account {
	return {
		id: `${MULTISIG_ACCOUNT_PREFIX}${a.address}`,
		name: a.genesis.name || a.label || `Multisig ${a.threshold}/${a.owners.length}`,
		address: a.address,
		public_key_hex: '',
		created_at_iso: new Date(a.addedMs).toISOString(),
		keys: [],
		sign_in_key: null,
		signing_domain: '',
		signing_venue: { type: 'in_vela' }
	};
}

/** Every multisig here, as accounts; and the one shown, if any. */
export function multisigAccountsForExt(): { accounts: Account[]; active: Account | null } {
	const active = multisig.active;
	return {
		accounts: multisig.accounts.map(multisigAsAccount),
		active: active ? multisigAsAccount(active) : null
	};
}
