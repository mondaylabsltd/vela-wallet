// The app's one address book (founder, 2026-09-26: 通讯录 flashed unread on
// every visit, because each visit built a session of its own). One session,
// read once for the signed-in account, told again only when that changes.
//
// Deterministic under a loaded full-suite run: nothing real is initialised
// (the core loader and the session factory are stubs that resolve at once),
// the module is imported statically — never re-imported inside a test, where
// a transform under load once ran past the 5 s timeout — and every test builds
// its OWN book, so no session can outlive a test, failed or not. No timers,
// no polling: every step awaits the promise it depends on.
import { beforeEach, describe, expect, it, vi } from 'vitest';

type StubSession = {
	start: ReturnType<typeof vi.fn>;
	dispatch: ReturnType<typeof vi.fn>;
	dispose: ReturnType<typeof vi.fn>;
	onView: (view: unknown) => void;
};

const seams = vi.hoisted(() => ({ sessions: [] as StubSession[] }));

vi.mock('$lib/core/client', () => ({ loadCore: () => Promise.resolve() }));
vi.mock('./contacts', () => ({
	createContactsSession: (options: { onView: (view: unknown) => void }) => {
		const session: StubSession = {
			start: vi.fn(),
			dispatch: vi.fn(),
			dispose: vi.fn(),
			onView: options.onView
		};
		seams.sessions.push(session);
		return session;
	}
}));

import { ContactsBook } from './contacts-book.svelte';

const A = '0x' + 'aa'.repeat(20);
const B = '0x' + 'bb'.repeat(20);

beforeEach(() => {
	seams.sessions.length = 0;
});

describe('ContactsBook — one session for the app', () => {
	it('reads the book once for the signed-in account, however often it is told', async () => {
		const book = new ContactsBook();
		await Promise.all([book.setAccount(A), book.setAccount(A)]);
		await book.setAccount(A);
		expect(seams.sessions).toHaveLength(1);
		expect(seams.sessions[0].start).toHaveBeenCalledExactlyOnceWith({
			type: 'account_switched',
			my_address: A
		});
		expect(seams.sessions[0].dispatch).not.toHaveBeenCalled();
	});

	it('tells the core when — and only when — the account changes', async () => {
		const book = new ContactsBook();
		await book.setAccount(A);
		await book.setAccount(B);
		await book.setAccount(B);
		expect(seams.sessions).toHaveLength(1);
		expect(seams.sessions[0].dispatch).toHaveBeenCalledExactlyOnceWith({
			type: 'account_switched',
			my_address: B
		});
	});

	it('is the view every consumer reads, and the door every event goes through', async () => {
		const book = new ContactsBook();
		// Before the session exists an event has nowhere to go — and does not throw.
		book.dispatch({ type: 'history_changed' });
		await book.setAccount(A);
		seams.sessions[0].onView({ loaded: true, contacts: [] });
		expect(book.view).toEqual({ loaded: true, contacts: [] });
		book.dispatch({ type: 'history_changed' });
		expect(seams.sessions[0].dispatch).toHaveBeenCalledExactlyOnceWith({ type: 'history_changed' });
	});

	// Settings' storage page clears the contacts behind the book's back.
	it('reload reads the stores again, for the account it was told', async () => {
		const book = new ContactsBook();
		await book.setAccount(A);
		await book.setAccount(B);
		const reloading = book.reload();
		// The old book is gone at once — never shown over the cleared stores.
		expect(seams.sessions[0].dispose).toHaveBeenCalledOnce();
		expect(book.view).toBeNull();
		await reloading;
		expect(seams.sessions).toHaveLength(2);
		expect(seams.sessions[1].start).toHaveBeenCalledExactlyOnceWith({
			type: 'account_switched',
			my_address: B
		});
		// …and the same account is still not told twice afterwards.
		await book.setAccount(B);
		expect(seams.sessions[1].dispatch).not.toHaveBeenCalled();
	});

	it('reload before anything was read is a no-op', async () => {
		const book = new ContactsBook();
		await book.reload();
		expect(seams.sessions).toHaveLength(0);
	});
});
