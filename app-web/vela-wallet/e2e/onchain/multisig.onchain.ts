/**
 * Multisig accounts, end to end on a Gnosis fork: create one (nothing
 * deployed, the genesis recorded on the creator's passkey proof), fund it,
 * propose a transfer, collect the approvals, execute — the multisig and its
 * deployment in the executor's one user operation — and see the funds arrive.
 * Against the Safe Genesis Registry v4 (every record named; a Safe renames
 * itself) and the service built from the registry repository's tree.
 */
import { expect, test, type Page } from '@playwright/test';
import { canonicalSafeContracts } from '../core-facts';
import { en } from '../live-helpers';
import {
	EOA_ONE,
	EOA_TWO,
	FIXTURES,
	GENESIS,
	RELAY,
	UNREAD_CHAIN,
	balanceOf,
	calldata,
	enterParallel,
	hookedSafe,
	keccak,
	landedRecord,
	postApproval,
	recordOnChain,
	rpc,
	safeTx,
	servedGenesis,
	setBalance,
	signSafeTx,
	undeployedSafe,
	useHarness,
	withInjectedWallet,
	type WireSafeTx
} from './harness';

const SHOTS = 'e2e/onchain/.shots';
const RECIPIENT = '0x000000000000000000000000000000000000bEEF';
/** The r4-e attacker's module address. */
const ATTACKER_MODULE = '0x9813461E88aDD60D0eC0943B74A83368CA2E36D1';

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
		.getByRole('button', { name: en('componentsUi.dock.send'), exact: true })
		.first()
		.click();
	await expect(page.getByRole('heading', { name: en('send.selectTokenTitle') })).toBeVisible();
	// The picker's row, not the home's: on the desktop both are on screen, and
	// the picker (the third column) is the last — once it has drawn its rows,
	// which follow the balances; before that, the last one is the home's.
	const rows = page.getByRole('button', { name: coin.row });
	if ((page.viewportSize()?.width ?? 0) >= 1280)
		await expect(rows).toHaveCount(2, { timeout: 60_000 });
	await rows.last().click({ timeout: 60_000 });
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
	// r4C-10: with no member on this device, nobody here approves: "Propose".
	await page.getByRole('button', { name: /^Propose( and approve)?$/ }).click();
	await confirmReview(page);
}

/**
 * §3.6: the proposer's own review before its passkey — the one review every
 * member gets (what moves, who it reaches, the state-diff guard, the hashes) —
 * confirmed with its "Propose and approve". `count`: one per network.
 */
async function confirmReview(page: Page, count = 1): Promise<void> {
	for (let i = 0; i < count; i++) {
		const hashes = page.getByTestId('review-hashes');
		await expect(hashes).toBeVisible({ timeout: 60_000 });
		await hashes
			.locator('xpath=ancestor::*[@role="dialog"][1]')
			.getByRole('button', { name: /^Propose( and approve)?$/ })
			.click();
		if (i + 1 < count) await expect(hashes).toBeHidden({ timeout: 60_000 });
	}
}

async function shot(page: Page, name: string): Promise<void> {
	await page.screenshot({ path: `${SHOTS}/${name}.png`, fullPage: false });
}

function errors(page: Page): string[] {
	const seen: string[] = [];
	page.on('pageerror', (e) => {
		seen.push(e.message);
		console.log(`[pageerror] ${e.message.slice(0, 400)}`);
	});
	// The wallet's own log, for whoever reads a failed run.
	page.on('console', (m) => {
		if (!/worldchain|Chainlink|Failed to load resource|favicon/.test(m.text())) {
			console.log(`[page ${m.type()}] ${m.text().slice(0, 400)}`);
		}
	});
	page.on('response', async (r) => {
		if (!r.url().startsWith(GENESIS) && !r.url().startsWith(RELAY)) return;
		const body = await r.text().catch(() => '');
		console.log(
			`[net ${r.status()}] ${r.request().method()} ${r.url().replace(/^http:\/\/127\.0\.0\.1:/, '')} ${(r.request().postData() ?? '').slice(0, 160)} → ${body.slice(0, 300)}`
		);
	});
	return seen;
}

/**
 * r4-create: "First network" is a list, and nothing is picked when nothing
 * says which (no network used here yet, funds on several) — Gnosis, the
 * suite's index chain, as the sheet once defaulted to.
 */
