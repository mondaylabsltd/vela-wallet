/**
 * §12: where a review stands and the one rule a sign / propose / approve /
 * execute button follows — the core's `review.door` (the link door every
 * platform replays against `tests/vectors/multisig/review-state.json`). The
 * shell draws the door; it never opens one itself: a review not asked yet,
 * being read, or not read keeps the button shut, and so does a door the
 * core could not be asked for.
 */
import * as wasm from '../../../../../../rust/pkg-web/vela_core.js';
import type { MultisigReviewState } from '../generated/MultisigReviewState';
import type { MultisigSignDoor } from '../generated/MultisigSignDoor';
import type { ReviewStateMessages } from '../messages';

export type { MultisigReviewState, MultisigSignDoor };

/** A shut door, for a state the core could not be asked about. */
const SHUT: MultisigSignDoor = {
	shows: 'loading',
	enabled: false,
	anyway: false,
	reason_key: null,
	retry: false
};

/** The core's door for a review in `state` (`null`: not asked yet). */
export function reviewDoor(
	state: MultisigReviewState | null | undefined,
	review: unknown
): MultisigSignDoor {
	try {
		return JSON.parse(
			wasm.multisigLinkCall(
				'review.door',
				JSON.stringify({ state: state ?? null, review: review ?? null })
			)
		) as MultisigSignDoor;
	} catch {
		return SHUT;
	}
}

/** The state kept under `<chainId>:<safeTxHash>` (any case), if asked. */
export function stateFor(
	states: Record<string, MultisigReviewState> | undefined,
	chainId: number,
	hash: string
): MultisigReviewState | null {
	const key = `${chainId}:${hash}`.toLowerCase();
	const found = Object.entries(states ?? {}).find(([k]) => k.toLowerCase() === key);
	return found ? found[1] : null;
}

const PREFIX = 'home.multisig.review.state.';

/** A failed review's reason, in its words (an unknown key: "couldn't read"). */
export function reasonWords(m: ReviewStateMessages, key: string | null): string {
	const leaf = key?.startsWith(PREFIX) ? key.slice(PREFIX.length) : '';
	switch (leaf) {
		case 'timedOut':
			return m.timedOut;
		case 'gone':
			return m.gone;
		default:
			return m.unreadable;
	}
}

const UNREAD = 'home.multisig.proposal.unread.';
const SERVICE_KEY = 'home.multisig.proposal.mailboxDown';

/**
 * r13-3: the step before a review — a draft not built, a link's transaction
 * not opened — said as the core named the read that failed (`ActionState`
 * `unread.reason_key`, `MultisigLinkedView.reason_key`), `{{network}}` its
 * network's name. An unknown key says the network's line: the core names
 * the service with its own key.
 */
export function unreadWords(
	proposal: { mailboxDown: string; unread: Record<string, string> },
	key: string | null | undefined,
	network: string
): string {
	if (key === SERVICE_KEY) return proposal.mailboxDown;
	const leaf = key?.startsWith(UNREAD) ? key.slice(UNREAD.length) : '';
	const text = proposal.unread[leaf] || proposal.unread.chain;
	return text.replaceAll('{{network}}', network);
}
