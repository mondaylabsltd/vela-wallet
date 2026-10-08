/**
 * Standing up the packaged extension in a test (spec 027).
 *
 * Three facts had to be measured before any of this worked (spec 027 D39), and
 * they are the reason these helpers exist rather than a line of setup per file:
 *
 *   - Playwright's `headless: true` loads NO extension; `headless: false` with
 *     `--headless=new` in `args` does;
 *   - `chrome://extensions` is not navigable from Playwright, so the extension
 *     id cannot be discovered — it is PINNED by the manifest `key` and
 *     recomputed here from it, so the manifest and the tests can never disagree;
 *   - a CDP virtual authenticator is scoped to the target it was added to, so a
 *     second page has none and any ceremony there simply hangs.
 */
import { createHash } from 'node:crypto';
import { existsSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { chromium, type BrowserContext, type Page } from '@playwright/test';

const APP_ROOT = join(import.meta.dirname, '..');
/**
 * The package the suites load: `extension/dist`, or the isolated run's own
 * directory (`VELA_EXTENSION_DIST`, set by `playwright.isolated.config.ts`) —
 * so a test run never loads, or rebuilds, the package a running browser has
 * open (spec 082 RJ21).
 */
export const EXTENSION_DIST = process.env.VELA_EXTENSION_DIST
	? resolve(APP_ROOT, process.env.VELA_EXTENSION_DIST)
	: join(APP_ROOT, 'extension/dist');

/** Has `pnpm build:extension` run? Suites skip rather than fail obscurely. */
export const extensionBuilt = (): boolean => existsSync(join(EXTENSION_DIST, 'manifest.json'));

/**
 * The Chrome Web Store package every build derives beside the development one
 * (spec 094) — the build's and the package test's rule.
 */
export const STORE_DIST = process.env.VELA_EXTENSION_STORE_DIST
	? resolve(APP_ROOT, process.env.VELA_EXTENSION_STORE_DIST)
	: `${EXTENSION_DIST}-store`;
export const storeBuilt = (): boolean => existsSync(join(STORE_DIST, 'manifest.json'));

/**
 * Chrome's id for an unpacked extension with a `key`: the first 32 hex digits
 * of SHA-256 over the DER public key, with 0–f mapped onto a–p.
 */
export function extensionId(): string {
	const { key } = JSON.parse(readFileSync(join(APP_ROOT, 'extension/manifest.json'), 'utf8'));
	const digest = createHash('sha256').update(Buffer.from(key, 'base64')).digest('hex');
	return [...digest.slice(0, 32)].map((c) => String.fromCharCode(97 + parseInt(c, 16))).join('');
}

/**
 * A browser with the packaged extension installed.
 *
 * `surface` is where a dApp request is answered. The product's default is the
 * asking tab's side panel; the harness's default is the WINDOW, because a
 * side panel is not a Playwright `Page` (see `preferWindows`). A suite that
 * proves the panel passes `'panel'` and drives it with `sidePanelView`.
 */
export async function loadExtension(
	options: {
		viewport?: { width: number; height: number };
		surface?: 'window' | 'panel';
		/** More Chrome switches for this browser only (e.g. `--host-resolver-rules`). */
		args?: string[];
		/** Another package than the development one (the store's, spec 094). */
		dist?: string;
		/** Leave the welcome tab a fresh install opens (spec 094 S3) where it is. */
		keepWelcome?: boolean;
	} = {}
): Promise<BrowserContext> {
	const dist = options.dist ?? EXTENSION_DIST;
	const context = await chromium.launchPersistentContext('', {
		headless: false,
		args: [
			'--headless=new',
			`--disable-extensions-except=${dist}`,
			`--load-extension=${dist}`,
			...(options.args ?? [])
		],
		...(options.viewport ? { viewport: options.viewport } : {})
	});
	if (!options.keepWelcome) await closeWelcome(context);
	if ((options.surface ?? 'window') === 'window') {
		// Any document of the extension's origin can write its storage; the
		// manifest is the cheapest one to open.
		const page = await context.newPage();
		// A package with no `key` (the store's) has the id Chrome gave it.
		const id = options.dist ? await runningExtensionId(context) : extensionId();
		await page.goto(`chrome-extension://${id}/manifest.json`);
		await preferWindows(page);
		await page.close();
	}
	return context;
}

/** Is this the tab a fresh install opens (the doorway, then the welcome)? */
const isWelcomeTab = (page: Page): boolean => /^chrome-extension:\/\/[a-p]{32}\//.test(page.url());

/**
 * Every browser here is a fresh install, so each one opens the wallet's
 * welcome in a tab (spec 094 S3, `runtime.onInstalled`). It is closed as soon
 * as it shows, so a suite counts only the pages it opens itself; a suite about
 * the welcome passes `keepWelcome` and finds it with `welcomeTab`.
 */
async function closeWelcome(context: BrowserContext): Promise<void> {
	const tab = await welcomeTab(context).catch(() => null);
	await tab?.close().catch(() => {});
}

/** The tab a fresh install opened, once it shows (spec 094 S3). */
export async function welcomeTab(context: BrowserContext, timeoutMs = 15_000): Promise<Page> {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		const found = context.pages().find(isWelcomeTab);
		if (found) return found;
		await new Promise((r) => setTimeout(r, 100));
	}
	throw new Error('no welcome tab opened');
}

