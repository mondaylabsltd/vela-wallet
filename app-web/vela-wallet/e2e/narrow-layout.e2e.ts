/**
 * r14-1: the phone layout never runs past the screen, 320 to 430 wide.
 *
 * The Safari extension's wallet tab was found 429 pt of page in a 402 pt
 * screen — chevrons, the Scan card, the text size and the Settings tab cut
 * off at the right. The cause was a ZOOM, not a wide element: iOS zooms in by
 * 16/15 when a field with 15 px text takes focus, and never zooms back out
 * (`viewport-zoom.test.ts`; `app.html` caps the scale on iOS alone). These
 * hold the other half — that the layout itself fits — on the screens the
 * tester walked: the wallet home with a Vela account and with a multisig
 * shown, both carrying long names, the account switcher, the settings home
 * and its Switch account sheet; and they hold that an iPhone gets the cap.
 *
 * Every request off this machine is refused: the screens are drawn in the
 * states a failing network gives them, which carry the most words.
 */
import { expect, test, type Page } from '@playwright/test';
import { FIXTURES, multisigOf, storedRow, type Multisig } from './stub-multisig';

const WIDTHS = [320, 375, 402, 430] as const;
const LONG_NAME = 'Everyday wallet for the family budget and the long trips';
const MULTISIG_NAME = 'Harbor operations fund for the whole crew and partners';

const ACCOUNT = {
	id: 'e2e-credential-long',
	name: LONG_NAME,
	address: FIXTURES.one,
	public_key_hex: '04' + 'ab'.repeat(64),
	created_at_iso: '2026-01-01T00:00:00.000Z',
	keys: []
};

/** What runs past the screen: the document, or any box that scrolls sideways. */
async function overflow(page: Page): Promise<string[]> {
	return page.evaluate(() => {
		const width = window.innerWidth;
		const found: string[] = [];
		const doc = document.scrollingElement as HTMLElement;
		if (doc.scrollWidth > width) found.push(`document ${doc.scrollWidth} > ${width}`);
		for (const el of document.querySelectorAll<HTMLElement>('body *')) {
			const style = getComputedStyle(el);
			if (style.overflowX !== 'auto' && style.overflowX !== 'scroll') continue;
			// A strip that scrolls sideways on purpose says so.
			if (el.closest('[data-scrolls-x]')) continue;
			if (el.clientWidth > 0 && el.scrollWidth > el.clientWidth + 1)
				found.push(
					`${el.tagName.toLowerCase()}.${[...el.classList].join('.')} ${el.scrollWidth} > ${el.clientWidth}`
				);
		}
		for (const el of document.querySelectorAll<HTMLElement>('body *')) {
			const box = el.getBoundingClientRect();
			if (box.width === 0 || box.height === 0) continue;
			if (box.right > width + 0.5 || box.left < -0.5) {
				// Cut off inside a box that clips it — an ellipsis — and that box
				// fits: not past the screen. Nor is what is hidden on purpose.
				let clipped = el.closest('.visually-hidden, [data-scrolls-x]') !== null;
				for (let up = el.parentElement; up && !clipped; up = up.parentElement) {
					const x = getComputedStyle(up).overflowX;
					const edge = up.getBoundingClientRect();
					if (x !== 'visible' && edge.right <= width + 0.5 && edge.left >= -0.5) clipped = true;
				}
				if (!clipped && getComputedStyle(el).visibility !== 'hidden')
					found.push(
						`${el.tagName.toLowerCase()}.${[...el.classList].join('.')} [${box.left.toFixed(0)}, ${box.right.toFixed(0)}]`
					);
			}
		}
		return found;
	});
}

async function seed(page: Page, multisig?: Multisig): Promise<void> {
	await page.context().route(
		(url) => url.hostname !== 'localhost' && url.hostname !== '127.0.0.1',
		(route) => route.abort()
	);
	await page.addInitScript(
		({ account, row, address }) => {
			localStorage.setItem('vela.intro.seen', String(Date.now()));
			if (localStorage.getItem('vela.accounts') === null) {
				localStorage.setItem('vela.accounts', JSON.stringify([account]));
				localStorage.setItem('vela.activeAccountIndex', '0');
				if (row) {
					localStorage.setItem('vela.multisigAccounts', JSON.stringify([row]));
					localStorage.setItem('vela.activeMultisig', address);
				}
			}
		},
		{
			account: ACCOUNT,
			row: multisig ? storedRow(multisig) : null,
			address: multisig?.address ?? ''
		}
	);
}

for (const width of WIDTHS) {
	test.describe(`at ${width} wide`, () => {
		test.use({ viewport: { width, height: 800 } });

		test('the wallet home, its switcher, settings and Switch account fit the screen', async ({
			page
		}) => {
			await seed(page);
			await page.goto('/en/wallet');
			const name = page.getByRole('button', { name: LONG_NAME });
			await expect(name).toBeVisible();
			expect(await overflow(page)).toEqual([]);

			await name.click();
			const switcher = page.getByRole('dialog');
			await expect(switcher.getByRole('button', { name: 'Add account' })).toBeVisible();
			expect(await overflow(page)).toEqual([]);
			await switcher.getByRole('button', { name: 'Add account' }).click();
			await expect(switcher.getByRole('button', { name: /^Create multisig/ })).toBeVisible();
			expect(await overflow(page)).toEqual([]);

			await page.goto('/en/settings');
			const row = page.getByRole('button', { name: new RegExp(LONG_NAME) }).first();
			await expect(row).toBeVisible();
			expect(await overflow(page)).toEqual([]);
			await row.click();
			await expect(
				page.getByRole('dialog').getByRole('button', { name: 'Create New Account' })
			).toBeVisible();
			expect(await overflow(page)).toEqual([]);
		});

		test('the wallet home of a multisig with a long name fits the screen', async ({ page }) => {
			const ms = await multisigOf([FIXTURES.one, FIXTURES.two, FIXTURES.three], 2, MULTISIG_NAME);
			await seed(page, ms);
			await page.goto('/en/wallet');
			await expect(page.getByRole('button', { name: MULTISIG_NAME })).toBeVisible();
			expect(await overflow(page)).toEqual([]);

			await page.getByRole('button', { name: MULTISIG_NAME }).click();
			await expect(page.getByRole('dialog').getByText(LONG_NAME)).toBeVisible();
			expect(await overflow(page)).toEqual([]);
		});
	});
}

test.describe('on an iPhone', () => {
	test.use({
		viewport: { width: 402, height: 874 },
		userAgent:
			'Mozilla/5.0 (iPhone; CPU iPhone OS 26_2 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.2 Mobile/15E148 Safari/604.1'
	});

	test('the page caps its scale, so a focused field never zooms it', async ({ page }) => {
		await seed(page);
		await page.goto('/en/wallet');
		await expect(page.locator('meta[name="viewport"]')).toHaveAttribute(
			'content',
			'width=device-width, initial-scale=1, maximum-scale=1'
		);
	});
});

test('anywhere else, the viewport keeps its pinch-zoom', async ({ page }) => {
	await seed(page);
	await page.goto('/en/wallet');
	await expect(page.locator('meta[name="viewport"]')).toHaveAttribute(
		'content',
		'width=device-width, initial-scale=1'
	);
});
