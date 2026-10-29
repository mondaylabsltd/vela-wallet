import { fillTemplate } from '$lib/i18n/fill';
import type { MultisigMessages } from '../messages';

/**
 * "3/2 approved" never (r4b-17): the approvals that count, up to the
 * threshold — more than it adds nothing to run it.
 */
export function progressLine(m: MultisigMessages, approved: number, threshold: number): string {
	return fillTemplate(m.status.progress, {
		approved: Math.min(approved, threshold),
		threshold
	});
}
