/**
 * A token added by contract address (spec 028 T445 — SC-405).
 *
 * Type an address the chain knows, and the chain's OWN answer — name, symbol,
 * decimals — is what the card shows and what gets stored. Then the token is
 * where every other asset is, with the balance the chain reports, and it is
 * still there after a reload: `vela.customTokens` is a record in the KV, not
 * a screen's memory.
 *
 * The rules on the way are `manage_tokens`' (spec 017): validity, the
 * per-chain probe, the `!name || !symbol` admission, the dedupe read at save
 * time, and the cache invalidation that makes the balance list look again.
 * The shell here only typed and tapped.
 */
import { expect, test, type Page } from '@playwright/test';
import { CHAINS } from '../src/lib/services/chains';
import { en, seedSignedIn } from './live-helpers';
import { denyOffOrigin, seedNetworkOverrides, stubChainRegistry, stubJsonRpc } from './stub-chain';
import {
	abiString,
	answerAggregate3,
	isAggregate3,
	MULTICALL_SEL as SEL,
	roundData,
	word
} from './stub-multicall';

test.use({ viewport: { width: 390, height: 844 } });
test.setTimeout(120_000);

const STUB = 'https://stub-rpc.test/rpc';
const COFFEE = '0x' + 'c0ffee'.repeat(6) + 'c0ff';
const ONE_AND_A_HALF_ETH = 1_500_000_000_000_000_000n;
const FORTY_TWO_COFFEE = 42_000_000_000_000_000_000n;
const ETH_USD_8DP = 3000n * 100_000_000n;

/**
 * Ethereum knows one ERC-20 at COFFEE and nothing else. Every other chain
 * answers the probe with failure — which is what "not on this network" looks
 * like from a contract that was never deployed there.
 */
async function stubChain(page: Page): Promise<void> {
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
		if (method === 'eth_getLogs') return [];
		if (method === 'eth_call') {
			const call = params[0] as { data?: string } | undefined;
			// A plain call (the Safe's `nonce()`, a lone `balanceOf`) expects one
			// word, not an aggregate3 envelope — send-lands answers it with zero.
			if (!call?.data || !isAggregate3(call.data)) return '0x' + word(0);
			return answerAggregate3(call.data, (inner) => {
				if (chainId !== 1) return undefined;
				const isCoffee = inner.target === COFFEE.toLowerCase();
				switch (inner.selector) {
					case SEL.getEthBalance:
						return { success: true, data: '0x' + word(ONE_AND_A_HALF_ETH) };
					case SEL.name:
						return isCoffee ? { success: true, data: abiString('Coffee Token') } : undefined;
					case SEL.symbol:
						return isCoffee ? { success: true, data: abiString('COFFEE') } : undefined;
					case SEL.decimals:
						return isCoffee ? { success: true, data: '0x' + word(18) } : undefined;
					case SEL.balanceOf:
						return isCoffee ? { success: true, data: '0x' + word(FORTY_TWO_COFFEE) } : undefined;
					case SEL.latestRoundData:
						return { success: true, data: roundData(ETH_USD_8DP) };
					default:
						return undefined;
				}
			});
		}
		return undefined;
	});
}

async function openHome(page: Page): Promise<void> {
	await seedSignedIn(page);
	await stubChain(page);
	await page.goto('/en/wallet');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await seedNetworkOverrides(
		page,
		CHAINS.map((c) => ({ chainId: c.chainId, rpcURL: `${STUB}/${c.chainId}` }))
	);
	await page.reload();
	await expect(page.getByText('$4,500', { exact: true })).toBeVisible({ timeout: 25_000 });
}

/**
 * The assets screen, from the home's section header. Both section headers
 * carry an "All" action; the assets one is the second. The pushed screen has
 * its own h1, which is what proves the tap navigated rather than re-rendered.
 */
async function openAssets(page: Page): Promise<void> {
	// Several controls on the home say "All" (both section actions and the
	// network filter), so the tap is scoped to the Assets header itself.
	await page
		.getByRole('heading', { level: 2, name: en('assets.sectionTitle') })
		.locator('..')
		.getByRole('button', { name: en('history.filterAll') })
		.click();
	await expect(
		page.getByRole('heading', { level: 1, name: en('assets.sectionTitle') }),
		async () => `headings now: ${(await page.locator('h1, h2').allTextContents()).join(' / ')}`
	).toBeVisible({ timeout: 20_000 });
	await expect(page.getByText(en('assets.addByAddress'))).toBeVisible({ timeout: 20_000 });
}

test("a token added by address carries the chain's identity, is listed, and survives a reload", async ({
	page
}) => {
	await openHome(page);
	await openAssets(page);
	// Before: the held asset is ETH, and nothing is called COFFEE.
	await expect(page.getByText('COFFEE', { exact: true })).toHaveCount(0);

	await page.getByText(en('assets.addByAddress')).click();
	const field = page.getByRole('textbox', { name: en('addToken.tokenAddressLabel') });
	await expect(field).toBeVisible();
	await field.fill(COFFEE);

	// The card is the chain's answer, verbatim — not a name anyone typed.
	await expect(page.getByText('Coffee Token', { exact: true })).toBeVisible({ timeout: 20_000 });
	await expect(page.getByText(/COFFEE · .* 18 · Ethereum/)).toBeVisible();

	const add = page.getByRole('button', { name: en('addToken.addToWalletBtn') });
	await expect(add).toBeEnabled();
	await add.click();
	// The core's dedupe verdict, back on the same card.
	await expect(page.getByText(en('addToken.tokenAdded'), { exact: true })).toBeVisible({
		timeout: 20_000
	});

	// Close the sheet: the list behind it has been told to look again.
	await page.getByRole('button', { name: en('componentsUi.identiconViewer.close') }).click();
	await expect(page.getByText('COFFEE', { exact: true }).first()).toBeVisible({ timeout: 25_000 });
	await expect(page.getByText('42', { exact: true }).first()).toBeVisible();

	// Reload: the record, not the screen, is what remembers.
	await page.reload();
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await openAssets(page);
	await expect(page.getByText('COFFEE', { exact: true }).first()).toBeVisible({ timeout: 25_000 });
});

test('an address no chain knows is said to be unknown, and nothing is added', async ({ page }) => {
	await openHome(page);
	await openAssets(page);
	await page.getByText(en('assets.addByAddress')).click();
	await page
		.getByRole('textbox', { name: en('addToken.tokenAddressLabel') })
		.fill('0x' + 'de'.repeat(20));

	await expect(page.getByText(en('addToken.notFoundMessage'))).toBeVisible({ timeout: 20_000 });
	await expect(page.getByRole('button', { name: en('addToken.addToWalletBtn') })).toBeDisabled();
});
