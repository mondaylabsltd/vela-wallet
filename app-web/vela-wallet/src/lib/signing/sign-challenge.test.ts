/**
 * Where a signature goes (founder, 2026-09-26): the key the account signed in
 * with, over the route it signed in over — never a choice made per signature.
 * The account record says which, the real core reads it (`signingPlan`, spec
 * 102 — migrating a record from before), and
 * this asserts what the browser is then actually asked: that one credential,
 * over the transports the core names, with the chosen method's hint only when
 * the sign-in found the key where it was chosen — a key found somewhere else is
 * not steered away from it.
 *
 * A record written before it named its sign-in key must sign exactly as it
 * always did: every founding key allowed with its own transports, no hint, the
 * browser left to pick.
 *
 * And spec 102's P2-11: the web opens no signing page. A `getvela.app` account
 * whose venue is a page signs here natively; an account on a custom signing
 * domain is refused with the core's own reason, before any ceremony.
 */
import '$lib/i18n/wasm-init.server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { bytesToHex } from '$lib/onboarding/core/passkey';
import { saveAccount } from '$lib/onboarding/core/storage';
import type { Account } from '$lib/onboarding/generated/Account';
import { signChallenge, VenueBlockedError, type ChallengeSigner } from './sign-challenge';

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

/** The sign-in key as an older build wrote it (`signed_in_with`), which the core migrates. */
type LegacySignIn = {
	credential_id: string;
	method: 'platform' | 'hybrid' | 'security_key' | 'trusted_signer';
	transports?: string;
	signer_origin?: string;
};

/** Two founding keys: a built-in passkey, then a YubiKey — as an older build stored them. */
function wallet(signedInWith?: LegacySignIn): Account {
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
	} as unknown as Account;
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
	(request?.allowCredentials ?? []).map((c) => ({
		id: bytesToHex(c.id),
		transports: c.transports
	}));

describe('an account that names its sign-in key', () => {
	it('signs with THAT key over THAT route — the second key, as a security key', async () => {
		saveAccount(
			wallet({ credential_id: 'bb02', method: 'security_key', transports: 'usb,nfc,ble,hybrid' })
		);
		const { asked: request } = await attempt();
		expect(asked).toHaveLength(1);
		// Not keys[0], and not every key for the browser to choose between.
		expect(pinned(request).map((c) => c.id)).toEqual(['bb02']);
	});

	it('found where it was chosen: the method’s hint, over the method’s transports', async () => {
		const cases = [
			['platform', 'internal', 'client-device', ['internal']],
			['hybrid', 'internal', 'hybrid', ['hybrid', 'internal']],
			['security_key', 'usb,nfc,ble', 'security-key', ['usb', 'nfc', 'ble']]
		] as const;
		for (const [method, found, hint, transports] of cases) {
			saveAccount(wallet({ credential_id: 'bb02', method, transports: found }));
			const { asked: request } = await attempt();
			expect(request?.hints, method).toEqual([hint]);
			expect(pinned(request), method).toEqual([{ id: 'bb02', transports: [...transports] }]);
		}
	});

	it('a record that says nothing about where it was found reads as found where chosen', async () => {
		saveAccount(wallet({ credential_id: 'aa01', method: 'hybrid' }));
		const { asked: request } = await attempt();
		expect(request?.hints).toEqual(['hybrid']);
		expect(pinned(request)).toEqual([{ id: 'aa01', transports: ['hybrid', 'internal'] }]);
	});

	it('"This device" answered by a phone or a key: every place named, and no hint', async () => {
		saveAccount(
			wallet({ credential_id: 'aa01', method: 'platform', transports: 'usb,nfc,ble,hybrid' })
		);
		const { asked: request } = await attempt();
		expect(asked).toHaveLength(1);
		expect('hints' in (request ?? {})).toBe(false);
		expect(pinned(request)).toEqual([
			{ id: 'aa01', transports: ['internal', 'usb', 'nfc', 'ble', 'hybrid'] }
		]);
	});

	it('a record this build wrote (sign_in_key, domain, venue) signs the same way', async () => {
		saveAccount({
			...wallet(),
			sign_in_key: { credential_id: 'bb02', method: 'security_key', transports: 'usb,nfc,ble' },
			signing_domain: 'getvela.app',
			signing_venue: { type: 'in_vela' }
		});
		const { asked: request } = await attempt();
		expect(request?.hints).toEqual(['security-key']);
		expect(pinned(request)).toEqual([{ id: 'bb02', transports: ['usb', 'nfc', 'ble'] }]);
	});
});

