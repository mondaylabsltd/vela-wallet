/**
 * The wallet's end of the bridge (spec 103), over a fake ClientMCP client:
 * another tab taking the connection is said, and recovered from; the
 * browser's own offline/online is heard at once; and a wallet the person has
 * not allowed is not read through `vela_review` or `vela_wait`.
 */
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

type StatusFn = (status: string, detail?: { code: number; reason: string }) => void;
type Handler = (args: Record<string, unknown>, ctx: { signal: AbortSignal }) => unknown;

const clients: {
	onStatus: StatusFn;
	closed: boolean;
	tools: Map<string, Handler>;
}[] = [];

vi.mock('./clientmcp', () => ({
	ClientMCP: class {
		entry: (typeof clients)[number];
		constructor(options: { onStatus: StatusFn }) {
			this.entry = { onStatus: options.onStatus, closed: false, tools: new Map() };
			clients.push(this.entry);
		}
		expose(name: string, tool: { handler: Handler }) {
			this.entry.tools.set(name, tool.handler);
			return () => {};
		}
		connect() {
			this.entry.onStatus('connecting');
			return Promise.resolve();
		}
		close() {
			this.entry.closed = true;
		}
	},
	completed: (data = {}) => ({ ...data, status: 'completed' }),
	accepted: (operation: string, data = {}) => ({ ...data, status: 'accepted', operation }),
	rejected: (code: string, detail?: string) => ({ status: 'rejected', reason: { code, detail } })
}));

/** The core's verdict on reading, by operation id (`null`: the review). */
const reads = new Map<string | null, { code: string; detail: string } | null>();
vi.mock('$lib/core/client', () => ({
	loadCore: () => Promise.resolve(),
	intentCatalog: () => '[]',
	intentAgentReads: (_world: string, operation?: string) =>
		JSON.stringify(reads.get(operation ?? null) ?? null)
}));
const stop = vi.fn();
vi.mock('$lib/intents/index.svelte', () => ({
	intents: { stop, invoke: vi.fn(), approvals: [], activity: [] },
	world: () => ({})
}));
vi.mock('$lib/intents/world', () => ({ worldJson: () => '{}' }));
vi.mock('$lib/intents/state', () => ({ agentState: () => ({}) }));
const operation = { id: 'transfer-1', state: 'awaiting_confirmation' };
vi.mock('$lib/intents/transfers.svelte', () => ({
	transfers: { get: (id: string) => (id === operation.id ? operation : null), current: () => null }
}));
vi.mock('$lib/intents/review', () => ({ reviewOf: () => ({ note: 'n', transfer: null }) }));
vi.mock('$lib/flows/send-desk.svelte', () => ({ sendDesk: { port: null } }));
vi.mock('$lib/session/core/session.svelte', () => ({ session: { view: { address: '' } } }));
vi.mock('$lib/signing/core/sign-resident.svelte', () => ({ signRequest: { view: {} } }));
vi.mock('$lib/multisig/core/shown', () => ({ shownAddress: (a: string) => a }));
vi.mock('$lib/settings/core/network-admin.svelte', () => ({
	networkAdmin: { boot: () => Promise.resolve(), view: { loaded: false } }
}));
vi.mock('$lib/wallet/core/balance.svelte', () => ({ balance: { view: null } }));
vi.mock('$lib/wallet/core/feed.svelte', () => ({ feed: { view: null } }));
const settings = {
	ready: true,
	url: 'ws://127.0.0.1:8095/connect',
	token: 'tok',
	boot: vi.fn()
};
vi.mock('./settings.svelte', () => ({ agentSettings: settings }));

/** Web Locks, as one browser holds them for its tabs. */
function fakeLocks() {
	let holder: { release: () => void } | null = null;
	const queue: { grant: () => void; signal?: AbortSignal }[] = [];
	const next = () => {
		while (holder === null && queue.length > 0) {
			const waiting = queue.shift();
			if (!waiting || waiting.signal?.aborted) continue;
			waiting.grant();
		}
	};
	return {
		held: () => holder !== null,
		/** The tab holding it is gone. */
		drop: () => {
			holder = null;
			next();
		},
		request(
			_name: string,
			options: { steal?: boolean; signal?: AbortSignal },
			callback: () => unknown
		): Promise<unknown> {
			return new Promise((resolve, reject) => {
				const grant = () => {
					const mine = { release: () => {} };
					holder = mine;
					mine.release = () => {
						if (holder === mine) {
							holder = null;
							next();
						}
					};
					void Promise.resolve(callback()).then((value) => {
						mine.release();
						resolve(value);
					});
				};
				options.signal?.addEventListener('abort', () => reject(new Error('aborted')));
				if (options.steal || holder === null) grant();
				else queue.push({ grant, signal: options.signal });
			});
		}
	};
}

