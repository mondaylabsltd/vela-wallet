/**
 * Paying many people at once (spec 026 T253 — US3).
 *
 * Paste a table, watch the core parse and price it, and send one operation
 * that carries every recipient. Two properties are the point:
 *
 *   1. When nobody can price the chosen currency, the importer REFUSES rather
 *      than converting at 1:1. That is the ~7x payroll mistake the machine was
 *      written to prevent, and it is checked here against the real screen.
 *   2. SheetJS never reaches the startup path. It is ~1 MB, and a person who
 *      never opens a spreadsheet must not pay for it.
 */
import { expect, test } from '@playwright/test';
import { CHAINS } from '../src/lib/services/chains';
import { chunksCarrying, collectScripts, en } from './live-helpers';
import {
	abiWord,
	aggregate3CallCount,
	denyOffOrigin,
	encodeAggregate3Result,
	happyRelay,
	seedNetworkOverrides,
	stubChainRegistry,
	stubJsonRpc,
	stubRelay
} from './stub-chain';

test.use({ viewport: { width: 390, height: 844 } });
test.setTimeout(120_000);

const STUB = 'https://stub-rpc.test/rpc';
const RELAY = /vela-relay\.getvela\.app/;
const ETH = 1_000_000_000_000_000_000n;
const PRICE_8DP = 3000n * 100_000_000n;
const ALICE = '0x' + 'a1'.repeat(20);
const BOB = '0x' + 'b2'.repeat(20);
const CARO = '0x' + 'c3'.repeat(20);

async function openSendForm(page: import('@playwright/test').Page): Promise<void> {
	await denyOffOrigin(page);
	await stubChainRegistry(page, {
		1: {
			chainId: 1,
			name: 'Ethereum',
			nativeCurrency: { name: 'Ether', symbol: 'ETH', decimals: 18 },
			stables: [],
			wrappedNativeToken: null,
			dex: null,
			rpc: [`${STUB}/1`]
		}
	});
	await stubJsonRpc(page, /stub-rpc\.test\/rpc\/(\d+)/, (method, params, url) => {
		const chainId = Number(/\/rpc\/(\d+)/.exec(url)?.[1]);
		if (method === 'eth_chainId') return '0x' + chainId.toString(16);
		if (method === 'eth_blockNumber') return '0x10';
		if (method === 'eth_getCode') return '0x6080';
		if (method === 'eth_gasPrice') return '0x3b9aca00';
		if (method === 'eth_maxPriorityFeePerGas') return '0x3b9aca00';
		if (method === 'eth_getBlockByNumber') return { baseFeePerGas: '0x3b9aca00' };
		if (method === 'eth_call') {
			const call = params[0] as { data?: string } | undefined;
			const n = call?.data ? aggregate3CallCount(call.data) : 0;
			const balance = chainId === 1 ? ETH * 3n : 0n;
			const data =
				'0x' + abiWord(balance) + abiWord(PRICE_8DP) + abiWord(0) + abiWord(0) + abiWord(0);
			if (n === 0) return '0x' + abiWord(0);
			return encodeAggregate3Result(Array.from({ length: n }, () => ({ success: true, data })));
		}
		if (method === 'eth_getLogs') return [];
		return undefined;
	});
	await stubRelay(page, RELAY, happyRelay('0x' + 'd4'.repeat(32), '0x' + 'e5'.repeat(32)));

	await page.goto('/en/parallel');
	// The button is in the server's HTML before the page hydrates; pressed then,
	// it does nothing and the wait below runs out the whole test. The fixture
	// list is filled on mount, after the core loads — a row in it means the
	// handlers are attached. Invisible on a quiet machine, a flake on a busy one.
	await page.locator('li code').first().waitFor({ timeout: 60_000 });
	await page.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
	await page.waitForURL(/\/en\/wallet$/);
	await seedNetworkOverrides(
		page,
		CHAINS.map((c) => ({ chainId: c.chainId, rpcURL: `${STUB}/${c.chainId}` }))
	);
	await page.reload();
	// "$9,000" or, under a decimal-comma preset, "$9.000".
	await expect(page.getByText(/^\$9[.,]000$/)).toBeVisible({ timeout: 25_000 });

	await page
		.getByRole('button', { name: en('componentsUi.dock.send') })
		.first()
		.click();
	await page.getByText('ETH', { exact: true }).first().click();
	await expect(page.getByRole('textbox', { name: en('send.recipientLabel') })).toBeVisible();
}

