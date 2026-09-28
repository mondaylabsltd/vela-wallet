/* eslint-disable @typescript-eslint/no-explicit-any -- the fake `chrome` namespace mirrors an untyped browser API */
/**
 * The service worker owns each request's life (spec 082 T078, RB1–RB11) —
 * driven through a fake `chrome` namespace.
 *
 * Every case here is a device row that failed on 079's build: a reload that
 * left the old sheet up (G17/EX5), a second tab whose request never showed
 * (G18/EX4), a worker restart that lost a live request (G19/EX8), Chrome's
 * panel ✕ that answered nothing (EX6), and records left behind in
 * `storage.local`. The worker module is imported fresh for each case, the way
 * Chrome starts it.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

type Listener = (...args: any[]) => unknown;

interface FakePort {
	name: string;
	sender: Record<string, unknown>;
	postMessage(message: unknown): void;
	disconnect(): void;
	onMessage: { addListener(fn: Listener): void };
	onDisconnect: { addListener(fn: Listener): void };
}

function memArea() {
	const data: Record<string, unknown> = {};
	return {
		data,
		async get(keys: string | string[] | null) {
			if (keys === null) return { ...data };
			const list = Array.isArray(keys) ? keys : [keys];
			const out: Record<string, unknown> = {};
			for (const key of list) if (key in data) out[key] = structuredClone(data[key]);
			return out;
		},
		async set(items: Record<string, unknown>) {
			for (const [k, v] of Object.entries(items)) data[k] = structuredClone(v);
		},
		async remove(keys: string | string[]) {
			for (const key of Array.isArray(keys) ? keys : [keys]) delete data[key];
		}
	};
}

/** A page's content script, as the worker sees it: it owns ids and takes answers. */
interface FakePage {
	tabId: number;
	windowId: number;
	documentId: string;
	origin: string;
	owed: Set<string>;
	answers: { id: string; result?: unknown; error?: { code: number; message: string } }[];
	claimed: string[];
	port: FakePort | null;
}

