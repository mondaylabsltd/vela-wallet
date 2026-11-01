/**
 * r9: counts of approvals, said in the locale's plural form — "Require 1
 * approval", never "Require 1 approvals" (`pluralForm`: the core's CLDR rule,
 * Russian's `_few` included).
 */
import { fillTemplate } from '$lib/i18n/fill';
import { pluralForm } from '$lib/i18n/plural';
import type { MultisigMessages } from '../messages';

function count(threshold: string | number | bigint): number {
	const n = Number(threshold);
	return Number.isFinite(n) ? n : 0;
}

/** "Require 2 approvals" — a change of the approvals a multisig needs. */
export function requireApprovals(
	m: Pick<MultisigMessages, 'action'>,
	threshold: string | number | bigint
): string {
	return fillTemplate(pluralForm(m.action.changeThreshold, count(threshold)), {
		count: String(threshold)
	});
}

/** The line under Reject: the cancellation needs as many approvals. */
export function rejectHint(
	m: Pick<MultisigMessages, 'proposal'>,
	threshold: string | number | bigint
): string {
	return fillTemplate(pluralForm(m.proposal.rejectHint, count(threshold)), {
		count: String(threshold)
	});
}

/**
 * r9-9: why nothing is signed, as the core said it (`MultisigReview.refused.key`);
 * `null` when the review refuses nothing (or says a reason this build lacks).
 */
export function refusedLine(
	m: Pick<MultisigMessages, 'review'>,
	key: string | null | undefined
): string | null {
	switch (key) {
		case 'home.multisig.review.blocked':
			return m.review.blocked;
		case 'home.multisig.review.refusedWhy.delegatecall':
			return m.review.refusedWhy.delegatecall;
		case 'home.multisig.review.refusedWhy.tooDeep':
			return m.review.refusedWhy.tooDeep;
		case 'home.multisig.review.refusedWhy.refund':
			return m.review.refusedWhy.refund;
		case 'home.multisig.review.refusedWhy.other':
			return m.review.refusedWhy.other;
		default:
			return null;
	}
}