test('a pasted table becomes a split send — and an unpriceable currency refuses to convert', async ({
	page
}) => {
	// The fiat endpoint prices nothing: the importer must not fall back to 1:1.
	await page.route(/vela-currency\.getvela\.app/, (route) =>
		route.fulfill({ contentType: 'application/json', body: JSON.stringify([]) })
	);
	await openSendForm(page);

	// One recipient becomes many — the core's own transition — and the split
	// form is what offers the importer.
	await page
		.getByRole('button', { name: en('send.addRecipient') })
		.first()
		.click();
	await page
		.getByRole('button', { name: en('send.batchImport') })
		.first()
		.click();
	const paste = page
		.getByRole('textbox', { name: en('send.batchPastePlaceholder') })
		.or(page.locator('textarea'));
	await expect(paste.first()).toBeVisible({ timeout: 20_000 });

	await paste.first().fill(`${ALICE},5000\n${BOB},3000\n${CARO},2000`);

	// The rows are parsed — three of them — but no rate exists for the display
	// currency, so nothing is converted and the CTA stays shut.
	await expect(page.getByText('3', { exact: false }).first()).toBeVisible();
	const apply = page.getByRole('button', {
		name: new RegExp(en('send.batchApply_other').replace(/\{\{.*?\}\}/g, '.*'))
	});
	if (await apply.count()) await expect(apply.first()).toBeDisabled();
});

test('a list the account cannot pay says so beside the button it disarms (issue 204)', async ({
	page
}) => {
	await openSendForm(page);
	await page
		.getByRole('button', { name: en('send.addRecipient') })
		.first()
		.click();
	await page
		.getByRole('button', { name: en('send.batchImport') })
		.first()
		.click();
	const paste = page.locator('textarea').first();
	await expect(paste).toBeVisible({ timeout: 20_000 });

	// Token mode: the figures ARE the amounts, and the screen says so where the
	// rate would otherwise be — with no rate row pretending to convert them.
	await page
		.getByRole('tab', { name: en('send.batchUnitToken').replace('{{sym}}', 'ETH') })
		.click();
	await expect(
		page.getByText(en('send.batchTokenHint').replace('{{sym}}', 'ETH'), { exact: true })
	).toBeVisible();
	await expect(page.getByText(en('send.batchRateSection'), { exact: true })).toHaveCount(0);

	// 2 + 2 ETH against a balance of 3. The core refuses; the reporter saw a dark
	// button and nothing else.
	await paste.fill(`${ALICE},2\n${BOB},2`);
	const apply = page.getByRole('button', {
		name: new RegExp(en('send.batchApply_other').replace(/\{\{.*?\}\}/g, '.*'))
	});
	await expect(apply.first()).toBeDisabled();
	await expect(
		page.getByText(en('send.batchOverBalance').replace('{{sym}}', 'ETH'), { exact: true })
	).toBeVisible();
	await expect(page.getByText('4 ETH', { exact: true })).toBeVisible();

	// Within the balance the refusal goes and the button arms.
	await paste.fill(`${ALICE},1\n${BOB},1.5`);
	await expect(apply.first()).toBeEnabled();
	await expect(
		page.getByText(en('send.batchOverBalance').replace('{{sym}}', 'ETH'), { exact: true })
	).toHaveCount(0);
});

// The five things the split could not say or do, driven through the REAL core
// (no vitest instantiates the send or the importer machine): which line of a
// sheet is bad, which recipient is unfinished, importing ADDS to what was typed,
// Max is not offered where it fills nothing, and a decimal comma is read as one.

const row = (page: import('@playwright/test').Page, n: number) =>
	page.getByRole('textbox', {
		name: `${en('send.recipientN').replace('{{n}}', String(n))} · ${en('send.recipientLabel')}`
	});
const amount = (page: import('@playwright/test').Page, n: number) =>
	page.getByRole('textbox', {
		name: `${en('send.recipientN').replace('{{n}}', String(n))} · ETH`
	});
