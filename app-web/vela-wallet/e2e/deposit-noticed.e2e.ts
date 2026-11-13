/**
 * A deposit is noticed without a manual refresh (spec 025 T136 — SC-103),
 * hermetically.
 *
 * Same stub as the home test: every chain's RPC is a seeded override to the
 * stub host, the registry knows only Ethereum, and one `latestRoundData`-
 * shaped answer serves both decoders. The balance the stub reports is
 * MUTABLE: while the receive screen is open (the watcher's lifetime — the
 * core polls every 3s at first), it rises from 1.5 to 2.5 ETH. The
 * `receive_watch` core sees the delta, signals, and the shell refreshes the
 * balances; back on the home the total reads $7,500 with no reload and no
 * tap. The stub's call count is the clock: one round for the baseline, then
 * two more (the poll that notices, the refresh it asks for).
 */
import { expect, test } from '@playwright/test';
import { CHAINS } from '../src/lib/services/chains';
import { en, seedSignedIn, TEST_ACCOUNT_SHORT } from './live-helpers';
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
test.setTimeout(90_000);

const STUB = 'https://stub-rpc.test/rpc';
const ETH = 1_000_000_000_000_000_000n;
const PRICE_8DP = 3000n * 100_000_000n;

test('a deposit landing while the receive screen is open updates the total by itself', async ({
	page
}) => {
	// Mutable on purpose: the test raises it mid-flight.
	let balanceWei = (ETH * 3n) / 2n; // 1.5 ETH
	let mainnetCalls = 0;

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
			if (chainId === 1) mainnetCalls += 1;
			const call = params[0] as { data?: string } | undefined;
			const n = call?.data ? aggregate3CallCount(call.data) : 0;
			const balance = chainId === 1 ? balanceWei : 0n;
			const data =
				'0x' + abiWord(balance) + abiWord(PRICE_8DP) + abiWord(0) + abiWord(0) + abiWord(0);
			return encodeAggregate3Result(Array.from({ length: n }, () => ({ success: true, data })));
		}
		// eth_getLogs (the receipt scan) and anything else: nothing to report.
		if (method === 'eth_getLogs') return [];
		return undefined;
	});

	await page.goto('/en/wallet');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await seedNetworkOverrides(
		page,
		CHAINS.map((c) => ({ chainId: c.chainId, rpcURL: `${STUB}/${c.chainId}` }))
	);
	await page.reload();
	await expect(page.getByText('$4,500', { exact: true })).toBeVisible({ timeout: 20_000 });

	// Open Receive: the watcher's lifetime begins with THIS screen.
	const beforeOpen = mainnetCalls;
	await page
		.getByRole('button', { name: en('componentsUi.dock.receive') })
		.first()
		.click();
	await expect(page.getByRole('heading', { name: en('receive.title') }).first()).toBeVisible();
	// Every network row carries the person's real (derived) address — one row
	// per network of the space in front, the mainnet space a device starts in
	// (spec 105), and of them the twenty shown by default (owner decision
	// 2026-10-29).
	await expect(page.getByText(TEST_ACCOUNT_SHORT, { exact: true })).toHaveCount(
		CHAINS.filter((c) => !c.testnet && c.shownByDefault !== false).length
	);
	// The watcher takes its baseline.
	await expect.poll(() => mainnetCalls, { timeout: 15_000 }).toBeGreaterThan(beforeOpen);

	// The chain moves: a deposit lands.
	const beforeDeposit = mainnetCalls;
	balanceWei = (ETH * 5n) / 2n; // 2.5 ETH
	// The next poll notices; the refresh it triggers is the round after.
	await expect
		.poll(() => mainnetCalls, { timeout: 30_000 })
		.toBeGreaterThanOrEqual(beforeDeposit + 2);

	// Back on the home the total has followed the chain, unprompted.
	await page.goBack();
	await expect(page.getByText('$7,500', { exact: true })).toBeVisible({ timeout: 15_000 });
});
