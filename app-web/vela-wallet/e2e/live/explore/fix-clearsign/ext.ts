/**
 * Exploratory helpers for the dApp area: the packaged extension in a profile
 * under the scratch folder, a virtual passkey shared (in memory only) with
 * every extension page that opens, the fork, and a tiny dApp served locally.
 */
import { createHash } from 'node:crypto';
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync } from 'node:fs';
import { createServer, type Server } from 'node:http';
import { join } from 'node:path';
import { chromium, type BrowserContext, type CDPSession, type Page } from '@playwright/test';

export const D =
	'/private/tmp/claude-501/-Volumes-data-production2-vela-wallet/046bc8f0-055e-4828-9ec5-8b8fd835f088/scratchpad/explore/fix-clearsign';
export const SHOTS = join(D, 'shots');
const APP_ROOT = join(import.meta.dirname, '../../../..');
export const DIST = join(APP_ROOT, 'extension/dist');
export const FORK_RPC = 'http://127.0.0.1:8545';
export const BASE_RPC = 'http://127.0.0.1:8546';

export function extensionId(): string {
	const { key } = JSON.parse(readFileSync(join(APP_ROOT, 'extension/manifest.json'), 'utf8'));
	const digest = createHash('sha256').update(Buffer.from(key, 'base64')).digest('hex');
	return [...digest.slice(0, 32)].map((c) => String.fromCharCode(97 + parseInt(c, 16))).join('');
}
export const ID = extensionId();
export const EXT = `chrome-extension://${ID}`;

export async function shot(page: Page, name: string): Promise<void> {
	mkdirSync(SHOTS, { recursive: true });
	await page.screenshot({ path: join(SHOTS, `${name}.png`) }).catch((e) => {
		console.log('shot failed', name, String(e).slice(0, 120));
	});
}

/** The credential(s) of the virtual passkey, held in memory only. */
type Cred = Record<string, unknown>;
export const passkeys: { creds: Cred[] } = { creds: [] };
export const authLog: string[] = [];
export const authDefaults = { presence: true };
const attached = new WeakMap<Page, { cdp: CDPSession; authenticatorId: string }>();

const attaching = new WeakMap<Page, Promise<void>>();
export let primary: { cdp: CDPSession; authenticatorId: string } | null = null;
export function attachAuthenticator(context: BrowserContext, page: Page): Promise<void> {
	let p = attaching.get(page);
	if (!p) {
		p = attachOnce(context, page);
		attaching.set(page, p);
	}
	return p;
}
async function attachOnce(context: BrowserContext, page: Page): Promise<void> {
	if (attached.has(page)) return;
	const cdp = await context.newCDPSession(page);
	await cdp.send('WebAuthn.enable');
	const added = await cdp.send('WebAuthn.addVirtualAuthenticator', {
		options: {
			protocol: 'ctap2',
			transport: 'internal',
			hasResidentKey: true,
			hasUserVerification: true,
			isUserVerified: true,
			automaticPresenceSimulation: authDefaults.presence,
			defaultBackupEligibility: true,
			defaultBackupState: true
		}
	}).catch((e) => {
		authLog.push(`addVirtualAuthenticator on ${page.url().slice(0, 80)}: ${String(e).slice(0, 160)}`);
		return null;
	});
	if (!added) return;
	const { authenticatorId } = added;
	authLog.push(`authenticator added on ${page.url().slice(0, 80)}`);
	for (const credential of passkeys.creds) {
		await cdp.send('WebAuthn.addCredential', { authenticatorId, credential } as never);
	}
	attached.set(page, { cdp, authenticatorId });
	primary ??= { cdp, authenticatorId };
}

/** Read back the passkey(s) the authenticator on `page` holds (kept in memory). */
export async function harvest(page: Page): Promise<void> {
	const a = attached.get(page) ?? primary;
	if (!a) throw new Error('no authenticator on that page');
	const { credentials } = (await a.cdp.send('WebAuthn.getCredentials', {
		authenticatorId: a.authenticatorId
	})) as { credentials: Cred[] };
	passkeys.creds = credentials;
}

