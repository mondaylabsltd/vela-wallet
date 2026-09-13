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
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
	ERASE_KEEP_KEYS,
	EraseIncompleteError,
	eraseDeviceData,
	isErasableKey,
	VELA_KEY_PREFIX
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

/** The IndexedDB KV, stubbed at the module seam the sweep goes through. */
const kv = new Map<string, string>();
vi.mock('./storage', () => ({
	getAllKeys: () => Promise.resolve([...kv.keys()]),
	removeItem: (key: string) => {
		kv.delete(key);
		return Promise.resolve();
	}
}));

beforeEach(() => {
	kv.clear();
	vi.unstubAllGlobals();
});

describe('the rule, stated as a rule', () => {
	it('erases the whole namespace — including keys written after this test', () => {
		// The point of the prefix: a feature shipped next year is covered on the
		// day it first writes, with no edit here and no line in a list.
		expect(isErasableKey('vela.contacts')).toBe(true);
		expect(isErasableKey('vela.somethingNobodyHasWrittenYet')).toBe(true);
		expect(isErasableKey('vela.perm.https://example.com')).toBe(true);
		expect(VELA_KEY_PREFIX).toBe('vela.');
	});

	it('leaves what is not ours alone', () => {
		expect(isErasableKey('theme')).toBe(false);
		expect(isErasableKey('velaXYZ')).toBe(false);
		expect(isErasableKey('not.vela.contacts')).toBe(false);
	});

	it('keeps exactly the one exception the contract names', () => {
		// contracts/erase-scope.md is the authority; this is that document as a
		// test. A second exception added without a reason fails here.
		expect(ERASE_KEEP_KEYS).toEqual(['vela.pendingUploads']);
		expect(isErasableKey('vela.pendingUploads')).toBe(false);
	});
});

describe('the sweep', () => {
	it('empties every store of everything under the namespace', async () => {
		const local = fakeLocalStorage({
			'vela.accounts': '[]',
			'vela.intro.seen': '1',
			'vela.pendingUploads': '[{"key":"unconfirmed"}]',
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
		// The outbox and the foreign keys stand.
		expect(local.snapshot()).toEqual({
			'vela.pendingUploads': '[{"key":"unconfirmed"}]',
			dev_unlocked: '1'
		});
		expect([...kv.keys()]).toEqual(['somethingElse']);
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

	it('a store this build does not have contributes nothing', async () => {
		// `chrome.storage.local` exists only in the extension build. Its absence
		// is not a failure — and must not be reported as one.
		vi.stubGlobal('localStorage', fakeLocalStorage({}));
		await expect(eraseDeviceData()).resolves.toEqual([]);
	});
});