function makeEnv() {
	const listeners: Record<string, Listener[]> = {};
	const on = (name: string) => ({
		addListener: (fn: Listener) => {
			(listeners[name] ??= []).push(fn);
		}
	});
	const emit = (name: string, ...args: unknown[]) => {
		for (const fn of listeners[name] ?? []) fn(...args);
	};
	const local = memArea();
	const session = memArea();
	const pages = new Map<string, FakePage>();
	const panelWindows = new Set<number>();
	/** Chrome reports the manifest's global side panel with `windowId: -1`. */
	const globalPanel = { up: false };
	const windows = new Set<number>([3]);
	let nextWindow = 100;
	const logs: string[] = [];

	const chrome = {
		runtime: {
			id: 'ext',
			getURL: (path = '') => `chrome-extension://ext/${path}`,
			onMessage: on('message'),
			onConnect: on('connect'),
			getContexts: async () =>
				globalPanel.up ? [{ windowId: -1 }] : [...panelWindows].map((windowId) => ({ windowId }))
		},
		storage: { local, session, onChanged: on('storageChanged') },
		tabs: {
			sendMessage: async (
				tabId: number,
				message: { type: string; id?: string; ids?: string[]; result?: unknown; error?: any },
				options?: { documentId?: string }
			) => {
				const page = options?.documentId ? pages.get(options.documentId) : undefined;
				if (!page || page.tabId !== tabId) throw new Error('Could not establish connection');
				switch (message.type) {
					case 'answer': {
						const ok = page.owed.delete(message.id!);
						if (ok)
							page.answers.push({ id: message.id!, result: message.result, error: message.error });
						return { ok };
					}
					case 'alive':
						return { alive: (message.ids ?? []).filter((id) => page.owed.has(id)) };
					case 'claimed':
						page.claimed.push(message.id!);
						return { ok: page.owed.has(message.id!) };
					default:
						return undefined;
				}
			},
			query: async () => [],
			create: async () => ({}),
			update: async () => ({}),
			onRemoved: on('tabRemoved'),
			onReplaced: on('tabReplaced')
		},
		windows: {
			create: vi.fn(async () => {
				nextWindow += 1;
				windows.add(nextWindow);
				return { id: nextWindow };
			}),
			remove: vi.fn(async (id: number) => {
				windows.delete(id);
			}),
			getAll: async () => [...windows].map((id) => ({ id })),
			update: async () => ({}),
			onRemoved: on('windowRemoved')
		},
		sidePanel: {
			open: vi.fn(async ({ tabId }: { tabId: number }) => {
				const page = [...pages.values()].find((p) => p.tabId === tabId);
				if (page) panelWindows.add(page.windowId);
			})
		},
		action: { onClicked: on('actionClicked') },
		i18n: { getUILanguage: () => 'en' }
	};

	function portPair(name: string, sender: Record<string, unknown>) {
		const toWorker: Listener[] = [];
		const workerDisconnect: Listener[] = [];
		const toClient: Listener[] = [];
		let open = true;
		const worker: FakePort = {
			name,
			sender,
			postMessage: (message) => {
				if (open) for (const fn of toClient) fn(structuredClone(message));
			},
			disconnect: () => {
				open = false;
			},
			onMessage: { addListener: (fn) => toWorker.push(fn) },
			onDisconnect: { addListener: (fn) => workerDisconnect.push(fn) }
		};
		const client = {
			received: [] as any[],
			post: (message: unknown) => {
				if (open) for (const fn of toWorker) fn(structuredClone(message));
			},
			close: () => {
				if (!open) return;
				open = false;
				for (const fn of workerDisconnect) fn();
			}
		};
		toClient.push((message) => client.received.push(message));
		emit('connect', worker);
		return client;
	}

	/** A page message into the worker; resolves with its reply. */
	function sendFromPage(page: FakePage, message: Record<string, unknown>) {
		return new Promise<any>((resolve) => {
			const sender = {
				origin: page.origin,
				url: `${page.origin}/`,
				documentId: page.documentId,
				tab: { id: page.tabId, windowId: page.windowId, url: `${page.origin}/` }
			};
			let answered = false;
			for (const fn of listeners.message ?? []) {
				const keep = fn(message, sender, (reply: unknown) => {
					answered = true;
					resolve(reply);
				});
				if (keep !== true && !answered) resolve(undefined);
			}
		});
	}

	function openPage(tabId: number, documentId: string, origin = 'https://a.example', windowId = 3) {
		const page: FakePage = {
			tabId,
			windowId,
			documentId,
			origin,
			owed: new Set(),
			answers: [],
			claimed: [],
			port: null
		};
		pages.set(documentId, page);
		return page;
	}

	/** What content.js does for a sign/connect: own the id, hold the doc port, send. */
	async function ask(page: FakePage, id: string, method = 'personal_sign') {
		page.owed.add(id);
		const docPort = portPair('vela.doc', {
			documentId: page.documentId,
			tab: { id: page.tabId, windowId: page.windowId }
		});
		const reply = await sendFromPage(page, {
			type: 'rpc',
			id,
			method,
			params: ['0x48656c6c6f', `0x${'a1'.repeat(20)}`],
			sentAt: Date.now()
		});
		return { reply, docPort };
	}

	/** The document goes away (reload / navigation): its content script dies with it. */
	function leave(page: FakePage, docPort: { close(): void }) {
		pages.delete(page.documentId);
		docPort.close();
	}

	function panel(windowId = 3) {
		const port = portPair('vela.surface', { url: 'chrome-extension://ext/en/wallet.html?panel' });
		port.post({ type: 'hello', kind: 'panel', windowId });
		return port;
	}

	return {
		chrome,
		local,
		session,
		pages,
		panelWindows,
		globalPanel,
		windows,
		logs,
		emit,
		portPair,
		sendFromPage,
		openPage,
		ask,
		leave,
		panel
	};
}

type Env = ReturnType<typeof makeEnv>;

