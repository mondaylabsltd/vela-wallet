/**
 * Multisig accounts, end to end on a Gnosis fork: create one (nothing
 * deployed, the genesis recorded on the creator's passkey proof), fund it,
 * propose a transfer, collect the approvals, execute — the multisig and its
 * deployment in the executor's one user operation — and see the funds arrive.
 */
import { expect, test, type Page } from '@playwright/test';
import { en } from '../live-helpers';
import {
	EOA_ONE,
	EOA_TWO,
	FIXTURES,
	UNREAD_CHAIN,
	balanceOf,
	enterParallel,
	setBalance,
	undeployedSafe,
	useHarness,
	withInjectedWallet
} from './harness';

const SHOTS = 'e2e/onchain/.shots';
const RECIPIENT = '0x000000000000000000000000000000000000bEEF';

/**
 * The send every account has — token, recipient, amount, Continue — whose
 * confirm, for a multisig, proposes and approves instead of sending.
 */
async function proposeSend(
	page: Page,
	amount: string,
	name: string,
	coin = { row: /^xDAI Gnosis/, symbol: 'xDAI' }
): Promise<void> {
	await page
		.getByRole('button', { name: en('componentsUi.dock.send') })
		.first()
		.click();
	await expect(page.getByRole('heading', { name: en('send.selectTokenTitle') })).toBeVisible();
	// The picker's row, not the home's: on the desktop both are on screen, and
	// the picker (the third column) is the last.
	await page.getByRole('button', { name: coin.row }).last().click({ timeout: 60_000 });
	await page.getByRole('textbox', { name: en('send.recipientLabel') }).fill(RECIPIENT);
	await page.getByRole('textbox', { name: coin.symbol }).fill(amount);
	await expect(page.getByText('2 of', { exact: false }).first()).toBeVisible();
	await shot(page, `${name}-form`);
	const advance = page.getByRole('button', { name: en('send.continueBtn') });
	await expect(advance).toBeEnabled({ timeout: 30_000 });
	await advance.click();
	await expect(page.getByRole('heading', { name: en('send.confirmTitle') })).toBeVisible();
	await expect(page.getByText(`${amount} ${coin.symbol}`, { exact: true }).first()).toBeVisible();
	await shot(page, `${name}-confirm`);
	await page.getByRole('button', { name: 'Propose and approve' }).click();
}

async function shot(page: Page, name: string): Promise<void> {
	await page.screenshot({ path: `${SHOTS}/${name}.png`, fullPage: false });
}

function errors(page: Page): string[] {
	const seen: string[] = [];
	page.on('pageerror', (e) => seen.push(e.message));
	// The wallet's own log, for whoever reads a failed run.
	page.on('console', (m) => {
		if (!/worldchain|Chainlink|Failed to load resource|favicon/.test(m.text())) {
			console.log(`[page ${m.type()}] ${m.text().slice(0, 400)}`);
		}
	});
	page.on('response', async (r) => {
		if (!/127\.0\.0\.1:(8787|8790)/.test(r.url())) return;
		const body = await r.text().catch(() => '');
		console.log(
			`[net ${r.status()}] ${r.request().method()} ${r.url().replace(/^http:\/\/127\.0\.0\.1:/, '')} ${(r.request().postData() ?? '').slice(0, 160)} → ${body.slice(0, 300)}`
		);
	});
	return seen;
}

