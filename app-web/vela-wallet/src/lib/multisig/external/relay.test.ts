import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	DEFAULT_RELAY,
	TOKEN_ENDPOINT,
	TRIAL_ENDPOINT,
	forgetRelayToken,
	needsToken,
	relayToken,
	relayUrl
} from './relay';

const YEAR = () => Math.floor(Date.now() / 1000) + 365 * 24 * 3600;

function memoryStorage() {
	const map = new Map<string, string>();
	return {
		getItem: (k: string) => map.get(k) ?? null,
		setItem: (k: string, v: string) => void map.set(k, v),
		removeItem: (k: string) => void map.delete(k),
		map
	};
}

let storage: ReturnType<typeof memoryStorage>;
let calls: { url: string; body: unknown }[];
let answers: Record<string, () => Response>;

beforeEach(() => {
	storage = memoryStorage();
	calls = [];
	answers = {};
	vi.stubGlobal('localStorage', storage);
	vi.stubGlobal('fetch', async (url: string, init?: RequestInit) => {
		calls.push({ url, body: init?.body ? JSON.parse(String(init.body)) : undefined });
		const answer = answers[url];
		if (!answer) throw new TypeError('offline');
		return answer();
	});
});

afterEach(() => {
	vi.unstubAllGlobals();
});

const ok =
	(token: string, expires_at = YEAR()) =>
	() =>
		new Response(JSON.stringify({ token, expires_at, relay_url: DEFAULT_RELAY }));

describe('which relay asks for a token', () => {
	it('is only the public one: a profile’s own relay gets none', () => {
		expect(needsToken(DEFAULT_RELAY)).toBe(true);
		expect(needsToken(`${DEFAULT_RELAY}/`)).toBe(true);
		expect(needsToken('ws://127.0.0.1:8792')).toBe(false);
		expect(relayUrl()).toBe(DEFAULT_RELAY);
		storage.setItem('vela.peerconnectRelay', 'ws://127.0.0.1:8792');
		expect(relayUrl()).toBe('ws://127.0.0.1:8792');
	});
});

describe('relayToken', () => {
	it('asks Vela’s site under one device ID, then keeps the token', async () => {
		answers[TOKEN_ENDPOINT] = ok('t-site');
		expect(await relayToken()).toBe('t-site');
		expect(await relayToken()).toBe('t-site');
		expect(calls).toHaveLength(1);
		const device = (calls[0].body as { device: string }).device;
		expect(device).toMatch(/^[0-9a-f]{32}$/);
		forgetRelayToken();
		await relayToken();
		expect((calls[1].body as { device: string }).device).toBe(device);
	});

	it('falls back to a trial token when the site cannot issue one', async () => {
		answers[TOKEN_ENDPOINT] = () => new Response('{"error":"not_configured"}', { status: 503 });
		answers[TRIAL_ENDPOINT] = ok('t-trial', Math.floor(Date.now() / 1000) + 7200);
		expect(await relayToken()).toBe('t-trial');
		expect(calls.map((c) => c.url)).toEqual([TOKEN_ENDPOINT, TRIAL_ENDPOINT]);
	});

	it('replaces a token about to expire', async () => {
		answers[TOKEN_ENDPOINT] = ok('t-old', Math.floor(Date.now() / 1000) + 60);
		await relayToken();
		answers[TOKEN_ENDPOINT] = ok('t-new');
		expect(await relayToken()).toBe('t-new');
	});

	it('shares one request between rooms opening at once', async () => {
		answers[TOKEN_ENDPOINT] = ok('t-site');
		const [a, b] = await Promise.all([relayToken(), relayToken()]);
		expect([a, b]).toEqual(['t-site', 't-site']);
		expect(calls).toHaveLength(1);
	});

	it('throws when nobody answers, and forgets only the refused token', async () => {
		await expect(relayToken()).rejects.toThrow();
		answers[TOKEN_ENDPOINT] = ok('t-1');
		await relayToken();
		forgetRelayToken('t-other');
		expect(await relayToken()).toBe('t-1');
		forgetRelayToken('t-1');
		answers[TOKEN_ENDPOINT] = ok('t-2');
		expect(await relayToken()).toBe('t-2');
	});
});