async function startWorker(env: Env) {
	(globalThis as { chrome?: unknown }).chrome = env.chrome;
	vi.resetModules();
	const worker = await import('../../../extension/background.js');
	await worker.ready;
	return worker;
}

/** Let every queued promise run. */
async function settleAll() {
	for (let i = 0; i < 20; i += 1) await new Promise((resolve) => setTimeout(resolve, 0));
}

const owedOf = (port: { received: any[] }) =>
	port.received.filter((m) => m.type === 'owed').map((m) => m.request.rid);
const reqKeys = (area: { data: Record<string, unknown> }) =>
	Object.keys(area.data).filter((k) => k.startsWith('vela.req.'));

beforeEach(() => {
	vi.spyOn(console, 'info').mockImplementation(() => {});
});

afterEach(() => {
	vi.restoreAllMocks();
	vi.unstubAllGlobals();
	delete (globalThis as { chrome?: unknown }).chrome;
});

describe('the ledger (RB1)', () => {
	it('clears records a pre-082 worker left in storage.local at start', async () => {
		const env = makeEnv();
		env.local.data['vela.req.7:old'] = { rid: '7:old' };
		env.local.data['vela.perm.https://a.example'] = { address: '0x' };
		await startWorker(env);
		expect(reqKeys(env.local)).toEqual([]);
		expect(env.local.data['vela.perm.https://a.example']).toBeDefined();
	});

	it('writes a request into storage.session the moment it arrives, and answers `accepted`', async () => {
		const env = makeEnv();
		await startWorker(env);
		const page = env.openPage(7, 'doc-a');
		const { reply } = await env.ask(page, 'u1:1');
		await settleAll();
		expect(reply).toEqual({ accepted: true });
		expect(reqKeys(env.session)).toEqual(['vela.req.7:u1:1']);
		expect(reqKeys(env.local)).toEqual([]);
		expect(env.chrome.sidePanel.open).toHaveBeenCalledWith({ tabId: 7 });
	});
});

describe('a reload (G17, EX5)', () => {
	it('settles the old request as page_left and shows only the new one', async () => {
		const env = makeEnv();
		await startWorker(env);
		const panel = env.panel();
		const first = env.openPage(7, 'doc-1');
		const { docPort } = await env.ask(first, 'u1:1');
		await settleAll();
		expect(owedOf(panel)).toContain('7:u1:1');

		env.leave(first, docPort);
		await settleAll();
		expect(panel.received).toContainEqual({ type: 'withdrawn', rid: '7:u1:1', cause: 'page_left' });
		expect(reqKeys(env.session)).toEqual([]);

		const second = env.openPage(7, 'doc-2');
		await env.ask(second, 'u2:1');
		await settleAll();
		expect(owedOf(panel).at(-1)).toBe('7:u2:1');
		expect(reqKeys(env.session)).toEqual(['vela.req.7:u2:1']);
		const log = String(env.session.data['vela.sw.log']);
		expect(log).toMatch(/req\.settled cause=page_left/);
	});

	it('a page that closes its port with nothing owed is not a page leaving', async () => {
		const env = makeEnv();
		await startWorker(env);
		const panel = env.panel();
		const page = env.openPage(7, 'doc-1');
		const { docPort } = await env.ask(page, 'u1:1');
		await settleAll();
		panel.post({ type: 'answer', rid: '7:u1:1', result: '0xsig' });
		await settleAll();
		// content.js: nothing owed → `idle`, then close.
		(docPort as unknown as { post(m: unknown): void }).post({ type: 'idle' });
		docPort.close();
		await settleAll();
		expect(page.answers).toEqual([{ id: 'u1:1', result: '0xsig', error: undefined }]);
		expect(String(env.session.data['vela.sw.log'])).not.toMatch(/req\.settled/);
	});
});

