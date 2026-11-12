/**
 * §39 (r22-2): the words for a gate the chain could not settle, apart from
 * the gate itself (a sheet's test stands the gate in, never these words).
 */
import { fillTemplate } from '$lib/i18n/fill';
import { chainName } from '$lib/services/networks';

/**
 * `signUnchecked` ("couldn't check who can act…"), or `signDisagreed` when
 * two independent nodes disagreed (`ActionState::SigningDisagreed`: nothing
 * signed, no "… anyway") — never "Can't reach": the network answered.
 */
export function uncheckedWords(
	m: { signUnchecked: string; signDisagreed: string },
	error: { chainId: number; disagreed?: boolean }
): string {
	return fillTemplate(error.disagreed === true ? m.signDisagreed : m.signUnchecked, {
		network: chainName(error.chainId)
	});
}