async function pickFirstNetwork(page: Page, name = 'Gnosis'): Promise<void> {
	const group = page.getByRole('radiogroup', { name: 'First network' });
	const radio = group.getByRole('radio', { name, exact: true });
	if (!(await radio.isVisible()))
		await group.getByRole('button', { name: /^More networks/ }).click();
	await radio.click();
	await expect(radio).toBeChecked();
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
		await page.getByRole('textbox', { name: /^Name/ }).fill('Team fund');
		await expect(page.getByText('New multisig')).toBeVisible();
		await page.getByText('Parallel Two', { exact: true }).click();
		await page.getByText('Parallel Three', { exact: true }).click();
		await shot(page, '03-members');
		await page.getByRole('button', { name: 'Next' }).click();
		await expect(page.getByText('2 of 3')).toBeVisible();
		await shot(page, '04-threshold');
		await page.getByRole('button', { name: 'Next' }).click();
		await shot(page, '05-review');
		await pickFirstNetwork(page);
		await page.getByRole('button', { name: 'Create', exact: true }).click();
		await expect(page.getByText('Multisig created')).toBeVisible();
		await shot(page, '06-created');
		await page.getByRole('button', { name: 'Done' }).click();
	});

	const multisig = await page.evaluate(() => localStorage.getItem('vela.activeMultisig'));
	expect(multisig).toMatch(/^0x[0-9a-fA-F]{40}$/);
	await expect(page.getByRole('link', { name: 'Needs 2 of 3 members' }).first()).toBeVisible();
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
		await pageA.getByRole('textbox', { name: /^Name/ }).fill('Shared 2/2');
		await pageA.getByPlaceholder("Member's address (0x…)").fill(FIXTURES.two);
		await pageA.getByRole('button', { name: 'Add', exact: true }).click();
		await expect(pageA.getByText('Members · 2')).toBeVisible();
		await pageA.getByRole('button', { name: 'Next' }).click();
		await expect(pageA.getByText('2 of 2')).toBeVisible();
		await expect(pageA.getByText(/Every member must approve/)).toBeVisible();
		await shot(pageA, 'x1-threshold-all');
		await pageA.getByRole('button', { name: 'Next' }).click();
		await pickFirstNetwork(pageA);
		await pageA.getByRole('button', { name: 'Create', exact: true }).click();
		await pageA.getByRole('button', { name: 'Done' }).click();
		await expect(pageA.getByRole('link', { name: 'Needs 2 of 2 members' }).first()).toBeVisible();
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
		await expect(pageA.getByRole('link', { name: 'Needs 2 of 2 members' }).first()).toBeVisible();
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
	await page.getByRole('textbox', { name: /^Name/ }).fill('Desk fund');
	await page.getByText('Parallel Two', { exact: true }).click();
	await shot(page, 'd2-create');
	await page.getByRole('button', { name: 'Next' }).click();
	await page.getByRole('button', { name: 'Next' }).click();
	await pickFirstNetwork(page);
	await page.getByRole('button', { name: 'Create', exact: true }).click();
	await page.getByRole('button', { name: 'Done' }).click();
	const multisig = (await page.evaluate(() => localStorage.getItem('vela.activeMultisig')))!;
	await setBalance(multisig, 2n * 10n ** 18n);
	await page.reload();
	await expect(page.getByRole('link', { name: 'Needs 2 of 2 members' }).first()).toBeVisible();
	await shot(page, 'd3-home');
	await proposeSend(page, '0.5', 'd4-propose');
	await expect(page.getByText('1/2 approved').first()).toBeVisible();
	await shot(page, 'd5-proposal');
	await page.keyboard.press('Escape');

	// The waiting transaction is a row of Activity, as a pending transfer is.
	await expect(page.getByText('Waiting for approval').first()).toBeVisible();
	await shot(page, 'd6-activity');

	// Its settings are where a Vela account's are: Settings → Account.
	await page.getByRole('link', { name: 'Needs 2 of 2 members' }).first().click();
	await expect(page.getByRole('heading', { name: 'Members', exact: true })).toBeVisible({
		timeout: 30_000
	});
	await expect(page.getByText('Back up its setup on Ethereum')).toBeVisible();
	await shot(page, 'd7-settings');

	// Every page wears the same account: Contacts too, and its switcher.
	await page.getByRole('button', { name: 'Contacts' }).first().click();
	await expect(page.getByRole('heading', { name: 'Contacts' }).first()).toBeVisible();
	await expect(page.getByText('Desk fund').first()).toBeVisible();
	await expect(page.getByRole('link', { name: 'Needs 2 of 2 members' }).first()).toBeVisible();
	await page
		.getByRole('button', { name: /^Desk fund/ })
		.first()
		.click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByRole('button', { name: /^Desk fund/ })).toBeVisible();
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
	await page.getByRole('textbox', { name: /^Name/ }).fill('Base fund');
	await page.getByText('Parallel Two', { exact: true }).click();
	await page.getByRole('button', { name: 'Next' }).click();
	await page.getByRole('button', { name: 'Next' }).click();
	await pickFirstNetwork(page);
	await page.getByRole('button', { name: 'Create', exact: true }).click();
	await page.getByRole('button', { name: 'Done' }).click();
	const multisig = (await page.evaluate(() => localStorage.getItem('vela.activeMultisig')))!;
	// Funds on "Base" only: the second fork, which the service has no RPC for.
	await setBalance(multisig, 2n * 10n ** 18n, UNREAD_CHAIN);
	await page.reload();
	await expect(page.getByRole('link', { name: 'Needs 2 of 2 members' }).first()).toBeVisible();

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
		await page.getByRole('textbox', { name: /^Name/ }).fill('With MetaMask');
		await page.getByPlaceholder("Member's address (0x…)").fill(EOA_ONE);
		await page.getByRole('button', { name: 'Add', exact: true }).click();
		await page.getByRole('button', { name: 'Next' }).click();
		await expect(page.getByText('2 of 2')).toBeVisible();
		await page.getByRole('button', { name: 'Next' }).click();
		await pickFirstNetwork(page);
		await page.getByRole('button', { name: 'Create', exact: true }).click();
		await page.getByRole('button', { name: 'Done' }).click();
	});
	const multisig = (await page.evaluate(() => localStorage.getItem('vela.activeMultisig')))!;
	await setBalance(multisig, 2n * 10n ** 18n);
	await page.reload();
	await proposeSend(page, '0.5', 'e1-propose');
	await expect(page.getByText('1/2 approved').first()).toBeVisible();

	// r7: a typed plain key (no code, no index record) is only hinted "Other
	// wallet"; not picked, nothing is kept and it is `unsure` (r6a-g2) — the
	// Vela link first, "Approve with another wallet" beside it, and the
	// outside member's own link (r4C-13). The outside owner signs from that
	// link, on the connect page alone.
	const thrownB = await signFromOutsideLink(browser, page, EOA_ONE, 'e3-outside-link');
	await expect(page.getByText('2/2 approved').first()).toBeVisible({ timeout: 90_000 });
	await shot(page, 'e5-approved');

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
});

/**
 * Sign from the outside member's own link (r4C-13: `outside-link` on Vela's
 * sheet — the connect page reads the transaction from the mailbox, its
 * wallet signs, the page hands the signature to the mailbox) in a fresh
 * browser holding `owner`'s wallet.
 */