const needs = (key: 'send.splitNeedsAddress' | 'send.splitNeedsAmount', n: number) =>
	en(key).replace('{{n}}', String(n));

test('a dark Continue names the recipient it waits on, and a wrong field says so', async ({
	page
}) => {
	await openSendForm(page);
	await page
		.getByRole('button', { name: en('send.addRecipient') })
		.first()
		.click();
	const advance = page.getByRole('button', { name: en('send.continueBtn') }).first();

	// Max fills the single amount; a split has none, so it is not offered.
	await expect(page.getByRole('button', { name: en('send.maxBtn'), exact: true })).toHaveCount(0);

	// Nothing typed: the first recipient needs an address. Nothing is called wrong.
	await expect(page.getByText(needs('send.splitNeedsAddress', 1), { exact: true })).toBeVisible();
	await expect(page.getByText(en('send.batchBadAddress'), { exact: true })).toHaveCount(0);
	await expect(advance).toBeDisabled();

	await row(page, 1).fill(ALICE);
	await expect(page.getByText(needs('send.splitNeedsAmount', 1), { exact: true })).toBeVisible();
	await amount(page, 1).fill('0.5');
	await expect(page.getByText(needs('send.splitNeedsAddress', 2), { exact: true })).toBeVisible();
	// What is left to give out: 3 held, 0.5 given.
	await expect(
		page.getByText(en('send.splitRemaining').replace('{{amount}}', '2.5 ETH'), { exact: true })
	).toBeVisible();

	// Something IN a field that cannot be used is called wrong, on its row.
	await row(page, 2).fill('0x1234');
	await expect(page.getByText(en('send.batchBadAddress'), { exact: true })).toBeVisible();
	await row(page, 2).fill(BOB);
	await expect(page.getByText(en('send.batchBadAddress'), { exact: true })).toHaveCount(0);

	// "1,5" used to ARM the button (judged by its leading digit) and then do
	// nothing when pressed. Under this browser's dot preset the comma is not
	// guessed at: the core refuses the figure, the row says so, the gate stays shut.
	await amount(page, 2).fill('1,5');
	await expect(page.getByText(en('send.badAmount'), { exact: true })).toBeVisible();
	await expect(advance).toBeDisabled();

	// One amount for every empty row: clear it, and the offer appears.
	await amount(page, 2).fill('');
	const fillAll = page.getByRole('button', {
		name: en('send.splitFillEmpty').replace('{{amount}}', '0.5 ETH')
	});
	await expect(fillAll).toBeVisible();
	await fillAll.click();
	await expect(amount(page, 2)).toHaveValue('0.5');
	await expect(fillAll).toHaveCount(0);
	await expect(advance).toBeEnabled();
});

test('importing adds to the people already typed, and says which line of the sheet is bad', async ({
	page
}) => {
	await openSendForm(page);
	await page
		.getByRole('button', { name: en('send.addRecipient') })
		.first()
		.click();
	await row(page, 1).fill(ALICE);
	await amount(page, 1).fill('0.5');

	await page
		.getByRole('button', { name: en('send.batchImport') })
		.first()
		.click();
	const paste = page.locator('textarea').first();
	await expect(paste).toBeVisible({ timeout: 20_000 });
	await page
		.getByRole('tab', { name: en('send.batchUnitToken').replace('{{sym}}', 'ETH') })
		.click();
	await paste.fill(`Bob,${BOB},0.25\nMallory,0x12zz,1\nCarol,${CARO},`);

	// The two refused lines are on the list, as written, each with its reason.
	await expect(page.getByText(/Mallory/).first()).toBeVisible();
	await expect(page.getByText(en('send.batchBadAddress'), { exact: true })).toBeVisible();
	await expect(page.getByText(en('send.badAmount'), { exact: true })).toBeVisible();

	// Somebody is already on the form: the import says it ADDS to them, and the
	// other choice is one press away — and says what it would do before it does.
	await expect(page.getByText(en('send.batchAddsToRows'))).toBeVisible();
	await page.getByRole('button', { name: en('send.batchReplaceInstead') }).click();
	await expect(page.getByText(en('send.batchReplacesRows'))).toBeVisible();
	await page.getByRole('button', { name: en('send.batchAddInstead') }).click();
	await expect(page.getByText(en('send.batchAddsToRows'))).toBeVisible();

	await page
		.getByRole('button', { name: en('send.batchApply_one').replace('{{count}}', '1') })
		.click();

	// Alice, typed by hand, is still first; the blank row is gone; Bob follows.
	await expect(row(page, 1)).toHaveValue(ALICE);
	await expect(amount(page, 1)).toHaveValue('0.5');
	await expect(row(page, 2)).toHaveValue(BOB);
	await expect(amount(page, 2)).toHaveValue('0.25');
	await expect(row(page, 3)).toHaveCount(0);
	await expect(page.getByText('0.75 ETH', { exact: true })).toBeVisible();
});