let locks: ReturnType<typeof fakeLocks>;
let win = new EventTarget();

// The first import compiles the bridge (runes): not a test's own time.
vi.setConfig({ testTimeout: 30_000 });
beforeAll(async () => {
	await import('./bridge.svelte');
});

beforeEach(() => {
	clients.length = 0;
	reads.clear();
	locks = fakeLocks();
	// A fresh window per test: each test's bridge is a fresh module.
	win = new EventTarget();
	vi.stubGlobal('navigator', { locks });
	vi.stubGlobal('window', win);
	vi.resetModules();
});

afterEach(() => {
	vi.unstubAllGlobals();
});

async function bridge() {
	const { agentBridge } = await import('./bridge.svelte');
	agentBridge.sync();
	await Promise.resolve();
	return agentBridge;
}

const tick = () => new Promise((done) => setTimeout(done, 0));

describe('another tab takes the agent (the bridge’s 4409)', () => {
	it('says so, and answers again once the other tab lets go', async () => {
		const agent = await bridge();
		clients[0].onStatus('online');
		expect(agent.status).toBe('online');
		expect(locks.held()).toBe(true);

		// The other tab dialled: it takes the lock, and the bridge replaces us.
		void locks.request('vela.agent.bridge', { steal: true }, () => new Promise(() => {}));
		clients[0].onStatus('closed', { code: 4409, reason: 'replaced' });
		expect(agent.status).toBe('elsewhere');
		expect(agent.closed).toBeNull();
		expect(clients).toHaveLength(1);

		// The other tab closes: this one dials again, with the settings as they are.
		locks.drop();
		await tick();
		expect(settings.boot).toHaveBeenCalled();
		expect(clients).toHaveLength(2);
		clients[1].onStatus('online');
		expect(agent.status).toBe('online');
	});

	it('is taken back here by the person', async () => {
		const agent = await bridge();
		clients[0].onStatus('online');
		void locks.request('vela.agent.bridge', { steal: true }, () => new Promise(() => {}));
		clients[0].onStatus('closed', { code: 4409, reason: 'replaced' });
		agent.useHere();
		expect(clients).toHaveLength(2);
		expect(clients[1].closed).toBe(false);
		clients[1].onStatus('online');
		expect(agent.status).toBe('online');
	});

	it('keeps a refusal for good as one', async () => {
		const agent = await bridge();
		clients[0].onStatus('closed', { code: 4401, reason: 'unauthorized' });
		expect(agent.status).toBe('closed');
		expect(agent.closed).toEqual({ code: 4401, reason: 'unauthorized' });
	});
});

describe('the browser’s network', () => {
	it('is offline at once, and dials again as soon as it is back', async () => {
		const agent = await bridge();
		clients[0].onStatus('online');
		win.dispatchEvent(new Event('offline'));
		expect(agent.status).toBe('offline');
		win.dispatchEvent(new Event('online'));
		expect(clients[0].closed).toBe(true);
		expect(clients).toHaveLength(2);
	});
});

describe('a wallet the person has not allowed (spec 107 E8)', () => {
	it('is not read through vela_review or vela_wait', async () => {
		await bridge();
		await tick();
		const tools = clients[0].tools;
		const signal = new AbortController().signal;
		const refusal = { code: 'wallet_not_allowed', detail: 'not this wallet' };

		reads.set(null, refusal);
		expect(await tools.get('vela_review')?.({}, { signal })).toEqual({
			status: 'rejected',
			reason: refusal
		});
		reads.set(null, null);
		expect(await tools.get('vela_review')?.({}, { signal })).toMatchObject({ note: 'n' });

		reads.set('transfer-1', refusal);
		expect(
			await tools.get('vela_wait')?.({ operation: 'transfer-1', timeout_ms: 0 }, { signal })
		).toEqual({ status: 'rejected', reason: refusal });
		reads.set('transfer-1', null);
		expect(
			await tools.get('vela_wait')?.({ operation: 'transfer-1', timeout_ms: 0 }, { signal })
		).toMatchObject({ transfer: operation, timed_out: true });
		// A transfer nobody has is still not_found.
		expect(
			await tools.get('vela_wait')?.({ operation: 'transfer-9', timeout_ms: 0 }, { signal })
		).toMatchObject({ reason: { code: 'not_found' } });
	});
});
