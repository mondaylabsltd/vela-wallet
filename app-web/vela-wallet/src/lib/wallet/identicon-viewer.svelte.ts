/**
 * The identicon viewer, app-resident (founder call, 2026-09-05).
 *
 * The artwork is a fingerprint of an address, and the founder's rule is that
 * it can be opened big — beside the address that drew it — from EVERY place it
 * is drawn: the header, a contact row, a recipient card, the signer line of a
 * signing sheet. Threading an `onidenticon` callback through every one of
 * those (three layers deep on some screens) would make each new artwork a
 * plumbing job, and the first one to be forgotten would be the one that
 * mattered. So the question "which address drew this?" is asked here, once,
 * and answered by one host per route.
 *
 * Resident for the same reason the session is: the question is not a property
 * of any one screen. Only `Identicon` asks it and only `IdenticonViewerHost`
 * answers; nothing else may reach in.
 */

export interface IdenticonSubject {
	/** The seed, verbatim: what the artwork was drawn from. */
	address: string;
	identiconSvg: string;
}

class IdenticonViewerStore {
	/** The artwork on show, or nothing. */
	current = $state<IdenticonSubject | null>(null);
	/**
	 * The accessible name of every artwork button, set by the host from the
	 * corpus. Empty until a host has mounted — the button then names itself
	 * by its address, which is true if terse.
	 */
	openLabel = $state('');

	open(subject: IdenticonSubject): void {
		this.current = subject;
	}

	close(): void {
		this.current = null;
	}
}

export const identiconViewer = new IdenticonViewerStore();
