/**
 * The wallet, running inside the packaged extension (spec 027 T314 — SC-306).
 *
 * The claim this suite exists to defend: **the extension is the SAME wallet**.
 * An address is derived from its keys, so if anything about running under
 * `chrome-extension://` changed the derivation, the extension would be a
 * different, empty wallet wearing the real one's face. Here the fixture keyset
 * is seeded through the app's own parallel space — which derives every address
 * by calling the core — and the multi-key golden Safe must come out at exactly
 * the address the hosted site shows and `core/golden-addresses.test.ts` pins.
 *
 * Getting a browser to run an extension at all took three measured facts
 * (spec 027 D39): Playwright's `headless: true` does not load extensions,
 * `--headless=new` does; `chrome://extensions` is not navigable, so the id is
 * PINNED by the manifest `key` and recomputed here from it; and a CDP virtual
 * authenticator is scoped to the target it was added to, though this suite does
 * not need one — the fixture signer replaces the authenticator entirely.
 */
import { expect, test } from '@playwright/test';
import { extensionBuilt, extensionId, hermetic, loadExtension } from './extension-helpers';

/** The multi-key golden Safe — derived, never stored (spec 019 invariant ②). */
const GOLDEN_SAFE = '0x88cCA0EeDbF2C4426110bbFc998F048689266894';
/** The first fixture Safe, shown on the parallel screen before anything is entered. */
const FIXTURE_ONE = '0xD400866e00B055B20752a826CD5C89b811de130b';

test.describe('the packaged extension', () => {
	test.skip(!extensionBuilt(), 'extension/dist is missing — run `pnpm build:extension`');
	test.setTimeout(120_000);

	test('boots the wallet and derives the SAME address the hosted site does', async () => {
		const context = await loadExtension({ viewport: { width: 420, height: 780 } });
		await hermetic(context);
		const id = extensionId();
		const page = await context.newPage();
		const failures: string[] = [];
		page.on('pageerror', (error) => failures.push(String(error).slice(0, 200)));
		await page.addInitScript(() => {
			localStorage.setItem('vela.intro.seen', String(Date.now()));
			localStorage.setItem('vela.dev.console', '1');
		});

		// The core runs here: this screen's addresses are DERIVED by it, at the
		// moment the page opens. If wasm could not compile under the manifest's
		// CSP, there would be no address to read.
		await page.goto(`chrome-extension://${id}/en/parallel.html`);
		await expect(page.getByText(FIXTURE_ONE, { exact: true })).toBeVisible({ timeout: 30_000 });
		await expect(page.getByText(GOLDEN_SAFE, { exact: true })).toBeVisible();

		// Entering does a FULL navigation on purpose (every resident store must
		// re-hydrate). Under the packaged app a route path is not a file, so this
		// is also the test that spec 027 D42's translation works: without it the
		// tab lands on `chrome-error://chromewebdata/`.
		await page.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
		await page.waitForURL(/\/en\/wallet\.html$/, { timeout: 30_000 });
		await expect(page.getByTestId('parallel-space-badge')).toBeVisible();

		// The multi-key account is the one founded on all three fixture keys.
		await page.evaluate(() => localStorage.setItem('vela.activeAccountIndex', '3'));
		await page.goto(`chrome-extension://${id}/en/wallet.html`);
		await expect(page.getByText('Parallel Multi').first()).toBeVisible({ timeout: 30_000 });
		// The shortened form the hero shows for the golden Safe. Same keys, same
		// core, same derivation — inside a `chrome-extension://` origin.
		await expect(page.getByText('0x88cCA0…266894').first()).toBeVisible();

		expect(failures, 'the packaged app threw while booting').toEqual([]);
		await context.close();
	});

	test('a reload after a client navigation still finds a document (D42)', async () => {
		const context = await loadExtension({ viewport: { width: 420, height: 780 } });
		await hermetic(context);
		const id = extensionId();
		const page = await context.newPage();
		await page.addInitScript(() => localStorage.setItem('vela.intro.seen', String(Date.now())));

		// No wallet: the route guard sends this visitor to Welcome, client-side.
		await page.goto(`chrome-extension://${id}/en/wallet.html`);
		await expect(page).toHaveURL(/\/en\.html$/, { timeout: 30_000 });

		// The address bar names a real document, so this survives — which is the
		// whole point: an extension URL resolves no directory index and no
		// extensionless twin, both measured.
		await page.reload();
		await expect(page.locator('body')).not.toHaveText('');
		expect(page.url()).toMatch(/\/en\.html$/);
		await context.close();
	});

	/**
	 * Spec 094 S4: Settings → Erase This Device inside the extension landed
	 * on `chrome-error://chromewebdata/` — its full navigation went to the
	 * route path `/en`, which is no file in a package. It goes to the
	 * welcome's DOCUMENT now, and the extension's own storage is clean too.
	 */
	test('erasing the device in the extension lands on the welcome, not on a Chrome error', async () => {
		const context = await loadExtension({ viewport: { width: 420, height: 780 } });
		await hermetic(context);
		const id = extensionId();
		const page = await context.newPage();
		await page.addInitScript(() => {
			localStorage.setItem('vela.intro.seen', String(Date.now()));
			localStorage.setItem('vela.dev.console', '1');
		});
		await page.goto(`chrome-extension://${id}/en/parallel.html`);
		await page.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
		await page.waitForURL(/\/en\/wallet\.html$/, { timeout: 30_000 });

		await page.goto(`chrome-extension://${id}/en/settings.html`);
		await page.getByText('Erase This Device', { exact: true }).click();
		await page.getByRole('button', { name: 'Erase Everything' }).click();
		await page.waitForURL(/\/en\.html$/, { timeout: 30_000 });
		expect(page.url()).toBe(`chrome-extension://${id}/en.html`);
		await expect(page.locator('body')).not.toHaveText('');
		const left = await page.evaluate(async () => {
			const all = await (
				window as unknown as {
					chrome: { storage: { local: { get(k: null): Promise<Record<string, unknown>> } } };
				}
			).chrome.storage.local.get(null);
			return Object.keys(all).filter(
				(key) => key.startsWith('vela.perm.') || key === 'vela.ext.cache'
			);
		});
		expect(left).toEqual([]);
		await context.close();
	});
});