describe('several tabs, one window (G18, EX4)', () => {
	it('queues a second tab’s request behind the first, and serves it next', async () => {
		const env = makeEnv();
		await startWorker(env);
		const panel = env.panel();
		const a = env.openPage(7, 'doc-a', 'https://a.example');
		const b = env.openPage(9, 'doc-b', 'https://b.example');
		await env.ask(a, 'a:1');
		await env.ask(b, 'b:1', 'eth_requestAccounts');
		await settleAll();
		expect(new Set(owedOf(panel))).toEqual(new Set(['7:a:1']));

		panel.post({ type: 'answer', rid: '7:a:1', result: '0xsig' });
		await settleAll();
		expect(panel.received).toContainEqual({ type: 'answered', rid: '7:a:1', delivered: true });
		expect(owedOf(panel).at(-1)).toBe('9:b:1');
		expect(a.answers).toHaveLength(1);
		expect(b.answers).toHaveLength(0);
	});

	it('answers each request exactly once', async () => {
		const env = makeEnv();
		await startWorker(env);
		const panel = env.panel();
		const a = env.openPage(7, 'doc-a');
		await env.ask(a, 'a:1');
		await settleAll();
		panel.post({ type: 'answer', rid: '7:a:1', result: '0x1' });
		panel.post({ type: 'answer', rid: '7:a:1', result: '0x2' });
		await settleAll();
		expect(a.answers).toEqual([{ id: 'a:1', result: '0x1', error: undefined }]);
		const answered = panel.received.filter((m) => m.type === 'answered');
		expect(answered).toHaveLength(2);
		expect(answered.filter((m) => m.delivered)).toEqual([
			{ type: 'answered', rid: '7:a:1', delivered: true }
		]);
	});
});

describe('a worker restart (G19, EX8 — resume, don’t settle)', () => {
	it('keeps a request whose page is still there, and it can still be answered', async () => {
		const env = makeEnv();
		const page = env.openPage(7, 'doc-a');
		page.owed.add('u1:1');
		env.panelWindows.add(3);
		env.session.data['vela.req.7:u1:1'] = {
			v: 1,
			rid: '7:u1:1',
			id: 'u1:1',
			method: 'personal_sign',
			params: [],
			origin: 'https://a.example',
			tabId: 7,
			windowId: 3,
			documentId: 'doc-a',
			sentAt: Date.now() - 10_000,
			at: Date.now() - 10_000,
			surface: 'panel',
			surfaceWindowId: 3,
			state: 'shown'
		};
		await startWorker(env);
		await settleAll();
		expect(reqKeys(env.session)).toEqual(['vela.req.7:u1:1']);
		expect(page.answers).toEqual([]);
		const log = String(env.session.data['vela.sw.log']);
		expect(log).toMatch(/req\.resumed/);
		expect(log).toMatch(/sw\.start records=1 recovered=1/);

		// The panel reconnects its port, claims, answers.
		const panel = env.panel();
		await settleAll();
		expect(owedOf(panel)).toContain('7:u1:1');
		panel.post({ type: 'claim', rid: '7:u1:1', phase: 'sign', nonce: 1 });
		await settleAll();
		expect(panel.received).toContainEqual({ type: 'claimResult', nonce: 1, live: true });
		panel.post({ type: 'answer', rid: '7:u1:1', result: '0xsig' });
		await settleAll();
		expect(page.answers).toEqual([{ id: 'u1:1', result: '0xsig', error: undefined }]);
	});

	it('keeps a panel request when Chrome cannot say which window its panel is in (windowId -1)', async () => {
		const env = makeEnv();
		const page = env.openPage(7, 'doc-a');
		page.owed.add('u1:1');
		env.globalPanel.up = true;
		env.session.data['vela.req.7:u1:1'] = {
			v: 1,
			rid: '7:u1:1',
			id: 'u1:1',
			method: 'personal_sign',
			params: [],
			origin: 'https://a.example',
			tabId: 7,
			windowId: 3,
			documentId: 'doc-a',
			sentAt: Date.now(),
			at: Date.now(),
			surface: 'panel',
			surfaceWindowId: 3,
			state: 'shown'
		};
		await startWorker(env);
		await settleAll();
		expect(reqKeys(env.session)).toEqual(['vela.req.7:u1:1']);
		expect(page.answers).toEqual([]);
		expect(String(env.session.data['vela.sw.log'])).toMatch(/recovered=1/);
	});

	it('settles what can no longer be answered: expired, no panel, page gone', async () => {
		const env = makeEnv();
		env.panelWindows.add(3);
		const base = {
			v: 1,
			method: 'personal_sign',
			params: [],
			origin: 'https://a.example',
			windowId: 3,
			surface: 'panel',
			surfaceWindowId: 3,
			state: 'shown'
		};
		const old = env.openPage(1, 'doc-old');
		old.owed.add('o');
		env.session.data['vela.req.1:o'] = {
			...base,
			rid: '1:o',
			id: 'o',
			tabId: 1,
			documentId: 'doc-old',
			sentAt: Date.now() - 400_000,
			at: Date.now() - 400_000
		};
		const orphan = env.openPage(2, 'doc-orphan', 'https://a.example', 5);
		orphan.owed.add('p');
		env.session.data['vela.req.2:p'] = {
			...base,
			rid: '2:p',
			id: 'p',
			tabId: 2,
			documentId: 'doc-orphan',
			surfaceWindowId: 5,
			sentAt: Date.now(),
			at: Date.now()
		};
		env.session.data['vela.req.3:q'] = {
			...base,
			rid: '3:q',
			id: 'q',
			tabId: 3,
			documentId: 'doc-gone',
			sentAt: Date.now(),
			at: Date.now()
		};
		await startWorker(env);
		await settleAll();
		expect(reqKeys(env.session)).toEqual([]);
		expect(old.answers[0].error).toEqual({
			code: 4900,
			message: 'Vela did not answer in time — check its activity'
		});
		expect(orphan.answers[0].error).toEqual({
			code: 4900,
			message: 'The browser closed before the request finished'
		});
		expect(String(env.session.data['vela.sw.log'])).toMatch(/req\.settled cause=page_left/);
	});
});