export async function setUserVerified(page: Page, ok: boolean): Promise<void> {
	const a = attached.get(page);
	if (!a) return;
	await a.cdp.send('WebAuthn.setUserVerified', { authenticatorId: a.authenticatorId, isUserVerified: ok });
}
export async function setPresence(page: Page, ok: boolean): Promise<void> {
	const a = attached.get(page);
	if (!a) return;
	await a.cdp.send('WebAuthn.setAutomaticPresenceSimulation', {
		authenticatorId: a.authenticatorId,
		enabled: ok
	});
}

export interface Logs {
	errors: string[];
	console: string[];
}

export function watch(page: Page, logs: Logs, tag: string): void {
	page.on('pageerror', (e) => logs.errors.push(`[${tag} ${page.url().slice(0, 80)}] pageerror ${e.message}`));
	page.on('console', (m) => {
		if (m.type() === 'error' || m.type() === 'warning') {
			const text = m.text();
			if (/favicon|Failed to load resource: the server responded with a status of 404|cross-world extension resource mismatch/.test(text)) return;
			logs.console.push(`[${tag} ${m.type()} ${page.url().slice(0, 70)}] ${text.slice(0, 300)}`);
		}
	});
}

/**
 * The extension in a profile directory under the scratch folder. `from`
 * copies a saved profile first (the account made once).
 */
export async function launch(
	name: string,
	opts: { from?: string; surface?: 'window' | 'panel'; viewport?: { width: number; height: number }; logs?: Logs } = {}
): Promise<BrowserContext> {
	const dir = join(D, 'profiles', name);
	rmSync(dir, { recursive: true, force: true });
	if (opts.from) cpSync(join(D, 'profiles', opts.from), dir, { recursive: true });
	else mkdirSync(dir, { recursive: true });
	const context = await chromium.launchPersistentContext(dir, {
		headless: false,
		args: ['--headless=new', `--disable-extensions-except=${DIST}`, `--load-extension=${DIST}`],
		viewport: opts.viewport ?? { width: 430, height: 900 }
	});
	// Every extension page gets the passkey; every page is watched.
	context.on('page', (page) => {
		if (opts.logs) watch(page, opts.logs, 'page');
		void (async () => {
			try {
				await page.waitForURL(/^chrome-extension:/, { timeout: 15_000 });
				await attachAuthenticator(context, page);
			} catch {
				/* not an extension page */
			}
		})();
	});
	await settings(context);
	if (!opts.from) await closeWelcome(context);
	const page = await context.newPage();
	await page.goto(`${EXT}/manifest.json`);
	await page.evaluate(
		(s) => (window as any).chrome.storage.local.set({ 'vela.ext.surface': s }),
		opts.surface ?? 'window'
	);
	await page.close();
	return context;
}

export async function setSurface(context: BrowserContext, s: 'window' | 'panel'): Promise<void> {
	const page = await context.newPage();
	await page.goto(`${EXT}/manifest.json`);
	await page.evaluate((s) => (window as any).chrome.storage.local.set({ 'vela.ext.surface': s }), s);
	await page.close();
}

async function closeWelcome(context: BrowserContext): Promise<void> {
	const deadline = Date.now() + 10_000;
	while (Date.now() < deadline) {
		const tab = context.pages().find((p) => p.url().startsWith('chrome-extension://'));
		if (tab) {
			await tab.waitForURL((u) => !u.pathname.endsWith('/open.html'), { timeout: 10_000 }).catch(() => {});
			await Promise.race([tab.close().catch(() => {}), new Promise((r) => setTimeout(r, 3000))]);
			if (!tab.isClosed()) {
				await tab
					.evaluate(async () => {
						const tabs = (window as any).chrome.tabs;
						await tabs.remove((await tabs.getCurrent()).id);
					})
					.catch(() => {});
			}
			return;
		}
		await new Promise((r) => setTimeout(r, 200));
	}
}

/** The fork's services for every page of the extension's origin. */
async function settings(context: BrowserContext): Promise<void> {
	await context.addInitScript(() => {
		if (!location.href.startsWith('chrome-extension://')) return;
		localStorage.setItem('vela.intro.seen', String(Date.now()));
		localStorage.setItem('vela.launch.played', String(Date.now()));
		localStorage.setItem('vela.genesisServiceURL', 'http://127.0.0.1:8787');
		localStorage.setItem('vela.serviceEndpoints', JSON.stringify({ bundlerServiceURL: 'http://127.0.0.1:8790' }));
		localStorage.setItem('vela.peerconnectRelay', 'ws://127.0.0.1:8792');
		localStorage.setItem('vela.dev.console', '1');
	});
}

