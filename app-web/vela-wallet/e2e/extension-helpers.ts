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
import { join } from 'node:path';
import { chromium, type BrowserContext, type Page } from '@playwright/test';

const APP_ROOT = join(import.meta.dirname, '..');
export const EXTENSION_DIST = join(APP_ROOT, 'extension/dist');

/** Has `pnpm build:extension` run? Suites skip rather than fail obscurely. */
export const extensionBuilt = (): boolean => existsSync(join(EXTENSION_DIST, 'manifest.json'));

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
	options: { viewport?: { width: number; height: number }; surface?: 'window' | 'panel' } = {}
): Promise<BrowserContext> {
	const context = await chromium.launchPersistentContext('', {
		headless: false,
		args: [
			'--headless=new',
			`--disable-extensions-except=${EXTENSION_DIST}`,
			`--load-extension=${EXTENSION_DIST}`
		],
		...(options.viewport ? { viewport: options.viewport } : {})
	});
	if ((options.surface ?? 'window') === 'window') {
		// Any document of the extension's origin can write its storage; the
		// manifest is the cheapest one to open.
		const page = await context.newPage();
		await page.goto(`chrome-extension://${extensionId()}/manifest.json`);
		await preferWindows(page);
		await page.close();
	}
	return context;
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
 * Drive the open side panel from another extension page.
 *
 * `chrome.extension.getViews()` hands an extension page the `Window` of every
 * other view of the extension in its process — the side panel included — so a
 * test can read its heading and press its buttons without a Page of its own.
 * Resolves with the panel's heading once it is showing something.
 */
export async function sidePanelView(
	extensionPage: Page,
	timeoutMs = 15_000
): Promise<{ heading: string; click(buttonText: string): Promise<void> }> {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		const heading = await extensionPage.evaluate(() => {
			const view = (window as unknown as ChromeLocal).chrome.extension
				.getViews()
				.find((w) => w.location.href.includes('/request.html') && !w.location.search);
			return view?.document.querySelector('h1')?.textContent ?? null;
		});
		if (heading) {
			return {
				heading,
				click: (buttonText: string) =>
					extensionPage.evaluate((text) => {
						const view = (window as unknown as ChromeLocal).chrome.extension
							.getViews()
							.find((w) => w.location.href.includes('/request.html') && !w.location.search);
						const button = [...(view?.document.querySelectorAll('button') ?? [])].find(
							(b) => b.textContent?.trim() === text
						);
						if (!button) throw new Error(`no "${text}" button in the side panel`);
						button.click();
					}, buttonText)
			};
		}
		await new Promise((r) => setTimeout(r, 250));
	}
	throw new Error('no side panel showed a request');
}

/** Is a side panel showing a request right now? */
export const sidePanelOpen = (extensionPage: Page): Promise<boolean> =>
	extensionPage.evaluate(() =>
		(window as unknown as ChromeLocal).chrome.extension
			.getViews()
			.some((w) => w.location.href.includes('/request.html') && !w.location.search)
	);

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
 * Confirm the slide control — by keyboard.
 *
 * The control commits on a pointer drag past 88% of its travel, and it also
 * commits on Enter, because a slider a keyboard user cannot operate is a
 * signature they cannot give. The keyboard path is what this drives: it is the
 * SAME `onconfirm`, and it is the half that would otherwise never be exercised.
 *
 * (A synthesized pointer drag was tried first and does not commit here — the
 * control captures the pointer, and Playwright's synthesized moves do not
 * reach the captured element. Worth knowing before spending an hour on it.)
 */
export async function slideToConfirm(page: Page): Promise<void> {
	const slider = page.getByRole('button', { name: /^Slide to confirm/ });
	await slider.waitFor({ state: 'visible', timeout: 30_000 });
	await slider.focus();
	await slider.press('Enter');
}
