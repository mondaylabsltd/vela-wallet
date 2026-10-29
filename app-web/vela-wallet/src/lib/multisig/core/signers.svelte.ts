/**
 * How a member signs, as this device can tell — which decides the way the
 * sheet offers them to approve (ms-11, r4-9):
 *
 * - `here`: a Vela account on this device — its passkey, on this sheet;
 * - `vela`: a smart account elsewhere — code at its address on that network
 *   (an EIP-7702 delegation is not: that is a person's wallet, which signs),
 *   or the passkey index knows it as a Vela account. It approves in Vela, on
 *   its own device, from the proposal's link; an outside wallet can never
 *   sign for it, so that path is never offered;
 * - `wallet`: a plain account (no code there, and the index does not know
 *   it) — its own wallet signs, through "Sign with another wallet".
 *
 * `null` while that is still being read: nothing is offered on a guess.
 */
import { queryWalletName } from '$lib/services/public-key-index';
import { hasContractCode } from './chain';
import { names } from './names.svelte';
import { localOwners } from './owners';

export type SignerKind = 'here' | 'vela' | 'wallet';

/** A code read that failed is asked again after this long, not on every redraw. */
const RETRY_MS = 10_000;

class SignerKinds {
	#code = $state<Record<string, boolean>>({});
	#asked = new Set<string>();

	kindOf(chainId: number, address: string): SignerKind | null {
		if (localOwners([address]).length > 0) return 'here';
		if (names.isVela(address)) return 'vela';
		const key = `${chainId}:${address.toLowerCase()}`;
		const code = this.#code[key];
		if (code === undefined) {
			this.#ask(chainId, address, key);
			return null;
		}
		if (code) return 'vela';
		// No code: a plain account — once the index has said it is no Vela account.
		return names.answered(address) ? 'wallet' : null;
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
 * (r4C-10): not this device's, no code at their address there, and not a
 * Vela account the passkey index knows. Read now, not from what is drawn: a
 * smart account is never listed to a wallet page as one that can sign.
 */
export async function plainAccounts(chainId: number, owners: string[]): Promise<string[]> {
	const kept = await Promise.all(
		owners.map(async (owner) => {
			if (localOwners([owner]).length > 0) return false;
			const [code, indexed] = await Promise.all([
				hasContractCode(chainId, owner).catch(() => false),
				queryWalletName(owner).catch(() => null)
			]);
			return !code && !indexed;
		})
	);
	return owners.filter((_, i) => kept[i]);
}
