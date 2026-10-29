/**
 * Arc Testnet, live: Vela accounts made with virtual passkeys, then a
 * multisig of them — every step a person takes, on the real chain and relay.
 */
import { expect, test, type Page } from '@playwright/test';
import { en } from '../live-helpers';
import {
	FUNDER,
	balanceOf,
	createAccount,
	fund,
	live,
	livePage,
	liveSettings,
	shot,
	toTestnet,
	withFunderWallet
} from './live-helpers';

test.skip(!live, 'needs ARC_FUNDER and ARC_FUNDER_KEY');

/** Reload, as a person pulls to refresh, until the USDC holding is on the home. */
/**
 * §3.6: the proposer's own review before its passkey (what moves, who it
 * reaches, the state-diff guard, the hashes), confirmed with its own
 * "Propose and approve".
 */
async function confirmReview(page: Page): Promise<void> {
	const hashes = page.getByTestId('review-hashes');
	await expect(hashes).toBeVisible({ timeout: 90_000 });
	await hashes
		.locator('xpath=ancestor::*[@role="dialog"][1]')
		.getByRole('button', { name: /^Propose( and approve)?$/ })
		.click();
}

async function seeHolding(page: Page): Promise<void> {
	const row = page.getByRole('button', { name: /^USDC Arc Testnet/ });
	for (let attempt = 0; attempt < 6; attempt++) {
		await page.reload();
		// The read settles first ("Checking…" goes); then the row is there or not.
		const seen = await row
			.first()
			.waitFor({ timeout: 60_000 })
			.then(() => true)
			.catch(() => false);
		if (seen) return;
	}
	throw new Error('the USDC holding never showed');
}

/** Send `amount` USDC on Arc to `to` with the regular send flow; returns once it landed. */
async function sendUsdc(page: Page, to: string, amount: string, name: string): Promise<void> {
	await page
		.getByRole('button', { name: en('componentsUi.dock.send'), exact: true })
		.first()
		.click();
	await expect(page.getByRole('heading', { name: en('send.selectTokenTitle') })).toBeVisible();
	await page
		.getByRole('button', { name: /^USDC Arc Testnet/ })
		.last()
		.click({ timeout: 120_000 });
	await page.getByRole('textbox', { name: en('send.recipientLabel') }).fill(to);
	await page.getByRole('textbox', { name: 'USDC' }).fill(amount);
	const advance = page.getByRole('button', { name: en('send.continueBtn') });
	await expect(advance).toBeEnabled({ timeout: 120_000 });
	await shot(page, `${name}-form`);
	await advance.click();
	await expect(page.getByRole('heading', { name: en('send.confirmTitle') })).toBeVisible();
	await shot(page, `${name}-confirm`);
}

test('a Vela account on Arc Testnet: create with a passkey, receive, send', async ({ browser }) => {
	const context = await browser.newContext();
	const page = await livePage(context);
	page.on('response', (r) => {
		const u = r.url();
		if (/balance|5042002|arc\.network|tokens|portfolio/i.test(u) && !u.includes('localhost'))
			console.log('[net]', r.status(), r.request().method(), u.slice(0, 140));
	});
	page.on('console', (m) => {
		if (/balance|5042002|Arc/i.test(m.text())) console.log('[log]', m.text().slice(0, 200));
	});
	await page.goto('/en');
	await page.getByText('Create Wallet').click();
	const alice = await createAccount(page, 'Arc Alice');
	await toTestnet(page);
	await shot(page, 'a1-home');

	await test.step('receive: funds arrive and show', async () => {
		// A new account's first send deploys it: about 0.2 USDC at Arc's prices.
		fund(alice, '0.5');
		await seeHolding(page);
		await shot(page, 'a2-funded');
	});

	await test.step('send 0.2 USDC back to the funder, through the relay', async () => {
		const before = balanceOf(FUNDER);
		await sendUsdc(page, FUNDER, '0.2', 'a3');
		await page.getByRole('button', { name: en('send.confirmSendBtn') }).click();
		await expect(page.getByText(en('send.txSubmittedTitle'))).toBeVisible({ timeout: 120_000 });
		await expect
			.poll(() => balanceOf(FUNDER) - before, { timeout: 180_000, intervals: [3000] })
			.toBe(2n * 10n ** 17n);
		await shot(page, 'a4-sent');
	});
	await context.close();
});

