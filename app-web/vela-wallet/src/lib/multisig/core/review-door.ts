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

/**
 * A failed review's reason, in its words (an unknown key: "couldn't read");
 * `network` names the review's network where the words do (r14-13).
 */
export function reasonWords(m: ReviewStateMessages, key: string | null, network = ''): string {
	const leaf = key?.startsWith(PREFIX) ? key.slice(PREFIX.length) : '';
	switch (leaf) {
		case 'chainUnread':
			return m.chainUnread.replaceAll('{{network}}', network);
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

/** §18 (b): what a sheet opened without a link says (`review.missing`). */
export interface MissingSaid {
	key: string;
	retry: boolean;
}

/**
 * §18 (b), round 14: a proposal sheet opened WITHOUT a link whose
 * transaction is not in the list — the core's rule over the machine's facts
 * (`loading`, the network `unreadable`, the service's list read: `mailbox`,
 * null where the multisig is not read on this network). A core that could
 * not be asked says it is still opening, with nothing to press.
 */
export function missingSaid(
	loading: boolean,
	unreadable: boolean,
	mailbox: boolean | null
): MissingSaid {
	try {
		return JSON.parse(
			wasm.multisigLinkCall('review.missing', JSON.stringify({ loading, unreadable, mailbox }))
		) as MissingSaid;
	} catch {
		return { key: 'home.multisig.proposal.opening', retry: false };
	}
}

const PROPOSAL = 'home.multisig.proposal.';

/** {@link MissingSaid}'s key in the sheet's words, `{{network}}` its network's name. */
export function missingWords(
	proposal: {
		opening: string;
		cantCheck: string;
		notInMailbox: string;
		notHere: string;
		unread: Record<string, string>;
	},
	key: string,
	network: string
): string {
	const leaf = key.startsWith(PROPOSAL) ? key.slice(PROPOSAL.length) : '';
	const text =
		leaf === 'cantCheck'
			? proposal.cantCheck
			: leaf === 'notInMailbox'
				? proposal.notInMailbox
				: leaf === 'notHere'
					? proposal.notHere
					: leaf.startsWith('unread.')
						? proposal.unread[leaf.slice('unread.'.length)] || proposal.opening
						: proposal.opening;
	return text.replaceAll('{{network}}', network);
}