export async function seedNetwork(page: Page): Promise<void> {
	await page.evaluate(
		(networks) =>
			new Promise<void>((resolve, reject) => {
				const open = indexedDB.open('vela', 1);
				open.onupgradeneeded = () => open.result.createObjectStore('kv');
				open.onerror = () => reject(open.error);
				open.onsuccess = () => {
					const tx = open.result.transaction('kv', 'readwrite');
					tx.objectStore('kv').put(JSON.stringify(networks), 'vela.networkConfig');
					tx.oncomplete = () => resolve();
					tx.onerror = () => reject(tx.error);
				};
			}),
		[
			{ chainId: 100, rpcURL: FORK_RPC },
			{ chainId: 8453, rpcURL: BASE_RPC }
		]
	);
}

export async function rpc<T>(method: string, params: unknown[], url = FORK_RPC): Promise<T> {
	const response = await fetch(url, {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params })
	});
	const body = (await response.json()) as { result?: T; error?: { message: string } };
	if (body.error) throw new Error(body.error.message);
	return body.result as T;
}
export const setBalance = (a: string, wei: bigint, url = FORK_RPC) =>
	rpc('anvil_setBalance', [a, `0x${wei.toString(16)}`], url);
export const balanceOf = async (a: string, url = FORK_RPC) =>
	BigInt(await rpc<string>('eth_getBalance', [a, 'latest'], url));

/** The explore dApp, at one port per origin. */
export function serveDApp(port: number): Promise<Server> {
	const server = createServer((req, res) => {
		const html = readFileSync(join(D, 'site.html'));
		res.setHeader('content-type', 'text/html; charset=utf-8');
		res.setHeader('cache-control', 'no-store');
		res.end(html);
	});
	return new Promise((resolve) => server.listen(port, '127.0.0.1', () => resolve(server)));
}
export async function closeServer(server: Server): Promise<void> {
	server.closeAllConnections();
	await new Promise((r) => server.close(() => r(null)));
}

export interface AskResult {
	ok: boolean;
	code?: number;
	message?: string;
	result?: unknown;
}

export const ask = (page: Page, method: string, params: unknown = [], label?: string) =>
	page.evaluate(([m, p, l]) => (window as any).__ask(m, p, l), [method, params, label] as const) as Promise<AskResult>;
export const dappState = (page: Page) => page.evaluate(() => (window as any).__state());

export async function requestWindow(context: BrowserContext, timeoutMs = 20_000, not?: Page): Promise<Page> {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		const found = context.pages().find((p) => p !== not && !p.isClosed() && p.url().includes('/request.html'));
		if (found) {
			await found.waitForLoadState('domcontentloaded').catch(() => {});
			return found;
		}
		await new Promise((r) => setTimeout(r, 200));
	}
	throw new Error('no request window opened');
}
export const requestWindows = (context: BrowserContext) =>
	context.pages().filter((p) => !p.isClosed() && p.url().includes('/request.html'));

export async function noRequestWindow(context: BrowserContext, timeoutMs = 15_000): Promise<boolean> {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (requestWindows(context).length === 0) return true;
		await new Promise((r) => setTimeout(r, 150));
	}
	return false;
}

/** Race a promise against a timeout; returns 'TIMEOUT' if it did not settle. */
export async function within<T>(p: Promise<T>, ms: number): Promise<T | 'TIMEOUT'> {
	return Promise.race([p, new Promise<'TIMEOUT'>((r) => setTimeout(() => r('TIMEOUT'), ms))]);
}

export const profileExists = (name: string) => existsSync(join(D, 'profiles', name));

export async function storageLocal(context: BrowserContext, keys: string[] | null = null): Promise<Record<string, unknown>> {
	const page = await context.newPage();
	await page.goto(`${EXT}/manifest.json`);
	const all = await page.evaluate((k) => (window as any).chrome.storage.local.get(k), keys);
	await page.close();
	return all;
}

/** The authenticator CDP handle of a page (each extension page gets its own). */
export function authOf(page: Page): { cdp: CDPSession; authenticatorId: string } | undefined {
	return attached.get(page);
}