/**
 * The id Chrome gave the extension — from its service worker, for a package
 * with no `key` to compute it from (the store's, spec 094).
 */
export async function runningExtensionId(context: BrowserContext): Promise<string> {
	const worker =
		context.serviceWorkers().find((w) => w.url().startsWith('chrome-extension://')) ??
		(await context.waitForEvent('serviceworker', { timeout: 15_000 }));
	return new URL(worker.url()).host;
}

/** Refuse every request that is not to the extension's own origin. */
export async function hermetic(context: BrowserContext): Promise<void> {
	await context.route('**/*', (route) =>
		route.request().url().startsWith('chrome-extension://') ? route.continue() : route.abort()
	);
}

type ChromeLocal = {
	chrome: {
		storage: { local: { set(items: Record<string, unknown>): Promise<void> } };
		extension: { getViews(): Window[] };
	};
};

/**
 * Answer requests in a WINDOW rather than the side panel, for this browser.
 *
 * The side panel is the product's surface, and it opens on a user gesture —
 * which Playwright's `evaluate` carries (CDP `userGesture: true`). But a side
 * panel is not a Playwright `Page`: it is a CDP target the harness never
 * attaches, so nothing here can click in it. The worker honours
 * `vela.ext.surface = 'window'`, and every suite that drives the window sets
 * it from an extension page right after seeding. The panel itself is proven
 * by `sidePanelView` below, through the one door the harness has into it.
 */
export async function preferWindows(extensionPage: Page): Promise<void> {
	await extensionPage.evaluate(() =>
		(window as unknown as ChromeLocal).chrome.storage.local.set({ 'vela.ext.surface': 'window' })
	);
}

/**
 * The side panel's `Window`, as another extension page sees it.
 *
 * Since spec 077 the panel IS the wallet — `wallet.html?panel` — with a
 * request rising over it as a dialog; and since spec 082 (RB9) it stays the
 * panel after Wallet → Settings → Wallet drops the query, by a sessionStorage
 * mark. This used to look for `request.html` with no query, a page the panel
 * has not shown since 077 — so the "panel" checks were watching nothing
 * (L-PANEL).
 */
const FIND_PANEL = `() => chrome.extension.getViews().find((w) => {
	try {
		if (!w.location.pathname.endsWith('/wallet.html')) return false;
		return (
			new URLSearchParams(w.location.search).has('panel') ||
			w.sessionStorage.getItem('vela.surface.panel') === '1'
		);
	} catch {
		return false;
	}
})`;

/**
 * Drive the side panel from another extension page, once a request's dialog
 * is showing in it.
 *
 * `chrome.extension.getViews()` hands an extension page the `Window` of every
 * other view of the extension in its process — the side panel included — so a
 * test can read the dialog and press its buttons without a Page of its own.
 * `heading` is the dialog's `aria-label` (the card's title, which names the
 * site); a click only ever lands INSIDE the dialog, never on the wallet under
 * it.
 */