describe('Chrome’s panel ✕ (EX6)', () => {
	it('settles every request the panel owed with surface_closed, at once', async () => {
		const env = makeEnv();
		await startWorker(env);
		const panel = env.panel();
		const a = env.openPage(7, 'doc-a');
		await env.ask(a, 'a:1');
		await settleAll();
		panel.close();
		await settleAll();
		expect(a.answers).toEqual([
			{
				id: 'a:1',
				result: undefined,
				error: { code: 4900, message: 'The browser closed before the request finished' }
			}
		]);
		expect(reqKeys(env.session)).toEqual([]);
		expect(String(env.session.data['vela.sw.log'])).toMatch(/req\.settled cause=surface_closed/);
	});
});

describe('claims (RB5)', () => {
	it('refuses a claim for a page that is gone, and settles it', async () => {
		const env = makeEnv();
		const page = env.openPage(7, 'doc-a');
		page.owed.add('a:1');
		env.panelWindows.add(3);
		env.session.data['vela.req.7:a:1'] = {
			v: 1,
			rid: '7:a:1',
			id: 'a:1',
			method: 'personal_sign',
			params: [],
			origin: 'https://a.example',
			tabId: 7,
			windowId: 3,
			documentId: 'doc-a',
			sentAt: Date.now(),
			at: Date.now(),
			surface: 'panel',
			surfaceWindowId: 3,
			state: 'shown'
		};
		// A restarted worker: the record is resumed (the page answered `alive`)…
		await startWorker(env);
		await settleAll();
		const panel = env.panel();
		await settleAll();
		// …and then the page went away before its port came back.
		env.pages.delete('doc-a');
		panel.post({ type: 'claim', rid: '7:a:1', phase: 'sign', nonce: 9 });
		await settleAll();
		expect(panel.received).toContainEqual({
			type: 'claimResult',
			nonce: 9,
			live: false,
			cause: 'page_left'
		});
		expect(panel.received).toContainEqual({ type: 'withdrawn', rid: '7:a:1', cause: 'page_left' });
		expect(reqKeys(env.session)).toEqual([]);
	});

	it('a submit needs the earlier claim, and a live claim tells the page', async () => {
		const env = makeEnv();
		await startWorker(env);
		const panel = env.panel();
		const a = env.openPage(7, 'doc-a');
		await env.ask(a, 'a:1', 'eth_sendTransaction');
		await settleAll();
		panel.post({ type: 'claim', rid: '7:a:1', phase: 'submit', nonce: 1 });
		await settleAll();
		expect(panel.received).toContainEqual({
			type: 'claimResult',
			nonce: 1,
			live: false,
			cause: 'not_claimed'
		});
		panel.post({ type: 'claim', rid: '7:a:1', phase: 'sign', nonce: 2 });
		panel.post({ type: 'claim', rid: '7:a:1', phase: 'submit', nonce: 3 });
		await settleAll();
		expect(panel.received).toContainEqual({ type: 'claimResult', nonce: 2, live: true });
		expect(panel.received).toContainEqual({ type: 'claimResult', nonce: 3, live: true });
		expect(a.claimed).toEqual(['a:1', 'a:1']);
	});

	it('another window’s panel cannot claim or answer', async () => {
		const env = makeEnv();
		await startWorker(env);
		env.panel(3);
		const other = env.panel(4);
		const a = env.openPage(7, 'doc-a');
		await env.ask(a, 'a:1');
		await settleAll();
		other.post({ type: 'claim', rid: '7:a:1', phase: 'sign', nonce: 1 });
		other.post({ type: 'answer', rid: '7:a:1', result: '0xsig' });
		await settleAll();
		expect(other.received).toContainEqual({
			type: 'claimResult',
			nonce: 1,
			live: false,
			cause: 'wrong_surface'
		});
		expect(a.answers).toEqual([]);
	});
});

