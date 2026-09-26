/**
 * What a signature hands the browser (founder, 2026-09-26: an account signs
 * with the key it signed in with). Never a WebAuthn hint — the web's sign-in
 * applies none, and a signature must never be stricter than the ceremony that
 * proved the key answers — and the transports exactly as they were handed over,
 * which for a route are the core's. A screenshot cannot show any of this.
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

describe('any key', () => {
	it('the request is what it always was', async () => {
		await signWithAny('00', CREDENTIALS).catch(() => {});
		const request = asked.at(-1) ?? {};
		expect('hints' in request).toBe(false);
		expect(request.allowCredentials?.map((c) => c.transports)).toEqual([['internal'], undefined]);
	});
});

describe('one key', () => {
	it('carries exactly the transports handed over, and no hint', async () => {
		for (const transports of ['internal', 'hybrid,internal', 'internal,usb,nfc,ble,hybrid']) {
			await sign('00', 'aabb', transports).catch(() => {});
			const request = asked.at(-1) ?? {};
			expect('hints' in request, transports).toBe(false);
			expect(
				request.allowCredentials?.map((c) => c.transports),
				transports
			).toEqual([transports.split(',')]);
		}
	});

	it('with nothing known about where it lives, names no transports at all', async () => {
		await sign('00', 'aabb').catch(() => {});
		expect(asked.at(-1)?.allowCredentials?.[0].transports).toBeUndefined();
	});
});
