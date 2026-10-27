/**
 * The multisig's owners that are Vela accounts on THIS device, and the two
 * signatures they make for it: an approval of a transaction, and the proof
 * that asks the genesis service to record a new multisig.
 *
 * Both go through `signChallenge` — the account's own key, over the route it
 * signed in with — exactly as every other signature of the account does.
 * What is signed is the core's: `approvalChallenge` is the hash the owner's
 * on-chain handler checks (never the dApp message hash, which can never pass
 * for an approval), `sponsorChallenge` the service's.
 */
import type { Account } from '$lib/core/generated/Account';
import { toHex } from '$lib/core/kernels';
import { loadAccounts } from '$lib/onboarding/core/storage';
import { signChallenge } from '$lib/signing/sign-challenge';
import { keySetOf } from '$lib/services/safe-transaction';
import { toSignerAccount } from '$lib/services/accounts';
import {
	approvalChallenge,
	approvalFromAssertion,
	sameAddress,
	sponsorChallenge,
	velaAccountGenesis,
	type FoundingKey,
	type PasskeyApproval
} from './kernel';
import type { OwnerProof } from './service';

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

function fromHexString(hex: string): Uint8Array {
	const clean = hex.startsWith('0x') ? hex.slice(2) : hex;
	const out = new Uint8Array(clean.length / 2);
	for (let i = 0; i < out.length; i += 1) out[i] = parseInt(clean.slice(i * 2, i * 2 + 2), 16);
	return out;
}

async function signAs(
	owner: Account,
	challengeHex: string,
	method: string,
	chainId: number,
	params: unknown
) {
	const keys = foundingKeys(owner);
	return signChallenge(fromHexString(challengeHex), {
		account: owner.address,
		keys,
		credentials: keys.map((k) => ({ id: k.credentialId })),
		request: { method, params, origin: '', chainId }
	});
}

/** `owner` approves `safeTxHash` of a multisig on `chainId`: one passkey prompt. */
export async function signApproval(
	owner: Account,
	chainId: number,
	safeTxHash: string
): Promise<PasskeyApproval> {
	const challenge = approvalChallenge(chainId, owner.address, safeTxHash);
	const assertion = await signAs(owner, challenge, 'vela_multisigApprove', chainId, { safeTxHash });
	return approvalFromAssertion(foundingKeys(owner), {
		credentialId: assertion.credentialId,
		authenticatorData: assertion.authenticatorDataHex,
		clientDataJSON: assertion.clientDataJSONHex,
		signature: assertion.signatureHex
	});
}

/** `owner` asks the genesis service to record `multisig`: one passkey prompt. */
export async function signOwnerProof(
	owner: Account,
	multisig: string,
	chainId: number
): Promise<OwnerProof> {
	const challenge = sponsorChallenge(multisig);
	const assertion = await signAs(owner, challenge, 'vela_multisigCreate', chainId, { multisig });
	const clientDataJSON = new TextDecoder().decode(fromHexString(assertion.clientDataJSONHex));
	return {
		owner: owner.address,
		ownerGenesis: velaAccountGenesis(foundingKeys(owner)).genesis,
		authenticatorData: assertion.authenticatorDataHex.startsWith('0x')
			? assertion.authenticatorDataHex
			: `0x${assertion.authenticatorDataHex}`,
		clientDataJSON,
		signature: assertion.signatureHex.startsWith('0x')
			? assertion.signatureHex
			: `0x${assertion.signatureHex}`
	};
}

export { toHex };