test('a 2-of-3 multisig on one device: create, fund, propose, approve and execute', async ({
	page,
	context
}) => {
	const thrown = errors(page);
	await useHarness(context);
	await enterParallel(page);
	await shot(page, '01-wallet');

	await test.step('create', async () => {
		await page
			.getByRole('button', { name: /Parallel One/ })
			.first()
			.click();
		await shot(page, '02-switcher');
		await page.getByRole('button', { name: 'Add account' }).click();
		await page.getByRole('button', { name: /^Create multisig/ }).click();
		await page.getByRole('textbox', { name: 'Name' }).fill('Team fund');
		await expect(page.getByText('New multisig')).toBeVisible();
		await page.getByText('Parallel Two', { exact: true }).click();
		await page.getByText('Parallel Three', { exact: true }).click();
		await shot(page, '03-members');
		await page.getByRole('button', { name: 'Next' }).click();
		await expect(page.getByText('2 of 3')).toBeVisible();
		await shot(page, '04-threshold');
		await page.getByRole('button', { name: 'Next' }).click();
		await shot(page, '05-review');
		await page.getByRole('button', { name: 'Create', exact: true }).click();
		await expect(page.getByText('Multisig created')).toBeVisible();
		await shot(page, '06-created');
		await page.getByRole('button', { name: 'Done' }).click();
	});

	const multisig = await page.evaluate(() => localStorage.getItem('vela.activeMultisig'));
	expect(multisig).toMatch(/^0x[0-9a-fA-F]{40}$/);
	await expect(page.getByRole('link', { name: '2 of 3 members approve' }).first()).toBeVisible();
	// The name is the multisig's own, from its genesis.
	await expect(page.getByText('Team fund', { exact: true }).first()).toBeVisible();
	await shot(page, '07-home');

	await test.step('fund and propose', async () => {
		await setBalance(multisig!, 5n * 10n ** 18n);
		await page.reload();
		await proposeSend(page, '1.5', '08-propose');
		await expect(page.getByText('1/2 approved').first()).toBeVisible();
		await shot(page, '09-proposal');
	});

	await test.step('approve and execute as the second member', async () => {
		const before = await balanceOf(RECIPIENT);
		await page.getByRole('button', { name: 'Approve and execute' }).click();
		await expect(page.getByText('Executed', { exact: true }).first()).toBeVisible({
			timeout: 120_000
		});
		await shot(page, '10-executed');
		const after = await balanceOf(RECIPIENT);
		expect(after - before).toBe(15n * 10n ** 17n);
	});

	expect(thrown).toEqual([]);
	void FIXTURES;
});

test('two devices: one proposes, the other opens the link, approves and executes', async ({
	browser
}) => {
	const a = await browser.newContext({ viewport: { width: 430, height: 900 } });
	const b = await browser.newContext({ viewport: { width: 430, height: 900 } });
	await useHarness(a);
	await useHarness(b);
	const pageA = await a.newPage();
	const pageB = await b.newPage();
	const thrownA = errors(pageA);
	const thrownB = errors(pageB);
	await enterParallel(pageA, [FIXTURES.one]);
	await enterParallel(pageB, [FIXTURES.two]);

	await test.step('device A creates a 2-of-2 with a member added by address', async () => {
		await pageA
			.getByRole('button', { name: /Parallel One/ })
			.first()
			.click();
		await pageA.getByRole('button', { name: 'Add account' }).click();
		await pageA.getByRole('button', { name: /^Create multisig/ }).click();
		await pageA.getByRole('textbox', { name: 'Name' }).fill('Shared 2/2');
		await pageA.getByPlaceholder("Member's address (0x…)").fill(FIXTURES.two);
		await pageA.getByRole('button', { name: 'Add', exact: true }).click();
		await expect(pageA.getByText('Members · 2')).toBeVisible();
		await pageA.getByRole('button', { name: 'Next' }).click();
		await expect(pageA.getByText('2 of 2')).toBeVisible();
		await expect(pageA.getByText(/Every member must approve/)).toBeVisible();
		await shot(pageA, 'x1-threshold-all');
		await pageA.getByRole('button', { name: 'Next' }).click();
		await pageA.getByRole('button', { name: 'Create', exact: true }).click();
		await pageA.getByRole('button', { name: 'Done' }).click();
		await expect(pageA.getByRole('link', { name: '2 of 2 members approve' }).first()).toBeVisible();
		await shot(pageA, 'x2-home-a');
	});

	const multisig = (await pageA.evaluate(() => localStorage.getItem('vela.activeMultisig')))!;
	await setBalance(multisig, 3n * 10n ** 18n);

	let link = '';
	await test.step('device A proposes and shares the link', async () => {
		await pageA.reload();
		await proposeSend(pageA, '0.25', 'x2-propose-a');
		await expect(pageA.getByText('1/2 approved').first()).toBeVisible();
		// Device A cannot finish it: the other member must approve.
		await expect(pageA.getByRole('button', { name: 'Approve and execute' })).toHaveCount(0);
		link = (await pageA.locator('code.link').textContent())!.trim();
		await shot(pageA, 'x3-proposal-a');
	});
	expect(link).toContain('#ms=100.');

	await test.step('device B opens the link, approves and executes', async () => {
		const before = await balanceOf(RECIPIENT);
		await pageB.goto(link);
		await expect(pageB.getByText('Send 0.25 xDAI').first()).toBeVisible({ timeout: 90_000 });
		await shot(pageB, 'x4-proposal-b');
		// The name came with the genesis the service holds, checked by the core.
		await expect(pageB.getByText('Shared 2/2', { exact: true }).first()).toBeVisible();
		await pageB.getByRole('button', { name: 'Approve and execute' }).click();
		await expect(pageB.getByText('Executed', { exact: true }).first()).toBeVisible({
			timeout: 120_000
		});
		await shot(pageB, 'x5-executed-b');
		expect((await balanceOf(RECIPIENT)) - before).toBe(25n * 10n ** 16n);
	});

	await test.step('device A sees it done', async () => {
		await pageA.reload();
		await expect(pageA.getByRole('link', { name: '2 of 2 members approve' }).first()).toBeVisible();
		// Nothing left waiting: the executed transaction is gone from the panel.
		await expect(pageA.getByText('Waiting for approval')).toHaveCount(0, { timeout: 30_000 });
		// Executed on device B, found on chain here, and in this device's activity.
		await expect(pageA.getByRole('main').getByText(/0\.25/).first()).toBeVisible({
			timeout: 60_000
		});
		await expect(pageA.getByText(/Not shared yet/)).toHaveCount(0);
		await shot(pageA, 'x6-home-a-after');
	});

	expect(thrownA).toEqual([]);
	expect(thrownB).toEqual([]);
	await a.close();
	await b.close();
});

