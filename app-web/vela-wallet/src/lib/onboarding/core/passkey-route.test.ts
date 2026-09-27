/**
 * What each ceremony hands the browser for the method the person chose
 * (founder, 2026-09-26: the create and sign-in screens ask where the passkey
 * is, so the ceremony goes there). One mapping serves every ceremony — WebAuthn
 * `hints`, plus the authenticator attachment when a key is minted — and no
 * method changes nothing, which is what keeps a record from before the sign-in
 * key signing byte-for-byte as it always did. A screenshot cannot show any of
 * this.
 */
import '$lib/i18n/wasm-init.server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { KeyMethod } from '../generated/KeyMethod';
import { createOnboardingExecutor } from './executor';
import { authenticate, methodRouting, register, sign, signWithAny } from './passkey';

type Asked = {
	hints?: string[];
	authenticatorSelection?: Record<string, unknown>;
	allowCredentials?: { transports?: string[] }[];
};
let asked: Asked[] = [];

beforeEach(() => {
	asked = [];
	const location = { protocol: 'https:', hostname: 'getvela.app' };
	vi.stubGlobal('location', location);
	vi.stubGlobal('window', { location, PublicKeyCredential: function () {} });
	vi.stubGlobal('PublicKeyCredential', function () {});
	// The ceremony is not what is under test: record what was asked, then refuse.
	const record = (options: { publicKey: Asked }) => {
		asked.push(options.publicKey);
		return Promise.reject(new DOMException('cancelled', 'NotAllowedError'));
	};
	vi.stubGlobal('navigator', { credentials: { get: record, create: record } });
});

afterEach(() => {
	vi.unstubAllGlobals();
});

const CHOSEN = {
	platform: { hint: 'client-device', attachment: 'platform' },
	hybrid: { hint: 'hybrid', attachment: 'cross-platform' },
	security_key: { hint: 'security-key', attachment: 'cross-platform' }
} as const;
const METHODS = Object.keys(CHOSEN) as (keyof typeof CHOSEN)[];

describe('the one mapping', () => {
	it('names a hint and an attachment for each place a passkey can be', () => {
		for (const method of METHODS) {
			expect(methodRouting(method), method).toEqual({
				hints: [CHOSEN[method].hint],
				attachment: CHOSEN[method].attachment
			});
		}
	});

	it('the Trusted Signer, or no method, adds nothing', () => {
		expect(methodRouting('trusted_signer')).toEqual({});
		expect(methodRouting(undefined)).toEqual({});
	});
});

describe('minting a key', () => {
	it('goes where it was asked to live', async () => {
		for (const method of METHODS) {
			await register('Ann', [], method).catch(() => {});
			const request = asked.at(-1);
			expect(request?.hints, method).toEqual([CHOSEN[method].hint]);
			expect(request?.authenticatorSelection, method).toMatchObject({
				authenticatorAttachment: CHOSEN[method].attachment,
				residentKey: 'required',
				userVerification: 'required'
			});
		}
	});

	it('with no method, is what it always was', async () => {
		await register('Ann', []).catch(() => {});
		const request = asked.at(-1) ?? {};
		expect('hints' in request).toBe(false);
		expect('authenticatorAttachment' in (request.authenticatorSelection ?? {})).toBe(false);
	});
});

describe('signing in', () => {
	it('looks where the person said', async () => {
		for (const method of METHODS) {
			await authenticate(method).catch(() => {});
			expect(asked.at(-1)?.hints, method).toEqual([CHOSEN[method].hint]);
		}
	});
});

describe('signing with one key', () => {
	it('carries the method’s hint and exactly the transports handed over', async () => {
		await sign('00', 'aabb', 'usb,nfc,ble', 'security_key').catch(() => {});
		expect(asked.at(-1)?.hints).toEqual(['security-key']);
		expect(asked.at(-1)?.allowCredentials?.[0].transports).toEqual(['usb', 'nfc', 'ble']);
	});

	it('with no method: no hint, and nothing invented about where it lives', async () => {
		await sign('00', 'aabb', 'internal,usb,nfc,ble,hybrid').catch(() => {});
		expect('hints' in (asked.at(-1) ?? {})).toBe(false);
		expect(asked.at(-1)?.allowCredentials?.[0].transports).toEqual(
			'internal,usb,nfc,ble,hybrid'.split(',')
		);
		await sign('00', 'aabb').catch(() => {});
		expect(asked.at(-1)?.allowCredentials?.[0].transports).toBeUndefined();
	});
});

describe('any key (a record from before the sign-in key)', () => {
	it('the request is what it always was', async () => {
		await signWithAny('00', [{ id: 'aabb', transports: 'internal' }, { id: 'ccdd' }]).catch(
			() => {}
		);
		const request = asked.at(-1) ?? {};
		expect('hints' in request).toBe(false);
		expect(request.allowCredentials?.map((c) => c.transports)).toEqual([['internal'], undefined]);
	});
});

/**
 * The core says which method each ceremony is for; the executor must hand it
 * over. The screens asked, and a ceremony that dropped the answer would be the
 * bug this file exists for.
 */
describe('the onboarding executor', () => {
	const execute = createOnboardingExecutor({
		prompt: async () => false,
		complete: async () => {}
	});
	const run = (operation: Parameters<typeof execute>[0]['operation']) =>
		execute({ id: 1, operation }, new AbortController().signal).catch(() => {});

	it('mints, signs in and proves over the method the core names', async () => {
		for (const method of METHODS as KeyMethod[]) {
			const hint = CHOSEN[method as keyof typeof CHOSEN].hint;
			await run({ type: 'register_passkey', name: 'Ann', exclude_credential_ids: [], method });
			expect(asked.at(-1)?.hints, `create ${method}`).toEqual([hint]);
			await run({ type: 'authenticate_passkey', method });
			expect(asked.at(-1)?.hints, `sign in ${method}`).toEqual([hint]);
			await run({
				type: 'sign_proof',
				credential_id: 'aabb',
				transports: 'internal',
				method,
				purpose: 'verify'
			});
			expect(asked.at(-1)?.hints, `proof ${method}`).toEqual([hint]);
		}
	});
});
