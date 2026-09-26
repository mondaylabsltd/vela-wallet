/**
 * The one `contacts` core the web app has — APP-RESIDENT, like the balance and
 * feed residents (founder, 2026-09-26).
 *
 * It used to be route-scoped: the contacts page built a session on every
 * mount and threw it away on the way out, and the wallet page built a second
 * one for the send picker. So every visit to 通讯录 started unread — the
 * title and search, then the book popping in underneath — and the two
 * sessions each read the three stores for themselves. iOS keeps one store,
 * read at sign-in, and never shows an unread frame; this is that store.
 *
 * One session per app, created on first use (the wallet page asks for it at
 * sign-in, beside the balances and the feed), told the signed-in account and
 * told again ONLY when that account changes. Nothing disposes it on a route
 * change. The core's `AccountSwitched` for the account already open and read
 * no longer wipes the book, so even a repeated tell never unloads it.
 *
 * Every consumer reads `view` and sends its events through `dispatch`; no
 * logic lives here.
 */
import { loadCore } from '$lib/core/client';
import type { ContactEvent } from '$lib/core/generated/ContactEvent';
import type { ContactsView } from '$lib/core/generated/ContactsView';
import { createContactsSession, type ContactsSession } from './contacts';

/** Exported for tests, which build a fresh one each; the app uses `contactsBook`. */
export class ContactsBook {
	/** `null` until the session's first view; then the core's, always. */
	view = $state<ContactsView | null>(null);

	#session: ContactsSession | null = null;
	#booting: Promise<void> | null = null;
	/** The account the core was last told about (`undefined` = never told). */
	#told: string | null | undefined = undefined;

	/**
	 * The signed-in account (or `null` for none). Idempotent: the first call
	 * builds the session and reads the book for this account; later calls
	 * tell the core only when the account is a different one.
	 */
	async setAccount(address: string | null): Promise<void> {
		await this.#boot(address);
		if (this.#told === address) return;
		this.#told = address;
		this.#session?.dispatch({ type: 'account_switched', my_address: address });
	}

	/** An event for the book. Dropped only before the session exists. */
	dispatch(event: ContactEvent): void {
		this.#session?.dispatch(event);
	}

	/**
	 * Read the three stores again, from scratch. For the one path that changes
	 * them behind the book's back — Settings' storage page clearing the
	 * contacts — where the book in memory would otherwise go on showing, and
	 * on its next write re-save, what was just cleared. A session per visit
	 * used to re-read on every mount; the resident is told instead.
	 */
	reload(): Promise<void> {
		if (!this.#session) return Promise.resolve();
		const address = this.#told ?? null;
		this.#session.dispose();
		this.#session = null;
		this.#booting = null;
		this.view = null;
		return this.#boot(address);
	}

	#boot(address: string | null): Promise<void> {
		if (this.#booting) return this.#booting;
		this.#booting = (async () => {
			await loadCore();
			this.#session = createContactsSession({
				onView: (next) => {
					this.view = next;
				},
				onError: (error) => console.error('[contacts] core fault:', error)
			});
			// Hydrate: the three stores and the local send history, for this
			// account — the core's own account boundary (contacts.rs).
			this.#told = address;
			this.#session.start({ type: 'account_switched', my_address: address });
		})();
		return this.#booting;
	}
}

/** Browser-only: the first `setAccount` loads wasm — callers call it on mount. */
export const contactsBook = new ContactsBook();
