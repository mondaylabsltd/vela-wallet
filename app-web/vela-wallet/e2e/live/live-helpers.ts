/**
 * Live testnet helpers: a browser whose passkeys are Chrome's virtual
 * authenticator (credentials synced-style, so one key is enough), the steps
 * that create a Vela account the way a person does, and funding from a test
 * key held in the environment (expanded by the shell for `cast`, never on a
 * command line, never committed).
 */
import { execFileSync } from 'node:child_process';
import { expect, type BrowserContext, type Page } from '@playwright/test';

export const ARC_CHAIN = 5042002;
export const ARC_RPC = process.env.ARC_RPC ?? 'https://rpc.testnet.arc.network';
export const ARC_GENESIS = process.env.ARC_GENESIS ?? 'http://127.0.0.1:8788';
/**
 * The PeerConnect relay an outside owner's session goes through: the public
 * one, the app's default, unless LIVE_PEERCONNECT names another (a local
 * `peerconnect-relay`, as the fork harness runs it). The public relay asks
 * each side for a token, which Vela's site issues; LIVE_TOKEN_ENDPOINT points
 * both browsers at a local getvela.app holding the app secret.
 */
const PEERCONNECT = process.env.LIVE_PEERCONNECT ?? 'default';
const TOKEN_ENDPOINT =
	process.env.LIVE_TOKEN_ENDPOINT ?? 'http://127.0.0.1:5191/api/peerconnect-token';
export const FUNDER = process.env.ARC_FUNDER ?? '';
const FUNDER_KEY = process.env.ARC_FUNDER_KEY ?? '';
export const live = FUNDER !== '' && FUNDER_KEY !== '';

export const SHOTS = 'e2e/live/.shots';

export async function shot(page: Page, name: string): Promise<void> {
	await page.screenshot({ path: `${SHOTS}/${name}.png` });
}

/** This profile's services: the chain's genesis service, the relay and its tokens; no splash. */
export async function liveSettings(context: BrowserContext): Promise<void> {
	await context.addInitScript(
		([genesis, relay, tokens]) => {
			localStorage.setItem('vela.intro.seen', String(Date.now()));
			localStorage.setItem('vela.launch.played', String(Date.now()));
			// The multisig service is a service endpoint (Settings → Service
			// Endpoints). Seeded into that record — once, so an address set
			// in Settings during a test stands over a reload; the old key is
			// still read (and moved into the record) for older profiles.
			const endpoints = JSON.parse(localStorage.getItem('vela.serviceEndpoints') ?? '{}');
			if (endpoints.genesisServiceURL === undefined) {
				endpoints.genesisServiceURL = genesis;
				localStorage.setItem('vela.serviceEndpoints', JSON.stringify(endpoints));
			}
			if (relay !== 'default') localStorage.setItem('vela.peerconnectRelay', relay);
			if (tokens !== 'default') localStorage.setItem('vela.peerconnectTokenEndpoint', tokens);
		},
		[ARC_GENESIS, PEERCONNECT, TOKEN_ENDPOINT] as const
	);
}

/** A profile with virtual passkeys and `liveSettings`. */
export async function livePage(context: BrowserContext): Promise<Page> {
	await liveSettings(context);
	const page = await context.newPage();
	const cdp = await context.newCDPSession(page);
	await cdp.send('WebAuthn.enable');
	await cdp.send('WebAuthn.addVirtualAuthenticator', {
		options: {
			protocol: 'ctap2',
			transport: 'internal',
			hasResidentKey: true,
			hasUserVerification: true,
			isUserVerified: true,
			automaticPresenceSimulation: true,
			defaultBackupEligibility: true,
			defaultBackupState: true
		}
	});
	return page;
}

