/**
 * The erase, and the list it is not (spec 028 T435 — contracts/erase-scope.md).
 *
 * The predecessor of this module was a hand-maintained list of eleven keys, and
 * it drifted out of date in silence, because **nothing about a delete-list
 * fails when the app grows a key**. So the property under test is not "these
 * keys are deleted" — that is the same mistake with a test around it. It is:
 *
 *   a key nobody has written yet is erased by default,
 *   the named exception survives, and
 *   a survivor is REPORTED rather than reported as success.
 *
 * The rule is the core's since spec 072 (`storage_catalog::is_erasable_key`),
 * so these run over the real one.
 */
import '$lib/i18n/wasm-init.server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
	ERASE_HELD,
	EraseIncompleteError,
	KV_DATABASE,
	eraseDeviceData,
	isErasableKey
} from './erase-device';

/** An in-memory `localStorage`, with the two methods a sweep needs. */
function fakeLocalStorage(seed: Record<string, string>) {
	const map = new Map(Object.entries(seed));
	return {
		get length() {
			return map.size;
		},
		key: (i: number) => [...map.keys()][i] ?? null,
		getItem: (k: string) => map.get(k) ?? null,
		setItem: (k: string, v: string) => void map.set(k, v),
		removeItem: (k: string) => void map.delete(k),
		clear: () => map.clear(),
		snapshot: () => Object.fromEntries(map)
	};
}

/**
 * The IndexedDB KV, stubbed at the module seam the sweep goes through — with
 * the seal the real one has (`storage.ts`): a write is refused while sealed.
 * `whileRemoving` is the rest of the wallet, still running during the sweep.
 */
const kv = new Map<string, string>();
const store = {
	sealed: false,
	whileRemoving: (): void => {},
	deleted: 0,
	deleteFails: false,
	deleteHeld: false
};
const write = (key: string, value: string): Promise<void> => {
	if (store.sealed) return Promise.reject(new Error(`sealed: ${key}`));
	kv.set(key, value);
	return Promise.resolve();
};
/** The real module's error for a delete another connection holds back (`storage.ts`). */
const { DatabaseHeldError } = vi.hoisted(() => ({
	DatabaseHeldError: class DatabaseHeldError extends Error {}
}));
vi.mock('./storage', () => ({
	DatabaseHeldError,
	getAllKeys: () => Promise.resolve([...kv.keys()]),
	removeItem: (key: string) => {
		kv.delete(key);
		store.whileRemoving();
		return Promise.resolve();
	},
	sealStorage: () => {
		store.sealed = true;
	},
	// The whole database goes, foreign keys and all (ob-15).
	deleteDatabase: () => {
		if (store.deleteHeld) return Promise.reject(new DatabaseHeldError());
		if (store.deleteFails) return Promise.reject(new Error('indexedDB delete failed'));
		kv.clear();
		store.deleted += 1;
		return Promise.resolve();
	},
	unsealStorage: () => {
		store.sealed = false;
	}
}));

beforeEach(() => {
	kv.clear();
	store.sealed = false;
	store.whileRemoving = () => {};
	store.deleted = 0;
	store.deleteFails = false;
	store.deleteHeld = false;
	vi.unstubAllGlobals();
});

describe('the rule, stated as a rule', () => {
	it('erases the whole namespace — including keys written after this test', () => {
		// The point of the prefix: a feature shipped next year is covered on the
		// day it first writes, with no edit here and no line in a list.
		expect(isErasableKey('vela.contacts')).toBe(true);
		expect(isErasableKey('vela.somethingNobodyHasWrittenYet')).toBe(true);
		expect(isErasableKey('vela.perm.https://example.com')).toBe(true);
		// The one cache written outside the namespace is ours too: the web's own
		// rule used to leave the recipient lookups behind.
		expect(isErasableKey('recipient_id:0xabc')).toBe(true);
	});

	it('leaves what is not ours alone', () => {
		expect(isErasableKey('theme')).toBe(false);
		expect(isErasableKey('velaXYZ')).toBe(false);
		expect(isErasableKey('not.vela.contacts')).toBe(false);
	});

	it('keeps nothing of ours — the pending-upload outbox included', () => {
		// Erasing means everything (owner, 2026-10-28). The outbox used to be
		// the one exception; the confirmation now names the wallets it would
		// lose instead, and asks for an explicit confirm.
		expect(isErasableKey('vela.pendingUploads')).toBe(true);
	});
});

