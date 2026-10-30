import '$lib/i18n/wasm-init.server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

/** The vendored client, as far as the token goes: what `connect` was given, and its events. */
const vendored = vi.hoisted(() => ({
	last: null as null | {
		token?: () => Promise<string>;
		connection: EventTarget & { state: string; room: { relay: string } };
	}
}));
vi.mock('$lib/vendor/peerconnect/index.js', () => ({
	connect: (room: { relay: string }, options: { token?: () => Promise<string> } = {}) => {
		const connection = Object.assign(new EventTarget(), { state: 'offline', room });
		vendored.last = { token: options.token, connection };
		return connection;
	}
}));

const {
	DEFAULT_RELAY,
	TOKEN_ENDPOINT,
	TRIAL_ENDPOINT,
	connectRoom,
	forgetRelayToken,
	relaySchedule,
	relayToken,
	relayUrl
} = await import('./relay');

// The core's schedule (its rules' own cases are in rust/…/tests/app_multisig_outside_relay.rs).
const { backoffMs: BACKOFF_MS, unreachableAfterMs: UNREACHABLE_AFTER_MS } = relaySchedule();

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

describe('which relay rooms go through', () => {
	it('is the public one unless a profile names its own', () => {
		expect(relayUrl()).toBe(DEFAULT_RELAY);
		storage.setItem('vela.peerconnectRelay', 'ws://127.0.0.1:8792');
		expect(relayUrl()).toBe('ws://127.0.0.1:8792');
	});
});

describe('relayToken', () => {
	it('asks Vela’s site under one device ID, then keeps the token', async () => {
		answers[TOKEN_ENDPOINT] = ok('t-site');
		expect((await relayToken()).token).toBe('t-site');
		expect((await relayToken()).token).toBe('t-site');
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
		expect((await relayToken()).token).toBe('t-trial');
		expect(calls.map((c) => c.url)).toEqual([TOKEN_ENDPOINT, TRIAL_ENDPOINT]);
	});

	it('replaces a token about to expire', async () => {
		answers[TOKEN_ENDPOINT] = ok('t-old', Math.floor(Date.now() / 1000) + 60);
		await relayToken();
		answers[TOKEN_ENDPOINT] = ok('t-new');
		expect((await relayToken()).token).toBe('t-new');
	});

	it('shares one request between rooms opening at once', async () => {
		answers[TOKEN_ENDPOINT] = ok('t-site');
		const [a, b] = await Promise.all([relayToken(), relayToken()]);
		expect([a.token, b.token]).toEqual(['t-site', 't-site']);
		expect(calls).toHaveLength(1);
	});

	it('throws when nobody answers, and forgets only the refused token', async () => {
		await expect(relayToken()).rejects.toThrow();
		answers[TOKEN_ENDPOINT] = ok('t-1');
		await relayToken();
		forgetRelayToken('t-other');
		expect((await relayToken()).token).toBe('t-1');
		forgetRelayToken('t-1');
		answers[TOKEN_ENDPOINT] = ok('t-2');
		expect((await relayToken()).token).toBe('t-2');
	});
});

/**
 * r4C-1: a token the relay refuses is never followed by another at once.
 * The vendored client asks for one on every `unauthorized`; this side waits
 * (2 s, then 4 s), takes a fresh one from the site, then the trial's once,
 * and stops — said as failed, until Retry. r4C-4: no token at all backs off
 * to 30 s, then stops, said with what each place answered.
 */