test('a multisig on Arc Testnet: two passkey members, propose, approve, execute', async ({
	browser
}) => {
	const context = await browser.newContext();
	const page = await livePage(context);
	await page.goto('/en');
	await page.getByText('Create Wallet').click();
	const alice = await createAccount(page, 'Arc Alice');
	await toTestnet(page);

	await test.step('a second account, from "Add account"', async () => {
		await page
			.getByRole('button', { name: /Arc Alice/ })
			.first()
			.click();
		await page.getByRole('button', { name: 'Add account' }).click();
		await page.getByRole('button', { name: /^Create New Account/ }).click();
		await page.waitForURL(/\/en\/create/);
		await createAccount(page, 'Arc Bob');
		// Bob proposes (a signature, no transaction): he needs no funds. Alice
		// approves and executes, deploying her account and the Safe: she pays.
		fund(alice, '0.6');
	});

	let multisig = '';
	await test.step('create a 2-of-2 "Arc Team"', async () => {
		await page
			.getByRole('button', { name: /Arc Bob/ })
			.first()
			.click();
		await page.getByRole('button', { name: 'Add account' }).click();
		await page.getByRole('button', { name: /^Create multisig/ }).click();
		await page.getByRole('textbox', { name: /^Name/ }).fill('Arc Team');
		await page.getByText('Arc Alice', { exact: true }).click();
		await page.getByRole('button', { name: 'Next' }).click();
		await expect(page.getByText('2 of 2')).toBeVisible();
		await page.getByRole('button', { name: 'Next' }).click();
		await page.getByRole('button', { name: 'First network' }).click();
		await page.getByRole('option', { name: 'Arc Testnet' }).click();
		await shot(page, 'm1-review');
		await page.getByRole('button', { name: 'Create', exact: true }).click();
		await expect(page.getByText('Multisig created')).toBeVisible({ timeout: 120_000 });
		await page.getByRole('button', { name: 'Done' }).click();
		multisig = (await page.evaluate(() => localStorage.getItem('vela.activeMultisig')))!;
		expect(multisig).toMatch(/^0x[0-9a-fA-F]{40}$/);
		await expect(page.getByText('Arc Team').first()).toBeVisible();
	});

	await test.step('fund it and propose 0.1 USDC through the regular send', async () => {
		fund(multisig, '0.15');
		await seeHolding(page);
		await sendUsdc(page, FUNDER, '0.1', 'm2');
		await page.getByRole('button', { name: 'Propose and approve' }).click();
		await confirmReview(page);
		await expect(page.getByText('1/2 approved').first()).toBeVisible({ timeout: 120_000 });
		await shot(page, 'm3-proposal');
	});

	await test.step('the other member approves and executes; the Safe deploys', async () => {
		const before = balanceOf(FUNDER);
		await page.getByRole('button', { name: 'Approve and execute' }).click();
		// The sheet's own word: executed (the transaction on chain), not executing.
		await expect(page.getByRole('status').filter({ hasText: 'Executed' })).toBeVisible({
			timeout: 240_000
		});
		await expect(page.getByText('Ask members to approve')).toHaveCount(0);
		await expect
			.poll(() => balanceOf(FUNDER) - before, { timeout: 180_000, intervals: [3000] })
			.toBe(10n ** 17n);
		await shot(page, 'm4-executed');
	});
	await context.close();
});

