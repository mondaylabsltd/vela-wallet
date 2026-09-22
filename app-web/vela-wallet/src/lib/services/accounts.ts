/**
 * The stored wallet, as the SIGNER needs it.
 *
 * Ported from src/services/storage.ts:56-71 @ f9bcb278 (the account finders)
 * — over the web's onboarding storage, whose records are the generated
 * `Account` shape (snake_case, `keys[]` in founding order). The one adapter
 * here turns that into the camelCase key set `safe-transaction.ts::keySetOf`
 * was written against, so the 2,838-line port stays verbatim.
 */
import type { Account } from '$lib/core/generated/Account';
import { loadAccounts } from '$lib/onboarding/core/storage';

/** The legacy stored-account shape `keySetOf` reads. */
export interface SignerAccount {
	id: string;
	address: string;
	publicKeyHex: string;
	keys?: { credentialId: string; publicKeyHex: string; transports?: string }[];
}

export function toSignerAccount(account: Account): SignerAccount {
	return {
		id: account.id,
		address: account.address,
		publicKeyHex: account.public_key_hex,
		keys: (account.keys ?? []).map((key) => ({
			credentialId: key.credential_id,
			publicKeyHex: key.public_key_hex,
			transports: key.transports
		}))
	};
}

/** A multi-key wallet is owned by ANY of its founding credentials, not just `id` (= keys[0]). */
export function findAccountByCredentialId(id: string): SignerAccount | undefined {
	const wanted = id.toLowerCase();
	const hit = loadAccounts().find(
		(a) =>
			a.id.toLowerCase() === wanted ||
			(a.keys ?? []).some((k) => k.credential_id.toLowerCase() === wanted)
	);
	return hit ? toSignerAccount(hit) : undefined;
}

export function findAccountByAddress(address: string): SignerAccount | undefined {
	const wanted = address.toLowerCase();
	const hit = loadAccounts().find((a) => a.address.toLowerCase() === wanted);
	return hit ? toSignerAccount(hit) : undefined;
}
