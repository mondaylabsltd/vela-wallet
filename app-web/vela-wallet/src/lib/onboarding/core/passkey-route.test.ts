/**
 * What a pinned route actually hands the browser (founder, 2026-09-26: an
 * account signs with the key it signed in with, over the route it signed in
 * over). A screenshot cannot show that "Security key" changes the request —
 * only this can — nor that a ceremony with no route changes nothing, which is
 * what keeps every record from before the sign-in key byte-for-byte the same.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { sign, signWithAny } from './passkey';

type Asked = {
	hints?: string[];
	allowCredentials?: { transports?: string[] }[];
};
let asked: Asked[] = [];

beforeEach(() => {
	asked = [];
	const location = { protocol: 'https:', hostname: 'getvela.app' };
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

const CREDENTIALS = [{ id: 'aabb', transports: 'internal' }, { id: 'ccdd' }];

describe('no route', () => {
	it('any key: the request is what it always was', async () => {
		await signWithAny('00', CREDENTIALS).catch(() => {});
		const request = asked.at(-1) ?? {};
		expect('hints' in request).toBe(false);
		expect(request.allowCredentials?.map((c) => c.transports)).toEqual([['internal'], undefined]);
	});

	it('one key, as a proof during create or sign-in: no hint, its own transports', async () => {
		await sign('00', 'aabb', 'hybrid,internal').catch(() => {});
		expect('hints' in (asked.at(-1) ?? {})).toBe(false);
		expect(asked.at(-1)?.allowCredentials?.[0].transports).toEqual(['hybrid', 'internal']);
	});
});

describe('the route the account signed in over', () => {
	it('is a WebAuthn hint, over the transports the route carries', async () => {
		const expected = {
			platform: ['client-device', 'internal'],
			// A credential registered as `internal` is never offered over a QR
			// code unless the request also says `hybrid`.
			hybrid: ['hybrid', 'hybrid,internal'],
			security_key: ['security-key', 'usb,nfc,ble']
		} as const;
		for (const [route, [hint, transports]] of Object.entries(expected)) {
			await sign('00', 'aabb', transports, route as keyof typeof expected).catch(() => {});
			const request = asked.at(-1);
			expect(request?.hints, route).toEqual([hint]);
			expect(
				request?.allowCredentials?.map((c) => c.transports),
				route
			).toEqual([transports.split(',')]);
		}
	});
});
