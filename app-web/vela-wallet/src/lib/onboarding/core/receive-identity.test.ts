/**
 * §37 (review 1-7): the stored account's `identity` survives every save, the
 * Receive screens ask the core whether it may be received at, and the
 * chain-only re-check saves what it found through the core — only when the
 * core says to check it first.
 */
import '$lib/i18n/wasm-init.server';
import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('$lib/core/client', async (original) => ({
	...(await original<typeof import('$lib/core/client')>()),
	loadCore: async () => {},
	whenCoreLoaded: async () => {}
}));
const chain = vi.hoisted(() => ({
	says: 'confirmed' as 'confirmed' | 'unconfirmed' | 'refuted',
	asked: [] as [string, string][]
}));
vi.mock('./registry', () => ({
	confirmIdentity: vi.fn(async (publicKeyHex: string, address: string) => {
		chain.asked.push([publicKeyHex, address]);
		return { identity: chain.says, verifiedBy: chain.says === 'unconfirmed' ? 'none' : 'gnosis' };
	})
}));

import { accountWithIdentity } from '$lib/core/kernels';
import type { Account } from '../generated/Account';
import { receiveIdentity } from './receive-identity.svelte';
import { readStoredAccounts, saveAccount, STORAGE_KEYS } from './storage';

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

const KEY_A = '04' + '11'.repeat(64);
const KEY_B = '04' + '22'.repeat(64);
const RECORD: Account = {
	id: 'cred-a',
	name: 'Group',
	address: '0x3333333333333333333333333333333333333333',
	public_key_hex: KEY_A,
	created_at_iso: '2026-11-08T00:00:00.000Z',
	keys: [
		{ credential_id: 'cred-a', public_key_hex: KEY_A, name: 'A', transports: 'internal' },
		{ credential_id: 'cred-b', public_key_hex: KEY_B, name: 'B', transports: 'internal' }
	],
	signing_domain: 'getvela.app',
	signing_venue: { type: 'in_vela' }
} as unknown as Account;

const stored = () =>
	JSON.parse(localStorage.getItem(STORAGE_KEYS.accounts) ?? '[]') as Record<string, unknown>[];

beforeEach(() => {
	(globalThis as { localStorage?: Storage }).localStorage = new MemoryStorage();
	chain.says = 'confirmed';
	chain.asked.length = 0;
	receiveIdentity.refresh();
});

describe('the stored identity', () => {
	it('survives a save and a load; confirmed is never written', () => {
		saveAccount({ ...RECORD, identity: 'unconfirmed' });
		expect(stored()[0].identity).toBe('unconfirmed');
		expect(readStoredAccounts()[0].identity).toBe('unconfirmed');
		// A re-save of the record as read keeps it — a dropped field would confirm silently.
		saveAccount({ ...readStoredAccounts()[0], name: 'Renamed' });
		expect(stored()[0]).toMatchObject({ identity: 'unconfirmed', name: 'Renamed' });
		// What the core writes for a confirmed one: no field at all.
		saveAccount(accountWithIdentity(readStoredAccounts()[0], 'confirmed')!);
		expect('identity' in stored()[0]).toBe(false);
	});
});

describe('receiveIdentity', () => {
	it('a confirmed account is received at; an unconfirmed one is held, with the core’s keys', () => {
		saveAccount(RECORD);
		expect(receiveIdentity.held(RECORD.address)).toBeNull();
		saveAccount({ ...RECORD, identity: 'unconfirmed' });
		receiveIdentity.refresh();
		expect(receiveIdentity.held(RECORD.address)).toEqual({
			receivable: false,
			identity: 'unconfirmed',
			title_key: 'receive.unconfirmed.title',
			body_key: 'receive.unconfirmed.body',
			check_first: true
		});
	});

	it('an address no stored Vela account is at has no standing (a multisig): Receive as ever', () => {
		saveAccount(RECORD);
		expect(receiveIdentity.standing('0x' + '44'.repeat(20))).toBeNull();
	});

	it('the chain confirms it: saved through the core, and Receive opens', async () => {
		saveAccount({ ...RECORD, identity: 'unconfirmed' });
		receiveIdentity.refresh();
		await receiveIdentity.check(RECORD.address);
		// The founding key, and the account's own address — never the index.
		expect(chain.asked).toEqual([[KEY_A, RECORD.address]]);
		expect('identity' in stored()[0]).toBe(false);
		expect(receiveIdentity.held(RECORD.address)).toBeNull();
	});

	it('the chain refutes it: saved refuted, and nothing to check again', async () => {
		chain.says = 'refuted';
		saveAccount({ ...RECORD, identity: 'unconfirmed' });
		receiveIdentity.refresh();
		await receiveIdentity.check(RECORD.address);
		expect(stored()[0].identity).toBe('refuted');
		expect(receiveIdentity.held(RECORD.address)?.check_first).toBe(false);
		await receiveIdentity.check(RECORD.address);
		expect(chain.asked).toHaveLength(1);
	});

	it('no chain answered: nothing changes, and it stays held', async () => {
		chain.says = 'unconfirmed';
		saveAccount({ ...RECORD, identity: 'unconfirmed', name: 'Kept' });
		receiveIdentity.refresh();
		const before = localStorage.getItem(STORAGE_KEYS.accounts);
		await receiveIdentity.check(RECORD.address);
		expect(localStorage.getItem(STORAGE_KEYS.accounts)).toBe(before);
		expect(receiveIdentity.held(RECORD.address)?.identity).toBe('unconfirmed');
	});

	it('a confirmed account is never walked', async () => {
		saveAccount(RECORD);
		await receiveIdentity.check(RECORD.address);
		expect(chain.asked).toEqual([]);
	});
});
