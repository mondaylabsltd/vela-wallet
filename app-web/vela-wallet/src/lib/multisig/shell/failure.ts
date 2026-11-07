/**
 * Why a passkey or a send did not happen, as the shell saw it (`live.ts`):
 * JSON in a result's `detail`, which the core hands back in its views. Kept
 * apart from the pipelines so a sheet reads it without loading them.
 *
 * Round 16 leftover 4: every detail is a KIND (the core's own failures too:
 * `passkey_check`, `unknown`); its words are the catalog's for that kind
 * (`review.failure`), never its text — a `message` here is for a log only.
 */
import { failureSaid } from '../core/review-door';
import {
	PRODUCTION_WRITE_REFUSED_ENGLISH,
	PRODUCTION_WRITE_REFUSED_KEY
} from '$lib/core/generated/serviceDefaults';

export type ShellFailure = (
	| { kind: 'venue_blocked'; block: unknown }
	| { kind: 'page_declined' }
	| { kind: 'page_sign_failed'; page: string }
	| { kind: 'page_approval'; failure: string; code: string | null; line: unknown }
	| { kind: 'passkey'; failure: string; timedOut: boolean }
	/** What was signed does not check out as the member's (the shell's, or the core's own). */
	| { kind: 'passkey_check'; detail?: string }
	/** r16-18 (the desktop's): a read before the passkey on a failing network. */
	| { kind: 'chain_unreachable'; chain_id?: number }
	| { kind: 'not_sent' }
	| { kind: 'cannot_pay'; need: string; have: string }
	/** r14-9 (the core's): the fee moved since the press — nothing was signed. */
	| { kind: 'fee_moved'; amount: string; chainId: number; executor: string }
	/** Anything else — the core's own refusal (`reason`, a stable code) or the shell's. */
	| { kind: 'unknown'; message?: string; reason?: string }
) & {
	/** What the error itself said — for a log, never drawn. */
	message?: string;
};

export function shellFailure(failure: ShellFailure): string {
	return JSON.stringify(failure);
}

/** A `detail` the shell wrote, or `null` for one it did not (the core's own words). */
export function readShellFailure(detail: string | null | undefined): ShellFailure | null {
	if (!detail || !detail.startsWith('{')) return null;
	try {
		const parsed = JSON.parse(detail) as { kind?: unknown };
		return typeof parsed.kind === 'string' ? (parsed as ShellFailure) : null;
	} catch {
		return null;
	}
}

/** The corpus words {@link failureWords} reads (a sheet's `m.proposal`). */
export interface FailureMessages {
	notSent: string;
	reason: {
		passkeyClosed: string;
		passkeyTimedOut: string;
		passkeyMissing: string;
		passkeyFailed: string;
		/** `{{network}}` */
		chainUnreachable: string;
		unknown: string;
	};
}

const PROPOSAL = 'home.multisig.proposal.';

/**
 * Round 16 leftover 4: a failure (a create's, a share-again's) in the
 * catalog's words for its KIND — the core's (`review.failure`), never the
 * text of the error or a `message` kept beside it. A kind whose words are not
 * among these (a signing page's, the fee's) is said as "something went
 * wrong": those flows never reach them.
 */
export function failureWords(
	detail: string | null | undefined,
	m: FailureMessages,
	network = '',
	/** r18-8: `common.testBuild.productionWriteRefused` in the page's language. */
	refused?: string
): string {
	const { key } = failureSaid(detail);
	// r18-8 (§28, §32): this test build refused the write before it left —
	// its own sentence, never the network's.
	if (key === PRODUCTION_WRITE_REFUSED_KEY) return refused || PRODUCTION_WRITE_REFUSED_ENGLISH;
	const leaf = key?.startsWith(PROPOSAL) ? key.slice(PROPOSAL.length) : '';
	if (leaf === 'notSent') return m.notSent;
	if (leaf.startsWith('reason.')) {
		const reason = leaf.slice('reason.'.length) as keyof FailureMessages['reason'];
		const text = m.reason[reason];
		if (typeof text === 'string' && text) return text.replaceAll('{{network}}', network);
	}
	return m.reason.unknown;
}