test('the desktop: create, propose and the transaction dialog', async ({ browser }) => {
	const ctx = await browser.newContext({ viewport: { width: 1360, height: 900 } });
	await useHarness(ctx);
	const page = await ctx.newPage();
	const thrown = errors(page);
	await enterParallel(page);
	await page
		.getByRole('button', { name: /Parallel One/ })
		.first()
		.click();
	await shot(page, 'd1-switcher');
	await expect(page.getByRole('button', { name: 'Add account' })).toBeVisible();
	await shot(page, 'd1-switcher-open');
	await page.getByRole('button', { name: 'Add account' }).click();
	await shot(page, 'd1-add-account');
	await page.getByRole('button', { name: /^Create multisig/ }).click();
	await page.getByRole('textbox', { name: 'Name' }).fill('Desk fund');
	await page.getByText('Parallel Two', { exact: true }).click();
	await shot(page, 'd2-create');
	await page.getByRole('button', { name: 'Next' }).click();
	await page.getByRole('button', { name: 'Next' }).click();
	await page.getByRole('button', { name: 'Create', exact: true }).click();
	await page.getByRole('button', { name: 'Done' }).click();
	const multisig = (await page.evaluate(() => localStorage.getItem('vela.activeMultisig')))!;
	await setBalance(multisig, 2n * 10n ** 18n);
	await page.reload();
	await expect(page.getByRole('link', { name: '2 of 2 members approve' }).first()).toBeVisible();
	await shot(page, 'd3-home');
	await proposeSend(page, '0.5', 'd4-propose');
	await expect(page.getByText('1/2 approved').first()).toBeVisible();
	await shot(page, 'd5-proposal');
	await page.keyboard.press('Escape');

	// The waiting transaction is a row of Activity, as a pending transfer is.
	await expect(page.getByText('Waiting for approval').first()).toBeVisible();
	await shot(page, 'd6-activity');

	// Its settings are where a Vela account's are: Settings → Account.
	await page.getByRole('link', { name: '2 of 2 members approve' }).first().click();
	await expect(page.getByRole('heading', { name: 'Members' })).toBeVisible({ timeout: 30_000 });
	await expect(page.getByText('Save its setup to Ethereum')).toBeVisible();
	await shot(page, 'd7-settings');

	// Every page wears the same account: Contacts too, and its switcher.
	await page.getByRole('button', { name: 'Contacts' }).first().click();
	await expect(page.getByRole('heading', { name: 'Contacts' }).first()).toBeVisible();
	await expect(page.getByText('Desk fund').first()).toBeVisible();
	await expect(page.getByRole('link', { name: '2 of 2 members approve' }).first()).toBeVisible();
	await page
		.getByRole('button', { name: /Desk fund/ })
		.first()
		.click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByRole('button', { name: /Desk fund/ })).toBeVisible();
	await expect(dialog.getByRole('button', { name: 'Add account' })).toBeVisible();
	await shot(page, 'd8-contacts-switcher');
	expect(thrown).toEqual([]);
	await ctx.close();
});