async function signFromOutsideLink(
	browser: import('@playwright/test').Browser,
	vela: Page,
	owner: string,
	name: string
): Promise<string[]> {
	const outsideLink = vela.getByTestId('outside-link');
	await expect(outsideLink).toBeVisible({ timeout: 30_000 });
	const link = (await outsideLink.textContent())!.trim();
	expect(link).toMatch(/\/en\/connect#for=.*&ms=100\./);
	// Nothing in the query, which a server would log.
	expect(new URL(link).search).toBe('');
	const ctx = await browser.newContext({ viewport: { width: 430, height: 900 } });
	// The page reads the mailbox itself: the harness's service, not the public one.
	await useHarness(ctx);
	await withInjectedWallet(ctx, owner);
	const page = await ctx.newPage();
	const thrown = errors(page);
	await page.goto(link);
	// r4C-7: reviewed on the page first; the wallet is asked on the tap.
	await page.getByRole('button', { name: /^Sign with / }).click({ timeout: 60_000 });
	await expect(page.getByTestId('outside-approved')).toBeVisible({ timeout: 60_000 });
	await shot(page, name);
	await ctx.close();
	return thrown;
}

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
	// r4C-7: reviewed on the page first; the wallet is asked on the tap.
	await page.getByRole('button', { name: /^Sign with / }).click({ timeout: 30_000 });
	await expect(page.getByTestId('connect-signed')).toBeVisible({ timeout: 30_000 });
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
		// r7: each member's kind is asked first — plain keys: "Other wallet"
		// hinted once the index answers, else asked. Left as it stands: unsure.
		await page.getByRole('button', { name: 'Next' }).click();
		await expect(page.getByText('How does each member approve?')).toBeVisible();
		await expect(page.getByTestId('member-kind')).toHaveCount(3);
		await shot(page, 'i1-kinds');
		await page.getByRole('button', { name: 'Import 2' }).click();
		await expect(page.getByText('Imported', { exact: true })).toHaveCount(2, { timeout: 90_000 });
		await shot(page, 'i2-imported');
		await page.getByRole('button', { name: 'Done' }).click();
	});

	await test.step('the undeployed one: fund it and send, no owner on this device', async () => {
		await page
			.getByRole('button', { name: /^(Parallel One|EOA vault|Public treasury)/ })
			.first()
			.click();
		await page
			.getByRole('button', { name: /^EOA vault/ })
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

	// The second owner: nothing kept on import (r7: the "Other wallet" hint
	// was not picked), so `unsure` (r6a-g2) — its row offers the Vela link and
	// "Approve with another wallet", and the outside member's own link; it
	// signs from that link.
	const thrownTwo = await signFromOutsideLink(browser, page, EOA_TWO, 'i6-signed-two');
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

/**
 * Registry v4: the multisig renames its record itself — a transaction of its
 * own, proposed from its settings, approved and executed like any other. A
 * rename never deploys it (owner, 2026-10-29): until it is deployed on
 * Gnosis, where its record is, Settings says only the multisig itself can
 * rename it there and offers nothing; once a transfer there deployed it, the
 * rename is offered. The device's label is untouched; the name shown is the
 * record's, read back from the chain.
 */
test('rename on chain: a proposal of its own, approved and executed; the registry holds the new name', async ({
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
	await page.getByRole('textbox', { name: /^Name/ }).fill('Old name');
	// Two and Three, Two first: Two's passkey asks for the record, so the
	// sponsor's per-owner budget (5 a day, spent by the tests above on One)
	// still pays and the record lands on chain, where a rename needs it.
	await page.locator('ul.choices').getByText('Parallel One', { exact: true }).click();
	await page.getByText('Parallel Two', { exact: true }).click();
	await page.getByText('Parallel Three', { exact: true }).click();
	await page.getByRole('button', { name: 'Next' }).click();
	await page.getByRole('button', { name: 'Next' }).click();
	await pickFirstNetwork(page);
	await page.getByRole('button', { name: 'Create', exact: true }).click();
	await page.getByRole('button', { name: 'Done' }).click();
	const multisig = (await page.evaluate(() => localStorage.getItem('vela.activeMultisig')))!;
	// The service wrote it on its home chain, with the name it was created with.
	expect((await landedRecord(multisig)).genesis.name).toBe('Old name');
	expect(await recordOnChain(multisig)).toEqual({ name: 'Old name', renamedAt: 0 });

	await test.step('not deployed on Gnosis: nothing that deploys is offered', async () => {
		await page.getByRole('link', { name: 'Needs 2 of 2 members' }).first().click();
		await expect(page.getByRole('heading', { name: 'Members', exact: true })).toBeVisible({
			timeout: 30_000
		});
		await expect(page.getByTestId('rename-not-deployed')).toBeVisible({ timeout: 30_000 });
		await expect(page.getByRole('button', { name: 'Change the shared name' })).toHaveCount(0);
		await shot(page, 'r0-not-deployed');
	});

	await test.step('a transfer on Gnosis deploys it there', async () => {
		await setBalance(multisig, 2n * 10n ** 18n);
		await page.goto('/en/wallet');
		await proposeSend(page, '0.1', 'r0-transfer');
		await page.getByRole('button', { name: 'Approve and execute' }).click();
		await expect(page.getByText('Executed', { exact: true }).first()).toBeVisible({
			timeout: 120_000
		});
		expect(await rpc<string>('eth_getCode', [multisig, 'latest'])).not.toBe('0x');
		// Closed before Settings: the sheet is the wallet's, over any page.
		await expect(async () => {
			await page.keyboard.press('Escape');
			await expect(page.getByRole('heading', { name: 'Multisig transaction' })).toHaveCount(0, {
				timeout: 1_000
			});
		}).toPass({ timeout: 15_000 });
	});

	await test.step('propose the rename from its settings', async () => {
		await page.getByRole('link', { name: 'Needs 2 of 2 members' }).first().click();
		await expect(page.getByRole('heading', { name: 'Members', exact: true })).toBeVisible({
			timeout: 30_000
		});
		await expect(page.getByTestId('registry-name')).toHaveText('Old name');
		await page.getByRole('button', { name: 'Change the shared name' }).click();
		await page.getByRole('textbox', { name: /^Name/ }).fill('Team treasury');
		await shot(page, 'r1-rename');
		await page.getByRole('button', { name: 'Propose and approve' }).click();
		await confirmReview(page);
		// The members read it as what it is.
		await expect(page.getByText('Change the shared name to “Team treasury”').first()).toBeVisible({
			timeout: 60_000
		});
		await expect(page.getByText('1/2 approved').first()).toBeVisible();
		await shot(page, 'r2-proposal');
	});

	await test.step('the second member approves and executes', async () => {
		await page.getByRole('button', { name: 'Approve and execute' }).click();
		await expect(page.getByText('Executed', { exact: true }).first()).toBeVisible({
			timeout: 120_000
		});
		await shot(page, 'r3-executed');
	});

	const record = await recordOnChain(multisig);
	expect(record?.name).toBe('Team treasury');
	expect(record?.renamedAt).toBeGreaterThan(0);
	// The wallet shows the record's new name, read from the chain.
	await page.keyboard.press('Escape');
	await expect(page.getByTestId('registry-name')).toHaveText('Team treasury', { timeout: 30_000 });
	await page.reload();
	await expect(page.getByText('Team treasury', { exact: true }).first()).toBeVisible({
		timeout: 30_000
	});
	await shot(page, 'r4-renamed');
	expect(thrown).toEqual([]);
	await ctx.close();
});

/**
 * A multisig recorded in an earlier registry (v1–v3, which the v4 service no
 * longer reads) — here, one this device holds as shared whose record no v4
 * registry has: said as not shared, and shared again on a member's passkey.
 */
test('a multisig the service has no record of: said as not shared, and shared again', async ({
	browser
}) => {
	const ctx = await browser.newContext({ viewport: { width: 430, height: 900 } });
	await useHarness(ctx);
	const page = await ctx.newPage();
	const thrown = errors(page);
	await enterParallel(page);
	// Three first: its passkey shares it again (a budget the tests above leave alone).
	const old = await undeployedSafe([FIXTURES.three, FIXTURES.two], 2, String(Date.now()));
	expect(await recordOnChain(old.address)).toBeNull();
	await page.evaluate(
		({ address, genesis, owners }) => {
			localStorage.setItem(
				'vela.multisigAccounts',
				JSON.stringify([
					{
						address,
						genesis,
						owners,
						threshold: 2,
						// What a device that created it under registry v2 holds.
						recorded: true,
						shared: true,
						chainIds: [100],
						addedMs: Date.now() - 86_400_000
					}
				])
			);
			localStorage.setItem('vela.activeMultisig', address);
		},
		{ address: old.address, genesis: old.genesis, owners: [FIXTURES.three, FIXTURES.two] }
	);
	await page.goto('/en/settings');
	await expect(page.getByRole('heading', { name: 'Members', exact: true })).toBeVisible({
		timeout: 30_000
	});
	await expect(page.getByText(/^Not shared yet/).first()).toBeVisible({ timeout: 30_000 });
	await shot(page, 'o1-not-shared');
	await page.getByRole('button', { name: 'Share it again' }).click();
	await expect(page.getByText(/^Not shared yet/)).toHaveCount(0, { timeout: 60_000 });
	await shot(page, 'o2-shared-again');
	// Recorded on the fork's v4 registry, under the name the service would
	// have given it (it came without one).
	const landed = await landedRecord(old.address);
	expect(landed.genesis.name).toMatch(/^[A-Z][a-z]+ [A-Z][a-z]+$/);
	expect((await recordOnChain(old.address))?.name).toBe(landed.genesis.name);
	expect(thrown).toEqual([]);
	await ctx.close();
});

/**
 * Owners are each network's (owner, 2026-10-29). A 1-of-2 — this device's
 * account and an outside wallet — adds a third owner on Base (a network the
 * service does not read, where it can only know the genesis) by a proposal
 * the outside owner signs and this device executes. On the next proposal
 * there, the new owner's approval is taken by the mailbox (flagged as not a
 * genesis owner, never refused) and counted by the wallet, which reads
 * Base's members itself.
 */
test('an owner added on one network: its approval is taken by the mailbox and counted there', async ({
	browser
}) => {
	const ctx = await browser.newContext({ viewport: { width: 430, height: 900 } });
	await useHarness(ctx);
	const page = await ctx.newPage();
	const thrown = errors(page);
	await enterParallel(page);

	await test.step('create a 1-of-2 with an outside owner', async () => {
		await page
			.getByRole('button', { name: /Parallel One/ })
			.first()
			.click();
		await page.getByRole('button', { name: 'Add account' }).click();
		await page.getByRole('button', { name: /^Create multisig/ }).click();
		await page.getByRole('textbox', { name: /^Name/ }).fill('Grows on Base');
		await page.getByPlaceholder("Member's address (0x…)").fill(EOA_ONE);
		await page.getByRole('button', { name: 'Add', exact: true }).click();
		await expect(page.getByText('Members · 2')).toBeVisible();
		await page.getByRole('button', { name: 'Next' }).click();
		await page.getByRole('button', { name: 'Fewer approvals' }).click();
		await expect(page.getByText('1 of 2')).toBeVisible();
		await page.getByRole('button', { name: 'Next' }).click();
		await pickFirstNetwork(page);
		await page.getByRole('button', { name: 'Create', exact: true }).click();
		await page.getByRole('button', { name: 'Done' }).click();
	});
	const multisig = (await page.evaluate(() => localStorage.getItem('vela.activeMultisig')))!;
	const genesis = await servedGenesis(multisig);
	await setBalance(multisig, 2n * 10n ** 18n, UNREAD_CHAIN);

	let added = '';
	await test.step('the outside owner proposes adding a member on Base; this device executes it', async () => {
		const tx = safeTx(
			multisig,
			await calldata('addOwnerWithThreshold(address,uint256)', EOA_TWO, '2'),
			0
		);
		const posted = await postApproval(
			UNREAD_CHAIN,
			multisig,
			tx,
			await signSafeTx(EOA_ONE, UNREAD_CHAIN, multisig, tx),
			genesis
		);
		expect(posted.status).toBe(201);
		expect(posted.body.proposal?.approvals[0]?.signerIsGenesisOwner).toBe(true);
		added = posted.body.proposal!.safeTxHash;
		await page.goto(`/en/wallet#ms=${UNREAD_CHAIN}.${multisig}.${added}`);
		// r4a-1: a member change reads in full — whom it adds, by the whole address.
		await expect(page.getByTestId('member-change')).toContainText(EOA_TWO, { timeout: 90_000 });
		await expect(page.getByText('1/1 approved').first()).toBeVisible();
		await shot(page, 'w1-add-owner');
		await page.getByRole('button', { name: 'Execute', exact: true }).click();
		await expect(page.getByText('Executed', { exact: true }).first()).toBeVisible({
			timeout: 120_000
		});
		await shot(page, 'w2-added');
		const owners = await calldata('getOwners()');
		const read = await rpc<string>(
			'eth_call',
			[{ to: multisig, data: owners }, 'latest'],
			UNREAD_CHAIN
		);
		expect(read.toLowerCase()).toContain(EOA_TWO.slice(2).toLowerCase());
		await page.keyboard.press('Escape');
	});

	let next = '';
	await test.step('the next proposal on Base waits for 2 of 3 there', async () => {
		await page.reload();
		await proposeSend(page, '0.5', 'w3-propose', { row: /^ETH Base/, symbol: 'ETH' });
		await expect(page.getByText('1/2 approved').first()).toBeVisible();
		// r4C-13: every member left to approve is an outside wallet, so the link
		// shown is the connect page's, naming the transaction (`ms=` in it).
		const link = (await page.locator('code.link').first().textContent())!.trim();
		const named = link.match(/[#&]ms=(\d+)\.0x[0-9a-fA-F]{40}\.(0x[0-9a-fA-F]{64})/);
		expect(named?.[1]).toBe(String(UNREAD_CHAIN));
		next = named![2];
	});

	await test.step("the new owner's approval: taken by the mailbox, counted by the wallet", async () => {
		const listed = (await (
			await fetch(`${GENESIS}/api/proposals/${multisig}?chainId=${UNREAD_CHAIN}`)
		).json()) as { proposals: { safeTxHash: string; safeTx: WireSafeTx }[] };
		const tx = listed.proposals.find(
			(p) => p.safeTxHash.toLowerCase() === next.toLowerCase()
		)!.safeTx;
		const posted = await postApproval(
			UNREAD_CHAIN,
			multisig,
			tx,
			await signSafeTx(EOA_TWO, UNREAD_CHAIN, multisig, tx)
		);
		expect(posted.status).toBe(200);
		expect(posted.body.status).toBe('approved');
		const mine = posted.body.proposal!.approvals.find(
			(a) => a.owner.toLowerCase() === EOA_TWO.toLowerCase()
		);
		// Not a genesis owner: flagged, never refused.
		expect(mine?.signerIsGenesisOwner).toBe(false);
		// The wallet reads Base's members itself: the new owner counts.
		await expect(page.getByText('2/2 approved').first()).toBeVisible({ timeout: 60_000 });
		await shot(page, 'w4-counted');
		const before = await balanceOf(RECIPIENT, UNREAD_CHAIN);
		await page.getByRole('button', { name: 'Execute', exact: true }).click();
		await expect(page.getByText('Executed', { exact: true }).first()).toBeVisible({
			timeout: 120_000
		});
		expect((await balanceOf(RECIPIENT, UNREAD_CHAIN)) - before).toBe(5n * 10n ** 17n);
		await page.keyboard.press('Escape');
	});

	await test.step('Settings: the original members, and the current ones on Base', async () => {
		await page
			.getByRole('link', { name: /^(Needs \d+ of \d+ members|Rule differs by network)$/ })
			.first()
			.click();
		await expect(page.getByRole('heading', { name: 'Original members' })).toBeVisible({
			timeout: 30_000
		});
		await expect(page.getByRole('heading', { name: 'Current members on Base' })).toBeVisible({
			timeout: 30_000
		});
		await expect(page.getByTestId('member-added').first()).toBeVisible();
		await shot(page, 'w5-members');
	});
	expect(thrown).toEqual([]);
	await ctx.close();
});

/**
 * Proposals are found on every chain: a member proposes on a network where
 * the multisig is deployed nowhere yet, and another member's device — which
 * has never used it there and got no link — finds it in the mailbox's
 * cross-chain list, makes that network one of its networks, and approves.
 */
test('a proposal on a network it is not deployed on: found by another device without a link', async ({
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

	await pageA
		.getByRole('button', { name: /Parallel One/ })
		.first()
		.click();
	await pageA.getByRole('button', { name: 'Add account' }).click();
	await pageA.getByRole('button', { name: /^Create multisig/ }).click();
	await pageA.getByRole('textbox', { name: /^Name/ }).fill('Found on Base');
	await pageA.getByPlaceholder("Member's address (0x…)").fill(FIXTURES.two);
	await pageA.getByRole('button', { name: 'Add', exact: true }).click();
	await pageA.getByRole('button', { name: 'Next' }).click();
	await pageA.getByRole('button', { name: 'Next' }).click();
	await pickFirstNetwork(pageA);
	await pageA.getByRole('button', { name: 'Create', exact: true }).click();
	await pageA.getByRole('button', { name: 'Done' }).click();
	const multisig = (await pageA.evaluate(() => localStorage.getItem('vela.activeMultisig')))!;
	const genesis = await servedGenesis(multisig);
	await setBalance(multisig, 2n * 10n ** 18n, UNREAD_CHAIN);

	await test.step('device B has it, used on Gnosis only', async () => {
		await pageB.evaluate(
			({ address, genesis, owners }) => {
				localStorage.setItem(
					'vela.multisigAccounts',
					JSON.stringify([
						{
							address,
							genesis,
							owners,
							threshold: 2,
							recorded: false,
							shared: true,
							chainIds: [100],
							addedMs: Date.now()
						}
					])
				);
				localStorage.setItem('vela.activeMultisig', address);
			},
			{ address: multisig, genesis, owners: [FIXTURES.one, FIXTURES.two] }
		);
		await pageB.reload();
		await expect(pageB.getByRole('link', { name: 'Needs 2 of 2 members' }).first()).toBeVisible();
	});

	await test.step('device A proposes on Base, where it is deployed nowhere', async () => {
		await pageA.reload();
		await proposeSend(pageA, '0.25', 'f1-propose', { row: /^ETH Base/, symbol: 'ETH' });
		await expect(pageA.getByText('1/2 approved').first()).toBeVisible();
		expect(await rpc<string>('eth_getCode', [multisig, 'latest'], UNREAD_CHAIN)).toBe('0x');
	});

	await test.step('device B finds it in the cross-chain list — no link — and executes', async () => {
		await pageB.reload();
		await expect(pageB.getByText('Waiting for approval').first()).toBeVisible({ timeout: 60_000 });
		const row = pageB.getByText('Send 0.25 ETH').first();
		await expect(row).toBeVisible({ timeout: 60_000 });
		await shot(pageB, 'f2-found');
		const kept = await pageB.evaluate(
			() =>
				(
					JSON.parse(localStorage.getItem('vela.multisigAccounts') ?? '[]') as {
						chainIds: number[];
					}[]
				)[0]?.chainIds
		);
		expect(kept).toContain(UNREAD_CHAIN);
		await row.click();
		const before = await balanceOf(RECIPIENT, UNREAD_CHAIN);
		await pageB.getByRole('button', { name: 'Approve and execute' }).click();
		await expect(pageB.getByText('Executed', { exact: true }).first()).toBeVisible({
			timeout: 120_000
		});
		await shot(pageB, 'f3-executed');
		expect((await balanceOf(RECIPIENT, UNREAD_CHAIN)) - before).toBe(25n * 10n ** 16n);
	});
	expect(thrownA).toEqual([]);
	expect(thrownB).toEqual([]);
	await a.close();
	await b.close();
});

/**
 * §3.5: the pinned code hashes are what the chain holds — every canonical
 * Safe contract the core accepts, read on the fork (Gnosis's state) and
 * hashed here.
 */
test('the canonical Safe contracts on the fork hold exactly the code the core pins', async () => {
	const table = await canonicalSafeContracts();
	expect(table.length).toBeGreaterThanOrEqual(18);
	for (const c of table) {
		const code = await rpc<string>('eth_getCode', [c.address, 'latest']);
		expect(code.length, c.name).toBeGreaterThan(2);
		expect(await keccak(code), `${c.name} ${c.address}`).toBe(c.codeHash);
	}
});

/**
 * r4-e (S1): a malicious genesis service hands out a genesis whose setup
 * enables an attacker's module. The address derives — and the wallet says
 * no: undeployed, the core refuses it (nothing adds it, nothing deploys it);
 * deployed with its module, the chain shows it, said plainly, and it is added
 * only on the person's explicit tap, flagged from then on. Every signature
 * for it on Gnosis then asks first, naming the module; and multisigs kept
 * before the check are checked again on the next visit — a refused genesis
 * read-only, a deployed one's module found again.
 */
test('a genesis that hands the multisig to a module: refused; deployed, said and added only on a tap', async ({
	browser
}) => {
	const ctx = await browser.newContext({ viewport: { width: 430, height: 900 } });
	await useHarness(ctx);
	const hooked = await hookedSafe([FIXTURES.one], ATTACKER_MODULE, String(Date.now()));
	// The malicious service: the hooked genesis for this address, as found and checked.
	await ctx.route('**/api/genesis/**', async (route) => {
		if (!route.request().url().toLowerCase().includes(hooked.address.toLowerCase())) {
			return route.continue();
		}
		await route.fulfill({
			status: 200,
			contentType: 'application/json',
			body: JSON.stringify({
				found: true,
				genesis: { ...hooked.genesis, name: 'Treasury' },
				recordedOn: [100],
				readFrom: 100,
				verified: true,
				nameBySafe: true,
				renamedAt: 0
			})
		});
	});
	const page = await ctx.newPage();
	const thrown = errors(page);
	await enterParallel(page);
	await page
		.getByRole('button', { name: /Parallel One/ })
		.first()
		.click();
	await page.getByRole('button', { name: 'Add account' }).click();
	await page.getByRole('button', { name: /^Add multisig/ }).click();
	const field = page.getByPlaceholder('Multisig address (0x…)');
	const add = page.getByRole('button', { name: 'Add', exact: true });
	const module = new RegExp(`module that can move funds without its members: ${ATTACKER_MODULE}`);

	await test.step('undeployed: the core refuses its genesis, and nothing adds it', async () => {
		await field.fill(hooked.address);
		await add.click();
		// Nothing says which network it is on: the person names one.
		// r4-create: the networks are a list on the sheet; a pick only picks.
		await page
			.getByRole('radiogroup', { name: 'Network' })
			.getByRole('radio', { name: 'Gnosis', exact: true })
			.click();
		await add.click();
		const notice = page.getByTestId('authority-refused');
		await expect(notice).toBeVisible({ timeout: 60_000 });
		await expect(notice).toContainText("Vela won't use this multisig");
		await expect(notice).toContainText(module);
		await expect(add).toBeDisabled();
		await expect(page.getByRole('button', { name: /add it anyway/ })).toHaveCount(0);
		await shot(page, 'h1-refused');
		expect(await page.evaluate(() => localStorage.getItem('vela.multisigAccounts'))).toBeNull();
	});

	await hooked.deploy();
	expect(
		await rpc<string>('eth_call', [
			{ to: hooked.address, data: `0x2d9ad53d${ATTACKER_MODULE.slice(2).padStart(64, '0')}` },
			'latest'
		])
	).toBe(`0x${'0'.repeat(63)}1`);

	await test.step('deployed with its module: shown plainly, added only on the tap', async () => {
		await field.fill('');
		await field.fill(hooked.address);
		await add.click();
		const notice = page.getByTestId('authority-risk');
		await expect(notice).toBeVisible({ timeout: 60_000 });
		await expect(notice).toContainText(
			new RegExp(`On Gnosis.*, this multisig has a ${module.source}`)
		);
		expect(await page.evaluate(() => localStorage.getItem('vela.multisigAccounts'))).toBeNull();
		await shot(page, 'h2-high-risk');
		await page.getByRole('button', { name: 'I understand — add it anyway' }).click();
		await expect(page.getByText('Treasury', { exact: true }).first()).toBeVisible({
			timeout: 30_000
		});
		const kept = JSON.parse(
			(await page.evaluate(() => localStorage.getItem('vela.multisigAccounts')))!
		) as { address: string; findings?: unknown[] }[];
		expect(kept[0].findings).toEqual([
			{ from: 'chain', chainId: 100, risk: { kind: 'module', address: ATTACKER_MODULE } }
		]);
	});

	await test.step('and it stays said: the switcher and its settings', async () => {
		await page.goto('/en/settings');
		await expect(page.getByTestId('authority-risk')).toContainText(module, { timeout: 30_000 });
		await shot(page, 'h3-settings');
	});

	await test.step('each signature on Gnosis asks first, naming the module — every time', async () => {
		await setBalance(hooked.address, 2n * 10n ** 18n);
		await page.goto('/en/wallet');
		await page
			.getByRole('button', { name: en('componentsUi.dock.send'), exact: true })
			.first()
			.click();
		await expect(page.getByRole('heading', { name: en('send.selectTokenTitle') })).toBeVisible();
		await page
			.getByRole('button', { name: /^xDAI Gnosis/ })
			.last()
			.click({ timeout: 60_000 });
		await page.getByRole('textbox', { name: en('send.recipientLabel') }).fill(RECIPIENT);
		await page.getByRole('textbox', { name: 'xDAI' }).fill('0.5');
		const advance = page.getByRole('button', { name: en('send.continueBtn') });
		await expect(advance).toBeEnabled({ timeout: 30_000 });
		await advance.click();
		const propose = page.getByRole('button', { name: 'Propose and approve' });
		const confirm = page.getByTestId('risk-confirm');
		await propose.click();
		await confirmReview(page);
		await expect(confirm).toBeVisible({ timeout: 30_000 });
		await expect(confirm).toContainText(module);
		await shot(page, 'h4-sign-asks');
		await page.getByRole('button', { name: "Don't sign" }).click();
		await expect(confirm).toHaveCount(0);
		// Nothing signed: the send stands, to be proposed again — reviewed again
		// (§3.6: each proposal is drawn at its number before its passkey) and
		// asked again.
		await expect(propose).toBeEnabled();
		await propose.click();
		await confirmReview(page);
		await expect(confirm).toBeVisible({ timeout: 30_000 });
		await page.getByRole('button', { name: 'I understand the risk — sign' }).click();
		await expect(page.getByText('1/1 approved').first()).toBeVisible({ timeout: 60_000 });
		// Executing is a signature of its own: asked again, before its passkey.
		const before = await balanceOf(RECIPIENT);
		await page.getByRole('button', { name: /^Execute/ }).click();
		await expect(confirm).toBeVisible({ timeout: 30_000 });
		await page.getByRole('button', { name: 'I understand the risk — sign' }).click();
		await expect(page.getByText('Executed', { exact: true }).first()).toBeVisible({
			timeout: 120_000
		});
		expect((await balanceOf(RECIPIENT)) - before).toBe(5n * 10n ** 17n);
		await shot(page, 'h5-executed');
	});

	await test.step('kept here before the check: checked again on the next visit, and said', async () => {
		// One kept with nothing found (a build before the rule), and one not
		// deployed whose genesis hands it to the module, never vetted.
		const older = await hookedSafe([FIXTURES.one], ATTACKER_MODULE, String(Date.now() + 1));
		await page.evaluate(
			({ owner, extra }) => {
				const rows = JSON.parse(localStorage.getItem('vela.multisigAccounts')!) as Record<
					string,
					unknown
				>[];
				for (const r of rows) delete r.findings;
				rows.push({
					address: extra.address,
					genesis: { ...extra.genesis, name: 'Old vault' },
					owners: [owner],
					threshold: 1,
					recorded: true,
					shared: true,
					chainIds: [100],
					addedMs: 1
				});
				localStorage.setItem('vela.multisigAccounts', JSON.stringify(rows));
				localStorage.setItem('vela.activeMultisig', extra.address);
			},
			{ owner: FIXTURES.one, extra: { address: older.address, genesis: older.genesis } }
		);
		await page.goto('/en/settings');
		const refused = page.getByTestId('authority-refused');
		await expect(refused).toContainText('Read-only on Gnosis', { timeout: 60_000 });
		await expect(refused).toContainText(module);
		await shot(page, 'h6-refused-kept');
		// The deployed one's module, found again and kept on it.
		await expect
			.poll(
				async () =>
					(
						JSON.parse(
							(await page.evaluate(() => localStorage.getItem('vela.multisigAccounts')))!
						) as { address: string; findings?: unknown[] }[]
					).find((r) => r.address === hooked.address)?.findings,
				{ timeout: 60_000 }
			)
			.toEqual([
				{ from: 'chain', chainId: 100, risk: { kind: 'module', address: ATTACKER_MODULE } }
			]);
		// The switcher says both, each as what it is.
		await page.goto('/en/wallet');
		await page
			.getByRole('button', { name: /^Old vault/ })
			.first()
			.click();
		await expect(page.getByTestId('flag-refused')).toBeVisible({ timeout: 30_000 });
		await expect(page.getByTestId('flag-risk')).toBeVisible();
		await shot(page, 'h7-switcher');
	});
	expect(thrown).toEqual([]);
	await ctx.close();
});

/** A Safe's owners and threshold on `chainId`, as the chain holds them (`getOwners`, `getThreshold`). */
async function ownersOn(
	safe: string,
	chainId: number
): Promise<{ owners: string; threshold: number }> {
	const read = async (signature: string) =>
		rpc<string>('eth_call', [{ to: safe, data: await calldata(signature) }, 'latest'], chainId);
	return {
		owners: (await read('getOwners()')).toLowerCase(),
		threshold: Number(BigInt(await read('getThreshold()')))
	};
}

/**
 * Members changed from Settings (r4D-3): one action — Add member — is one
 * proposal per network the multisig is deployed on, each built against that
 * network's own members; the two read as one change with each network's
 * progress, and each is approved and executed there like any other
 * transaction. The chain says the rest.
 */
test('members changed from Settings: one proposal per deployed network, executed on each', async ({
	browser
}) => {
	test.setTimeout(480_000);
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
	await page.getByRole('textbox', { name: /^Name/ }).fill('Changes hands');
	await page.getByText('Parallel Two', { exact: true }).click();
	await page.getByText('Parallel Three', { exact: true }).click();
	await page.getByRole('button', { name: 'Next' }).click();
	await expect(page.getByText('2 of 3', { exact: true })).toBeVisible();
	// The rule chosen, the way back from a lost passkey said with it.
	await expect(page.getByTestId('create-recovery')).toBeVisible();
	await page.getByRole('button', { name: 'Next' }).click();
	await pickFirstNetwork(page);
	await page.getByRole('button', { name: 'Create', exact: true }).click();
	await page.getByRole('button', { name: 'Done' }).click();
	const multisig = (await page.evaluate(() => localStorage.getItem('vela.activeMultisig')))!;

	const closeSheet = () =>
		expect(async () => {
			await page.keyboard.press('Escape');
			await expect(page.getByRole('heading', { name: 'Multisig transaction' })).toHaveCount(0, {
				timeout: 1_000
			});
		}).toPass({ timeout: 15_000 });

	await test.step('deployed on Gnosis and on Base: a transfer out on each', async () => {
		await setBalance(multisig, 2n * 10n ** 18n);
		await setBalance(multisig, 2n * 10n ** 18n, UNREAD_CHAIN);
		await page.reload();
		for (const coin of [
			{ row: /^xDAI Gnosis/, symbol: 'xDAI' },
			{ row: /^ETH Base/, symbol: 'ETH' }
		]) {
			await proposeSend(page, '0.1', `mc0-${coin.symbol}`, coin);
			await page.getByRole('button', { name: 'Approve and execute' }).click();
			await expect(page.getByText('Executed', { exact: true }).first()).toBeVisible({
				timeout: 120_000
			});
			await closeSheet();
			await page.goto('/en/wallet');
		}
		expect(await rpc<string>('eth_getCode', [multisig, 'latest'])).not.toBe('0x');
		expect(await rpc<string>('eth_getCode', [multisig, 'latest'], UNREAD_CHAIN)).not.toBe('0x');
	});

	await test.step('Add member: the change on each network, then one proposal each', async () => {
		await page.getByRole('link', { name: 'Needs 2 of 3 members' }).first().click();
		const changes = page.getByTestId('member-changes');
		await expect(changes.locator('.rows li')).toHaveCount(3, { timeout: 60_000 });
		await expect(page.getByTestId('recovery')).toBeVisible();
		await page.getByRole('button', { name: 'Add member' }).click();
		const entry = page.getByTestId('member-entry');
		// The multisig itself is no member: refused in words, on each network.
		await entry.fill(multisig);
		await entry.press('Tab');
		await expect(page.getByTestId('member-plan')).toContainText(
			"Gnosis: The multisig can't be its own member."
		);
		await entry.fill(EOA_ONE);
		await entry.press('Tab');
		await expect(page.getByTestId('member-resolved')).toContainText(EOA_ONE);
		await page.getByTestId('member-name').fill('Outside One');
		await expect(page.getByTestId('member-plan')).toContainText('Gnosis: 2 of 3 → 2 of 4');
		await expect(page.getByTestId('member-plan')).toContainText('Base: 2 of 3 → 2 of 4');
		await shot(page, 'mc1-add-plan');
		await page.getByRole('button', { name: 'Propose and approve' }).click();
		await confirmReview(page, 2);
		// One change, two networks (in the wallet's network order), each 1 of 2 so far.
		await expect(page.getByTestId('member-group')).toHaveAttribute(
			'data-progress',
			'Base 1/2 · Gnosis 1/2',
			{ timeout: 90_000 }
		);
		await expect(page.getByTestId('member-group')).toContainText('Add Outside One');
		await shot(page, 'mc2-group');
	});

	await test.step('each network approves and executes its own', async () => {
		for (const network of ['Gnosis', 'Base']) {
			await page
				.getByTestId('member-group')
				.getByRole('button', { name: new RegExp(`^${network} `) })
				.click();
			// r4a-1: read as the change it is, the member by the name kept here.
			await expect(page.getByTestId('member-change')).toContainText('Outside One', {
				timeout: 30_000
			});
			await page.getByRole('button', { name: 'Approve and execute' }).click();
			await expect(page.getByText('Executed', { exact: true }).first()).toBeVisible({
				timeout: 120_000
			});
			await shot(page, `mc3-executed-${network}`);
			await closeSheet();
		}
		for (const chainId of [100, UNREAD_CHAIN]) {
			const now = await ownersOn(multisig, chainId);
			expect(now.owners).toContain(EOA_ONE.slice(2).toLowerCase());
			expect(now.threshold).toBe(2);
		}
		// Nothing waits any more; the rows are the four members now.
		await page.reload();
		await expect(page.getByTestId('member-changes').locator('.rows li')).toHaveCount(4, {
			timeout: 60_000
		});
		await expect(page.getByTestId('member-group')).toHaveCount(0);
		await shot(page, 'mc4-four');
	});
	expect(thrown).toEqual([]);
	await ctx.close();
});
