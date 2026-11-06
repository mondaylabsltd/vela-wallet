/**
 * Round 16 (r16-3): a review coming back from an outage. On Arc, one Try
 * again after the outage lifted landed on an ENABLED "Approve anyway" —
 * for 1.06 s with Arc refused, 5.5 s with Arc hung — and then on "Approve":
 * the network answered its multisig's own state first while its safety
 * checks were still turned away, and the review was published "unchecked"
 * before they settled.
 *
 * The rule (the core's, `review_state::checks_final`): a review back from
 * failed goes failed → Reading (every sign button off) → ready, and never
 * exposes "… anyway" until its checks are final for the read. A DOM watcher
 * runs from before the press to the end — every mutation and every frame —
 * and fails the run if any enabled "… anyway" button appears.
 *
 * Against the hermetic multisig world (`stub-multisig.ts`): its chain goes
 * down (refused at connect, or never answering), comes back, and then turns
 * its safety checks away for a moment more, as Arc did.
 */
import { expect, test, type Browser } from '@playwright/test';
import {
	FIXTURES,
	MultisigWorld,
	OUTSIDE,
	enterSpace,
	holdMultisig,
	multisigOf,
	transferTx
} from './stub-multisig';

test.setTimeout(240_000);

const RECIPIENT = '0x000000000000000000000000000000000000bEEF';

declare global {
	interface Window {
		/** Every enabled "… anyway" button the watcher saw, and when. */
		__anyway?: string[];
		/** Whether the watcher saw "Reading the transaction…". */
		__reading?: boolean;
	}
}

test('back from a refused network: failed → Reading → Approve, never an enabled "… anyway"', ({
	browser
}) => backFrom(browser, 'refused'));

test('back from a hung network: failed → Reading → Approve, never an enabled "… anyway"', ({
	browser
}) => backFrom(browser, 'hung'));

async function backFrom(browser: Browser, mode: 'refused' | 'hung'): Promise<void> {
	const world = new MultisigWorld();
	world.simulates = true;
	const ms = await multisigOf(
		[FIXTURES.one, OUTSIDE.address],
		2,
		'Harbor fund',
		mode === 'refused' ? '0x16' : '0x17'
	);
	world.record(ms);
	await world.proposeAsOutsideOwner(100, ms.address, transferTx(RECIPIENT, 10n ** 17n, 0));
	const context = await browser.newContext({ viewport: { width: 430, height: 900 } });
	await world.install(context);
	const page = await context.newPage();
	await enterSpace(page, [FIXTURES.one]);
	await holdMultisig(page, ms);
	const row = page.getByRole('main').getByText('Send 0.1 xDAI', { exact: true }).first();
	await expect(row).toBeVisible({ timeout: 60_000 });

	await test.step('the network goes down: the review fails, nothing signable', async () => {
		world.outage = mode;
		await row.click();
		await expect(page.getByRole('heading', { name: 'Multisig transaction' })).toBeVisible();
		await expect(
			page.getByText("Couldn't reach Gnosis to read this transaction.").first()
		).toBeVisible({ timeout: 90_000 });
		await expect(page.getByRole('button', { name: /anyway$/ })).toHaveCount(0);
	});

	await test.step('it comes back; Try again: Reading, then Approve — never "… anyway"', async () => {
		await page.evaluate(() => {
			window.__anyway = [];
			window.__reading = false;
			const started = performance.now();
			const look = () => {
				for (const b of document.querySelectorAll('button')) {
					const name = (b.textContent ?? '').replace(/\s+/g, ' ').trim();
					if (/anyway$/i.test(name) && !b.disabled && b.getAttribute('aria-disabled') !== 'true')
						window.__anyway!.push(`${Math.round(performance.now() - started)} ms: ${name}`);
				}
				if ((document.body.textContent ?? '').includes('Reading the transaction'))
					window.__reading = true;
			};
			new MutationObserver(look).observe(document.body, {
				subtree: true,
				childList: true,
				attributes: true,
				characterData: true
			});
			const frame = () => {
				look();
				requestAnimationFrame(frame);
			};
			frame();
		});
		// The outage lifts. The app's own reads find the network again (its
		// pool lets the node back after its cooldown); then, as Arc did, the
		// network turns the review's safety checks away for a moment more —
		// while answering its multisig's own state — and the person presses
		// Try again.
		world.outage = null;
		const answered = world.rpcAsked.length;
		await expect.poll(() => world.rpcAsked.length > answered, { timeout: 120_000 }).toBe(true);
		world.checksTurnedAwayUntil = Date.now() + 2_500;
		const sheet = page.getByRole('dialog').filter({ hasText: 'Multisig transaction' }).last();
		await sheet.getByRole('button', { name: 'Try again', exact: true }).first().click();
		const approve = sheet.getByRole('button', { name: /^Approve( and execute)?$/ }).first();
		try {
			await expect(approve).toBeEnabled({ timeout: 90_000 });
		} finally {
			await page.screenshot({
				path: test.info().outputPath(`r16-3-${mode}-end.png`),
				fullPage: true
			});
		}
		// The world turned the checks away, and they were asked again.
		expect(world.rpcAsked.filter((m) => m === '100 eth_simulateV1').length).toBeGreaterThan(0);
		const seen = await page.evaluate(() => ({
			anyway: window.__anyway ?? [],
			reading: window.__reading ?? false
		}));
		expect(seen.anyway, 'never an enabled "… anyway" on the way back').toEqual([]);
		expect(seen.reading, 'Reading on the way').toBe(true);
	});
	await context.close();
}