test('a decimal comma is read as one where the person writes numbers that way', async ({
	browser
}) => {
	const context = await browser.newContext({ viewport: { width: 390, height: 844 } });
	const page = await context.newPage();
	await page.addInitScript(() =>
		localStorage.setItem('vela.localePrefs', JSON.stringify({ numberFormat: 'dot_comma' }))
	);
	await openSendForm(page);

	// The single amount: "0,5" is a half, and the field keeps showing it that way.
	const single = page.getByRole('textbox', { name: 'ETH', exact: true });
	await single.fill('0,5');
	await expect(single).toHaveValue('0,5');
	await expect(page.getByText(/≈\s*\$1\.500,00/)).toBeVisible();

	// And a split row: 0,5 + 0,25 is three quarters, not "cannot be summed".
	await page
		.getByRole('button', { name: en('send.addRecipient') })
		.first()
		.click();
	await row(page, 1).fill(ALICE);
	await row(page, 2).fill(BOB);
	await amount(page, 2).fill('0,25');
	await expect(page.getByText('0,75 ETH', { exact: true })).toBeVisible();
	await expect(page.getByText(en('send.badAmount'), { exact: true })).toHaveCount(0);
	await context.close();
});

test('the same payee twice is named on the row that repeats it (issue 203)', async ({ page }) => {
	await openSendForm(page);

	// One recipient becomes many, and both rows are typed by hand — the path
	// the importer's de-dupe never covered.
	await page
		.getByRole('button', { name: en('send.addRecipient') })
		.first()
		.click();
	const row = (n: number) =>
		page.getByRole('textbox', {
			name: `${en('send.recipientN').replace('{{n}}', String(n))} · ${en('send.recipientLabel')}`
		});
	const amount = (n: number) =>
		page.getByRole('textbox', {
			name: `${en('send.recipientN').replace('{{n}}', String(n))} · ETH`
		});
	await row(1).fill(ALICE);
	await amount(1).fill('0.1');
	await row(2).fill(BOB);
	await amount(2).fill('0.2');

	const warning = page.getByText(en('send.recipientDuplicate').replace('{{n}}', '1'), {
		exact: true
	});
	await expect(warning).toHaveCount(0);

	// Row 2 becomes row 1's address: the second row says so, the first does not,
	// and nothing is disabled — the batch is still exactly what was asked for.
	await row(2).fill(ALICE);
	await expect(warning).toBeVisible();
	await expect(warning).toHaveCount(1);
	// Warned, not refused: a repeated payee may be exactly what was meant, so
	// the batch the person asked for stays sendable.
	await expect(page.getByRole('button', { name: en('send.continueBtn') }).first()).toBeEnabled();

	// Correct it and the warning goes with it.
	await row(2).fill(BOB);
	await expect(warning).toHaveCount(0);
});

test('SheetJS is never on the startup path', async ({ page }) => {
	const scripts = collectScripts(page);
	await denyOffOrigin(page);
	await page.addInitScript(() => localStorage.setItem('vela.intro.seen', String(Date.now())));
	await page.goto('/en/wallet');
	await page.waitForLoadState('networkidle');

	// SheetJS announces itself in every build; a page that never opens a
	// spreadsheet must not carry ~1 MB of parser.
	expect(
		chunksCarrying(scripts, /sheetjs|XLSX\.utils|SheetJS/i),
		'the spreadsheet parser reached a startup chunk'
	).toEqual([]);
});