describe('the page’s own deadline (RB11)', () => {
	it('an `abandon` settles the record as expired and withdraws the sheet', async () => {
		const env = makeEnv();
		await startWorker(env);
		const panel = env.panel();
		const a = env.openPage(7, 'doc-a');
		await env.ask(a, 'a:1');
		await settleAll();
		await env.sendFromPage(a, { type: 'abandon', id: 'a:1' });
		await settleAll();
		expect(panel.received).toContainEqual({ type: 'withdrawn', rid: '7:a:1', cause: 'expired' });
		expect(reqKeys(env.session)).toEqual([]);
	});
});

describe('chain reads (RF2, G20, G33)', () => {
	const catalog = {
		version: 1,
		chains: {
			'100': {
				chainId: 100,
				name: 'Gnosis',
				rpc: ['https://dead.example/rpc', 'https://live.example/rpc'],
				bundler: 'https://relay.example/100'
			}
		}
	};

	it('skips an endpoint that just failed on the next call', async () => {
		const env = makeEnv();
		env.local.data['vela.ext.chains'] = catalog;
		env.local.data['vela.chain.https://a.example'] = 100;
		const calls: string[] = [];
		vi.stubGlobal(
			'fetch',
			vi.fn(async (url: string) => {
				calls.push(new URL(url).host);
				if (url.includes('dead')) throw new TypeError('Failed to fetch');
				return new Response(JSON.stringify({ jsonrpc: '2.0', id: 1, result: '0x10' }), {
					status: 200
				});
			})
		);
		await startWorker(env);
		const page = env.openPage(7, 'doc-a');
		const first = await env.sendFromPage(page, {
			type: 'rpc',
			id: 'r1',
			method: 'eth_blockNumber',
			params: []
		});
		expect(first).toEqual({ result: '0x10' });
		expect(calls).toEqual(['dead.example', 'live.example']);
		calls.length = 0;
		const second = await env.sendFromPage(page, {
			type: 'rpc',
			id: 'r2',
			method: 'eth_blockNumber',
			params: []
		});
		expect(second).toEqual({ result: '0x10' });
		expect(calls).toEqual(['live.example']);
		const log = String(env.session.data['vela.sw.log']);
		expect(log).toMatch(/read\.fail chain=100 host=dead\.example kind=network/);
		expect(log).not.toMatch(/\/rpc/);
	});

	it('answers plain words, with the chain’s name, when no node answered', async () => {
		const env = makeEnv();
		env.local.data['vela.ext.chains'] = catalog;
		env.local.data['vela.chain.https://a.example'] = 100;
		vi.stubGlobal(
			'fetch',
			vi.fn(async () => {
				throw new TypeError('Failed to fetch');
			})
		);
		await startWorker(env);
		const page = env.openPage(7, 'doc-a');
		const reply = await env.sendFromPage(page, {
			type: 'rpc',
			id: 'r1',
			method: 'eth_blockNumber',
			params: []
		});
		expect(reply).toEqual({
			error: { code: -32603, message: 'Vela could not reach a node for chain Gnosis (100)' }
		});
		await settleAll();
		expect(String(env.session.data['vela.sw.log'])).toMatch(/read\.exhausted chain=100 tried=2/);
	});
});

