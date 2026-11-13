/**
 * A site's transaction while a multisig is the connected account (dapp-1,
 * dapp-2; docs/multisig-vault-design.md §6), through the packaged extension,
 * its real core and the parallel space's passkeys — against the hermetic
 * multisig world (`stub-multisig.ts`), so it runs without a chain.
 *
 * The request becomes a proposal: reviewed first, signed on "Propose and
 * approve" by the member here, and HELD — the site is answered only when the
 * transaction runs. Here the other member, an outside wallet, approves and
 * executes it from its own wallet; the wallet finds its `ExecutionSuccess`
 * on chain and hands the site the transaction that ran it. A batch
 * (`wallet_sendCalls`) is answered its id once proposed, and
 * `wallet_getCallsStatus` says 100 (pending) until it ran, then 200 with
 * that transaction's receipt.
 *
 * Chromium only: an extension loads in no other engine.
 */
import { holdPress } from './stub-chain';
import { createServer, type Server } from 'node:http';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test, type BrowserContext, type Page } from '@playwright/test';
import {
	extensionBuilt,
	extensionId,
	loadExtension,
	noRequestWindow,
	requestWindow
} from './extension-helpers';
import {
	FIXTURES,
	MultisigWorld,
	OUTSIDE,
	multisigOf,
	storedRow,
	useWorldNetworks,
	type Multisig
} from './stub-multisig';

const PORT = 8834;
const RECIPIENT = '0x000000000000000000000000000000000000bEEF';

interface AskResult {
	ok: boolean;
	code?: number;
	message?: string;
	result?: unknown;
}

function serveDApp(): Promise<Server> {
	const html = readFileSync(join(import.meta.dirname, 'testdapp/index.html'));
	const server = createServer((_req, res) => {
		res.setHeader('content-type', 'text/html; charset=utf-8');
		res.end(html);
	});
	return new Promise((resolve) => server.listen(PORT, () => resolve(server)));
}

/**
 * The extension's wallet in the parallel space — fixture one alone, its
 * chains this world's — holding `ms` as the account shown, published to the
 * service worker for sites.
 */
async function walletHolding(context: BrowserContext, ms: Multisig): Promise<Page> {
	const id = extensionId();
	const wallet = await context.newPage();
	await wallet.addInitScript(() => {
		localStorage.setItem('vela.intro.seen', String(Date.now()));
		// §28: the development package refuses writes to production hosts. This
		// world answers the multisig service itself and denies every other host
		// (`MultisigWorld`'s default-deny net), so nothing leaves the machine:
		// the suite opts in, and the stub sees what the wallet sends.
		localStorage.setItem('vela.dev.allowProductionWrites', '1');
	});
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
	await wallet.reload();
	await expect(wallet.getByText('Harbor fund', { exact: true }).first()).toBeVisible({
		timeout: 30_000
	});
	return wallet;
}

/** The site's ask, left running: it settles when the wallet answers. */
function ask(page: Page, method: string, params: unknown[]): Promise<AskResult> {
	const asked = page.evaluate(([m, p]) => window.__ask(m as string, p as unknown[]), [
		method,
		params
	] as const) as Promise<AskResult>;
	asked.catch(() => {});
	return asked;
}

/** Whether `promise` has settled yet. */
async function settled(promise: Promise<unknown>): Promise<boolean> {
	const marker = Symbol('pending');
	return (await Promise.race([promise.then(() => true), Promise.resolve(marker)])) !== marker;
}

