/**
 * This device's Vela accounts, as the multisig machines need them
 * (`MultisigLocalAccount`): the address, the name, the founding keys the
 * owner proof is made with, and how each approves (in Vela, on its trusted
 * page, or not from here). Read from the session's own store.
 */
import type { Account } from '$lib/core/generated/Account';
import { multisigApprovalRoute } from '$lib/core/kernels';
import { loadAccounts } from '$lib/onboarding/core/storage';
import { foundingKeys } from '../core/owners';
import type { MultisigLocalAccount } from '../generated/MultisigLocalAccount';

function stored(): Account[] {
	try {
		return loadAccounts();
	} catch {
		return [];
	}
}

export function localAccounts(accounts: Account[] = stored()): MultisigLocalAccount[] {
	return accounts.map((account) => {
		const route = multisigApprovalRoute(account);
		return {
			address: account.address,
			name: account.name,
			keys: foundingKeys(account).map((k) => ({
				credential_id: k.credentialId,
				public_key_hex: k.publicKeyHex
			})),
			route: route?.type === 'page' ? 'page' : route?.type === 'blocked' ? 'blocked' : 'vela'
		};
	});
}
