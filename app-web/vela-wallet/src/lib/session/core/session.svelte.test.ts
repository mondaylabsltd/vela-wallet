/**
 * The session follows the OTHER documents of its origin (spec 086, issue #315).
 *
 * The extension's side panel and a wallet tab are two documents, and each
 * restored the session once. The person switched to "26" in the tab; the
 * panel went on believing the previous account was signed in — and answered
 * a dApp, and published the worker's snapshot, as that account. Here the
 * real session core boots over real localStorage, and another document's
 * write arrives the way the browser delivers it: a `storage` event.
 */
import { beforeAll, describe, expect, it, vi } from 'vitest';
import { loadOnboardingCore } from '$lib/onboarding/core/wasm-client';
import { fixtureStoredAccounts } from '$lib/dev/parallel-space';
import { STORAGE_KEYS } from '$lib/onboarding/core/storage';
import { session } from './session.svelte';

let accounts: ReturnType<typeof fixtureStoredAccounts>;

beforeAll(async () => {
	await loadOnboardingCore();
	accounts = fixtureStoredAccounts();
	// What this document finds when it boots: the fixture wallets, the
	// second one signed in.
	localStorage.setItem(STORAGE_KEYS.accounts, JSON.stringify(accounts));
	localStorage.setItem(STORAGE_KEYS.activeAccountIndex, '1');
});

/** Another document of this origin writes `key` — the browser's own event. */
function otherDocumentWrites(key: string, value: string | null): void {
	if (value === null) localStorage.removeItem(key);
	else localStorage.setItem(key, value);
	window.dispatchEvent(
		new StorageEvent('storage', { key, newValue: value, storageArea: localStorage })
	);
}

// One document, one session (the module is its singleton): the cases run in
// order, each starting from where the last one left the session.
describe('the session other documents change (#315)', () => {
	it('settled() answers with the restored account, never the restore’s in-between view', async () => {
		const view = await session.settled();
		expect(view.loading).toBe(false);
		expect(view.address).toBe(accounts[1].address);
	});

	it('follows a switch made in another document', async () => {
		otherDocumentWrites(STORAGE_KEYS.activeAccountIndex, '0');
		await vi.waitFor(() => expect(session.view.address).toBe(accounts[0].address));
		expect((await session.settled()).address).toBe(accounts[0].address);
	});

	it('is not restarted by a write it does not read', async () => {
		const before = await session.settled();
		otherDocumentWrites('vela.theme', 'dark');
		await new Promise((resolve) => setTimeout(resolve, 50));
		expect(session.view).toBe(before);
	});

	it('follows a sign-out, and a sign-in to another wallet, made in another document', async () => {
		otherDocumentWrites(STORAGE_KEYS.accounts, null);
		otherDocumentWrites(STORAGE_KEYS.activeAccountIndex, null);
		await vi.waitFor(() => expect(session.view.has_wallet).toBe(false));
		expect(session.view.address).toBe('');
		expect((await session.settled()).address).toBe('');

		otherDocumentWrites(STORAGE_KEYS.accounts, JSON.stringify([accounts[2]]));
		await vi.waitFor(() => expect(session.view.address).toBe(accounts[2].address));
	});
});
