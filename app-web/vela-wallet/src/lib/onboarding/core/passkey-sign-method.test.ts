/**
 * "Sign with" (founder, 2026-09-19): what each choice actually hands the
 * browser. A screenshot can show the row; only this can show that choosing
 * "Phone or tablet" changes the request — and that choosing nothing changes
 * nothing, which is what keeps every existing signature byte-for-byte the same.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setSignMethod, sign, signWithAny } from './passkey';

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
	setSignMethod('auto');
	vi.unstubAllGlobals();
});

const CREDENTIALS = [{ id: 'aabb', transports: 'internal' }, { id: 'ccdd' }];

async function attempt(): Promise<Asked> {
	await signWithAny('00', CREDENTIALS).catch(() => {});
	return asked.at(-1) ?? {};
}

describe('the signing method', () => {
	it('auto adds nothing: the request is what it always was', async () => {
		const request = await attempt();
		expect('hints' in request).toBe(false);
		expect(request.allowCredentials?.map((c) => c.transports)).toEqual([['internal'], undefined]);
	});

	it('each choice is a WebAuthn hint plus the transports that make it reachable', async () => {
		const expected = {
			platform: ['client-device', ['internal']],
			// A credential registered as `internal` is never offered over a QR
			// code unless the request also says `hybrid`.
			hybrid: ['hybrid', ['hybrid', 'internal']],
			security_key: ['security-key', ['usb', 'nfc', 'ble']]
		} as const;
		for (const [method, [hint, transports]] of Object.entries(expected)) {
			setSignMethod(method as keyof typeof expected);
			const request = await attempt();
			expect(request.hints, method).toEqual([hint]);
			expect(
				request.allowCredentials?.map((c) => c.transports),
				method
			).toEqual([[...transports], [...transports]]);
		}
	});

	it('the single-credential ceremony obeys it too', async () => {
		setSignMethod('hybrid');
		await sign('00', 'aabb', 'internal').catch(() => {});
		expect(asked.at(-1)?.hints).toEqual(['hybrid']);
		expect(asked.at(-1)?.allowCredentials?.[0].transports).toEqual(['hybrid', 'internal']);
	});
});
