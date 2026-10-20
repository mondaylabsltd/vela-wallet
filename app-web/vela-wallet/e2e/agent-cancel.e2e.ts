/**
 * The bridge giving up on an agent's call (spec 103): its call timeout, or the
 * agent's own cancel, reaches the wallet as ClientMCP's `cancel`. A request
 * still waiting for the person is withdrawn — the sheet goes — and nothing is
 * done after it, so an agent told "timed out" never finds the change made.
 */
import { expect, test } from '@playwright/test';
import { en } from './live-helpers';
import { enableAgent, playBridge } from './agent-bridge';
import { denyOffOrigin } from './stub-chain';

test.use({ viewport: { width: 390, height: 844 } });
test.setTimeout(120_000);

const BOB = '0x' + 'b0'.repeat(20);

test('a cancelled call withdraws its approval, and the contact stays', async ({ page }) => {
	await denyOffOrigin(page);
	const bridge = await playBridge(page);
	await page.goto('/en/parallel');
	await page.locator('li code').first().waitFor({ timeout: 60_000 });
	await page.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
	await page.waitForURL(/\/en\/wallet$/);
	await enableAgent(page);
	await page.reload();
	await expect.poll(() => bridge.tools().length, { timeout: 30_000 }).toBeGreaterThan(30);

	expect(await bridge.call('vela_save_contact', { address: BOB, name: 'Bob' })).toMatchObject({
		status: 'completed'
	});

	// A manage-level request: the person is asked in the wallet.
	const deleting = bridge.start('vela_delete_contact', { address: BOB });
	const sheet = page.getByText(en('settings.agent.approveTitle'));
	await expect(sheet).toBeVisible({ timeout: 15_000 });

	// The bridge gives up on the call (its timeout): the request is withdrawn.
	deleting.cancel();
	await expect(sheet).toBeHidden({ timeout: 10_000 });

	const state = await bridge.call('vela_state');
	const contacts = state.contacts as { address: string }[];
	expect(contacts.some((c) => c.address.toLowerCase() === BOB)).toBe(true);
	expect((state.waiting_for_person as unknown[]).length).toBe(0);
	expect(deleting.answered(), 'a cancelled call is not answered').toBe(false);
});
