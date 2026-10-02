/**
 * A wallet home that tells the truth (spec 025 T126 — SC-101), hermetically.
 *
 * Every chain's RPC is a user override pointing at the stub host (so the
 * pool's fastest-endpoint race is deterministic), the chain registry knows
 * only Ethereum (native coin, no stables, no DEX), and the stub answers every
 * multicall with N results shaped as Chainlink `latestRoundData` whose first
 * word doubles as the balance: `decU256` reads word 0 (the balance),
 * `decChainlinkUsd` reads word 1 (the price). One shape, both decoders — so
 * 1.5 ETH at $3,000 renders as $4,500 with zero live traffic.
 */
import { expect, test } from '@playwright/test';
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
const ONE_AND_A_HALF_ETH = 1_500_000_000_000_000_000n;
const THREE_THOUSAND_USD_8DP = 3000n * 100_000_000n;

test.beforeEach(async ({ page }) => {
	await seedSignedIn(page);
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
		if (method === 'eth_call') {
			const call = params[0] as { data?: string } | undefined;
			const n = call?.data ? aggregate3CallCount(call.data) : 0;
			const balance = chainId === 1 ? ONE_AND_A_HALF_ETH : 0n;
			const data =
				'0x' +
				abiWord(balance) +
				abiWord(THREE_THOUSAND_USD_8DP) +
				abiWord(0) +
				abiWord(0) +
				abiWord(0);
			return encodeAggregate3Result(Array.from({ length: n }, () => ({ success: true, data })));
		}
		return undefined;
	});
});

async function openHome(page: import('@playwright/test').Page): Promise<void> {
	await page.goto('/en/wallet');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	// Overrides are read at pool-config time; seed them, then let the core
	// re-hydrate for the account through a reload.
	await seedNetworkOverrides(
		page,
		CHAINS.map((c) => ({ chainId: c.chainId, rpcURL: `${STUB}/${c.chainId}` }))
	);
	await page.reload();
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
}

test("balances are the chain's, not a fixture's (SC-101)", async ({ page }) => {
	await openHome(page);
	// The held asset, from the stubbed multicall.
	await expect(page.getByText('ETH', { exact: true }).first()).toBeVisible({ timeout: 20_000 });
	await expect(page.getByText('1.5', { exact: true }).first()).toBeVisible();
	// The total is the core's aggregation at the on-chain Chainlink price.
	await expect(page.getByText('$4,500', { exact: true })).toBeVisible();
	// The spec-015 fixture identities never appear on a live home.
	await expect(page.getByText('$1,383')).toHaveCount(0);
	await expect(page.getByText('大表哥')).toHaveCount(0);
});

test("tap-to-hide masks every figure and survives a reload (privacy is the core's)", async ({
	page
}) => {
	await openHome(page);
	await expect(page.getByText('$4,500', { exact: true })).toBeVisible({ timeout: 20_000 });

	// The figure itself is the tap-to-hide target, named by the corpus.
	await page.getByRole('button', { name: en('home.a11yHideBalance') }).click();
	await expect(page.getByText('••••••').first()).toBeVisible();
	await expect(page.getByText('$4,500')).toHaveCount(0);

	await page.reload();
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await expect(page.getByText('••••••').first()).toBeVisible({ timeout: 20_000 });
});

test('an unreachable chain is SAID, never shown as a confident zero (T152 finding)', async ({
	page
}) => {
	// Ethereum's only endpoint answers 500 to everything: the pool exhausts it,
	// the core marks the chain failed, and — since it is not rate-limited — it
	// is a banner chain. Registered last so it wins over the JSON stub.
	await page.route(/stub-rpc\.test\/rpc\/1$/, (route) =>
		route.fulfill({ status: 500, body: 'no' })
	);
	await openHome(page);
	// Spec 092 (F08): said without "RPC" — a fact, not an alarm.
	await expect(page.getByText("Can't reach Ethereum right now", { exact: true })).toBeVisible({
		timeout: 20_000
	});
	// Not "Live · listening for payments": a partial zero is not a live zero.
	await expect(page.getByText(en('home.liveIndicator'), { exact: true })).toHaveCount(0);
});

