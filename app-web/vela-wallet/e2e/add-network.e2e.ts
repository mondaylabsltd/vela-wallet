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

/** The P-256 verifier's address (EIP-7951 / RIP-7212): `network_admin::P256_PRECOMPILE`. */
const P256_PRECOMPILE = '0x0000000000000000000000000000000000000100';

/**
 * What the chain under test answers the check with. The default is a chain
 * that passes; the two refusals are one fact each: no contract code, or a
 * P-256 probe that does not answer `1`.
 */
interface ChainFacts {
	/** Every required contract has code. */
	contracts: boolean;
	/**
	 * The P-256 verifier (EIP-7951 / RIP-7212 at 0x100) is there. The core
	 * probes twice — a call that must answer `1`, then code at the address —
	 * and a chain without it fails both.
	 */
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
		if (method === 'eth_getCode') {
			// The verifier's second probe: code AT the precompile's address.
			// A chain with no verifier has none there, whatever else it has.
			const address = String(params[0] ?? '').toLowerCase();
			if (address === P256_PRECOMPILE) return facts.p256 ? '0x6001' : '0x';
			// Every required contract is deployed — unless this chain has none.
			return facts.contracts ? '0x6001' : '0x';
		}
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
/**
 * A refusal draws no RPC field and no "Re-check with this RPC" (PR 3 final
 * note F22, the core's `rpc_field: none`): another endpoint would not change
 * the verdict. The re-check stood here with no field to read.
 */
async function expectNoRpcField(page: Page): Promise<void> {
	await expect(
		page.getByRole('button', { name: en('settingsModals.addNetwork.recheckWithRpc') })
	).toHaveCount(0);
	await expect(
		page.getByRole('textbox', { name: en('settingsModals.addNetwork.customRpcTitle') })
	).toHaveCount(0);
	await expect(
		page.getByRole('textbox', { name: en('settingsModals.network.fieldRpcUrl'), exact: true })
	).toHaveCount(0);
}

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
		await expectNoRpcField(page);
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
		await expectNoRpcField(page);
		// The hint's hex is written "0x100" — and drawn so (the UI face would
		// make it "0×100" if its contextual alternates were left on).
		await expect(line).toContainText('0x100');
		expect(await line.evaluate((el) => getComputedStyle(el).fontVariantLigatures)).toBe(
			'no-contextual'
		);
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

/**
 * A wizard that STOPS says why (PR 3 notes 5, 10 and 18), in the core's own
 * sentence (`NetWizardView.error_key`). Three of its stops had no words here:
 * they read "Incompatible" in the model — and on this page a stop with no
 * network to show drew nothing at all, so tapping a suggestion emptied the
 * list and that was the whole answer.
 */
test.describe('a wizard that stops says why', () => {
	/** The search index, with `entries` beside the chain under test. */
	async function indexAlso(
		page: Page,
		entries: { chainId: number; name: string; shortName: string }[]
	): Promise<void> {
		await page.route(/\/index\/fuse-chains\.json$/, (route) =>
			route.fulfill({
				contentType: 'application/json',
				body: JSON.stringify({
					data: entries.map((entry) => ({
						...entry,
						nativeCurrencySymbol: 'ETH',
						hasLogo: false
					}))
				})
			})
		);
	}

	async function openWizard(page: Page): Promise<void> {
		await page.goto('/en/settings');
		await expect(page.getByText('E2E Wallet').first()).toBeVisible();
		await page.getByText(en('settings.sections.advanced'), { exact: true }).click();
		await page.getByText(en('settings.advanced.addNetworkTitle'), { exact: true }).click();
	}

	async function pick(page: Page, query: string, name: string): Promise<void> {
		await page.getByPlaceholder(en('settingsModals.addNetwork.searchPlaceholder')).fill(query);
		await page
			.getByRole('button', { name: new RegExp(name) })
			.first()
			.click();
	}

	const neverIncompatible = async (page: Page) =>
		expect(
			page.getByText(en('settingsModals.addNetwork.incompatible'), { exact: true })
		).toHaveCount(0);

	test('a network the wallet already has: said under the search', async ({ page }, testInfo) => {
		await stubEverything(page);
		// Base is built in; the index offers it all the same.
		await indexAlso(page, [{ chainId: 8453, name: 'Base', shortName: 'base' }]);
		await openWizard(page);
		await pick(page, '8453', 'Base');
		await expect(page.getByText(en('addToken.errorAlreadyAdded'), { exact: true })).toBeVisible();
		await neverIncompatible(page);
		// The search is still there to try another name with, saying what was asked…
		const search = page.getByPlaceholder(en('settingsModals.addNetwork.searchPlaceholder'));
		await expect(search).toHaveValue('8453');
		await page.screenshot({ path: testInfo.outputPath('stop-already-added.png') });
		// …and typing again takes the sentence away.
		await search.fill('linea');
		await expect(page.getByText(en('addToken.errorAlreadyAdded'), { exact: true })).toHaveCount(0);
	});

	test('a chain the registry has no document for: said under the search', async ({
		page
	}, testInfo) => {
		await stubEverything(page);
		// In the index, and its document answers 404.
		await indexAlso(page, [{ chainId: 424242, name: 'Ghostnet', shortName: 'ghost' }]);
		await openWizard(page);
		await pick(page, '424242', 'Ghostnet');
		await expect(page.getByText(en('addToken.errorChainNotFound'), { exact: true })).toBeVisible({
			timeout: 30_000
		});
		await neverIncompatible(page);
		// The sentence answers what was searched, and the field still says it:
		// it came back from the resolving candidate EMPTY, over a placeholder.
		await expect(
			page.getByPlaceholder(en('settingsModals.addNetwork.searchPlaceholder'))
		).toHaveValue('424242');
		await page.screenshot({ path: testInfo.outputPath('stop-not-found.png') });
	});

	test('no RPC endpoint listed: the sentence, a field to enter one, and the re-check that reads it', async ({
		page
	}, testInfo) => {
		await stubEverything(page);
		// The chain's document, with no RPC in it. Registered last: it wins.
		await page.route(new RegExp(`/chains/eip155-${CHAIN_ID}\\.json$`), (route) =>
			route.fulfill({
				contentType: 'application/json',
				body: JSON.stringify({
					chainId: CHAIN_ID,
					name: NAME,
					shortName: 'linea',
					nativeCurrency: { name: 'Ether', symbol: 'ETH', decimals: 18 },
					rpc: [],
					explorers: []
				})
			})
		);
		await openWizard(page);
		await pick(page, String(CHAIN_ID), NAME);
		await expect(page.getByText(en('settingsModals.addNetwork.noRpcEndpoint'))).toBeVisible({
			timeout: 30_000
		});
		await neverIncompatible(page);
		await page.screenshot({ path: testInfo.outputPath('stop-no-rpc.png') });

		// "Enter one, then re-check." — into a field that is not called
		// "(optional)" under the sentence that asks for it.
		await expect(
			page.getByRole('textbox', { name: en('settingsModals.addNetwork.customRpcTitle') })
		).toHaveCount(0);
		await page
			.getByRole('textbox', { name: en('settingsModals.network.fieldRpcUrl'), exact: true })
			.fill(`${STUB}/${CHAIN_ID}`);
		await page
			.getByRole('button', { name: en('settingsModals.addNetwork.recheckWithRpc') })
			.click();
		await expect(
			page.getByText(en('settingsModals.addNetwork.compatible'), { exact: true })
		).toBeVisible({ timeout: 30_000 });
		await expect(page.getByText(en('settingsModals.addNetwork.noRpcEndpoint'))).toHaveCount(0);
	});
});

/**
 * PR 3 final note F14 — the add-token sheet's network tab draws the same
 * wizard, and its no-RPC stop said "Enter one, then re-check" over nothing to
 * enter it in. It has the field the core gives ("RPC URL") and the re-check
 * that reads it, and they do what Settings' do.
 */
test('the add-token sheet’s network tab: no RPC listed → the field it asks for → re-check → compatible', async ({
	page
}) => {
	await stubEverything(page);
	await page.route(new RegExp(`/chains/eip155-${CHAIN_ID}\\.json$`), (route) =>
		route.fulfill({
			contentType: 'application/json',
			body: JSON.stringify({
				chainId: CHAIN_ID,
				name: NAME,
				shortName: 'linea',
				nativeCurrency: { name: 'Ether', symbol: 'ETH', decimals: 18 },
				rpc: [],
				explorers: []
			})
		})
	);
	await page.goto('/en/wallet');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();

	// Assets → "Add a token by address" → the network tab. (The home's rows
	// are in the server's HTML before the page hydrates: press until the list
	// answers.)
	const byAddress = page.getByRole('button', { name: en('assets.addByAddress') });
	await expect(async () => {
		if (!(await byAddress.isVisible())) {
			// The second "All": the Assets section's (the first is Activity's).
			await page
				.getByRole('button', { name: en('history.filterAll'), exact: true })
				.last()
				.click();
		}
		await expect(byAddress).toBeVisible({ timeout: 1_000 });
	}).toPass({ timeout: 30_000 });
	await byAddress.click();
	const sheet = page.getByRole('dialog');
	await expect(sheet).toBeVisible();
	await sheet.getByText(en('addToken.tabNative'), { exact: true }).click();
	await sheet.getByRole('textbox', { name: en('addToken.netSearchLabel') }).fill(String(CHAIN_ID));
	await sheet
		.getByRole('button', { name: new RegExp(NAME) })
		.first()
		.click();

	// The stop, in the core's sentence — and what it asks for, under it.
	await expect(sheet.getByText(en('settingsModals.addNetwork.noRpcEndpoint'))).toBeVisible({
		timeout: 30_000
	});
	const field = sheet.getByRole('textbox', {
		name: en('settingsModals.network.fieldRpcUrl'),
		exact: true
	});
	await expect(field).toBeVisible();
	await expect(
		sheet.getByRole('textbox', { name: en('settingsModals.addNetwork.customRpcTitle') })
	).toHaveCount(0);
	const recheck = sheet.getByRole('button', {
		name: en('settingsModals.addNetwork.recheckWithRpc')
	});
	await expect(recheck).toBeVisible();

	await field.fill(`${STUB}/${CHAIN_ID}`);
	await recheck.click();
	await expect(sheet.getByText(en('addToken.compatible'), { exact: true })).toBeVisible({
		timeout: 30_000
	});
	await expect(sheet.getByText(en('settingsModals.addNetwork.noRpcEndpoint'))).toHaveCount(0);
	// Compatible: the field is the optional one now, with what was typed in it.
	await expect(
		sheet.getByRole('textbox', { name: en('settingsModals.addNetwork.customRpcTitle') })
	).toHaveValue(`${STUB}/${CHAIN_ID}`);
	await expect(sheet.getByRole('button', { name: en('addToken.addNetworkBtn') })).toBeEnabled();
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
