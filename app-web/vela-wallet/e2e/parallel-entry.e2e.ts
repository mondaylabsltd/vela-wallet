/**
 * The parallel space, end to end (spec 026 T226 — US4).
 *
 * Three promises, checked against the built artifact the site actually serves:
 *   1. entering swaps in the fixture wallet AND marks it — a test wallet must
 *      never wear the real one's face;
 *   2. leaving gives the person's wallet back exactly as it was;
 *   3. a visit with no gate never loads the fixture keys — the private keys
 *      are public by design, so their presence in a production chunk would be
 *      a real hazard, not a tidiness question.
 */
import { expect, test } from '@playwright/test';
import { readKv } from './stub-chain';
import { chunksCarrying, collectScripts, en, seedSignedIn } from './live-helpers';

test.use({ viewport: { width: 390, height: 844 } });

/** The multi-key golden Safe — derived, never stored (spec 019 invariant ②). */
const GOLDEN_SAFE = '0x88cCA0EeDbF2C4426110bbFc998F048689266894';

test('entering swaps in the fixture wallet and marks it; leaving gives the real one back', async ({
	page
}) => {
	await seedSignedIn(page);
	await page.goto('/en/wallet');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	const realWallet = await page.evaluate(() => localStorage.getItem('vela.accounts'));
	await expect(page.getByTestId('parallel-space-badge')).toHaveCount(0);

	await page.goto('/en/parallel');
	// Pressed before hydration, the server-rendered button does nothing (batch.e2e.ts):
	// the fixture list fills on mount, so a row in it means the handlers are on.
	await page.locator('li code').first().waitFor({ timeout: 60_000 });
	await page.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();

	// The badge is the promise: a fixture wallet always says so — here, and
	// on the wallet the entry lands on.
	await expect(page.getByTestId('parallel-space-badge')).toBeVisible();
	await page.waitForURL(/\/en\/wallet$/);
	await expect(page.getByText('Parallel One').first()).toBeVisible();
	await expect(page.getByTestId('parallel-space-badge')).toBeVisible();

	const swapped = await page.evaluate(() => localStorage.getItem('vela.accounts'));
	expect(swapped).not.toBe(realWallet);
	expect(JSON.parse(swapped!)).toHaveLength(4);
	// The fixture contact is seeded into the book the core reads.
	const contacts = await readKv(page, 'vela.contacts');
	expect(contacts).toContain('Alice Chen');

	// The badge is a door back out.
	await page.getByTestId('parallel-space-badge').click();
	await expect(page).toHaveURL(/\/en\/parallel$/);
	await page.getByRole('button', { name: 'Leave (restore real wallet)' }).click();

	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await expect(page.getByTestId('parallel-space-badge')).toHaveCount(0);
	expect(await page.evaluate(() => localStorage.getItem('vela.accounts'))).toBe(realWallet);
	expect(await readKv(page, 'vela.contacts')).not.toContain('Alice Chen');
});

test('the space survives a reload, and the multi-key golden Safe is one of its wallets', async ({
	page
}) => {
	await page.goto('/en/parallel');
	// Pressed before hydration, the server-rendered button does nothing (batch.e2e.ts):
	// the fixture list fills on mount, so a row in it means the handlers are on.
	await page.locator('li code').first().waitFor({ timeout: 60_000 });
	await page.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
	// Entering navigates on its own; reloading mid-flight would abort it.
	await page.waitForURL(/\/en\/wallet$/);
	await expect(page.getByTestId('parallel-space-badge')).toBeVisible();

	await page.reload();
	// Re-armed on boot, not merely remembered in memory: the badge is back and
	// the signer is installed again.
	await expect(page.getByTestId('parallel-space-badge')).toBeVisible();

	// The parallel screen lists the golden Safe the live sweep spends from.
	await page.goto('/en/parallel');
	await expect(page.getByText(GOLDEN_SAFE, { exact: true })).toBeVisible();
});

test('a normal visit never loads the fixture keys', async ({ page }) => {
	const scripts = collectScripts(page);

	await seedSignedIn(page);
	await page.goto('/en/wallet');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await page.waitForLoadState('networkidle');
	expect(scripts.length).toBeGreaterThan(0);

	// The first fixture private key, verbatim. It must appear in NOTHING this
	// page loaded — the parallel space is one dynamic import away, never in a
	// chunk a visitor pays for.
	const SEED = 'd80133c59ce0943689a9c1ff6006242c27b19412439fbc88f94feb5ca1e802d5';
	expect(
		chunksCarrying(scripts, SEED),
		'fixture keys reached a chunk a normal visit loads'
	).toEqual([]);

	// And the wallet is the real one: no badge, no requester, no parallel verbs.
	await expect(page.getByTestId('parallel-space-badge')).toHaveCount(0);
	expect(
		await page.evaluate(
			() => (window as unknown as { vela?: { parallel?: unknown } }).vela?.parallel !== undefined
		)
	).toBe(false);
	// The wallet still works — the guard did not cost the page anything.
	await expect(page.getByRole('button', { name: en('componentsUi.dock.receive') })).toBeVisible();
});
