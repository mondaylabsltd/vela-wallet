/**
 * The multisig's owners that are Vela accounts on THIS device, and what a
 * failed passkey check is said as. Every signature an owner makes for a
 * multisig — an approval, the proof that records a new one — is the
 * machines' (`passkey` operations, the core's challenge and verification);
 * the shell only prompts (`shell/live.ts`).
 */
import type { Account } from '$lib/core/generated/Account';
import { loadAccounts } from '$lib/onboarding/core/storage';
import { keySetOf } from '$lib/services/safe-transaction';
import { toSignerAccount } from '$lib/services/accounts';
import { sameAddress, type FoundingKey } from './kernel';

/** A Vela account of this device that owns the multisig. */
export interface LocalOwner {
	address: string;
	name: string;
	account: Account;
}

/** This device's Vela accounts among `owners`, in the owners' order. */
export function localOwners(owners: string[]): LocalOwner[] {
	const accounts = loadAccounts();
	const out: LocalOwner[] = [];
	for (const owner of owners) {
		const account = accounts.find((a) => sameAddress(a.address, owner));
		if (account && !out.some((o) => sameAddress(o.address, owner))) {
			out.push({ address: account.address, name: account.name, account });
		}
	}
	return out;
}

export function foundingKeys(account: Account): FoundingKey[] {
	return keySetOf(toSignerAccount(account)).keys;
}

/**
 * r4b-12: the passkey answered, and what it signed does not check out as
 * this account's (another account's passkey, a key it does not hold, a
 * signature the core cannot verify): nothing was signed for it.
 */
export class PasskeyCheckError extends Error {
	constructor(detail: string) {
		super(`the passkey's signature did not check out: ${detail}`);
		this.name = 'PasskeyCheckError';
	}
}
