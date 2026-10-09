/**
 * Spec 048: the retired client wrote `vela.accounts` in camelCase at the same
 * origin this shell now serves. Reading it must not strand the person, and
 * the list must be rewritten once in the current spelling.
 */
import { beforeEach, describe, expect, it } from 'vitest';
import { loadAccounts, normaliseAccount, saveAccount, STORAGE_KEYS } from './storage';

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

	/**
	 * Founder, 2026-09-26: every signature reuses the key the account signed in
	 * with, and the record is the only place that is written down. A save that
	 * lost it would quietly hand the choice of key back to the browser. Spec 102
	 * renamed it (`sign_in_key`) and added the domain and venue beside it; an
	 * older build's `signed_in_with` copy rides along too.
	 */
	it('keeps the sign-in key, the domain and the venue through a save and a load', () => {
		const signedIn = {
			...CURRENT,
			keys: [
				...CURRENT.keys,
				{ credential_id: 'cred-4', public_key_hex: '04aa', name: 'YubiKey', transports: 'usb,nfc' }
			],
			sign_in_key: { credential_id: 'cred-4', method: 'security_key' as const },
			signing_domain: 'getvela.app',
			signing_venue: { type: 'page' as const, url: 'https://sign.getvela.app/' },
			signed_in_with: { credential_id: 'cred-4', method: 'security_key' as const }
		};
		saveAccount(signedIn);
		expect(loadAccounts()).toEqual([signedIn]);
		// Signing in again with another key re-saves the held record by id.
		const again = {
			...signedIn,
			sign_in_key: { credential_id: 'cred-3', method: 'hybrid' as const },
			signing_venue: { type: 'in_vela' as const }
		};
		saveAccount(again);
		expect(loadAccounts()).toEqual([again]);
	});
});

describe('normaliseAccount', () => {
	it('returns the same object when nothing changes', () => {
		expect(normaliseAccount(CURRENT)).toBe(CURRENT);
	});

	/**
	 * Spec 075 wrote the page a key was minted on beside it. Spec 102 routes by
	 * the account's domain and venue instead, but the core's reader migrates a
	 * record from the key's origin, and an older build still opens the page it
	 * names — so the field survives every normalisation.
	 */
	it('carries the page a key was minted on through a rewrite', () => {
		const behind = normaliseAccount({
			...EXPO_WITH_KEYS,
			keys: [
				{
					credentialId: 'cred-1',
					publicKeyHex: '04ab',
					name: 'Ann',
					signerOrigin: 'https://sign.getvela.app'
				}
			]
		});
		expect(behind?.keys[0].signer_origin).toBe('https://sign.getvela.app');
		const already = normaliseAccount({
			...EXPO_WITH_KEYS,
			keys: [{ credentialId: 'cred-1', publicKeyHex: '04ab', signer_origin: 'https://me.example' }]
		});
		expect(already?.keys[0].signer_origin).toBe('https://me.example');
		// A key that lives nowhere special carries no field at all.
		expect('signer_origin' in (normaliseAccount(EXPO_WITH_KEYS)?.keys[0] ?? {})).toBe(false);
	});

	/**
	 * Spec 102: what a record MEANS is the core reader's to say, so a rewrite
	 * carries every field it does not respell — the sign-in key under either
	 * name, the domain, the venue. A hand copy is how one of them got dropped.
	 */
	it('carries the sign-in key, the domain and the venue through a rewrite', () => {
		const signedInWith = { credential_id: 'cred-2', method: 'hybrid' as const };
		const out = normaliseAccount({ ...EXPO_WITHOUT_KEYS, signed_in_with: signedInWith });
		expect((out as unknown as Record<string, unknown>).signed_in_with).toEqual(signedInWith);
		expect(out?.keys).toEqual([]);
		const venue = { type: 'page', url: 'https://sign.example.com/' };
		const current = normaliseAccount({
			...EXPO_WITHOUT_KEYS,
			sign_in_key: signedInWith,
			signing_domain: 'sign.example.com',
			signing_venue: venue
		});
		expect(current?.sign_in_key).toEqual(signedInWith);
		expect(current?.signing_domain).toBe('sign.example.com');
		expect(current?.signing_venue).toEqual(venue);
		// A record that never named one carries no field at all.
		expect('signed_in_with' in (normaliseAccount(EXPO_WITHOUT_KEYS) ?? {})).toBe(false);
		expect('sign_in_key' in (normaliseAccount(EXPO_WITHOUT_KEYS) ?? {})).toBe(false);
	});

	it('respells the old fields and keeps the rest of an old record', () => {
		const out = normaliseAccount({ ...EXPO_WITHOUT_KEYS, extra: 1 });
		expect(out?.created_at_iso).toBe('2026-08-01T00:00:00.000Z');
		const raw = out as unknown as Record<string, unknown>;
		expect(raw.createdAt).toBeUndefined();
		expect(raw.publicKeyHex).toBeUndefined();
		expect(raw.extra).toBe(1);
	});
});
