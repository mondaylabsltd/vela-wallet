// The app's one address book (founder, 2026-09-26: 通讯录 flashed unread on
// every visit, because each visit built a session of its own). One session,
// read once for the signed-in account, told again only when that changes.
import { beforeEach, describe, expect, it, vi } from 'vitest';

const seams = vi.hoisted(() => ({
	sessions: [] as {
		start: ReturnType<typeof vi.fn>;
		dispatch: ReturnType<typeof vi.fn>;
		dispose: ReturnType<typeof vi.fn>;
		onView: (view: unknown) => void;
	}[]
}));

vi.mock('$lib/core/client', () => ({ loadCore: vi.fn(async () => {}) }));
vi.mock('./contacts', () => ({
	createContactsSession: (options: { onView: (view: unknown) => void }) => {
		const session = {
			start: vi.fn(),
			dispatch: vi.fn(),
			dispose: vi.fn(),
			onView: options.onView
		};
		seams.sessions.push(session);
		return session;
	}
}));

const A = '0x' + 'aa'.repeat(20);
const B = '0x' + 'bb'.repeat(20);

async function freshBook() {
	vi.resetModules();
	seams.sessions.length = 0;
	return (await import('./contacts-book.svelte')).contactsBook;
}

beforeEach(() => {
	seams.sessions.length = 0;
});

describe('contactsBook — one session for the app', () => {
	it('reads the book once for the signed-in account, however often it is told', async () => {
		const book = await freshBook();
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
		const book = await freshBook();
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
		const book = await freshBook();
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
		const book = await freshBook();
		await book.setAccount(A);
		await book.setAccount(B);
		book.reload();
		await vi.waitFor(() => expect(seams.sessions).toHaveLength(2));
		expect(seams.sessions[0].dispose).toHaveBeenCalledOnce();
		expect(book.view).toBeNull();
		expect(seams.sessions[1].start).toHaveBeenCalledExactlyOnceWith({
			type: 'account_switched',
			my_address: B
		});
		// …and the same account is still not told twice afterwards.
		await book.setAccount(B);
		expect(seams.sessions[1].dispatch).not.toHaveBeenCalled();
	});
});
