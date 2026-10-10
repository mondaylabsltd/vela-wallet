/**
 * Why a network was refused, and what can be done about it — in the core's
 * words (`NetCompatibility.hint_key`, `.setup_url`).
 *
 * A network the check answered and refused is one of two things, and what a
 * person can do about them is opposite:
 *
 * - **No P-256 verifier** (`blocker: 'no_p256'`): the network cannot check a
 *   passkey signature, only the network's own team can change that, and money
 *   sent to a Vela address there would be stuck. Nothing to deploy — so no
 *   "Open Chain Setup Tool".
 * - **Missing contracts** (`'missing_contracts'`): the verifier is there and
 *   contracts anyone can deploy are not. Chain Setup shows which, and the link
 *   opens on that chain (`?chain=<id>`).
 *
 * Both places a network is added — Settings' wizard and a dApp's
 * `wallet_addEthereumChain` sheet — read this one function, so neither decides
 * the line or the button from `phase` / `compatible` any more. Pure.
 */
import type { NetCompatibility } from '$lib/core/generated/NetCompatibility';

export interface NetRefusal {
	/** The line under the check; absent when the core named none (or one this build has no words for). */
	hint?: string;
	/**
	 * Where "Open Chain Setup Tool" goes. Absent ⇒ no such button: only a
	 * deployable gap has anything to set up.
	 */
	setupUrl?: string;
}

export function netRefusal(
	compat: NetCompatibility | null | undefined,
	/** The resolved hint lines, by corpus key (`NET_HINT_KEYS`). */
	hints: Readonly<Record<string, string | undefined>>
): NetRefusal {
	if (!compat) return {};
	const hint = compat.hint_key === null ? undefined : hints[compat.hint_key];
	return {
		...(hint === undefined ? {} : { hint }),
		...(compat.setup_url === null ? {} : { setupUrl: compat.setup_url })
	};
}