test('a network the genesis service does not read: propose, approve and execute there', async ({
	browser
}) => {
	const ctx = await browser.newContext({ viewport: { width: 430, height: 900 } });
	await useHarness(ctx);
	const page = await ctx.newPage();
	const thrown = errors(page);
	await enterParallel(page);
	await page
		.getByRole('button', { name: /Parallel One/ })
		.first()
		.click();
	await page.getByRole('button', { name: 'Add account' }).click();
	await page.getByRole('button', { name: /^Create multisig/ }).click();
	await page.getByRole('textbox', { name: 'Name' }).fill('Base fund');
	await page.getByText('Parallel Two', { exact: true }).click();
	await page.getByRole('button', { name: 'Next' }).click();
	await page.getByRole('button', { name: 'Next' }).click();
	await page.getByRole('button', { name: 'Create', exact: true }).click();
	await page.getByRole('button', { name: 'Done' }).click();
	const multisig = (await page.evaluate(() => localStorage.getItem('vela.activeMultisig')))!;
	// Funds on "Base" only: the second fork, which the service has no RPC for.
	await setBalance(multisig, 2n * 10n ** 18n, UNREAD_CHAIN);
	await page.reload();
	await expect(page.getByRole('link', { name: '2 of 2 members approve' }).first()).toBeVisible();

	await proposeSend(page, '0.5', 'u1-propose', { row: /^ETH Base/, symbol: 'ETH' });
	await expect(page.getByText('1/2 approved').first()).toBeVisible();
	await shot(page, 'u2-proposal');

	const before = await balanceOf(RECIPIENT, UNREAD_CHAIN);
	await page.getByRole('button', { name: 'Approve and execute' }).click();
	await expect(page.getByText('Executed', { exact: true }).first()).toBeVisible({
		timeout: 120_000
	});
	await shot(page, 'u3-executed');
	expect((await balanceOf(RECIPIENT, UNREAD_CHAIN)) - before).toBe(5n * 10n ** 17n);
	expect(thrown).toEqual([]);
	await ctx.close();
});

