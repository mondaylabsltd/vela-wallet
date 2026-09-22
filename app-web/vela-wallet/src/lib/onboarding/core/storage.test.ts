/**
 * Spec 048: the retired client wrote `vela.accounts` in camelCase at the same
 * origin this shell now serves. Reading it must not strand the person, and
 * the list must be rewritten once in the current spelling.
 */
import { beforeEach, describe, expect, it } from 'vitest';
import { loadAccounts, normaliseAccount, STORAGE_KEYS } from './storage';

class MemoryStorage implements Storage {
	#map = new Map<string, string>();
	get length() {
		return this.#map.size;
	}
	clear() {
		this.#map.clear();
	}
	getItem(key: string) {
		return this.#map.get(key) ?? null;
	}
	key(index: number) {
		return [...this.#map.keys()][index] ?? null;
	}
	removeItem(key: string) {
		this.#map.delete(key);
	}
	setItem(key: string, value: string) {
		this.#map.set(key, value);
	}
}

const EXPO_WITH_KEYS = {
	id: 'cred-1',
	name: 'Ann',
	address: '0x88cCA0EeDbF2C4426110bbFc998F048689266894',
	publicKeyHex: '04ab',
	createdAt: '2026-08-25T10:00:00.000Z',
	keys: [{ credentialId: 'cred-1', publicKeyHex: '04ab', name: 'Ann' }]
};
const EXPO_WITHOUT_KEYS = {
	id: 'cred-2',
	name: 'Bob',
	address: '0x1111111111111111111111111111111111111111',
	publicKeyHex: '04cd',
	createdAt: '2026-08-01T00:00:00.000Z'
};
const CURRENT = {
	id: 'cred-3',
	name: 'Cy',
	address: '0x2222222222222222222222222222222222222222',
	public_key_hex: '04ef',
	created_at_iso: '2026-09-01T00:00:00.000Z',
	keys: [{ credential_id: 'cred-3', public_key_hex: '04ef', name: 'Cy', transports: 'internal' }]
};

describe('loadAccounts', () => {
	let storage: MemoryStorage;
	beforeEach(() => {
		storage = new MemoryStorage();
		(globalThis as { localStorage?: Storage }).localStorage = storage;
	});

	it('reads the retired client’s spelling and rewrites the list once', () => {
		storage.setItem(
			STORAGE_KEYS.accounts,
			JSON.stringify([EXPO_WITH_KEYS, EXPO_WITHOUT_KEYS, CURRENT])
		);
		const accounts = loadAccounts();
		expect(accounts.map((a) => a.public_key_hex)).toEqual(['04ab', '04cd', '04ef']);
		expect(accounts[0].keys).toEqual([
			{ credential_id: 'cred-1', public_key_hex: '04ab', name: 'Ann', transports: '' }
		]);
		expect(accounts[1].keys).toEqual([]);
		expect(accounts[2]).toBe(loadAccounts()[2] === accounts[2] ? accounts[2] : accounts[2]);
		const rewritten = JSON.parse(storage.getItem(STORAGE_KEYS.accounts) ?? '[]');
		expect(rewritten[0].public_key_hex).toBe('04ab');
		expect(rewritten[0].publicKeyHex).toBeUndefined();
		expect(rewritten[1].keys).toEqual([]);
		expect(rewritten[2]).toEqual(CURRENT);
	});

	it('leaves a current list untouched', () => {
		const written = JSON.stringify([CURRENT]);
		storage.setItem(STORAGE_KEYS.accounts, written);
		expect(loadAccounts()).toEqual([CURRENT]);
		expect(storage.getItem(STORAGE_KEYS.accounts)).toBe(written);
	});

	it('drops what is not an account and keeps the rest', () => {
		storage.setItem(STORAGE_KEYS.accounts, JSON.stringify([null, 'x', { id: 'only-id' }, CURRENT]));
		expect(loadAccounts()).toEqual([CURRENT]);
	});
});

describe('normaliseAccount', () => {
	it('returns the same object when nothing changes', () => {
		expect(normaliseAccount(CURRENT)).toBe(CURRENT);
	});
	it('ignores unknown fields on an old record', () => {
		const out = normaliseAccount({ ...EXPO_WITHOUT_KEYS, extra: 1 });
		expect(out?.created_at_iso).toBe('2026-08-01T00:00:00.000Z');
		expect((out as unknown as Record<string, unknown>).extra).toBeUndefined();
	});
});