describe('receipt reads for handed-out operation hashes (RF3)', () => {
	const OP = `0x${'ab'.repeat(32)}`;
	const TX = `0x${'cd'.repeat(32)}`;
	const catalog = {
		version: 1,
		chains: {
			'100': {
				chainId: 100,
				name: 'Gnosis',
				rpc: ['https://node.example/'],
				bundler: 'https://relay.example/100'
			}
		}
	};

	it('remembers the hash with the answer and translates the page’s receipt read', async () => {
		const env = makeEnv();
		env.local.data['vela.ext.chains'] = catalog;
		env.local.data['vela.chain.https://a.example'] = 1; // the page moved on; the op did not
		const asked: { host: string; method: string; params: unknown[] }[] = [];
		vi.stubGlobal(
			'fetch',
			vi.fn(async (url: string, init: { body: string }) => {
				const body = JSON.parse(init.body);
				asked.push({ host: new URL(url).host, method: body.method, params: body.params });
				const result =
					body.method === 'eth_getUserOperationReceipt'
						? { success: true, receipt: { transactionHash: TX } }
						: { transactionHash: TX, status: '0x1' };
				return new Response(JSON.stringify({ jsonrpc: '2.0', id: 1, result }), { status: 200 });
			})
		);
		await startWorker(env);
		const panel = env.panel();
		const page = env.openPage(7, 'doc-a');
		await env.ask(page, 'tx:1', 'eth_sendTransaction');
		await settleAll();
		panel.post({ type: 'answer', rid: '7:tx:1', result: OP, opHash: { chainId: 100 } });
		await settleAll();
		expect(env.local.data[`vela.ext.op.${OP}`]).toMatchObject({ chainId: 100 });

		const reply = await env.sendFromPage(page, {
			type: 'rpc',
			id: 'r1',
			method: 'eth_getTransactionReceipt',
			params: [OP]
		});
		expect(reply).toEqual({ result: { transactionHash: TX, status: '0x1' } });
		expect(asked).toEqual([
			{ host: 'relay.example', method: 'eth_getUserOperationReceipt', params: [OP] },
			{ host: 'node.example', method: 'eth_getTransactionReceipt', params: [TX] }
		]);
	});

	it('forwards a hash it never handed out as it is', async () => {
		const env = makeEnv();
		env.local.data['vela.ext.chains'] = catalog;
		env.local.data['vela.chain.https://a.example'] = 100;
		const asked: string[] = [];
		vi.stubGlobal(
			'fetch',
			vi.fn(async (_url: string, init: { body: string }) => {
				asked.push(JSON.parse(init.body).method);
				return new Response(JSON.stringify({ jsonrpc: '2.0', id: 1, result: null }), {
					status: 200
				});
			})
		);
		await startWorker(env);
		const page = env.openPage(7, 'doc-a');
		const reply = await env.sendFromPage(page, {
			type: 'rpc',
			id: 'r1',
			method: 'eth_getTransactionReceipt',
			params: [TX]
		});
		expect(reply).toEqual({ result: null });
		expect(asked).toEqual(['eth_getTransactionReceipt']);
	});
});