test('an outside owner signs with their own wallet, through the connect page', async ({
	browser
}) => {
	const a = await browser.newContext({ viewport: { width: 430, height: 900 } });
	await useHarness(a);
	const page = await a.newPage();
	const thrown = errors(page);
	await enterParallel(page);

	await test.step('a 2-of-2 with a member whose wallet is MetaMask-like', async () => {
		await page
			.getByRole('button', { name: /Parallel One/ })
			.first()
			.click();
		await page.getByRole('button', { name: 'Add account' }).click();
		await page.getByRole('button', { name: /^Create multisig/ }).click();
		await page.getByRole('textbox', { name: 'Name' }).fill('With MetaMask');
		await page.getByPlaceholder("Member's address (0x…)").fill(EOA_ONE);
		await page.getByRole('button', { name: 'Add', exact: true }).click();
		await page.getByRole('button', { name: 'Next' }).click();
		await expect(page.getByText('2 of 2')).toBeVisible();
		await page.getByRole('button', { name: 'Next' }).click();
		await page.getByRole('button', { name: 'Create', exact: true }).click();
		await page.getByRole('button', { name: 'Done' }).click();
	});
	const multisig = (await page.evaluate(() => localStorage.getItem('vela.activeMultisig')))!;
	await setBalance(multisig, 2n * 10n ** 18n);
	await page.reload();
	await proposeSend(page, '0.5', 'e1-propose');
	await expect(page.getByText('1/2 approved').first()).toBeVisible();

	let link = '';
	await test.step('Vela shows the connect link for the outside owner', async () => {
		await page.getByRole('button', { name: 'Sign with another wallet' }).click();
		link = (await page.getByTestId('external-link').textContent())!.trim();
		expect(link).toMatch(/\/en\/connect#ws/);
		await expect(page.getByTestId('external-status')).toContainText('Waiting for the wallet page');
		await shot(page, 'e2-external-sheet');
	});

	const b = await browser.newContext({ viewport: { width: 430, height: 900 } });
	await withInjectedWallet(b, EOA_ONE);
	const walletPage = await b.newPage();
	const thrownB = errors(walletPage);
	await test.step('the owner opens it where their wallet is, connects, and it signs', async () => {
		await walletPage.goto(link);
		await expect(walletPage.getByText('Vela asks to use this wallet')).toBeVisible({
			timeout: 30_000
		});
		await shot(walletPage, 'e3-connect-ask');
		await walletPage.getByRole('button', { name: 'Connect', exact: true }).click();
		await expect(walletPage.getByTestId('connect-connected')).toBeVisible({ timeout: 30_000 });
		// Vela checks the signature and hands it to the mailbox: the sheet closes, both approved.
		await expect(page.getByText('2/2 approved').first()).toBeVisible({ timeout: 60_000 });
		await shot(walletPage, 'e4-connect-done');
		await shot(page, 'e5-approved');
	});

	await test.step('the Vela member executes with both signatures', async () => {
		const before = await balanceOf(RECIPIENT);
		await page.getByRole('button', { name: /^Execute|Approve and execute/ }).click();
		await expect(page.getByText('Executed', { exact: true }).first()).toBeVisible({
			timeout: 120_000
		});
		expect((await balanceOf(RECIPIENT)) - before).toBe(5n * 10n ** 17n);
		await shot(page, 'e6-executed');
	});

	expect(thrown).toEqual([]);
	expect(thrownB).toEqual([]);
	await a.close();
	await b.close();
});

/** A public Safe v1.3.0 on Gnosis (the genesis registry's fixtures): its owners changed since. */
const PUBLIC_SAFE = '0xA42f6c68d9e0E13ac0910837356c0A0155413E8C';

/**
 * Sign through the connect page in a fresh browser holding `owner`'s wallet,
 * from the link Vela's sheet shows.
 */
async function signElsewhere(
	browser: import('@playwright/test').Browser,
	vela: Page,
	owner: string,
	name: string
): Promise<string[]> {
	const link = (await vela.getByTestId('external-link').textContent())!.trim();
	const ctx = await browser.newContext({ viewport: { width: 430, height: 900 } });
	await withInjectedWallet(ctx, owner);
	const page = await ctx.newPage();
	const thrown = errors(page);
	await page.goto(link);
	await page.getByRole('button', { name: 'Connect', exact: true }).click({ timeout: 30_000 });
	await expect(page.getByTestId('connect-connected')).toBeVisible({ timeout: 30_000 });
	await expect(vela.getByTestId('external-link')).toHaveCount(0, { timeout: 60_000 });
	await shot(page, name);
	await ctx.close();
	return thrown;
}

test('import from Safe{Wallet}: Safes owned by outside wallets, proposed, signed and executed', async ({
	browser
}) => {
	const ctx = await browser.newContext({ viewport: { width: 430, height: 900 } });
	await useHarness(ctx);
	const page = await ctx.newPage();
	const thrown = errors(page);
	await enterParallel(page);

	// An export as Safe{Wallet} writes it: a deployed Safe, an undeployed one, the address book.
	const vault = await undeployedSafe([EOA_ONE, EOA_TWO], 2, String(Date.now()));
	const file = test.info().outputPath('safe-export.json');
	const { writeFileSync } = await import('node:fs');
	writeFileSync(
		file,
		JSON.stringify({
			version: '3.0',
			data: {
				addedSafes: {
					'100': {
						[PUBLIC_SAFE]: {
							owners: [{ value: '0xe8460717Cf006b9482BEabb31d2f23B710caB2C0' }],
							threshold: 1
						}
					}
				},
				undeployedSafes: {
					'100': {
						[vault.address]: {
							props: vault.props,
							status: { status: 'AWAITING_EXECUTION', type: 'PayLater' }
						}
					}
				},
				addressBook: { '100': { [PUBLIC_SAFE]: 'Public treasury', [vault.address]: 'EOA vault' } },
				settings: {}
			}
		})
	);

	await test.step('import both', async () => {
		await page
			.getByRole('button', { name: /Parallel One/ })
			.first()
			.click();
		await page.getByRole('button', { name: 'Add account' }).click();
		await page.getByRole('button', { name: /^Add multisig/ }).click();
		await page.locator('input[type=file]').setInputFiles(file);
		await expect(page.getByText('Choose the Safes to import')).toBeVisible();
		await expect(page.getByText('Public treasury')).toBeVisible();
		await expect(page.getByText('EOA vault')).toBeVisible();
		await shot(page, 'i1-pick');
		await page.getByRole('button', { name: 'Import 2' }).click();
		await expect(page.getByText('Imported', { exact: true })).toHaveCount(2, { timeout: 90_000 });
		await shot(page, 'i2-imported');
		await page.getByRole('button', { name: 'Done' }).click();
	});

	await test.step('the undeployed one: fund it and send, no owner on this device', async () => {
		await page
			.getByRole('button', { name: /Parallel One|EOA vault|Public treasury/ })
			.first()
			.click();
		await page
			.getByRole('button', { name: /EOA vault/ })
			.first()
			.click();
		await expect(page.getByText('EOA vault', { exact: true }).first()).toBeVisible();
		await setBalance(vault.address, 2n * 10n ** 18n);
		await page.reload();
		await proposeSend(page, '0.5', 'i3-propose');
		// No owner here: the proposer's approval comes from an outside wallet.
		await expect(page.getByTestId('external-link')).toBeVisible({ timeout: 30_000 });
		// On top of the send flow, all of it on screen.
		await expect(page.getByTestId('external-link')).toBeInViewport();
		await expect(page.getByRole('button', { name: 'Copy link' })).toBeInViewport();
		await shot(page, 'i4-external-first');
	});

	const thrownOne = await signElsewhere(browser, page, EOA_ONE, 'i5-signed-one');
	await expect(page.getByText('1/2 approved').first()).toBeVisible({ timeout: 60_000 });

	await test.step('the second owner signs from another wallet', async () => {
		await page.getByRole('button', { name: 'Sign with another wallet' }).first().click();
		await expect(page.getByTestId('external-link')).toBeVisible();
	});
	const thrownTwo = await signElsewhere(browser, page, EOA_TWO, 'i6-signed-two');
	await expect(page.getByText('2/2 approved').first()).toBeVisible({ timeout: 60_000 });

	await test.step('a Vela account that is no owner executes, deploying the Safe', async () => {
		const before = await balanceOf(RECIPIENT);
		await page.getByRole('button', { name: /^Execute/ }).click();
		await expect(page.getByText('Executed', { exact: true }).first()).toBeVisible({
			timeout: 120_000
		});
		expect((await balanceOf(RECIPIENT)) - before).toBe(5n * 10n ** 17n);
		await shot(page, 'i7-executed');
	});

	expect(thrown).toEqual([]);
	expect(thrownOne).toEqual([]);
	expect(thrownTwo).toEqual([]);
	await ctx.close();
});