describe('a connection refused by the relay', () => {
	beforeEach(() => vi.useFakeTimers());
	afterEach(() => vi.useRealTimers());

	function open() {
		const watched = connectRoom({ relay: DEFAULT_RELAY } as never);
		const { token, connection } = vendored.last!;
		const refuse = () => connection.dispatchEvent(new Event('unauthorized'));
		return { watched, token: token!, refuse, connection };
	}

	it('waits before each new token, falls back to the trial once, then stops and says so', async () => {
		let n = 0;
		answers[TOKEN_ENDPOINT] = () => ok(`bogus-${++n}`)();
		answers[TRIAL_ENDPOINT] = ok('trial-1', Math.floor(Date.now() / 1000) + 7200);
		const { watched, token, refuse } = open();
		expect(await token()).toBe('bogus-1');

		refuse();
		let next = token();
		await vi.advanceTimersByTimeAsync(0);
		expect(watched.relay.status).toMatchObject({ kind: 'retrying', why: 'refused' });
		// Nothing asked during the wait.
		const asked = calls.length;
		await vi.advanceTimersByTimeAsync(BACKOFF_MS[0] - 10);
		expect(calls.length).toBe(asked);
		await vi.advanceTimersByTimeAsync(20);
		expect(await next).toBe('bogus-2');

		refuse();
		next = token();
		await vi.advanceTimersByTimeAsync(BACKOFF_MS[1] + 10);
		// The trial's, once.
		expect(await next).toBe('trial-1');
		expect(calls.at(-1)?.url).toBe(TRIAL_ENDPOINT);

		refuse();
		let settled = false;
		next = token().then((t) => ((settled = true), t));
		await vi.advanceTimersByTimeAsync(120_000);
		// Stopped: no loop, said as failed, nothing more asked.
		expect(settled).toBe(false);
		expect(watched.relay.status).toEqual({ kind: 'failed', why: 'refused', detail: 'HTTP 4403' });
		const stopped = calls.length;
		await vi.advanceTimersByTimeAsync(600_000);
		expect(calls.length).toBe(stopped);

		// Retry: asked again, from the site.
		watched.relay.retry();
		expect(await next).toBe('bogus-3');
		expect(calls.length).toBe(stopped + 1);
	});

	it('getting no token backs off to 30 s, then stops with what each place answered', async () => {
		answers[TOKEN_ENDPOINT] = () => new Response('{}', { status: 429 });
		answers[TRIAL_ENDPOINT] = () => new Response('{}', { status: 429 });
		const { watched, token } = open();
		let settled = false;
		const first = token().then((t) => ((settled = true), t));
		await vi.advanceTimersByTimeAsync(0);
		expect(watched.relay.status).toMatchObject({
			kind: 'retrying',
			why: 'noToken',
			detail: 'getvela.app: HTTP 429 · peerconnect.dev: HTTP 429'
		});
		const status = watched.relay.status as { at: number };
		expect(status.at - Date.now()).toBe(BACKOFF_MS[0]);
		await vi.advanceTimersByTimeAsync(BACKOFF_MS.reduce((a, b) => a + b, 0) + 100);
		expect(watched.relay.status).toMatchObject({ kind: 'failed', why: 'noToken' });
		expect(settled).toBe(false);
		// Asked once per wait, plus the first: 6 pairs, never a loop.
		expect(calls.length).toBe(2 * (BACKOFF_MS.length + 1));
		answers[TOKEN_ENDPOINT] = ok('t-back');
		watched.relay.retry();
		expect(await first).toBe('t-back');
	});

	it('is said as up once on the relay, and as unreachable when a token does not get it there', async () => {
		answers[TOKEN_ENDPOINT] = ok('t-1');
		const { watched, token, connection } = open();
		await token();
		await vi.advanceTimersByTimeAsync(UNREACHABLE_AFTER_MS + 10);
		expect(watched.relay.status).toMatchObject({ kind: 'retrying', why: 'unreachable' });
		connection.state = 'waiting';
		connection.dispatchEvent(new Event('state'));
		expect(watched.relay.status).toEqual({ kind: 'up' });
	});
});

describe('back online (r4C-15)', () => {
	it('a connection waiting out its backoff tries the relay at once; a left one never', () => {
		const win = new EventTarget();
		const doc = Object.assign(new EventTarget(), { visibilityState: 'visible' });
		vi.stubGlobal('window', win);
		vi.stubGlobal('document', doc);
		const watched = connectRoom({ relay: 'ws://127.0.0.1:8792' } as never);
		const connection = vendored.last!.connection as unknown as EventTarget & {
			state: string;
			reason?: string;
			reconnectNow: () => void;
		};
		let nudged = 0;
		connection.reconnectNow = () => void (nudged += 1);
		expect(watched).toBe(connection);
		win.dispatchEvent(new Event('online'));
		expect(nudged).toBe(1);
		// The page in front again: the same.
		doc.dispatchEvent(new Event('visibilitychange'));
		expect(nudged).toBe(2);
		// In the background, nothing: it is tried when it is looked at.
		doc.visibilityState = 'hidden';
		doc.dispatchEvent(new Event('visibilitychange'));
		expect(nudged).toBe(2);
		// Left on purpose: never brought back.
		doc.visibilityState = 'visible';
		connection.reason = 'left';
		connection.dispatchEvent(new Event('state'));
		win.dispatchEvent(new Event('online'));
		expect(nudged).toBe(2);
	});
});
