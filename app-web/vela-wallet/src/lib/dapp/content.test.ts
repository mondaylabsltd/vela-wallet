/* eslint-disable @typescript-eslint/no-explicit-any -- the fake runtime mirrors an untyped browser API */
/**
 * The page bridge's half of a request's life (spec 082 T079: RB3, RB4, RB6,
 * RB11) — `extension/content.js`, driven with a fake page and a fake runtime.
 *
 * G19 was Chrome's own sentence — "The message channel closed before a
 * response was received" — arriving in a dApp as an error, after which the
 * wallet could still sign. These pin that it never does, that each id is
 * answered once, that the page's deadline is strictly after the worker's, and
 * the four answers a dropped channel gets.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	CHANNEL,
	CONTENT_GRACE_MS,
	READ_DROPPED_MESSAGE,
	REQUEST_TTL_MS,
	SETTLE
} from '../../../extension/lib/protocol.js';

const CHROME_TEXT = 'The message channel closed before a response was received.';

interface Posted {
	ch: string;
	dir: string;
	id?: string;
	result?: unknown;
	error?: { code: number; message: string };
}

function makeEnv() {
	const pageListeners: ((ev: { source: unknown; data: unknown }) => void)[] = [];
	const posted: Posted[] = [];
	const win = {
		location: { origin: 'https://a.example' },
		addEventListener: (type: string, fn: (ev: { source: unknown; data: unknown }) => void) => {
			if (type === 'message') pageListeners.push(fn);
		},
		postMessage: (data: Posted) => {
			posted.push(data);
			for (const fn of pageListeners) fn({ source: win, data });
		}
	};
	const workerListeners: ((m: any, s: unknown, r: (x: unknown) => void) => unknown)[] = [];
	const sent: any[] = [];
	const ports: { name: string; messages: any[]; disconnected: boolean; drop(): void }[] = [];
	let reply: (message: any) => Promise<unknown> = async () => ({ accepted: true });
	const runtime = {
		id: 'ext' as string | undefined,
		lastError: undefined,
		sendMessage: vi.fn((message: any) => {
			sent.push(message);
			return reply(message);
		}),
		connect: vi.fn(({ name }: { name: string }) => {
			const closers: (() => void)[] = [];
			const port = {
				name,
				messages: [] as any[],
				disconnected: false,
				drop: () => {
					for (const fn of closers) fn();
				}
			};
			ports.push(port);
			return {
				postMessage: (m: unknown) => port.messages.push(m),
				disconnect: () => {
					port.disconnected = true;
				},
				onDisconnect: { addListener: (fn: () => void) => closers.push(fn) }
			};
		}),
		onMessage: {
			addListener: (fn: (m: any, s: unknown, r: (x: unknown) => void) => unknown) =>
				workerListeners.push(fn)
		}
	};
	return {
		win,
		posted,
		sent,
		ports,
		runtime,
		setReply(fn: (message: any) => Promise<unknown>) {
			reply = fn;
		},
		ask(id: string, method: string, params: unknown[] = []) {
			win.postMessage({ ch: CHANNEL, dir: 'req', id, method, params } as Posted);
		},
		/** A message from the worker, with content.js's synchronous reply. */
		fromWorker(message: Record<string, unknown>) {
			let answer: unknown;
			for (const fn of workerListeners) fn(message, {}, (x) => (answer = x));
			return answer;
		},
		responses: () => posted.filter((m) => m.dir === 'res')
	};
}

type Env = ReturnType<typeof makeEnv>;

async function load(env: Env) {
	(globalThis as Record<string, unknown>).window = env.win;
	(globalThis as Record<string, unknown>).chrome = { runtime: env.runtime };
	vi.resetModules();
	await import('../../../extension/content.js');
}

beforeEach(() => {
	vi.useFakeTimers();
});

afterEach(() => {
	vi.useRealTimers();
	delete (globalThis as Record<string, unknown>).window;
	delete (globalThis as Record<string, unknown>).chrome;
});

describe('a dropped channel never reaches the page as Chrome’s words (G19)', () => {
	it('a read is retried once, then answered in plain words', async () => {
		const env = makeEnv();
		env.setReply(async () => {
			throw new Error(CHROME_TEXT);
		});
		await load(env);
		env.ask('r1', 'eth_call');
		await vi.advanceTimersByTimeAsync(0);
		expect(env.runtime.sendMessage).toHaveBeenCalledTimes(2);
		expect(env.responses()).toEqual([
			{ ch: CHANNEL, dir: 'res', id: 'r1', error: { code: -32603, message: READ_DROPPED_MESSAGE } }
		]);
		expect(JSON.stringify(env.posted)).not.toContain('message channel');
	});

	it('a read that sends is never retried: 4900 restarted', async () => {
		const env = makeEnv();
		env.setReply(async () => undefined); // a dead worker's reply
		await load(env);
		env.ask('s1', 'eth_sendRawTransaction', ['0x02']);
		await vi.advanceTimersByTimeAsync(0);
		expect(env.runtime.sendMessage).toHaveBeenCalledTimes(1);
		expect(env.responses()[0].error).toEqual({ code: 4900, message: SETTLE.restarted.message });
	});

	it('a sign whose channel dropped twice answers 4900 restarted', async () => {
		const env = makeEnv();
		env.setReply(async () => {
			throw new Error(CHROME_TEXT);
		});
		await load(env);
		env.ask('p1', 'personal_sign', ['0x48', '0x' + 'a1'.repeat(20)]);
		await vi.advanceTimersByTimeAsync(0);
		expect(env.runtime.sendMessage).toHaveBeenCalledTimes(2);
		expect(env.responses()).toEqual([
			{
				ch: CHANNEL,
				dir: 'res',
				id: 'p1',
				error: { code: 4900, message: SETTLE.restarted.message }
			}
		]);
	});

	it('a sign whose first send dropped is retried, and then waits for its answer', async () => {
		const env = makeEnv();
		let calls = 0;
		env.setReply(async () => {
			calls += 1;
			if (calls === 1) throw new Error(CHROME_TEXT);
			return { accepted: true };
		});
		await load(env);
		env.ask('p1', 'personal_sign');
		await vi.advanceTimersByTimeAsync(0);
		expect(env.responses()).toEqual([]);
		expect(env.fromWorker({ type: 'answer', id: 'p1', result: '0xsig' })).toEqual({ ok: true });
		expect(env.responses()).toEqual([{ ch: CHANNEL, dir: 'res', id: 'p1', result: '0xsig' }]);
	});

	it('an extension that was reloaded or updated answers `updated`', async () => {
		const env = makeEnv();
		await load(env);
		env.runtime.id = undefined;
		env.ask('p1', 'eth_requestAccounts');
		env.ask('r1', 'eth_chainId');
		await vi.advanceTimersByTimeAsync(0);
		expect(env.responses().map((r) => [r.id, r.error])).toEqual([
			['p1', { code: 4900, message: SETTLE.updated.message }],
			['r1', { code: 4900, message: SETTLE.updated.message }]
		]);
	});
});

