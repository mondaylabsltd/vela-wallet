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
import { multisigApprovalRoute, signingPlan } from '$lib/core/kernels';
import { signChallenge, VenueBlockedError } from '$lib/signing/sign-challenge';
import { approveOnPage } from './page-approval.svelte';
import { keySetOf } from '$lib/services/safe-transaction';
import { toSignerAccount } from '$lib/services/accounts';
import {
	approvalChallenge,
	approvalFromAssertion,
	sameAddress,
	sponsorChallenge,
	velaAccountGenesis,
	type FoundingKey,
	type PasskeyApproval,
	type SafeTx
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

/**
 * What the member approves, for a member who reviews and signs on a page:
 * the page reads the SafeTx itself, from these fields, and recomputes the
 * hash. Absent, an approval can only be made in Vela.
 */
export interface ApprovalSubject {
	multisig: string;
	/** The multisig's name on this device (the page marks it "named by the app"). */
	name?: string | null;
	tx: SafeTx;
	signal?: AbortSignal;
}

/**
 * `owner` approves `safeTxHash` of a multisig on `chainId`: one passkey
 * prompt — in Vela, or on the trusted signing page the member chose to review
 * and sign on (`multisigApprovalRoute`, 2026-10-29). A member whose keys
 * nothing here can reach is refused with the core's reason.
 */
export async function signApproval(
	owner: Account,
	chainId: number,
	safeTxHash: string,
	subject?: ApprovalSubject
): Promise<PasskeyApproval> {
	const record = loadAccounts().find((a) => sameAddress(a.address, owner.address)) ?? owner;
	const route = multisigApprovalRoute(record);
	if (route?.type === 'blocked') throw new VenueBlockedError(route.block);
	if (route?.type === 'page') {
		if (!subject) throw new Error('an approval on a signing page needs the transaction itself');
		const plan = signingPlan(record);
		return approveOnPage({
			member: owner,
			page: route.url,
			chainId,
			multisig: subject.multisig,
			multisigName: subject.name,
			tx: subject.tx,
			safeTxHash,
			keys: foundingKeys(owner),
			keyRoute: plan?.key ?? undefined,
			keyLabel: plan?.key_label ?? null,
			signal: subject.signal
		});
	}
	const challenge = approvalChallenge(chainId, owner.address, safeTxHash);
	const assertion = await signAs(owner, challenge, 'vela_multisigApprove', chainId, { safeTxHash });
	try {
		return approvalFromAssertion(foundingKeys(owner), {
			credentialId: assertion.credentialId,
			authenticatorData: assertion.authenticatorDataHex,
			clientDataJSON: assertion.clientDataJSONHex,
			signature: assertion.signatureHex
		});
	} catch (error) {
		throw new PasskeyCheckError(error instanceof Error ? error.message : String(error));
	}
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