test('an outside owner on Arc Testnet signs with their own wallet', async ({ browser }) => {
	const context = await browser.newContext();
	const page = await livePage(context);
	await page.goto('/en');
	await page.getByText('Create Wallet').click();
	const alice = await createAccount(page, 'Arc Alice');
	await toTestnet(page);
	// Alice executes, deploying her account and the Safe: she pays.
	fund(alice, '0.4');

	let multisig = '';
	await test.step('a 2-of-2 with the funder as a MetaMask-like member', async () => {
		await page
			.getByRole('button', { name: /Arc Alice/ })
			.first()
			.click();
		await page.getByRole('button', { name: 'Add account' }).click();
		await page.getByRole('button', { name: /^Create multisig/ }).click();
		await page.getByRole('textbox', { name: /^Name/ }).fill('Arc With MetaMask');
		await page.getByPlaceholder("Member's address (0x…)").fill(FUNDER);
		await page.getByRole('button', { name: 'Add', exact: true }).click();
		await page.getByRole('button', { name: 'Next' }).click();
		await page.getByRole('button', { name: 'Next' }).click();
		await page.getByRole('button', { name: 'First network' }).click();
		await page.getByRole('option', { name: 'Arc Testnet' }).click();
		await page.getByRole('button', { name: 'Create', exact: true }).click();
		await expect(page.getByText('Multisig created')).toBeVisible({ timeout: 120_000 });
		await page.getByRole('button', { name: 'Done' }).click();
		multisig = (await page.evaluate(() => localStorage.getItem('vela.activeMultisig')))!;
		fund(multisig, '0.15');
		await seeHolding(page);
		await sendUsdc(page, FUNDER, '0.1', 'x1');
		await page.getByRole('button', { name: 'Propose and approve' }).click();
		await confirmReview(page);
		await expect(page.getByText('1/2 approved').first()).toBeVisible({ timeout: 120_000 });
	});

	await test.step('the funder signs through the connect page; Alice executes', async () => {
		await page.getByRole('button', { name: 'Approve with another wallet' }).first().click();
		const link = (await page.getByTestId('external-link').textContent())!.trim();
		// ms-14: the member asked for rides in the fragment, never the query.
		expect(new URL(link).search).toBe('');
		expect(new URL(link).hash).toContain(`for=${FUNDER.toLowerCase()}`);
		await shot(page, 'x2-external');
		const other = await browser.newContext({ viewport: { width: 430, height: 900 } });
		// Both sides bring a relay token of their own, issued by Vela's site.
		const issued: string[] = [];
		for (const [side, ctx] of [
			['vela', context],
			['owner', other]
		] as const) {
			ctx.on('response', (r) => {
				if (r.url().includes('/api/peerconnect-token')) issued.push(`${side} ${r.status()}`);
			});
		}
		await liveSettings(other);
		await withFunderWallet(other);
		const wallet = await other.newPage();
		await wallet.goto(link);
		await wallet.getByRole('button', { name: 'Connect', exact: true }).click({ timeout: 60_000 });
		// r4C-7: Vela's request is reviewed on the page; the wallet is asked on the tap.
		await expect(wallet.getByTestId('connect-review')).toBeVisible({ timeout: 60_000 });
		await expect(wallet.getByTestId('connect-wrong-account')).toHaveCount(0);
		await wallet.getByRole('button', { name: /^Sign with / }).click();
		await expect(page.getByText('2/2 approved').first()).toBeVisible({ timeout: 120_000 });
		await shot(wallet, 'x3-connect');
		if (!process.env.LIVE_PEERCONNECT) {
			expect(issued, 'each side’s token came from Vela’s site').toEqual(
				expect.arrayContaining(['owner 200'])
			);
			const kept = await page.evaluate(() => localStorage.getItem('vela.peerconnectToken.v1'));
			expect(kept, 'Vela’s side holds a token').toBeTruthy();
		}
		const before = balanceOf(FUNDER);
		await page.getByRole('button', { name: /^Execute/ }).click();
		await expect(page.getByRole('status').filter({ hasText: 'Executed' })).toBeVisible({
			timeout: 240_000
		});
		await expect
			.poll(() => balanceOf(FUNDER) - before, { timeout: 180_000, intervals: [3000] })
			.toBe(10n ** 17n);
		await shot(page, 'x4-executed');
		await other.close();
	});
	await context.close();
});
