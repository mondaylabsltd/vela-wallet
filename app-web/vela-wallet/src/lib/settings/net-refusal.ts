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
import type { NetWizardView } from '$lib/core/generated/NetWizardView';

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

/**
 * The sentence for a wizard that stopped — the core's `error_key`, resolved
 * (PR 3 notes 5, 10 and 18).
 *
 * The core names the sentence for every way the add-network wizard stops:
 * already added, chain not found, no RPC endpoint listed, a check that could
 * not be made ("unable to verify" — never worded as a refusal), and for a
 * refusal the check's own reason when it kept one. Every place the web draws
 * a stopped wizard — Settings' page and the add-token sheet's network tab —
 * asks this, so none of them maps `error.type` to words any more: three of
 * the five stops used to read "Incompatible" here.
 *
 * `undefined` when there is no stop, or for a key this build has no words
 * for: no line, never a dotted path. Pure.
 */
export function netStopLine(
	errorKey: string | null | undefined,
	/** The resolved sentences by corpus key: the stops' own, and the check's hints. */
	words: {
		stops: Readonly<Record<string, string | undefined>>;
		hints: Readonly<Record<string, string | undefined>>;
	}
): string | undefined {
	if (!errorKey) return undefined;
	// Own keys only: a corpus key that happens to be a property of `Object`
	// ("constructor") must not resolve to a function.
	const own = (bag: Readonly<Record<string, string | undefined>>) =>
		Object.hasOwn(bag, errorKey) ? bag[errorKey] : undefined;
	return own(words.stops) ?? own(words.hints);
}

/**
 * Is this stop the check's own refusal — the reason drawn is the one the
 * check kept (`error_key === compat.hint_key`)? Then the stop is drawn as the
 * wizard draws a refused check: the same reason, and "Open Chain Setup Tool"
 * where the core gave it somewhere to go. Any other stop gets neither — above
 * all an inconclusive check, which is never dressed as a refusal whatever
 * the check beside it holds (invariant ③).
 */
export function stopIsRefusal(
	errorKey: string | null | undefined,
	compat: NetCompatibility | null | undefined
): compat is NetCompatibility {
	return (
		!!errorKey &&
		!!compat &&
		compat.rpc_failure === null &&
		!compat.compatible &&
		compat.hint_key !== null &&
		compat.hint_key === errorKey
	);
}

/**
 * The RPC field under a wizard's result — and with it, always and only with
 * it, "Re-check with this RPC" (PR 3 final notes F4, F14 and F22).
 *
 * Whether naming another endpoint is a way on from here is the core's to say
 * (`NetWizardView.rpc_field`), and so is what the field is called
 * (`rpc_field_label_key`): "Custom RPC (optional)" where the check passed or
 * could not reach a verdict, "RPC URL" where the network lists no endpoint
 * and one typed here is the only way on, and no field at all under a refusal
 * another endpoint would not change. Every place the web draws the wizard —
 * Settings' page and the add-token sheet's network tab — asks this, so
 * neither decides it from the phase any more: Settings offered the re-check
 * under a refusal with no field to read, and the tab said "Enter one, then
 * re-check" with no field at all.
 *
 * `undefined` = no field and no re-check. A label key this build has no words
 * for reads as the plain "RPC URL", never a dotted path. Pure.
 */
export function netRpcField(
	wizard: Pick<NetWizardView, 'rpc_field' | 'rpc_field_label_key'>,
	/** The resolved labels, by corpus key (`NET_RPC_FIELD_KEYS`). */
	labels: Readonly<Record<string, string | undefined>>
): { label: string } | undefined {
	// Absent (a view from before the rule) is no field, as the core reads it.
	if ((wizard.rpc_field ?? 'none') === 'none') return undefined;
	const key = wizard.rpc_field_label_key;
	const named = key && Object.hasOwn(labels, key) ? labels[key] : undefined;
	return { label: named ?? labels['settingsModals.network.fieldRpcUrl'] ?? '' };
}