describe('the sweep', () => {
	it('empties every store of everything under the namespace', async () => {
		const local = fakeLocalStorage({
			'vela.accounts': '[]',
			'vela.intro.seen': '1',
			'vela.pendingUploads': '[{"key":"unconfirmed"}]',
			'recipient_id:0xabc': '{}',
			dev_unlocked: '1'
		});
		vi.stubGlobal('localStorage', local);
		kv.set('vela.contacts', '[]');
		kv.set('vela.transactionHistory', '[]');
		kv.set('somethingElse', 'x');

		const removed = await eraseDeviceData();

		expect(removed).toContain('vela.accounts');
		expect(removed).toContain('vela.contacts');
		expect(removed).toContain('vela.transactionHistory');
		expect(removed).toContain('recipient_id:0xabc');
		expect(removed).toContain('vela.pendingUploads');
		// Only what is not ours stands in localStorage…
		expect(local.snapshot()).toEqual({ dev_unlocked: '1' });
		// …and the IndexedDB database is gone whole, not left empty.
		expect(store.deleted).toBe(1);
		expect([...kv.keys()]).toEqual([]);
	});

	it('a database that will not go is an erase that did not finish', async () => {
		// Swept and verified, but the database itself survived (another tab
		// held it past the wait): said, and the store given back.
		vi.stubGlobal('localStorage', fakeLocalStorage({ 'vela.accounts': '[]' }));
		store.deleteFails = true;

		await expect(eraseDeviceData()).rejects.toMatchObject({
			remaining: [KV_DATABASE],
			held: false
		});
		expect(store.sealed).toBe(false);
	});

	it('held open by another tab, says that — the one thing that lets the next try finish', async () => {
		// Everything of ours is gone, verified; only the emptied database is
		// left, its delete queued behind a connection that will not close. Not
		// "some data is still here": close Vela there, then try again.
		const local = fakeLocalStorage({ 'vela.accounts': '[]' });
		vi.stubGlobal('localStorage', local);
		kv.set('vela.contacts', '[]');
		store.deleteHeld = true;

		const erased = eraseDeviceData();
		await expect(erased).rejects.toMatchObject({ remaining: [KV_DATABASE], held: true });
		await expect(erased).rejects.toThrow(ERASE_HELD);
		expect(local.snapshot()).toEqual({});
		expect([...kv.keys()]).toEqual([]);
		expect(store.sealed).toBe(false);

		// Let go: the next try finishes.
		store.deleteHeld = false;
		await expect(eraseDeviceData()).resolves.toEqual([]);
		expect(store.deleted).toBe(1);
	});

	it('REJECTS when something survives, instead of claiming a clean device', async () => {
		// The one outcome this feature cannot have is telling a person their
		// browser is clean while their transaction history is still in it.
		const stubborn = fakeLocalStorage({ 'vela.accounts': '[]' });
		stubborn.removeItem = () => {};
		vi.stubGlobal('localStorage', stubborn);

		await expect(eraseDeviceData()).rejects.toBeInstanceOf(EraseIncompleteError);
		await expect(eraseDeviceData()).rejects.toMatchObject({
			remaining: ['vela.accounts']
		});
	});

	it('a cache the wallet writes behind the sweep is refused — the erase is not "incomplete" for it', async () => {
		// A rates refresh was on its way when the person tapped Erase; its
		// answer lands while the sweep is removing keys, and its cache is
		// written fire-and-forget (`fiat-fx.ts`). Unsealed, that key is there at
		// the check and the person reads "the erase did not finish" — which is
		// what the e2e suite caught, with `vela.fxRates.v1` in its hands.
		vi.stubGlobal('localStorage', fakeLocalStorage({ 'vela.accounts': '[]' }));
		kv.set('vela.contacts', '[]');
		kv.set('vela.transactionHistory', '[]');
		const refused: string[] = [];
		store.whileRemoving = () => {
			void write('vela.fxRates.v1', '{"rates":{}}').catch(() => refused.push('vela.fxRates.v1'));
		};

		await expect(eraseDeviceData()).resolves.toEqual([
			'vela.accounts',
			'vela.contacts',
			'vela.transactionHistory'
		]);
		expect([...kv.keys()]).toEqual([]);
		// Refused, not swallowed: once per key the sweep removed.
		expect(refused).toEqual(['vela.fxRates.v1', 'vela.fxRates.v1']);
		// And still sealed: the caller replaces the document next, and nothing
		// is to land in an erased store before it does.
		expect(store.sealed).toBe(true);
		await expect(write('vela.late', 'x')).rejects.toThrow('sealed');
	});

	it('an erase that did NOT go through gives the store back: the wallet is still in use', async () => {
		const stubborn = fakeLocalStorage({ 'vela.accounts': '[]' });
		stubborn.removeItem = () => {};
		vi.stubGlobal('localStorage', stubborn);

		await expect(eraseDeviceData()).rejects.toBeInstanceOf(EraseIncompleteError);
		expect(store.sealed).toBe(false);
		await expect(write('vela.contacts', '[]')).resolves.toBeUndefined();
		expect(kv.get('vela.contacts')).toBe('[]');
	});

	it('a store this build does not have contributes nothing', async () => {
		// `chrome.storage.local` exists only in the extension build. Its absence
		// is not a failure — and must not be reported as one.
		vi.stubGlobal('localStorage', fakeLocalStorage({}));
		await expect(eraseDeviceData()).resolves.toEqual([]);
	});
});