describe('one answer per id', () => {
	it('takes the first answer and refuses the rest', async () => {
		const env = makeEnv();
		await load(env);
		env.ask('p1', 'personal_sign');
		await vi.advanceTimersByTimeAsync(0);
		expect(env.fromWorker({ type: 'answer', id: 'p1', result: '0x1' })).toEqual({ ok: true });
		expect(env.fromWorker({ type: 'answer', id: 'p1', result: '0x2' })).toEqual({ ok: false });
		// And the page's own deadline cannot add a third.
		await vi.advanceTimersByTimeAsync(REQUEST_TTL_MS + CONTENT_GRACE_MS + 1);
		expect(env.responses()).toEqual([{ ch: CHANNEL, dir: 'res', id: 'p1', result: '0x1' }]);
	});

	it('an answer for an id this page does not own is refused', async () => {
		const env = makeEnv();
		await load(env);
		expect(env.fromWorker({ type: 'answer', id: 'nope', result: '0x1' })).toEqual({ ok: false });
		expect(env.responses()).toEqual([]);
	});
});

describe('the deadlines (RB11)', () => {
	it('gives up 5 min + 5 s after an unclaimed request — after the worker — and tells it', async () => {
		const env = makeEnv();
		await load(env);
		env.ask('p1', 'personal_sign');
		await vi.advanceTimersByTimeAsync(REQUEST_TTL_MS);
		expect(env.responses()).toEqual([]);
		await vi.advanceTimersByTimeAsync(CONTENT_GRACE_MS);
		expect(env.responses()).toEqual([
			{ ch: CHANNEL, dir: 'res', id: 'p1', error: { code: 4900, message: SETTLE.expired.message } }
		]);
		expect(env.sent.at(-1)).toEqual({ type: 'abandon', id: 'p1' });
	});

	it('waits 5 min from the claim once the wallet claimed it', async () => {
		const env = makeEnv();
		await load(env);
		env.ask('p1', 'eth_sendTransaction');
		await vi.advanceTimersByTimeAsync(200_000);
		expect(env.fromWorker({ type: 'claimed', id: 'p1' })).toEqual({ ok: true });
		await vi.advanceTimersByTimeAsync(REQUEST_TTL_MS - 1);
		expect(env.responses()).toEqual([]);
		await vi.advanceTimersByTimeAsync(1);
		expect(env.responses()[0].error?.code).toBe(4900);
	});

	it('tells the worker which ids it still owns', async () => {
		const env = makeEnv();
		await load(env);
		env.ask('p1', 'personal_sign');
		await vi.advanceTimersByTimeAsync(0);
		expect(env.fromWorker({ type: 'alive', ids: ['p1', 'p2'] })).toEqual({ alive: ['p1'] });
	});
});

describe('the page’s port (RB3, RB4)', () => {
	it('is open only while an answer is owed, and says `idle` before it closes', async () => {
		const env = makeEnv();
		await load(env);
		env.ask('r1', 'eth_chainId');
		await vi.advanceTimersByTimeAsync(0);
		expect(env.ports).toHaveLength(0);
		env.ask('p1', 'personal_sign');
		await vi.advanceTimersByTimeAsync(0);
		expect(env.ports.map((p) => p.name)).toEqual(['vela.doc']);
		env.fromWorker({ type: 'answer', id: 'p1', result: '0xsig' });
		expect(env.ports[0].messages).toEqual([{ type: 'idle' }]);
		expect(env.ports[0].disconnected).toBe(true);
	});

	it('reconnects with backoff when the worker restarts, without answering the page', async () => {
		const env = makeEnv();
		await load(env);
		env.ask('p1', 'personal_sign');
		await vi.advanceTimersByTimeAsync(0);
		env.ports[0].drop();
		await vi.advanceTimersByTimeAsync(0);
		expect(env.ports).toHaveLength(2);
		env.ports[1].drop();
		await vi.advanceTimersByTimeAsync(249);
		expect(env.ports).toHaveLength(2);
		await vi.advanceTimersByTimeAsync(1);
		expect(env.ports).toHaveLength(3);
		expect(env.responses()).toEqual([]);
		expect(env.fromWorker({ type: 'answer', id: 'p1', result: '0xsig' })).toEqual({ ok: true });
	});
});
