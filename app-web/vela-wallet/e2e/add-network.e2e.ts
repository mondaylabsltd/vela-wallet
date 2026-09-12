/**
 * Adding a custom network, end to end (spec 028 Phase 8 — the e2e spec 024's
 * SC-001 named and did not have).
 *
 * The wizard is the core's (`network_admin`): the search index answers the
 * query, the chain document names the RPCs, the RPC race picks one, the
 * eleven required contracts and the P-256 precompile are probed on it, and
 * only a compatible verdict offers the add. Every one of those reads is
 * stubbed here, so the test drives the whole ladder without a network and
 * asserts what a person sees: the verdict, the row, its survival of a reload,
 * and its removal.
 */
import { expect, test, type Page } from '@playwright/test';
import { en, seedSignedIn } from './live-helpers';
import { denyOffOrigin, stubChainRegistry, stubJsonRpc } from './stub-chain';
import { aggregate3CallCount, encodeAggregate3Result, abiWord } from './stub-chain';
import { isAggregate3 } from './stub-multicall';

test.use({ viewport: { width: 390, height: 844 } });
test.setTimeout(120_000);

const STUB = 'https://stub-rpc.test/rpc';
const EXPLORER = 'https://explorer.op-stub.test';
// Not a built-in chain: the wizard refuses one already in the registry
// (`AlreadyAdded`), which is what an earlier draft of this test asked for.
const CHAIN_ID = 59144;
const NAME = 'Linea';

async function stubEverything(page: Page): Promise<void> {
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
		},
		[CHAIN_ID]: {
			chainId: CHAIN_ID,
			name: NAME,
			shortName: 'linea',
			nativeCurrency: { name: 'Ether', symbol: 'ETH', decimals: 18 },
			stables: [],
			wrappedNativeToken: null,
			dex: null,
			rpc: [`${STUB}/${CHAIN_ID}`],
			explorers: [{ name: 'Explorer', url: EXPLORER }]
		}
	});
	// The search index the wizard ranks (registered after the registry stub, so
	// it wins): one entry, the chain under test.
	await page.route(/\/index\/fuse-chains\.json$/, (route) =>
		route.fulfill({
			contentType: 'application/json',
			body: JSON.stringify({
				data: [
					{
						chainId: CHAIN_ID,
						name: NAME,
						shortName: 'linea',
						nativeCurrencySymbol: 'ETH',
						hasLogo: false
					}
				]
			})
		})
	);
	// The explorer's liveness probe is a `no-cors` GET; answer it.
	await page.route(`${EXPLORER}/**`, (route) => route.fulfill({ status: 200, body: '' }));
	await page.route(EXPLORER, (route) => route.fulfill({ status: 200, body: '' }));
	await stubJsonRpc(page, /stub-rpc\.test\/rpc\/(\d+)/, (method, params, url) => {
		const chainId = Number(/\/rpc\/(\d+)/.exec(url)?.[1]);
		if (method === 'eth_chainId') return '0x' + chainId.toString(16);
		if (method === 'eth_blockNumber') return '0x10';
		if (method === 'eth_getLogs') return [];
		// Every required contract is deployed.
		if (method === 'eth_getCode') return '0x6001';
		if (method === 'eth_call') {
			const call = params[0] as { data?: string } | undefined;
			// Only a real aggregate3 envelope has a count to read; the P-256 probe
			// is a plain call and decoding it as one is nonsense.
			const n = call?.data && isAggregate3(call.data) ? aggregate3CallCount(call.data) : 0;
			if (n > 0) {
				const data = '0x' + abiWord(0) + abiWord(0) + abiWord(0) + abiWord(0) + abiWord(0);
				return encodeAggregate3Result(Array.from({ length: n }, () => ({ success: true, data })));
			}
			// The P-256 probe: a 32-byte word whose value is exactly one.
			return '0x' + abiWord(1);
		}
		return undefined;
	});
}

async function openNetworks(page: Page): Promise<void> {
	await page.goto('/en/settings');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await page.getByText(en('settings.sections.advanced'), { exact: true }).click();
	await page.getByText(en('settings.advanced.networksTitle'), { exact: true }).click();
}

test.beforeEach(async ({ page }) => {
	await stubEverything(page);
});

test('search → verdict → add → listed → survives a reload → removed', async ({ page }) => {
	await page.goto('/en/settings');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await page.getByText(en('settings.sections.advanced'), { exact: true }).click();
	await page.getByText(en('settings.advanced.addNetworkTitle'), { exact: true }).click();

	// The query: a chain id. The index answers; the suggestion is the chain.
	await page
		.getByPlaceholder(en('settingsModals.addNetwork.searchPlaceholder'))
		.fill(String(CHAIN_ID));
	await page
		.getByRole('button', { name: new RegExp(NAME) })
		.first()
		.click();

	// The ladder: chain document, RPC race, eleven contracts, the precompile —
	// and the verdict a person reads.
	await expect(
		page.getByText(en('settingsModals.addNetwork.compatible'), { exact: true })
	).toBeVisible({ timeout: 30_000 });
	await page.getByRole('button', { name: en('settingsModals.addNetwork.addNetworkBtn') }).click();

	// Listed as a custom network, from the core's registry.
	await openNetworks(page);
	const row = page.getByRole('button', { name: new RegExp(NAME) }).first();
	await expect(row).toBeVisible();
	await expect(
		page.getByText(en('settings.networks.custom'), { exact: true }).first()
	).toBeVisible();

	// Persisted: the core wrote it, and a reload reads it back.
	await page.reload();
	await openNetworks(page);
	await expect(page.getByRole('button', { name: new RegExp(NAME) }).first()).toBeVisible();

	// Removed from its own row's control (named for what it does, not the add
	// label it used to borrow); a reload does not bring it back.
	await page.getByRole('button', { name: en('settingsModals.network.removeTitle') }).click();
	await expect(page.getByRole('button', { name: new RegExp(NAME) })).toHaveCount(0);
	await page.reload();
	await openNetworks(page);
	await expect(page.getByRole('button', { name: new RegExp(NAME) })).toHaveCount(0);
});
