/**
 * The two contact forms' copy (spec 024), typed once so the phone sheet and
 * the desktop column — which render the SAME form in different containers —
 * cannot drift on which strings they need.
 */
export interface ContactFormCopy {
	title: string;
	nameLabel: string;
	namePlaceholder: string;
	addressLabel: string;
	addressPlaceholder: string;
	save: string;
	cancel: string;
	invalidAddress: string;
	/** The address's mixed case is not its EIP-55 checksum (set-4). */
	invalidChecksum: string;
}

export interface GroupFormCopy {
	title: string;
	nameLabel: string;
	namePlaceholder: string;
	save: string;
	cancel: string;
}

export interface ContactDraft {
	name: string;
	address: string;
}

/**
 * The longest name a contact or a group may have — the core's
 * (`intents::NAME_MAX_CHARS`), which refuses a longer one.
 */
export const NAME_MAX_CHARS = 64;

/**
 * Characters that reorder the text around them (Unicode's bidirectional
 * controls) and other control characters: the core refuses a name carrying
 * them, so the person's own form leaves them out rather than have a save
 * refused unseen.
 */
// eslint-disable-next-line no-control-regex -- control characters are what it removes
const UNSEEN = /[\u0000-\u001f\u007f-\u009f\u061c\u200e\u200f\u202a-\u202e\u2066-\u2069]/g;

/** A typed name as it is saved: without what the core refuses, trimmed. */
export function cleanName(name: string): string {
	return name.replace(UNSEEN, '').trim();
}
