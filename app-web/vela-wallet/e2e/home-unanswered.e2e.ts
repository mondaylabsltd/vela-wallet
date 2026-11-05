/**
 * Networks that never answer (round 12), hermetically.
 *
 * r12-2: a plain account on mainnet while Soneium, Unichain and Celo did not
 * answer at all — a connection that hangs, not a 500 — must never read
 * "$0.00 · Live · listening for payments". On this host the three hang only
 * for a direct connection; the browser reached them through the system proxy
 * and answered, which is what the round's web saw. The rule still holds
 * here, with nodes that never answer anybody.
 *
 * r12-3: a warm outage. Everything answered, the home said so, and then a
 * network stopped answering. The home only aged "Updated 1m → 4m": its reads
 * after a complete round were the shell's five-minute cache, which said
 * "every network answered". The pool saw the network fail (the incoming scan
 * and the native-balance watch read through it) and nobody told the balance
 * machine. Now the pool's out-of-reach networks reach it (`pool_unreached`)
 * and it reads again at once, past the cache: the home names the network
 * without a reload. Without that this test sits on "Live" for its minute.
 */
import { expect, test, type Page, type Route } from '@playwright/test';
import { CHAINS } from '../src/lib/services/chains';
import { en, seedSignedIn } from './live-helpers';
import {
	abiWord,
	aggregate3CallCount,
	denyOffOrigin,
	encodeAggregate3Result,
	seedNetworkOverrides,
	stubChainRegistry,
	stubJsonRpc
} from './stub-chain';

test.use({ viewport: { width: 390, height: 844 } });

const STUB = 'https://stub-rpc.test/rpc';

/** Every node answers, and the account holds nothing anywhere. */
function holdingNothing(method: string, params: unknown[], url: string): unknown {
	const chainId = Number(/\/rpc\/(\d+)/.exec(url)?.[1]);
	if (method === 'eth_chainId') return '0x' + chainId.toString(16);
	if (method === 'eth_blockNumber') return '0x10';
	if (method === 'eth_getBalance') return '0x0';
	if (method === 'eth_getLogs') return [];
	if (method === 'eth_getBlockByNumber')
		return { number: '0x10', timestamp: '0x66000000', baseFeePerGas: '0x3b9aca00' };
	if (method === 'eth_call') {
		const call = params[0] as { data?: string } | undefined;
		const n = call?.data ? aggregate3CallCount(call.data) : 0;
		const data = '0x' + abiWord(0) + abiWord(0) + abiWord(0) + abiWord(0) + abiWord(0);
		return encodeAggregate3Result(Array.from({ length: n }, () => ({ success: true, data })));
	}
	return undefined;
}

/**
 * Requests to `pattern` are held and never answered while `on()` — a node
 * that takes the connection and says nothing. The page's own deadline is
 * what ends each one.
 */
async function neverAnswer(page: Page, pattern: RegExp, on: () => boolean): Promise<void> {
	await page.route(pattern, (route: Route) => {
		if (!on()) return route.fallback();
		// Held: neither fulfilled nor aborted. Playwright drops it with the page.
		return new Promise<void>(() => {});
	});
}

async function openHome(page: Page): Promise<void> {
	await page.goto('/en/wallet');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await seedNetworkOverrides(
		page,
		CHAINS.map((c) => ({ chainId: c.chainId, rpcURL: `${STUB}/${c.chainId}` }))
	);
	await page.reload();
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
}

test.beforeEach(async ({ page }) => {
	await seedSignedIn(page);
	await denyOffOrigin(page);
	await stubChainRegistry(page, {});
	await stubJsonRpc(page, /stub-rpc\.test\/rpc\/(\d+)/, holdingNothing);
});

test('r12-2: three networks that never answer are said — never a Live zero', async ({ page }) => {
	test.setTimeout(120_000);
	// Soneium, Unichain, Celo: held from the start. Registered last, so they win.
	await neverAnswer(page, /stub-rpc\.test\/rpc\/(1868|130|42220)$/, () => true);
	await openHome(page);
	const line = page.getByRole('button', { name: "Can't reach 3 networks right now" });
	await expect(line).toBeVisible({ timeout: 60_000 });
	await expect(page.getByText(en('home.liveIndicator'), { exact: true })).toHaveCount(0);
});

test('r12-3: a network that stops answering in a warm session is named without a reload', async ({
	page
}) => {
	test.setTimeout(150_000);
	let down = false;
	await neverAnswer(page, /stub-rpc\.test\/rpc\/1$/, () => down);
	await openHome(page);
	// Everything answered: a live zero, honestly.
	await expect(page.getByText(en('home.liveIndicator'), { exact: true })).toBeVisible({
		timeout: 30_000
	});

	// Ethereum stops answering. Nothing here reloads, focuses or pulls.
	down = true;
	await expect(page.getByText("Can't reach Ethereum right now", { exact: true })).toBeVisible({
		timeout: 100_000
	});
	await expect(page.getByText(en('home.liveIndicator'), { exact: true })).toHaveCount(0);
});

/**
 * r13-4: a warm outage where EVERY node of a network hangs (the round's
 * Soneium, Unichain and Celo: ~25 endpoints, thirdweb's among them, all
 * taking the connection and saying nothing — none refusing). The pool used
 * to sweep each call's whole list at 8 s a node before the network was out
 * of reach, and the home named it 187 s and ~100 hung requests after the
 * first. Now three silent nodes put it out of reach, the home names it at
 * once, and the reads after it probe two nodes at a time.
 */
test('r13-4: a network whose every node hangs in a warm session is named in time, not hammered', async ({
	page
}) => {
	test.setTimeout(150_000);
	let down = false;
	let firstHung = 0;
	let hung = 0;
	// Ethereum's own stub, and every off-origin JSON-RPC node (only
	// Ethereum's are asked: every other network answers from its stub).
	await page.route(
		(url) =>
			/stub-rpc\.test\/rpc\/1$/.test(url.href) ||
			!/stub-rpc\.test|localhost|127\.0\.0\.1/.test(url.href),
		(route: Route) => {
			const request = route.request();
			const rpc = request.method() === 'POST' && (request.postData() ?? '').includes('jsonrpc');
			if (!down || !rpc) return route.fallback();
			hung += 1;
			if (firstHung === 0) firstHung = Date.now();
			return new Promise<void>(() => {});
		}
	);
	await openHome(page);
	await expect(page.getByText(en('home.liveIndicator'), { exact: true })).toBeVisible({
		timeout: 30_000
	});

	down = true;
	await expect(page.getByText("Can't reach Ethereum right now", { exact: true })).toBeVisible({
		timeout: 120_000
	});
	const named = Date.now();
	expect(firstHung).toBeGreaterThan(0);
	// About as fast as cold (≤ ~25 s from the first hung read; slack for CI).
	expect(named - firstHung).toBeLessThan(35_000);
	// Not hammered: a handful of nodes asked, not every node three times over.
	expect(hung).toBeLessThan(25);
	await expect(page.getByText(en('home.liveIndicator'), { exact: true })).toHaveCount(0);
});
