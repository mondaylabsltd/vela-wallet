/**
 * Where a signature goes (founder, 2026-09-26): the key the account signed in
 * with, over the route it signed in over — never a choice made per signature.
 * The account record says which, the real core reads it (`signInRoute`), and
 * this asserts what the browser is then actually asked.
 *
 * A record written before it named its sign-in key must sign exactly as it
 * always did: every founding key allowed with its own transports, no hint, the
 * browser left to pick.
 */
import '$lib/i18n/wasm-init.server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { bytesToHex } from '$lib/onboarding/core/passkey';
import { saveAccount } from '$lib/onboarding/core/storage';
import type { Account } from '$lib/onboarding/generated/Account';
import { signChallenge, type ChallengeSigner } from './sign-challenge';

type Asked = {
	hints?: string[];
	allowCredentials?: { id: Uint8Array; transports?: string[] }[];
};
let asked: Asked[] = [];

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

beforeEach(() => {
	asked = [];
	const location = { protocol: 'https:', hostname: 'getvela.app' };
	vi.stubGlobal('localStorage', new MemoryStorage());
	vi.stubGlobal('location', location);
	vi.stubGlobal('window', { location, PublicKeyCredential: function () {} });
	vi.stubGlobal('PublicKeyCredential', function () {});
	vi.stubGlobal('navigator', {
		credentials: {
			get: (options: { publicKey: Asked }) => {
				asked.push(options.publicKey);
				// The ceremony is not what is under test: refuse after recording.
				return Promise.reject(new DOMException('cancelled', 'NotAllowedError'));
			}
		}
	});
});

afterEach(() => {
	vi.unstubAllGlobals();
});

const ADDRESS = '0x2222222222222222222222222222222222222222';

/** Two founding keys: a built-in passkey, then a YubiKey. */
function wallet(signedInWith?: Account['signed_in_with']): Account {
	return {
		id: 'aa01',
		name: 'Ann',
		address: ADDRESS,
		public_key_hex: '04' + '11'.repeat(64),
		created_at_iso: '2026-09-26T00:00:00.000Z',
		keys: [
			{
				credential_id: 'aa01',
				public_key_hex: '04' + '11'.repeat(64),
				name: 'Mac',
				transports: 'internal'
			},
			{
				credential_id: 'bb02',
				public_key_hex: '04' + '22'.repeat(64),
				name: 'YubiKey',
				transports: 'usb,nfc'
			}
		],
		...(signedInWith === undefined ? {} : { signed_in_with: signedInWith })
	};
}

/** What each path hands over, as it built it before: every key, each with what it registered. */
const SIGNER: ChallengeSigner = {
	account: ADDRESS,
	keys: [],
	credentials: [{ id: 'aa01', transports: 'internal' }, { id: 'bb02' }],
	request: { method: '', params: [], origin: '', chainId: 100 }
};

async function attempt(): Promise<{ asked: Asked | undefined; error: unknown }> {
	const error = await signChallenge(new Uint8Array([1, 2, 3]), SIGNER).then(
		() => null,
		(reason: unknown) => reason
	);
	return { asked: asked.at(-1), error };
}

const pinned = (request: Asked | undefined) =>
	request?.allowCredentials?.map((c) => ({ id: bytesToHex(c.id), transports: c.transports }));

describe('an account that names its sign-in key', () => {
	it('signs with THAT key over THAT route — the second key, as a security key', async () => {
		saveAccount(wallet({ credential_id: 'bb02', method: 'security_key' }));
		const { asked: request } = await attempt();
		expect(asked).toHaveLength(1);
		expect(request?.hints).toEqual(['security-key']);
		// Not keys[0], and not every key for the browser to choose between.
		expect(pinned(request)).toEqual([{ id: 'bb02', transports: ['usb', 'nfc', 'ble'] }]);
	});

	it('reaches a passkey registered on this device over a phone when that is how it signed in', async () => {
		saveAccount(wallet({ credential_id: 'aa01', method: 'hybrid' }));
		const { asked: request } = await attempt();
		expect(request?.hints).toEqual(['hybrid']);
		expect(pinned(request)).toEqual([{ id: 'aa01', transports: ['hybrid', 'internal'] }]);
	});

	it('refuses a key behind a Trusted Signer page — the web has none — without a ceremony', async () => {
		saveAccount(
			wallet({
				credential_id: 'bb02',
				method: 'trusted_signer',
				signer_origin: 'https://sign.example'
			})
		);
		const { error } = await attempt();
		expect(asked).toHaveLength(0);
		expect(String(error)).toContain('https://sign.example');
	});
});

describe('a record from before the sign-in key', () => {
	it('signs exactly as before: every key, its own transports, no hint', async () => {
		saveAccount(wallet());
		const { asked: request } = await attempt();
		expect(asked).toHaveLength(1);
		expect('hints' in (request ?? {})).toBe(false);
		expect(pinned(request)).toEqual([
			{ id: 'aa01', transports: ['internal'] },
			{ id: 'bb02', transports: undefined }
		]);
	});

	it('naming a key the wallet does not hold is read as no sign-in key at all', async () => {
		saveAccount(wallet({ credential_id: 'cc03', method: 'security_key' }));
		const { asked: request } = await attempt();
		expect('hints' in (request ?? {})).toBe(false);
		expect(pinned(request)?.map((c) => c.id)).toEqual(['aa01', 'bb02']);
	});

	it('with nothing stored for the address, the request is the path’s own', async () => {
		const { asked: request } = await attempt();
		expect(pinned(request)?.map((c) => c.id)).toEqual(['aa01', 'bb02']);
	});

	it('still refuses when its first key lives behind a Trusted Signer page', async () => {
		const behind = wallet();
		behind.keys[0] = { ...behind.keys[0], signer_origin: 'https://me.example' };
		saveAccount(behind);
		const { error } = await attempt();
		expect(asked).toHaveLength(0);
		expect(String(error)).toContain('https://me.example');
	});
});