/**
 * P2-11: the web opens no page (owner, 2026-09-23). Whether an account can be
 * signed for here is the core's R1 asked of the web's one venue — in Vela.
 */
describe('the web opens no signing page', () => {
	it('a getvela.app account whose venue is the trusted page signs HERE, natively', async () => {
		saveAccount({
			...wallet(),
			sign_in_key: { credential_id: 'aa01', method: 'platform', transports: 'internal' },
			signing_domain: 'getvela.app',
			signing_venue: { type: 'page', url: 'https://sign.getvela.app/' }
		});
		const { asked: request, error } = await attempt();
		// The browser was asked (and refused, in this test) — the venue was not.
		expect(error).not.toBeInstanceOf(VenueBlockedError);
		expect(asked).toHaveLength(1);
		expect(pinned(request)).toEqual([{ id: 'aa01', transports: ['internal'] }]);
	});

	it('an older record signed in on the official page migrates to its PLACE and signs here', async () => {
		// ≤ 0.9.7 wrote the page as the method; the key reported `internal`.
		saveAccount(
			wallet({
				credential_id: 'aa01',
				method: 'trusted_signer',
				signer_origin: 'https://sign.getvela.app'
			})
		);
		const { asked: request } = await attempt();
		expect(asked).toHaveLength(1);
		expect(pinned(request).map((c) => c.id)).toEqual(['aa01']);
		expect(request?.hints).toEqual(['client-device']);
	});

	it('refuses a custom-domain account with the core’s reason — and no ceremony', async () => {
		const own = wallet({
			credential_id: 'bb02',
			method: 'trusted_signer',
			signer_origin: 'https://sign.example.com'
		});
		saveAccount(own);
		const { error } = await attempt();
		expect(asked).toHaveLength(0);
		expect(error).toBeInstanceOf(VenueBlockedError);
		expect((error as VenueBlockedError).block).toEqual({
			type: 'app_cannot_reach',
			domain: 'sign.example.com'
		});
		expect(String(error)).toContain('sign.example.com');
	});

	it('refuses a custom-domain account even when its own venue is the page', async () => {
		saveAccount({
			...wallet(),
			sign_in_key: { credential_id: 'aa01', method: 'platform' },
			signing_domain: 'sign.example.com',
			signing_venue: { type: 'page', url: 'https://sign.example.com/' }
		});
		const { error } = await attempt();
		expect(asked).toHaveLength(0);
		expect(error).toBeInstanceOf(VenueBlockedError);
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
		expect(pinned(request).map((c) => c.id)).toEqual(['aa01', 'bb02']);
	});

	it('with nothing stored for the address, the request is the path’s own', async () => {
		const { asked: request } = await attempt();
		expect(pinned(request).map((c) => c.id)).toEqual(['aa01', 'bb02']);
	});

	it('still refuses when its first key was minted on a custom-domain page', async () => {
		const behind = wallet() as unknown as { keys: Record<string, unknown>[] };
		behind.keys[0] = { ...behind.keys[0], signer_origin: 'https://me.example' };
		saveAccount(behind as unknown as Account);
		const { error } = await attempt();
		expect(asked).toHaveLength(0);
		expect(error).toBeInstanceOf(VenueBlockedError);
		expect(String(error)).toContain('me.example');
	});

	it('signs as before when its first key was minted on the official page', async () => {
		const behind = wallet() as unknown as { keys: Record<string, unknown>[] };
		behind.keys[0] = { ...behind.keys[0], signer_origin: 'https://sign.getvela.app' };
		saveAccount(behind as unknown as Account);
		const { asked: request } = await attempt();
		expect(asked).toHaveLength(1);
		expect(pinned(request).map((c) => c.id)).toEqual(['aa01', 'bb02']);
	});
});
