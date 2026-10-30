/**
 * How a member signs, as this device can tell — which decides the way the
 * sheet offers them to approve (ms-11, r4-9, r4a-9):
 *
 * - `here`: a Vela account on this device — its passkey, on this sheet;
 * - `vela`: a Vela account elsewhere — it approved something of this
 *   multisig with a passkey, the passkey index knows it as a Vela account,
 *   or this device keeps it as one (typed when it was added, or an invite's
 *   word) and nothing reads otherwise. It approves in Vela, on its own
 *   device, from the proposal's link; an outside wallet can never sign for
 *   it, so that path is never offered;
 * - `contract` (r7): a smart contract account that is no Vela account the
 *   index knows — code at its address on one of the multisig's networks (an
 *   EIP-7702 delegation is not: that is a person's wallet, which signs), or
 *   kept as one. Code alone never says "Vela account" (there is no on-chain
 *   check that tells Vela's from another Safe or smart wallet). It approves
 *   from its own app (EIP-1271), which Vela can't collect yet: neither an
 *   outside wallet nor the Vela link is offered, and that is said;
 * - `wallet`: a plain account — it approved with a wallet's signature, or
 *   this device keeps it as an outside wallet, and no code is there — its
 *   own wallet signs, through "Sign with another wallet";
 * - `unsure`: no code, the index answered it holds none, and nothing else
 *   says — a plain account, or a Vela account not set up on that network yet
 *   (its passkey is only on chain from its first transaction). The Vela way
 *   is offered first, the outside wallet's beside it — never as the only one.
 *
 * `null` while that is still being read: nothing is offered on a guess.
 *
 * r7: the kind kept here is a label the person (or an invite) declared —
 * it steers what is offered, never authority, and evidence beats it: a
 * passkey approval or the index make a member a Vela account, and code at
 * its address on any of the multisig's networks (the index holding none) a
 * smart contract account, whatever is kept. (What the add screens pre-fill
 * that choice from is the core's: `multisig_signers::kind_evidence`.)
 */
import { lookupWalletName } from '$lib/services/public-key-index';
import { hasContractCode } from './chain';
import { isEcdsaApproval, isPasskeyApproval, sameAddress } from './kernel';
import { keptMember, names } from './names.svelte';
import { localOwners } from './owners';
import type { ApprovalRow } from './proposals.svelte';

export type SignerKind = 'here' | 'vela' | 'contract' | 'wallet' | 'unsure';

/** A code read that failed is asked again after this long, not on every redraw. */
const RETRY_MS = 10_000;

/** What the proposals this device reads say of how `address` approved, if it did (r4a-9). */
export function approvedWith(
	items: readonly { approvals: readonly ApprovalRow[] }[],
	address: string
): 'passkey' | 'wallet' | null {
	let wallet = false;
	for (const item of items) {
		for (const a of item.approvals) {
			// Verified, and from a member: only a counted approval is evidence.
			if (!a.counts || !sameAddress(a.owner, address)) continue;
			if (isPasskeyApproval(a.approval)) return 'passkey';
			if (isEcdsaApproval(a.approval)) wallet = true;
		}
	}
	return wallet ? 'wallet' : null;
}

class SignerKinds {
	#code = $state<Record<string, boolean>>({});
	#asked = new Set<string>();

	/**
	 * `items`: the proposals this device reads of the multisig (any network,
	 * executed ones too) — an approval given with a passkey on any of them
	 * says a Vela account, before and after this one (r4a-9).
	 */
	kindOf(
		chainId: number,
		address: string,
		items: readonly { approvals: readonly ApprovalRow[] }[] = [],
		alsoOn: readonly number[] = []
	): SignerKind | null {
		if (localOwners([address]).length > 0) return 'here';
		const approved = approvedWith(items, address);
		if (approved === 'passkey' || names.isVela(address)) return 'vela';
		const kept = keptMember(address).kind;
		const code = this.#read(chainId, address);
		// r7: code at its address here or on another of the multisig's
		// networks (only an answer counts: a network that does not answer
		// holds nothing up here) — a smart contract account. Code alone is no
		// Vela account: only the index (or a passkey approval, above) says
		// that, so once the index answered none it is `contract`, whatever is
		// kept; before, the person's word that it is a Vela account stands.
		if (code || alsoOn.some((id) => id !== chainId && this.#read(id, address) === true)) {
			if (names.answered(address)) return 'contract';
			return kept === 'passkey' ? 'vela' : null;
		}
		// The person's (or an invite's) word that it is a Vela account: it is
		// never offered to an outside wallet, whatever is read meanwhile.
		if (kept === 'passkey') return 'vela';
		if (kept === 'contract') return 'contract';
		if (code === undefined) return null;
		// No code there: a wallet's own signature, or this device's word.
		if (approved === 'wallet' || kept === 'wallet') return 'wallet';
		// The index has not answered (or could not): no verdict yet.
		if (!names.answered(address)) return null;
		return 'unsure';
	}

	/** The code read of `address` on `chainId`: asked the first time, `undefined` until it answers. */
	#read(chainId: number, address: string): boolean | undefined {
		const key = `${chainId}:${address.toLowerCase()}`;
		const code = this.#code[key];
		if (code === undefined) this.#ask(chainId, address, key);
		return code;
	}

	#ask(chainId: number, address: string, key: string): void {
		if (this.#asked.has(key)) return;
		this.#asked.add(key);
		void hasContractCode(chainId, address)
			.then((code) => {
				this.#code = { ...this.#code, [key]: code };
			})
			.catch(() => {
				setTimeout(() => this.#asked.delete(key), RETRY_MS);
			});
	}
}

export const signers = new SignerKinds();

/**
 * Of `owners`, those an outside wallet may be asked to sign for on `chainId`
 * (r4C-10): not this device's, no code at their address there, not a Vela
 * account the passkey index knows, and none that approved anything here with
 * a passkey (`items`, r4a-9). Read now, not from what is drawn: a smart
 * account is never listed to a wallet page as one that can sign.
 */
export async function plainAccounts(
	chainId: number,
	owners: string[],
	items: readonly { approvals: readonly ApprovalRow[] }[] = []
): Promise<string[]> {
	const kept = await Promise.all(
		owners.map(async (owner) => {
			if (localOwners([owner]).length > 0) return false;
			if (approvedWith(items, owner) === 'passkey') return false;
			const kept = keptMember(owner).kind;
			if (kept === 'passkey' || kept === 'contract') return false;
			const [code, indexed] = await Promise.all([
				hasContractCode(chainId, owner).catch(() => false),
				lookupWalletName(owner).catch(() => null)
			]);
			return !code && !indexed;
		})
	);
	return owners.filter((_, i) => kept[i]);
}
