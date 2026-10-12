/**
 * A lazy chunk fails to load — and the app does the right one of three
 * things (`src/lib/services/chunk-recovery.ts`), in a real browser, on the
 * built app.
 *
 * The layout used to answer every `vite:preloadError` with `location.reload()`.
 * That was wrong twice over:
 *
 * - **a page being left** has its in-flight lazy imports cancelled, Firefox
 *   reports each one, and the reload it got in answer cancelled the very
 *   navigation the person had asked for (`NS_BINDING_ABORTED`) — the reason
 *   three storage suites failed on Firefox, a different test each run;
 * - **a dropped connection** is not a moved build, and a reload for it throws
 *   away whatever was on screen.
 *
 * And the one case a reload does fix — the build moved, the old chunks are
 * gone — must not loop when the fresh document cannot load them either.
 *
 * Nothing here waits on a clock: a navigation is awaited, and "it did NOT
 * reload" is asserted once the app has demonstrably finished deciding (it has
 * asked the server which build is deployed, and been answered).
 */
import { expect, test, type Page, type Route } from '@playwright/test';
import { seedSignedIn } from './live-helpers';
import { denyOffOrigin } from './stub-chain';

test.use({ viewport: { width: 390, height: 844 } });
test.setTimeout(90_000);

const CHUNK = /\/_app\/immutable\/.*\.(js|css)(\?.*)?$/;
const VERSION = /\/_app\/version\.json(\?.*)?$/;

/** How many times each document of the app has been asked for. */
function countDocuments(page: Page): { of(path: string): number } {
	const seen: string[] = [];
	page.on('request', (request) => {
		if (request.isNavigationRequest() && request.frame() === page.mainFrame()) {
			seen.push(new URL(request.url()).pathname);
		}
	});
	return { of: (path) => seen.filter((p) => p === path).length };
}

/**
 * The chunks the wallet has loaded by the time it stops asking for more for
 * a moment: its first paint's, and what follows straight behind it. Not a
 * promise that nothing lazy is left — what it warms later (two dozen chunks,
 * seconds on) is not in the set, and nothing below needs it to be: every
 * chunk the first paint needs was loaded before the wallet's name was on the
 * page, so whatever is NOT in the set is a lazy import, like the dev console.
 */
async function settledChunks(page: Page): Promise<Set<string>> {
	const loaded = new Set<string>();
	page.on('requestfinished', (request) => {
		if (CHUNK.test(request.url())) loaded.add(new URL(request.url()).pathname);
	});
	await page.goto('/en/wallet');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	let last = -1;
	await expect
		.poll(
			() => {
				const still = loaded.size === last;
				last = loaded.size;
				return still;
			},
			{ timeout: 30_000, intervals: [1_000] }
		)
		.toBe(true);
	return new Set(loaded);
}

/**
 * "The old build's lazy chunks are gone": every chunk the settled wallet did
 * not load answers 404 from here on. The dev console — a lazy import the
 * layout makes when its gate is set — is then a failed import on every boot.
 */
async function loseOtherChunks(page: Page, kept: Set<string>): Promise<{ lost: () => number }> {
	let lost = 0;
	await page.route(CHUNK, (route: Route) => {
		if (kept.has(new URL(route.request().url()).pathname)) return route.fallback();
		lost += 1;
		return route.fulfill({ status: 404, contentType: 'text/plain', body: 'gone' });
	});
	await page.evaluate(() => localStorage.setItem('vela.dev.console', '1'));
	return { lost: () => lost };
}

/** The app's questions to the server about which build is deployed, and their answers. */
async function watchVersion(page: Page, answer?: string): Promise<{ answered: () => number }> {
	let answered = 0;
	if (answer !== undefined) {
		await page.route(VERSION, (route) =>
			route.fulfill({ contentType: 'application/json', body: JSON.stringify({ version: answer }) })
		);
	}
	page.on('requestfinished', (request) => {
		if (VERSION.test(request.url())) answered += 1;
	});
	return { answered: () => answered };
}

/** The decision that follows an answer has been taken: the page has run on past it. */
const ranOn = (page: Page) =>
	page.evaluate(
		() =>
			new Promise<void>((resolve) =>
				requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
			)
	);

test.beforeEach(async ({ page }) => {
	await seedSignedIn(page);
	await denyOffOrigin(page);
});