export async function sidePanelView(
	extensionPage: Page,
	timeoutMs = 15_000
): Promise<{ heading: string; click(buttonText: string): Promise<void> }> {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		const heading = await extensionPage.evaluate((finder) => {
			const find = new Function(`return (${finder})`)() as () => Window | undefined;
			const dialog = find()?.document.querySelector('[role="dialog"]');
			return dialog?.getAttribute('aria-label') ?? null;
		}, FIND_PANEL);
		if (heading) {
			return {
				heading,
				click: (buttonText: string) =>
					extensionPage.evaluate(
						([finder, text]) => {
							const find = new Function(`return (${finder})`)() as () => Window | undefined;
							const dialog = find()?.document.querySelector('[role="dialog"]');
							const button = [...(dialog?.querySelectorAll('button') ?? [])].find(
								(b) => b.textContent?.trim() === text
							);
							if (!button) throw new Error(`no "${text}" button in the side panel's dialog`);
							(button as HTMLButtonElement).click();
						},
						[FIND_PANEL, buttonText] as const
					)
			};
		}
		await new Promise((r) => setTimeout(r, 250));
	}
	throw new Error('no side panel showed a request');
}

/** Is the side panel open at all (the wallet, request or not)? */
export const sidePanelUp = (extensionPage: Page): Promise<boolean> =>
	extensionPage.evaluate((finder) => {
		const find = new Function(`return (${finder})`)() as () => Window | undefined;
		return find() !== undefined;
	}, FIND_PANEL);

/** Is the side panel showing a request's dialog right now? */
export const sidePanelShowsRequest = (extensionPage: Page): Promise<boolean> =>
	extensionPage.evaluate((finder) => {
		const find = new Function(`return (${finder})`)() as () => Window | undefined;
		return !!find()?.document.querySelector('[role="dialog"]');
	}, FIND_PANEL);

/**
 * Run `body` inside the side panel's window (a function of `panel: Window`),
 * from another extension page. For what the helpers above do not cover — a
 * MutationObserver, a confirm, a navigation.
 */
export function inSidePanel<T>(extensionPage: Page, body: string, arg?: unknown): Promise<T> {
	return extensionPage.evaluate(
		async ([finder, source, value]) => {
			const find = new Function(`return (${finder})`)() as () => Window | undefined;
			const panel = find();
			if (!panel) throw new Error('no side panel');
			const run = new Function('panel', 'arg', `return (async () => { ${source} })()`);
			return run(panel, value);
		},
		[FIND_PANEL, body, arg] as const
	) as Promise<T>;
}

/** The request window this extension opened, once it is showing something. */
export async function requestWindow(context: BrowserContext, timeoutMs = 15_000): Promise<Page> {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		const found = context.pages().find((p) => p.url().includes('/request.html'));
		if (found) {
			await found.waitForLoadState('domcontentloaded');
			return found;
		}
		await new Promise((r) => setTimeout(r, 250));
	}
	throw new Error('no request window opened');
}

/** Is any request window open right now? */
export const requestWindowOpen = (context: BrowserContext): boolean =>
	context.pages().some((p) => p.url().includes('/request.html'));

/**
 * Wait until no request window is open.
 *
 * A settled window closes on a short delay, so that the answer reaches the page
 * before the document goes away. A test that fires its NEXT request without
 * waiting can therefore grab the previous, closing window — which then never
 * shows what it was looking for. Found exactly that way.
 */
export async function noRequestWindow(context: BrowserContext, timeoutMs = 15_000): Promise<void> {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (!requestWindowOpen(context)) return;
		await new Promise((r) => setTimeout(r, 100));
	}
	throw new Error('a request window stayed open');
}

/**
 * Tap the signing sheet's confirm (issue 461: a button, as on the Send
 * screen). Found by its stable hook — its label is the request's action, in
 * the sheet's language — and pressed once the core's gate opens it: a click
 * waits for the button to be enabled, so a press made before the fee is
 * quoted is not one the sheet could have refused.
 */
export async function confirmSigning(page: Page): Promise<void> {
	const confirm = page.getByTestId('signing-confirm');
	await confirm.waitFor({ state: 'visible', timeout: 30_000 });
	await confirm.click({ timeout: 60_000 });
}
