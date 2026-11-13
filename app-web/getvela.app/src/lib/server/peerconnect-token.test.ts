import { describe, expect, it } from 'vitest';
import {
	PEERCONNECT_APP_ID,
	PEERCONNECT_SITE,
	issueToken,
	parseDevice,
	userId,
	type IssueDeps
} from './peerconnect-token';

const DEVICE = 'k3j5n2m4p6q8r1s3t5v7w9x2y4';
const TOKEN = {
	token: 'eyJ.x.y',
	expires_at: 1_900_000_000,
	relay_url: 'wss://relay.peerconnect.dev'
};

function post(body: unknown): Request {
	return new Request('https://getvela.app/api/peerconnect-token', {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: typeof body === 'string' ? body : JSON.stringify(body)
	});
}

function upstream(status: number, body: unknown) {
	const calls: { url: string; init: RequestInit }[] = [];
	const deps: IssueDeps = {
		secret: 'pcs_test',
		fetch: (async (url: string, init: RequestInit) => {
			calls.push({ url, init });
			return new Response(JSON.stringify(body), { status });
		}) as typeof fetch
	};
	return { deps, calls };
}

describe('parseDevice', () => {
	it('takes a device ID of 26–64 lowercase letters and digits', async () => {
		expect(await parseDevice(post({ device: DEVICE }))).toEqual({ ok: true, device: DEVICE });
		expect(await parseDevice(post({ device: 'a'.repeat(64) }))).toMatchObject({ ok: true });
	});

	it('refuses anything else, so no ID can smuggle in another namespace', async () => {
		for (const device of ['short', 'a'.repeat(65), `${DEVICE}:x`, DEVICE.toUpperCase(), 7, null]) {
			expect(await parseDevice(post({ device }))).toEqual({
				ok: false,
				status: 400,
				error: 'bad_device'
			});
		}
		expect(await parseDevice(post('not json'))).toMatchObject({ ok: false, status: 400 });
	});
});

describe('issueToken', () => {
	it('asks PeerConnect for the device’s token with the app secret, and hands back only the token', async () => {
		const { deps, calls } = upstream(200, { ...TOKEN, client_id: 'c-secretish' });
		const issued = await issueToken(DEVICE, deps);
		expect(issued).toEqual({ status: 200, body: TOKEN });
		expect(calls).toHaveLength(1);
		expect(calls[0].url).toBe(`${PEERCONNECT_SITE}/api/apps/${PEERCONNECT_APP_ID}/tokens`);
		expect(calls[0].init.method).toBe('POST');
		expect((calls[0].init.headers as Record<string, string>).authorization).toBe('Bearer pcs_test');
		expect(JSON.parse(String(calls[0].init.body))).toEqual({ user: userId(DEVICE) });
		expect(userId(DEVICE)).toBe(`web:${DEVICE}`);
	});

	it('says the user limit is reached, so the wallet can fall back', async () => {
		const { deps } = upstream(409, { message: 'limit' });
		expect(await issueToken(DEVICE, deps)).toEqual({ status: 409, body: { error: 'user_limit' } });
	});

	it('never passes an upstream refusal through as the device’s fault', async () => {
		expect((await issueToken(DEVICE, upstream(401, {}).deps)).status).toBe(502);
		expect((await issueToken(DEVICE, upstream(503, {}).deps)).status).toBe(503);
		expect((await issueToken(DEVICE, upstream(200, { token: 1 }).deps)).status).toBe(502);
	});

	it('is unavailable when PeerConnect cannot be reached', async () => {
		const deps: IssueDeps = {
			secret: 'pcs_test',
			fetch: (async () => {
				throw new TypeError('network');
			}) as typeof fetch
		};
		expect(await issueToken(DEVICE, deps)).toEqual({ status: 503, body: { error: 'unavailable' } });
	});
});