test('an unreachable chain’s status line opens the list, its row the RPC fix, and a working URL restores it (spec 028 Phase 8, 092)', async ({
	page
}) => {
	// The same dead endpoint as above; the JSON stub still answers any other
	// path under /rpc/<n>, which is what makes a "working URL" possible here.
	await page.route(/stub-rpc\.test\/rpc\/1$/, (route) =>
		route.fulfill({ status: 500, body: 'no' })
	);
	await openHome(page);
	await page.getByRole('button', { name: "Can't reach Ethereum right now" }).click();

	// The list (spec 092), its one row, and that row's fix.
	const sheet = page.getByRole('dialog');
	const list = sheet.getByTestId('unreachable-list');
	await expect(list.getByText('Ethereum', { exact: true })).toBeVisible();
	await list.getByRole('button', { name: en('assets.rpcFix') }).click();

	// SR2, for THIS chain, with its stored URL in the field.
	await expect(sheet.getByText(en('assets.rpcFixTitle')).first()).toBeVisible();
	await expect(sheet.getByText('Ethereum', { exact: true })).toBeVisible();
	await expect(sheet.getByText(en('assets.rpcFixWarning'))).toBeVisible();

	// Save a URL that answers. The core probes it (chain id 1 — no mismatch),
	// stores the override, and the sheet turns restored.
	const field = sheet.getByRole('textbox');
	await field.fill(`${STUB}/1?fixed`);
	await sheet.getByRole('button', { name: en('assets.rpcFixSaveBtn') }).click();
	await expect(sheet.getByText(en('assets.rpcFixRestored'))).toBeVisible({ timeout: 20_000 });

	// Done tells the balance core the chain is back and returns to the list,
	// which now says every network answers; the status line goes.
	await sheet.getByRole('button', { name: en('common.done') }).click();
	await expect(sheet.getByText(en('assets.unreachableNone')).first()).toBeVisible();
	await expect(list.getByRole('listitem')).toHaveCount(0);
	await sheet.getByRole('button', { name: en('componentsUi.identiconViewer.close') }).first().click();
	await expect(page.getByRole('dialog')).toHaveCount(0);
	await expect(page.getByText("Can't reach Ethereum right now", { exact: true })).toHaveCount(0, {
		timeout: 20_000
	});
	await expect(page.getByText('$4,500', { exact: true })).toBeVisible({ timeout: 20_000 });
});

test('every unreachable network is listed with what was last read there, and one that comes back leaves (spec 092)', async ({
	page
}, testInfo) => {
	// BNB Chain and Polygon are down from the start: never read.
	let bnbDown = true;
	await page.route(/stub-rpc\.test\/rpc\/(56|137)$/, (route) =>
		route.request().url().endsWith('/56') && !bnbDown
			? route.fallback()
			: route.fulfill({ status: 500, body: 'no' })
	);
	await openHome(page);
	await expect(page.getByText('$4,500', { exact: true })).toBeVisible({ timeout: 20_000 });
	const line = page.getByRole('button', { name: "Can't reach 2 networks right now" });
	await expect(line).toBeVisible({ timeout: 30_000 });
	await page.screenshot({ path: testInfo.outputPath('092-home-notice.png') });

	// Ethereum goes quiet after it was read holding 1.5 ETH. Opening the list
	// reads every chain again, so it joins — first, with what it last held.
	await page.route(/stub-rpc\.test\/rpc\/1$/, (route) =>
		route.fulfill({ status: 500, body: 'no' })
	);
	await line.click();
	const sheet = page.getByRole('dialog');
	const rows = sheet.getByTestId('unreachable-list').getByRole('listitem');
	await expect(sheet.getByText("Can't reach 3 networks right now").first()).toBeVisible({
		timeout: 30_000
	});
	await expect(sheet.getByText(en('assets.unreachableBody'))).toBeVisible();
	await expect(rows).toHaveCount(3);
	await expect(rows.nth(0)).toContainText('Ethereum');
	await expect(rows.nth(0)).toContainText('Last seen $4,500');
	await expect(rows.nth(1)).toContainText('BNB Chain');
	await expect(rows.nth(1)).toContainText(en('assets.notReadYet'));
	await expect(rows.nth(2)).toContainText('Polygon');
	await page.screenshot({ path: testInfo.outputPath('092-list.png') });

	// BNB Chain comes back: it leaves the open list and the count.
	bnbDown = false;
	await expect(rows).toHaveCount(2, { timeout: 30_000 });
	await expect(sheet.getByText("Can't reach 2 networks right now").first()).toBeVisible();
	await expect(sheet.getByTestId('unreachable-list')).not.toContainText('BNB Chain');
});
