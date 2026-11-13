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
 *
 * Round 17 (r17-1, r17-10): the same, pressed IMMEDIATELY after a refused
 * outage lifts — on the web and in the extension. The web's pool had marked
 * the refused node to be skipped for a while (its `not_connected` without
 * the wire), so the review's reads never reached the network: one press
 * landed on an enabled "Approve anyway" (a check never asked counted as
 * final), or said "Couldn't reach … to read this transaction" again until a
 * second press. The person's Try again is the person's own ask now — always
 * sent — and one press recovers.
 */
import { expect, test, type Browser, type BrowserContext, type Page } from '@playwright/test';
import { extensionBuilt, extensionId, loadExtension } from './extension-helpers';
import {
	FIXTURES,
	MultisigWorld,
	OUTSIDE,
	enterSpace,
	holdMultisig,
	multisigOf,
	storedRow,
	transferTx,
	useWorldNetworks,
	type Multisig
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

test('r17-1/r17-10: one Try again pressed right as a refused outage lifts reads the network — never "… anyway", never failed again', ({
	browser
}) => backFrom(browser, 'refused', { immediately: true }));

test.describe('in the extension', () => {
	test.skip(!extensionBuilt(), 'extension/dist is missing — run `pnpm build:extension`');
	test('r17-1/r17-10: one Try again pressed right as a refused outage lifts reads the network', ({
		browser
	}) => backFrom(browser, 'refused', { immediately: true, extension: true }));
});

/** The extension's wallet in the parallel space, fixture one alone, holding `ms`. */
async function extensionWallet(context: BrowserContext, ms: Multisig): Promise<Page> {
	const id = extensionId();
	const wallet = await context.newPage();
	await wallet.addInitScript(() => localStorage.setItem('vela.intro.seen', String(Date.now())));
	await wallet.goto(`chrome-extension://${id}/en/parallel.html`);
	await useWorldNetworks(wallet);
	await wallet.locator('li code').first().waitFor({ timeout: 60_000 });
	await wallet.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
	await wallet.waitForURL(/wallet\.html$/, { timeout: 30_000 });
	await wallet.evaluate(
		({ row, address, one }) => {
			const rows = JSON.parse(localStorage.getItem('vela.accounts') ?? '[]') as {
				address: string;
			}[];
			localStorage.setItem(
				'vela.accounts',
				JSON.stringify(rows.filter((r) => r.address.toLowerCase() === one.toLowerCase()))
			);
			localStorage.setItem('vela.activeAccountIndex', '0');
			localStorage.setItem('vela.multisigAccounts', JSON.stringify([row]));
			localStorage.setItem('vela.activeMultisig', address);
		},
		{ row: storedRow(ms), address: ms.address, one: FIXTURES.one }
	);
	await wallet.setViewportSize({ width: 430, height: 900 });
	await wallet.reload();
	return wallet;
}

async function backFrom(
	browser: Browser,
	mode: 'refused' | 'hung',
	how: { immediately?: boolean; extension?: boolean } = {}
): Promise<void> {
	const world = new MultisigWorld();
	world.simulates = true;
	// R22: both of the review's checks read once the network answers — its
	// one press after recovery is a plain Approve, never "… anyway".
	world.traces = true;
	const ms = await multisigOf(
		[FIXTURES.one, OUTSIDE.address],
		2,
		'Harbor fund',
		mode === 'refused' ? (how.immediately ? (how.extension ? '0x19' : '0x18') : '0x16') : '0x17'
	);
	world.record(ms);
	await world.proposeAsOutsideOwner(100, ms.address, transferTx(RECIPIENT, 10n ** 17n, 0));
	const context = how.extension
		? await loadExtension({ viewport: { width: 430, height: 900 } })
		: await browser.newContext({ viewport: { width: 430, height: 900 } });
	await world.install(context);
	let page: Page;
	if (how.extension) {
		page = await extensionWallet(context, ms);
	} else {
		page = await context.newPage();
		await enterSpace(page, [FIXTURES.one]);
		await holdMultisig(page, ms);
	}
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
		const sheet = page.getByRole('dialog').filter({ hasText: 'Multisig transaction' }).last();
		const tryAgain = sheet.getByRole('button', { name: 'Try again', exact: true }).first();
		if (how.immediately) {
			// r17-1 / r17-10: the outage lasts — the app's own reads keep
			// meeting the refused node, so its pool skips it ever longer (the
			// window doubles with each failure: 4 s after the fifth) — then
			// lifts just after a refusal, and the person presses at once:
			// before any read of the app's own has found the node again,
			// while its pool would still skip it. One press, and no other.
			await expect(tryAgain).toBeVisible();
			await expect
				.poll(() => world.refused, { timeout: 180_000, intervals: [500] })
				.toBeGreaterThanOrEqual(5);
			const last = world.refused;
			await expect
				.poll(() => world.refused > last, { timeout: 180_000, intervals: [50] })
				.toBe(true);
			const before = world.rpcAsked.length;
			world.outage = null;
			await tryAgain.click();
			await expect
				.poll(() => world.rpcAsked.slice(before).includes('100 eth_getCode'), {
					timeout: 10_000
				})
				.toBe(true);
		} else {
			// The outage lifts. The app's own reads find the network again (its
			// pool lets the node back after its cooldown); then, as Arc did, the
			// network turns the review's safety checks away for a moment more —
			// while answering its multisig's own state — and the person presses
			// Try again.
			world.outage = null;
			const answered = world.rpcAsked.length;
			await expect.poll(() => world.rpcAsked.length > answered, { timeout: 120_000 }).toBe(true);
			world.checksTurnedAwayUntil = Date.now() + 2_500;
			await tryAgain.click();
		}
		const approve = sheet.getByRole('button', { name: /^Approve( and execute)?$/ }).first();
		const where = `${how.extension ? 'ext-' : ''}${how.immediately ? 'now-' : ''}${mode}`;
		try {
			// One press recovers (r17-10): never "Couldn't reach Gnosis" again.
			await expect(approve).toBeEnabled({ timeout: how.immediately ? 30_000 : 90_000 });
		} finally {
			await page.screenshot({
				path: test.info().outputPath(`r16-3-${where}-end.png`),
				fullPage: true
			});
		}
		await expect(sheet.getByText("Couldn't reach Gnosis to read this transaction.")).toHaveCount(0);
		await expect(tryAgain).toHaveCount(0);
		// The checks were asked.
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
