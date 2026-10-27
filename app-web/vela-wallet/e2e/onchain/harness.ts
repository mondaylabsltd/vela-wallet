/**
 * The multisig fork harness, from a test: a browser profile pointed at the
 * local fork, genesis service and relay (`scripts/onchain/multisig-fork/up.sh`),
 * signed in to the parallel space's fixture accounts — the real app, with only
 * the passkey faked by committed throwaway keys.
 */
import type { BrowserContext, Page } from '@playwright/test';

export const FORK_RPC = 'http://127.0.0.1:8545';
export const GENESIS = 'http://127.0.0.1:8787';
export const RELAY = 'http://127.0.0.1:8790';

export const FIXTURES = {
	one: '0xD400866e00B055B20752a826CD5C89b811de130b',
	two: '0x031d7D57c99CAF891e1C250554691Fd12D84772b',
	three: '0x58cd0ce6A27099220543b31710d7860d75Ba1d3d'
} as const;

export async function rpc<T>(method: string, params: unknown[]): Promise<T> {
	const response = await fetch(FORK_RPC, {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params })
	});
	const body = (await response.json()) as { result?: T; error?: { message: string } };
	if (body.error) throw new Error(body.error.message);
	return body.result as T;
}

export async function setBalance(address: string, wei: bigint): Promise<void> {
	await rpc('anvil_setBalance', [address, `0x${wei.toString(16)}`]);
}

export async function balanceOf(address: string): Promise<bigint> {
	return BigInt(await rpc<string>('eth_getBalance', [address, 'latest']));
}

/** Point the profile at the harness before any page runs. */
export async function useHarness(context: BrowserContext): Promise<void> {
	await context.addInitScript(
		([genesis, relay]) => {
			localStorage.setItem('vela.intro.seen', String(Date.now()));
			localStorage.setItem('vela.genesisServiceURL', genesis);
			localStorage.setItem('vela.serviceEndpoints', JSON.stringify({ bundlerServiceURL: relay }));
		},
		[GENESIS, RELAY]
	);
}

async function seedNetwork(page: Page): Promise<void> {
	await page.evaluate((rpcURL) => {
		return new Promise<void>((resolve, reject) => {
			const open = indexedDB.open('vela', 1);
			open.onupgradeneeded = () => open.result.createObjectStore('kv');
			open.onerror = () => reject(open.error);
			open.onsuccess = () => {
				const tx = open.result.transaction('kv', 'readwrite');
				tx.objectStore('kv').put(JSON.stringify([{ chainId: 100, rpcURL }]), 'vela.networkConfig');
				tx.oncomplete = () => resolve();
				tx.onerror = () => reject(tx.error);
			};
		});
	}, FORK_RPC);
}

/**
 * Enter the parallel space and land on the wallet. `keep` names the fixture
 * accounts this "device" holds (the others are removed), so two profiles can
 * play two members.
 */
export async function enterParallel(page: Page, keep?: string[]): Promise<void> {
	await page.goto('/en/parallel');
	await seedNetwork(page);
	// The list renders once the core is aboard: the buttons are wired by then.
	await page.getByText('Parallel Three').waitFor();
	await page.getByRole('button', { name: /Enter \(seed fixture wallet\)/ }).click();
	await page.waitForURL(/\/en\/wallet/);
	if (keep) {
		await page.evaluate((wanted) => {
			const rows = JSON.parse(localStorage.getItem('vela.accounts') ?? '[]') as {
				address: string;
			}[];
			const kept = rows.filter((r) =>
				wanted.some((w) => w.toLowerCase() === r.address.toLowerCase())
			);
			localStorage.setItem('vela.accounts', JSON.stringify(kept));
			localStorage.setItem('vela.activeAccountIndex', '0');
		}, keep);
		await page.reload();
	}
}
