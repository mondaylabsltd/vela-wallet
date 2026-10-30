/**
 * How a member signs, as this device can tell — which decides the way the
 * sheet offers them to approve (ms-11, r4-9, r4a-9):
 *
 * - `here`: a Vela account on this device — its passkey, on this sheet;
 * - `vela`: a smart account elsewhere — it approved something of this
 *   multisig with a passkey, there is code at its address on that network
 *   (an EIP-7702 delegation is not: that is a person's wallet, which signs),
 *   the passkey index knows it as a Vela account, or this device keeps it as
 *   one (typed when it was added, or an invite's word). It approves in Vela,
 *   on its own device, from the proposal's link; an outside wallet can never
 *   sign for it, so that path is never offered;
 * - `wallet`: a plain account — it approved with a wallet's signature, or
 *   this device keeps it as an outside wallet, and no code is there — its
 *   own wallet signs, through "Sign with another wallet";
 * - `unsure`: no code, the index answered it holds none, and nothing else
 *   says — a plain account, or a Vela account not set up on that network yet
 *   (its passkey is only on chain from its first transaction). The Vela way
 *   is offered first, the outside wallet's beside it — never as the only one.
 *
 * `null` while that is still being read: nothing is offered on a guess.
 */
import { lookupWalletName } from '$lib/services/public-key-index';
import { hasContractCode } from './chain';
import { isEcdsaApproval, isPasskeyApproval, sameAddress } from './kernel';
import { keptMember, names } from './names.svelte';
import { localOwners } from './owners';
import type { ApprovalRow } from './proposals.svelte';

export type SignerKind = 'here' | 'vela' | 'wallet' | 'unsure';

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
		items: readonly { approvals: readonly ApprovalRow[] }[] = []
	): SignerKind | null {
		if (localOwners([address]).length > 0) return 'here';
		const approved = approvedWith(items, address);
		if (approved === 'passkey' || names.isVela(address)) return 'vela';
		const kept = keptMember(address).kind;
		const key = `${chainId}:${address.toLowerCase()}`;
		const code = this.#code[key];
		if (code === undefined) this.#ask(chainId, address, key);
		if (code) return 'vela';
		// The person's (or an invite's) word that it is a Vela account: it is
		// never offered to an outside wallet, whatever is read meanwhile.
		if (kept === 'passkey') return 'vela';
		if (code === undefined) return null;
		// No code there: a wallet's own signature, or this device's word.
		if (approved === 'wallet' || kept === 'wallet') return 'wallet';
		// The index has not answered (or could not): no verdict yet.
		if (!names.answered(address)) return null;
		return 'unsure';
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
			if (keptMember(owner).kind === 'passkey') return false;
			const [code, indexed] = await Promise.all([
				hasContractCode(chainId, owner).catch(() => false),
				lookupWalletName(owner).catch(() => null)
			]);
			return !code && !indexed;
		})
	);
	return owners.filter((_, i) => kept[i]);
}