/** Name it, agree, one passkey on this device, create, enter. Returns its address. */
export async function createAccount(page: Page, name: string): Promise<string> {
	await page.getByRole('textbox').first().fill(name);
	const boxes = page.getByRole('checkbox');
	for (let i = 0; i < (await boxes.count()); i++) await boxes.nth(i).check({ force: true });
	await page.getByRole('button', { name: 'Continue', exact: true }).click();
	await page.getByRole('button', { name: /^This device/ }).click();
	await page.getByRole('button', { name: 'Create Wallet', exact: true }).click();
	await expect(page.getByRole('button', { name: 'Enter Wallet' })).toBeVisible({
		timeout: 120_000
	});
	const address = page.getByText(/^0x[0-9a-fA-F]{40}$/).first();
	const text = (await address.textContent())!.trim();
	await page.getByRole('button', { name: 'Enter Wallet' }).click();
	await page.waitForURL(/\/en\/wallet/);
	return text;
}

/** The testnet space (Arc Testnet lives there). */
export async function toTestnet(page: Page): Promise<void> {
	const badge = page.getByTestId('network-space').first();
	if ((await badge.textContent())?.includes('Testnet')) return;
	await badge.click();
	await page.getByRole('button', { name: 'Switch to the testnet space' }).click();
	await expect(page.getByTestId('network-space').first()).toContainText('Testnet');
}

function cast(...args: string[]): string {
	return execFileSync('cast', args, { encoding: 'utf8' }).trim();
}

/**
 * Send `usdc` of Arc's native USDC from the funder. The key is expanded by
 * the shell from the environment, so a failure prints `$ARC_FUNDER_KEY`,
 * never the key.
 */
export function fund(address: string, usdc: string): void {
	execFileSync(
		'sh',
		[
			'-c',
			'cast send "$1" --value "$2" --rpc-url "$3" --private-key "$ARC_FUNDER_KEY" >/dev/null',
			'sh',
			address,
			`${usdc}ether`,
			ARC_RPC
		],
		{ encoding: 'utf8', env: { ...process.env, ARC_FUNDER_KEY: FUNDER_KEY } }
	);
}

/** Native balance in wei. */
export function balanceOf(address: string): bigint {
	return BigInt(cast('balance', address, '--rpc-url', ARC_RPC));
}

/**
 * A browser holding the funder's account as an injected wallet (EIP-6963),
 * as MetaMask would be: typed data is signed in Node by `cast` with the key
 * from the environment; transactions are not offered.
 */
export async function withFunderWallet(context: BrowserContext): Promise<void> {
	await context.exposeFunction('__liveSignTypedData', (json: string) =>
		execFileSync(
			'sh',
			['-c', 'cast wallet sign --data "$1" --private-key "$ARC_FUNDER_KEY"', 'sh', json],
			{ encoding: 'utf8', env: { ...process.env, ARC_FUNDER_KEY: FUNDER_KEY } }
		).trim()
	);
	await context.addInitScript(
		([owner, chainHex]) => {
			let chainId = chainHex;
			const listeners: Record<string, ((...a: unknown[]) => void)[]> = {};
			const sign = (window as unknown as { __liveSignTypedData: (j: string) => Promise<string> })
				.__liveSignTypedData;
			const provider = {
				async request({ method, params }: { method: string; params?: unknown[] }) {
					switch (method) {
						case 'eth_accounts':
						case 'eth_requestAccounts':
							return [owner];
						case 'eth_chainId':
							return chainId;
						case 'wallet_switchEthereumChain':
							chainId = (params?.[0] as { chainId: string }).chainId;
							for (const f of listeners.chainChanged ?? []) f(chainId);
							return null;
						case 'eth_signTypedData_v4':
							return sign(params?.[1] as string);
						default:
							throw { code: 4200, message: `not offered: ${method}` };
					}
				},
				on(event: string, f: (...a: unknown[]) => void) {
					(listeners[event] ??= []).push(f);
				},
				removeListener() {}
			};
			window.addEventListener('eip6963:requestProvider', () =>
				window.dispatchEvent(
					new CustomEvent('eip6963:announceProvider', {
						detail: Object.freeze({
							info: { uuid: 'live-wallet', name: 'Live Test Wallet', icon: '', rdns: 'dev.live' },
							provider
						})
					})
				)
			);
		},
		[FUNDER, `0x${ARC_CHAIN.toString(16)}`] as const
	);
}