test('leaving a page whose lazy chunks are still loading goes where it was asked to — no reload of the page being left', async ({
	page
}, testInfo) => {
	const documents = countDocuments(page);
	const reported: string[] = [];
	page.on('console', (message) => {
		if (message.text().startsWith('CHUNK-RECOVERY-PROBE')) reported.push(message.text());
	});
	await page.addInitScript(() => {
		addEventListener('vite:preloadError', () => console.log('CHUNK-RECOVERY-PROBE preloadError'));
	});
	const kept = await settledChunks(page);
	// A lazy import the wallet makes once it is up (the dev console, behind its
	// gate) is HELD on the wire: it is in flight when the person leaves.
	const held: Route[] = [];
	let hold = true;
	await page.route(CHUNK, (route) => {
		if (!hold || kept.has(new URL(route.request().url()).pathname)) return route.fallback();
		held.push(route);
	});
	await page.evaluate(() => localStorage.setItem('vela.dev.console', '1'));
	// Not until "load". A lazy import is preloaded with a `<link>`, and this
	// one is being held: when the import starts while the document is still
	// loading, WebKit does not fire `load` until that link answers — so the
	// reload waited out the test, one run in eight on a busy machine, and
	// three in three with that order forced (Chromium and Firefox fire it
	// regardless). The page is up when the wallet's name is on it.
	await page.reload({ waitUntil: 'commit' });
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await expect.poll(() => held.length, { timeout: 30_000 }).toBeGreaterThan(0);
	const walletLoads = documents.of('/en/wallet');

	// The person goes somewhere else. The browser cancels what is in flight;
	// Firefox reports each cancelled import to the page that is being left.
	hold = false;
	await page.goto('/en/settings');
	await expect(page).toHaveURL(/\/en\/settings$/);
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	// It went where it was asked — once — and the wallet was not asked for again.
	expect(documents.of('/en/settings')).toBe(1);
	expect(documents.of('/en/wallet')).toBe(walletLoads);
	testInfo.annotations.push({
		type: 'preload-errors',
		description: `${testInfo.project.name}: ${held.length} chunk(s) in flight when the page was left, ${reported.length} vite:preloadError reported to it`
	});
	for (const route of held) await route.abort().catch(() => undefined);
});

test('a lazy chunk the network dropped is not a moved build: no reload', async ({ page }) => {
	const documents = countDocuments(page);
	const kept = await settledChunks(page);
	// The build is where it was: the server's own `version.json` answers.
	const version = await watchVersion(page);
	const chunks = await loseOtherChunks(page, kept);
	const before = version.answered();
	await page.reload();
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	// A lazy import failed, and the app asked which build is deployed…
	await expect.poll(chunks.lost, { timeout: 30_000 }).toBeGreaterThan(0);
	await expect.poll(version.answered, { timeout: 30_000 }).toBeGreaterThan(before);
	await ranOn(page);
	// …the same one: the document stays. First visit + this test's reload.
	expect(documents.of('/en/wallet')).toBe(2);
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
});

test('the build moved: the tab reloads ONCE — and does not loop when what comes back still cannot load its chunks', async ({
	page
}) => {
	const documents = countDocuments(page);
	const kept = await settledChunks(page);
	// A deploy: the server reports another build, and the old lazy chunks are
	// gone — for this document AND for the one a reload brings back (a cached
	// shell in front of a moved deployment).
	const version = await watchVersion(page, 'another-build');
	const chunks = await loseOtherChunks(page, kept);
	await page.reload();

	// First visit, this test's reload, and ONE recovery.
	await expect.poll(() => documents.of('/en/wallet'), { timeout: 30_000 }).toBe(3);
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	// The tab remembers the build it reloaded away from.
	expect(await page.evaluate(() => sessionStorage.getItem('vela.chunkRecovery'))).toBeTruthy();

	// What came back fails the same import (one lost chunk per document that
	// booted), asks the server again, is told "moved" again — and stays where
	// it is.
	await expect.poll(chunks.lost, { timeout: 30_000 }).toBeGreaterThanOrEqual(2);
	await expect.poll(version.answered, { timeout: 30_000 }).toBeGreaterThanOrEqual(2);
	await ranOn(page);
	expect(documents.of('/en/wallet')).toBe(3);

	// However often it fails from here on: the person can keep using the page.
	for (let again = 0; again < 3; again++) {
		const asked = version.answered();
		await page.evaluate(() => {
			const failure = new Event('vite:preloadError', { cancelable: true });
			Object.assign(failure, { payload: new Error('error loading dynamically imported module') });
			window.dispatchEvent(failure);
		});
		await expect.poll(version.answered, { timeout: 30_000 }).toBeGreaterThan(asked);
		await ranOn(page);
	}
	expect(documents.of('/en/wallet')).toBe(3);
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
});