test.describe('a site asks a multisig', () => {
	test.skip(!extensionBuilt(), 'extension/dist is missing — run `bun run build:extension`');
	test.setTimeout(240_000);

	let server: Server;
	test.beforeAll(async () => {
		server = await serveDApp();
	});
	test.afterAll(async () => {
		server.closeAllConnections();
		await new Promise((resolve) => server.close(() => resolve(null)));
	});

	test('eth_sendTransaction and wallet_sendCalls are held until executed; getCallsStatus 100, then 200', async () => {
		const world = new MultisigWorld();
		// Two members: fixture one here, and an outside wallet that executes.
		const ms = await multisigOf([FIXTURES.one, OUTSIDE.address], 2, 'Harbor fund');
		world.record(ms);
		world.balances.set(`100:${ms.address.toLowerCase()}`, 3n * 10n ** 18n);
		const context = await loadExtension();
		await world.install(context);
		await walletHolding(context, ms);

		const site = await context.newPage();
		await site.goto(`http://localhost:${PORT}/`);

		await test.step('the site connects to the multisig', async () => {
			const asked = ask(site, 'eth_requestAccounts', []);
			const win = await requestWindow(context, 30_000);
			await expect(win.getByText('Harbor fund').first()).toBeVisible({ timeout: 30_000 });
			await win.getByRole('button', { name: 'Connect' }).click();
			expect((await asked).result).toEqual([ms.address]);
			await noRequestWindow(context);
			// A site on Gnosis, where the multisig lives.
			const switched = await ask(site, 'wallet_switchEthereumChain', [{ chainId: '0x64' }]);
			expect(switched.ok).toBe(true);
			expect((await ask(site, 'eth_chainId', [])).result).toBe('0x64');
		});

		await test.step('eth_sendTransaction: reviewed, proposed, held — answered once it ran', async () => {
			const sent = ask(site, 'eth_sendTransaction', [
				{ from: ms.address, to: RECIPIENT, value: '0x16345785d8a0000' }
			]);
			const win = await requestWindow(context, 30_000);
			await expect(win.getByText(`localhost:${PORT} asks for this transaction.`)).toBeVisible({
				timeout: 30_000
			});
			await expect(win.getByText('0.1 xDAI').first()).toBeVisible();
			// The review first: nothing signed or sent before the press.
			await expect(win.getByTestId('review-hashes')).toBeVisible();
			expect(world.submissions).toHaveLength(0);
			await holdPress(win.getByRole('button', { name: 'Propose and approve' }));
			await expect.poll(() => world.mailbox.length, { timeout: 30_000 }).toBe(1);
			const hash = world.mailbox[0].safeTxHash;
			await expect(win.getByText('1/2 approved').first()).toBeVisible({ timeout: 30_000 });
			// Proposed is not done: the site still waits.
			await site.waitForTimeout(2_000);
			expect(await settled(sent)).toBe(false);

			// The outside member approves and executes it from its own wallet.
			await world.approveAsOutsideOwner(100, hash);
			const tx = world.executeAsOutsideOwner(100, ms, hash);
			const answer = await sent;
			expect(answer).toEqual({ ok: true, result: tx });
			// Answered, the window has nothing left to hold, and goes.
			await noRequestWindow(context, 30_000);
		});

		await test.step('wallet_sendCalls: its id once proposed; getCallsStatus 100, then 200', async () => {
			const sent = ask(site, 'wallet_sendCalls', [
				{
					version: '2.0.0',
					chainId: '0x64',
					from: ms.address,
					atomicRequired: true,
					calls: [{ to: RECIPIENT, value: '0x2386f26fc10000' }]
				}
			]);
			const win = await requestWindow(context, 30_000);
			await expect(win.getByText(`localhost:${PORT} asks for this transaction.`)).toBeVisible({
				timeout: 30_000
			});
			await expect(win.getByTestId('review-hashes')).toBeVisible();
			// The next number: the first one ran.
			await expect(win.getByText('#1', { exact: true }).first()).toBeVisible();
			await holdPress(win.getByRole('button', { name: 'Propose and approve' }));
			const answer = await sent;
			expect(world.mailbox).toHaveLength(2);
			const hash = world.mailbox[1].safeTxHash;
			expect(answer).toEqual({ ok: true, result: { id: hash } });

			const status = async () =>
				(await ask(site, 'wallet_getCallsStatus', [hash])) as AskResult & {
					result?: { status?: number; receipts?: { transactionHash?: string }[] };
				};
			const pending = await status();
			// Known at once, as the site asks right after its answer (viem's
			// waitForCallsStatus): pending until it runs.
			expect(pending, JSON.stringify(pending)).toMatchObject({ ok: true });
			expect(pending.result?.status).toBe(100);

			await world.approveAsOutsideOwner(100, hash);
			const tx = world.executeAsOutsideOwner(100, ms, hash);
			await expect.poll(async () => (await status()).result?.status, { timeout: 60_000 }).toBe(200);
			const done = await status();
			expect(done.result).toMatchObject({
				id: hash,
				status: 200,
				receipts: [{ transactionHash: tx, status: '0x1' }]
			});
		});

		await context.close();
	});
});
