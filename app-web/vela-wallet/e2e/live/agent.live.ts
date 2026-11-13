/**
 * Arc Testnet, live, as an AI agent uses the wallet: over MCP, through a real
 * ClientMCP bridge (`clientmcp serve`), into the open wallet tab. The agent
 * reads, drafts and asks; the person — Playwright, with a virtual passkey —
 * approves and signs in the wallet, as they would. A Vela account first, then
 * a multisig of two of them.
 */
import { expect, test, type Page } from '@playwright/test';
import { en } from '../live-helpers';
import { Agent, linkAgent, mcpReady } from './mcp';
import {
	ARC_CHAIN,
	FUNDER,
	balanceOf,
	createAccount,
	fund,
	live,
	livePage,
	shot,
	toTestnet
} from './live-helpers';

test.skip(!live || !mcpReady, 'needs ARC_FUNDER(_KEY), a ClientMCP bridge and its tokens');

/** Text someone other than Vela wrote, as vela_state hands it over (r4e-2). */
type Untrusted = { untrusted_text: string; from: string };
type Account = {
	address: string;
	name: Untrusted;
	active: boolean;
	agent_allowed: boolean;
	multisig?: { threshold: number; members: string[]; waiting_proposals: number | null };
};
type Token = { chain_id: number; symbol: string | Untrusted; balance: string | null };

const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

async function accounts(agent: Agent): Promise<Account[]> {
	return (await agent.data('vela_state')).accounts as Account[];
}

/** The agent refreshes and reads until the account in front holds Arc's USDC. */
async function arcUsdc(agent: Agent): Promise<number> {
	let held = 0;
	await expect
		.poll(
			async () => {
				await agent.call('vela_refresh_balances');
				const state = await agent.data('vela_state');
				const tokens = ((state.balances as { tokens?: Token[] } | null)?.tokens ?? []) as Token[];
				held = Number(tokens.find((t) => t.chain_id === ARC_CHAIN)?.balance ?? 0);
				return held;
			},
			{ timeout: 180_000, intervals: [5_000] }
		)
		.toBeGreaterThan(0);
	return held;
}

/**
 * A manage-level request: the person allows it in the wallet's sheet. If the
 * wallet answers the agent before any sheet is up, that answer is the failure.
 */
async function allowed(
	page: Page,
	request: Promise<Record<string, unknown>>
): Promise<Record<string, unknown>> {
	const sheet = page.getByText(en('settings.agent.approveTitle'));
	const early = await Promise.race([
		request.then((answer) => ({ answer })),
		sheet
			.waitFor({ timeout: 30_000 })
			.then(() => null)
			.catch(() => ({ answer: null }))
	]);
	if (early) {
		await shot(page, 'a4-agent-no-sheet');
		const bar = await page
			.getByTestId('agent-bar')
			.textContent()
			.catch(() => null);
		const answer = early.answer ?? (await request);
		throw new Error(`no approval sheet; bar "${bar}"; answer ${JSON.stringify(answer)}`);
	}
	const allowButton = page.getByRole('button', { name: en('settings.agent.allow'), exact: true });
	await expect(allowButton).toBeEnabled({ timeout: 10_000 });
	await allowButton.click();
	return request;
}

