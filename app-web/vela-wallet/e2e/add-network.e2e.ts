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
import { en, seedSignedIn, zh } from './live-helpers';
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

/**
 * What the chain under test answers the check with. The default is a chain
 * that passes; the two refusals are one fact each: no contract code, or a
 * P-256 probe that does not answer `1`.
 */
interface ChainFacts {
	/** Every required contract has code. */
	contracts: boolean;
	/** The P-256 verifier (EIP-7951 / RIP-7212 at 0x100) answers. */
	p256: boolean;
}

async function stubEverything(
	page: Page,
	facts: ChainFacts = { contracts: true, p256: true }
): Promise<void> {
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
		// Every required contract is deployed — unless this chain has none.
		if (method === 'eth_getCode') return facts.contracts ? '0x6001' : '0x';
		if (method === 'eth_call') {
			const call = params[0] as { data?: string } | undefined;
			// Only a real aggregate3 envelope has a count to read; the P-256 probe
			// is a plain call and decoding it as one is nonsense.
			const n = call?.data && isAggregate3(call.data) ? aggregate3CallCount(call.data) : 0;
			if (n > 0) {
				const data = '0x' + abiWord(0) + abiWord(0) + abiWord(0) + abiWord(0) + abiWord(0);
				return encodeAggregate3Result(Array.from({ length: n }, () => ({ success: true, data })));
			}
			// The P-256 probe: a 32-byte word whose value is exactly one — or,
			// on a chain with no verifier at 0x100, nothing at all.
			return facts.p256 ? '0x' + abiWord(1) : '0x';
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

/** Search for the chain under test and pick it: the wizard runs its ladder. */
async function checkChain(page: Page, locale = 'en'): Promise<void> {
	const t = (key: string) => (locale === 'en' ? en(key) : zh(key));
	await page.goto(`/${locale}/settings`);
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await page.getByText(t('settings.sections.advanced'), { exact: true }).click();
	await page.getByText(t('settings.advanced.addNetworkTitle'), { exact: true }).click();
	await page
		.getByPlaceholder(t('settingsModals.addNetwork.searchPlaceholder'))
		.fill(String(CHAIN_ID));
	await page
		.getByRole('button', { name: new RegExp(NAME) })
		.first()
		.click();
	await expect(
		page.getByText(t('settingsModals.addNetwork.incompatible'), { exact: true })
	).toBeVisible({ timeout: 30_000 });
}

/**
 * A refused network says WHY (the core's `NetCompatibility.blocker`), and the
 * two reasons have opposite next steps. The wizard used to say "contracts are
 * missing" and offer the Chain Setup tool for both — which sent a person to
 * deploy onto a network that can never check a passkey signature, and said
 * nothing about money getting stuck there.
 */
test.describe('a refused network says why', () => {
	test('missing contracts: Chain Setup is offered, opened on this chain', async ({ page }) => {
		await stubEverything(page, { contracts: false, p256: true });
		await checkChain(page);
		await expect(page.getByText(en('settingsModals.addNetwork.incompatibleHint'))).toBeVisible();
		await expect(page.getByText(en('settingsModals.addNetwork.noP256Hint'))).toHaveCount(0);
		const setup = page.getByRole('link', {
			name: en('settingsModals.addNetwork.openChainSetupTool')
		});
		await expect(setup).toBeVisible();
		// The core's link: the page starts on the chain that was checked.
		await expect(setup).toHaveAttribute(
			'href',
			`https://getvela.app/chain-setup?chain=${CHAIN_ID}`
		);
		// No way to add it.
		await expect(
			page.getByRole('button', { name: en('settingsModals.addNetwork.addNetworkBtn') })
		).toHaveCount(0);
	});

	test('no P-256 verifier: the network cannot run Vela wallets — nothing to deploy, no button', async ({
		page
	}) => {
		// Every contract IS deployed here: the verifier alone is missing, and
		// that alone refuses the network.
		await stubEverything(page, { contracts: true, p256: false });
		await checkChain(page);
		const line = page.getByText(en('settingsModals.addNetwork.noP256Hint'));
		await expect(line).toBeVisible();
		await expect(line).toContainText("Don't send money to your Vela address on this network");
		await expect(page.getByText(en('settingsModals.addNetwork.incompatibleHint'))).toHaveCount(0);
		await expect(
			page.getByRole('link', { name: en('settingsModals.addNetwork.openChainSetupTool') })
		).toHaveCount(0);
		await expect(
			page.getByRole('button', { name: en('settingsModals.addNetwork.addNetworkBtn') })
		).toHaveCount(0);
	});

	test('both gone: no verifier wins — deploying the contracts would not make it work', async ({
		page
	}) => {
		await stubEverything(page, { contracts: false, p256: false });
		await checkChain(page, 'zh');
		await expect(page.getByText(zh('settingsModals.addNetwork.noP256Hint'))).toBeVisible();
		await expect(
			page.getByRole('link', { name: zh('settingsModals.addNetwork.openChainSetupTool') })
		).toHaveCount(0);
	});
});

test('search → verdict → add → listed → survives a reload → removed', async ({ page }) => {
	await stubEverything(page);
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
	// label it used to borrow) — after the question it asks first, titled with
	// the network (spec 072); a reload does not bring it back.
	await page.getByRole('button', { name: en('settingsModals.network.removeTitle') }).click();
	const question = page.getByRole('dialog', { name: new RegExp(NAME) });
	await expect(question).toContainText(en('settingsModals.network.removeBody'));
	await question
		.getByRole('button', { name: en('settingsModals.network.removeConfirm'), exact: true })
		.click();
	await expect(page.getByRole('button', { name: new RegExp(NAME) })).toHaveCount(0);
	await page.reload();
	await openNetworks(page);
	await expect(page.getByRole('button', { name: new RegExp(NAME) })).toHaveCount(0);
});