test('an AI agent over ClientMCP: a Vela account, then a multisig', async ({ browser }) => {
	const context = await browser.newContext();
	const page = await livePage(context);
	await page.goto('/en');
	await page.getByText('Create Wallet').click();
	const alice = await createAccount(page, 'Agent Alice');
	await toTestnet(page);
	// A new account's first transfer deploys it: its fee is most of 0.2 USDC.
	fund(alice, '0.3');

	const agent = new Agent();
	await test.step('the person links the agent; it sees only what it may use', async () => {
		await linkAgent(page, [alice]);
		await page.reload();
		// The wallet's tab dials the bridge and declares its tools; the agent then meets it.
		await expect
			.poll(() => agent.tools().then((t) => t.length), { timeout: 60_000 })
			.toBeGreaterThan(30);
		const init = await agent.initialize();
		expect(init.instructions ?? '').toContain('vela_state');
		const state = await agent.data('vela_state');
		expect(state).toMatchObject({
			signed_in: true,
			wallet_allowed: true,
			network_space: 'testnet'
		});
		const rows = state.accounts as Account[];
		expect(rows).toHaveLength(1);
		expect(rows[0]).toMatchObject({
			name: { untrusted_text: 'Agent Alice', from: 'this_device' },
			active: true,
			agent_allowed: true
		});
		expect(rows[0].multisig).toBeUndefined();
		expect(await arcUsdc(agent)).toBeCloseTo(0.3, 6);
	});

	await test.step('a transfer the agent drafts; the person confirms with the passkey', async () => {
		const before = balanceOf(FUNDER);
		const drafted = await agent.data('vela_prepare_transfer', {
			chain_id: ARC_CHAIN,
			token: null,
			to: FUNDER,
			amount: '0.02'
		});
		expect(drafted).toMatchObject({
			status: 'accepted',
			transfer: { state: 'awaiting_confirmation' }
		});
		const operation = drafted.operation as string;
		await expect(page.getByRole('heading', { name: en('send.confirmTitle') })).toBeVisible();
		const review = await agent.data('vela_review');
		expect(review).toMatchObject({
			transfer: {
				on_review: true,
				operation,
				drafted_by: 'agent',
				network: { chain_id: ARC_CHAIN }
			}
		});
		// The agent asks; only the person's tap signs.
		expect(await agent.data('vela_submit_transfer', { operation })).toMatchObject({
			status: 'accepted'
		});
		await shot(page, 'a1-agent-review');
		// The person reads the page as it settles: the first-time warning for
		// this recipient lands after the page (and moves the button).
		await expect(page.getByText(en('componentsUi.signing.firstTimeTag'))).toBeVisible();
		const confirm = page.getByRole('button', { name: en('send.confirmSendBtn') });
		await expect(confirm).toBeEnabled({ timeout: 120_000 });
		await confirm.click();
		const landed = await agent.data('vela_wait', { operation, timeout_ms: 100_000 });
		expect(landed).toMatchObject({ timed_out: false, transfer: { state: 'confirmed' } });
		await expect
			.poll(() => balanceOf(FUNDER) - before, { timeout: 120_000, intervals: [3000] })
			.toBe(2n * 10n ** 16n);
		// The receipt's Done, if it is still up.
		const done = page.getByRole('button', { name: 'Done', exact: true }).first();
		if (await done.isVisible()) await done.click();
	});

	let bob = '';
	let multisig = '';
	await test.step('a 2-of-2 of Alice and Bob, made by the person', async () => {
		await page
			.getByRole('button', { name: /Agent Alice/ })
			.first()
			.click();
		await page.getByRole('button', { name: 'Add account' }).click();
		await page.getByRole('button', { name: /^Create New Account/ }).click();
		await page.waitForURL(/\/en\/create/);
		bob = await createAccount(page, 'Agent Bob');
		fund(bob, '0.1');
		await page
			.getByRole('button', { name: /Agent Bob/ })
			.first()
			.click();
		await page.getByRole('button', { name: 'Add account' }).click();
		await page.getByRole('button', { name: /^Create multisig/ }).click();
		await page.getByRole('textbox', { name: /^Name/ }).fill('Agent Team');
		await page.getByText('Agent Alice', { exact: true }).click();
		await page.getByRole('button', { name: 'Next' }).click();
		await page.getByRole('button', { name: 'Next' }).click();
		await page.getByRole('button', { name: 'First network' }).click();
		await page.getByRole('option', { name: 'Arc Testnet' }).click();
		await page.getByRole('button', { name: 'Create', exact: true }).click();
		await expect(page.getByText('Multisig created')).toBeVisible({ timeout: 120_000 });
		await page.getByRole('button', { name: 'Done' }).click();
		multisig = (await page.evaluate(() => localStorage.getItem('vela.activeMultisig')))!;
		expect(multisig).toMatch(/^0x[0-9a-fA-F]{40}$/);
		fund(multisig, '0.1');
	});

	await test.step('the agent reads the multisig: an account, with its members', async () => {
		// Not on the person's list yet: listed, but not read.
		let rows = await accounts(agent);
		const row = rows.find((r) => same(r.address, multisig));
		expect(row).toMatchObject({ name: 'Agent Team', active: true, agent_allowed: false });
		expect(row?.multisig?.threshold).toBe(2);
		expect(row?.multisig?.members.map((m) => m.toLowerCase()).sort()).toEqual(
			[alice, bob].map((m) => m.toLowerCase()).sort()
		);
		expect((await agent.data('vela_state')).wallet_allowed).toBe(false);
		expect((await agent.data('vela_refresh_balances')).isError).toBe(true);
		// The person allows it: now it is read like any wallet.
		await linkAgent(page, [alice, bob, multisig]);
		await page.reload();
		await expect
			.poll(() => agent.tools().then((t) => t.length), { timeout: 60_000 })
			.toBeGreaterThan(30);
		rows = await accounts(agent);
		expect(rows.find((r) => same(r.address, multisig))).toMatchObject({
			active: true,
			agent_allowed: true,
			multisig: { waiting_proposals: 0 }
		});
		expect(rows.filter((r) => r.active)).toHaveLength(1);
		expect(await arcUsdc(agent)).toBeCloseTo(0.1, 6);
	});

	await test.step('the agent drafts from the multisig; the members propose and execute', async () => {
		let before = balanceOf(FUNDER);
		const drafted = await agent.data('vela_prepare_transfer', {
			chain_id: ARC_CHAIN,
			token: null,
			to: FUNDER,
			amount: '0.03'
		});
		expect(drafted).toMatchObject({ status: 'accepted' });
		const operation = drafted.operation as string;
		await expect(page.getByRole('heading', { name: en('send.confirmTitle') })).toBeVisible();
		expect(await agent.data('vela_review')).toMatchObject({
			transfer: { on_review: true, operation, drafted_by: 'agent' }
		});
		await agent.call('vela_submit_transfer', { operation });
		await shot(page, 'a2-agent-multisig-review');
		// Held a moment as the agent's review opens, then the person's to press.
		const propose = page.getByRole('button', { name: 'Propose and approve' });
		await expect(propose).toBeEnabled({ timeout: 120_000 });
		await propose.click();
		await expect(page.getByText('1/2 approved').first()).toBeVisible({ timeout: 120_000 });
		await expect
			.poll(
				async () =>
					(await accounts(agent)).find((r) => same(r.address, multisig))?.multisig
						?.waiting_proposals,
				{ timeout: 60_000, intervals: [2000] }
			)
			.toBe(1);
		// Proposed, not sent: in the members' hands, never "left without sending".
		const proposed = await agent.data('vela_wait', { operation, timeout_ms: 5_000 });
		expect(proposed.transfer).toMatchObject({ state: 'submitted' });
		expect((proposed.transfer as { safe_tx_hash?: string }).safe_tx_hash).toMatch(
			/^0x[0-9a-f]{64}$/
		);

		// The other member executes. One whose account cannot pay the fee
		// (deploying the multisig, then executing) is told so before any passkey.
		const execute = page.getByRole('button', { name: 'Approve and execute' });
		const cannotPay = page.getByText(/doesn't hold enough USDC for the network fee/);
		const executed = page.getByRole('status').filter({ hasText: 'Executed' });
		await execute.click();
		await expect(cannotPay.or(executed)).toBeVisible({ timeout: 240_000 });
		if (await cannotPay.isVisible()) {
			await shot(page, 'a3-agent-multisig-cannot-pay');
			const short = (await cannotPay.textContent()) ?? '';
			fund(short.includes('Agent Alice') ? alice : bob, '0.3');
			// The funder paid that too: what it receives is counted from here.
			before = balanceOf(FUNDER);
			await execute.click();
		}
		await expect(executed).toBeVisible({ timeout: 240_000 });
		await expect
			.poll(() => balanceOf(FUNDER) - before, { timeout: 180_000, intervals: [3000] })
			.toBe(3n * 10n ** 16n);
		await shot(page, 'a3-agent-multisig-executed');
		const done = await agent.data('vela_wait', { operation, timeout_ms: 30_000 });
		console.log('[agent] after executing:', JSON.stringify(done.transfer));
		await expect
			.poll(
				async () =>
					(await accounts(agent)).find((r) => same(r.address, multisig))?.multisig
						?.waiting_proposals,
				{ timeout: 60_000, intervals: [2000] }
			)
			.toBe(0);
	});

	await test.step('the agent switches back to Alice, with the person’s leave', async () => {
		await page.keyboard.press('Escape');
		expect(
			await allowed(page, agent.data('vela_switch_account', { address: alice }))
		).toMatchObject({ status: 'completed' });
		const rows = await accounts(agent);
		expect(rows.find((r) => r.active)?.address.toLowerCase()).toBe(alice.toLowerCase());
		expect(
			await allowed(page, agent.data('vela_switch_account', { address: multisig }))
		).toMatchObject({ status: 'completed' });
		expect((await accounts(agent)).find((r) => r.active)?.address.toLowerCase()).toBe(
			multisig.toLowerCase()
		);
	});
	await context.close();
});
