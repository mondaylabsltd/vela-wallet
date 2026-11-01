/**
 * The multisig fork harness, from a test: a browser profile pointed at the
 * local fork, genesis service and relay (`scripts/onchain/multisig-fork/up.sh`),
 * signed in to the parallel space's fixture accounts — the real app, with only
 * the passkey faked by committed throwaway keys.
 */
import type { BrowserContext, Page } from '@playwright/test';

/**
 * The harness's ports: up.sh's defaults, or the same names up.sh took them
 * from — a second harness beside a running one listens elsewhere.
 */
const port = (name: string, fallback: number) => Number(process.env[name] ?? fallback);

export const FORK_RPC = `http://127.0.0.1:${port('FORK_PORT', 8545)}`;
/**
 * The second fork, answering as Base (8453): a network the wallet has and the
 * genesis service is not configured to read.
 */
export const UNREAD_CHAIN = 8453;
export const UNREAD_RPC = `http://127.0.0.1:${port('UNREAD_PORT', 8546)}`;
const RPCS: Record<number, string> = { 100: FORK_RPC, [UNREAD_CHAIN]: UNREAD_RPC };
export const GENESIS = `http://127.0.0.1:${port('GENESIS_PORT', 8787)}`;
export const RELAY = `http://127.0.0.1:${port('RELAY_PORT', 8790)}`;
/** The PeerConnect relay (up.sh): Vela and an outside owner's wallet page meet through it. */
export const PEERCONNECT = `ws://127.0.0.1:${port('PEERCONNECT_PORT', 8792)}`;

/** anvil's unlocked dev account #1: an outside owner whose wallet signs through anvil. */
export const EOA_ONE = '0x70997970C51812dc3A010C7d01b50e0d17dc79C8';
/** anvil's unlocked dev account #2. */
export const EOA_TWO = '0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC';

export const FIXTURES = {
	one: '0xD400866e00B055B20752a826CD5C89b811de130b',
	two: '0x031d7D57c99CAF891e1C250554691Fd12D84772b',
	three: '0x58cd0ce6A27099220543b31710d7860d75Ba1d3d'
} as const;

export async function rpc<T>(method: string, params: unknown[], chainId = 100): Promise<T> {
	const response = await fetch(RPCS[chainId], {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params })
	});
	const body = (await response.json()) as { result?: T; error?: { message: string } };
	if (body.error) throw new Error(body.error.message);
	return body.result as T;
}

export async function setBalance(address: string, wei: bigint, chainId = 100): Promise<void> {
	await rpc('anvil_setBalance', [address, `0x${wei.toString(16)}`], chainId);
}

export async function balanceOf(address: string, chainId = 100): Promise<bigint> {
	return BigInt(await rpc<string>('eth_getBalance', [address, 'latest'], chainId));
}

/** Point the profile at the harness before any page runs. */
export async function useHarness(context: BrowserContext): Promise<void> {
	await context.addInitScript(
		([genesis, relay, peerconnect]) => {
			localStorage.setItem('vela.intro.seen', String(Date.now()));
			localStorage.setItem('vela.genesisServiceURL', genesis);
			localStorage.setItem(
				'vela.serviceEndpoints',
				JSON.stringify({ bundlerServiceURL: relay, genesisServiceURL: genesis })
			);
			localStorage.setItem('vela.peerconnectRelay', peerconnect);
		},
		[GENESIS, RELAY, PEERCONNECT]
	);
}

/**
 * A browser with an outside owner's wallet in it: an EIP-6963 provider whose
 * accounts are anvil's unlocked dev accounts, signing (`eth_signTypedData_v4`)
 * and sending through the fork itself — as MetaMask would, minus the prompt.
 */
export async function withInjectedWallet(context: BrowserContext, account: string): Promise<void> {
	await context.addInitScript(
		([rpcs, owner]) => {
			const forks = rpcs as Record<string, string>;
			let chainId = '0x64';
			const listeners: Record<string, ((...a: unknown[]) => void)[]> = {};
			const rpc = async (method: string, params: unknown) => {
				const url = forks[String(parseInt(chainId, 16))];
				const r = await fetch(url, {
					method: 'POST',
					headers: { 'content-type': 'application/json' },
					body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params })
				});
				const body = await r.json();
				if (body.error) throw body.error;
				return body.result;
			};
			const provider = {
				async request({ method, params }: { method: string; params?: unknown[] }) {
					switch (method) {
						case 'eth_accounts':
						case 'eth_requestAccounts':
							return [owner];
						case 'eth_chainId':
							return chainId;
						case 'wallet_switchEthereumChain': {
							const next = (params?.[0] as { chainId: string }).chainId;
							if (!forks[String(parseInt(next, 16))])
								throw { code: 4902, message: 'Unknown chain' };
							chainId = next;
							for (const f of listeners.chainChanged ?? []) f(chainId);
							return null;
						}
						default:
							return rpc(method, params ?? []);
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
							info: { uuid: 'test-wallet', name: 'Test Wallet', icon: '', rdns: 'dev.test' },
							provider
						})
					})
				)
			);
		},
		[RPCS, account] as const
	);
}

/** The fork as this profile's network for chain 100 (and the unread fork): what the person's own node override does. */
export async function seedNetwork(page: Page): Promise<void> {
	await page.evaluate(
		(networks) => {
			return new Promise<void>((resolve, reject) => {
				const open = indexedDB.open('vela', 1);
				open.onupgradeneeded = () => open.result.createObjectStore('kv');
				open.onerror = () => reject(open.error);
				open.onsuccess = () => {
					const tx = open.result.transaction('kv', 'readwrite');
					tx.objectStore('kv').put(JSON.stringify(networks), 'vela.networkConfig');
					tx.oncomplete = () => resolve();
					tx.onerror = () => reject(tx.error);
				};
			});
		},
		Object.entries(RPCS).map(([chainId, rpcURL]) => ({ chainId: Number(chainId), rpcURL }))
	);
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

const FACTORY_141 = '0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67';
const SAFE_L2_141 = '0x29fcB43b46531BcA003ddC8FCB67FFE91900C762';
const HANDLER_141 = '0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99';
const ZERO = '0x0000000000000000000000000000000000000000';

/**
 * A Safe v1.4.1 that Safe{Wallet} would keep undeployed: its creation props,
 * as an export carries them, and the address the factory gives them (asked of
 * the fork with a static call).
 */
export async function undeployedSafe(
	owners: string[],
	threshold: number,
	saltNonce: string
): Promise<{
	address: string;
	props: Record<string, unknown>;
	/** Its genesis as the wallet stores one (no name: as a Safe{Wallet} Safe comes). */
	genesis: { factory: string; singleton: string; saltNonce: string; initializer: string };
}> {
	const { execFileSync } = await import('node:child_process');
	const cast = (...args: string[]) => execFileSync('cast', args, { encoding: 'utf8' }).trim();
	const initializer = cast(
		'calldata',
		'setup(address[],uint256,address,bytes,address,address,uint256,address)',
		`[${owners.join(',')}]`,
		String(threshold),
		ZERO,
		'0x',
		HANDLER_141,
		ZERO,
		'0',
		ZERO
	);
	const address = cast(
		'call',
		FACTORY_141,
		'createProxyWithNonce(address,bytes,uint256)(address)',
		SAFE_L2_141,
		initializer,
		saltNonce,
		'--rpc-url',
		FORK_RPC
	);
	return {
		address,
		genesis: {
			factory: FACTORY_141,
			singleton: SAFE_L2_141,
			saltNonce: `0x${BigInt(saltNonce).toString(16)}`,
			initializer
		},
		props: {
			factoryAddress: FACTORY_141,
			masterCopy: SAFE_L2_141,
			safeAccountConfig: {
				threshold,
				owners,
				fallbackHandler: HANDLER_141,
				to: ZERO,
				data: '0x',
				paymentReceiver: ZERO
			},
			safeVersion: '1.4.1',
			saltNonce
		}
	};
}

/** Safe's SafeModuleSetup (safe-modules v0.3.0), on Gnosis. */
const MODULE_SETUP = '0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47';
/**
 * Who sends the hooked Safe's deployment: an address anvil impersonates (the
 * dev accounts carry EIP-7702 delegations on Gnosis, and the service's
 * sponsor key is busy with its own nonces).
 */
const DEPLOYER = '0x00000000000000000000000000000000000D3910';

/**
 * A genesis whose `setup` hands the Safe to `module`: it delegatecalls
 * SafeModuleSetup.enableModules([module]) — the address derives, the owners
 * read as `owners`, and the module can move its funds without them (the r4-e
 * finding). `deploy` deploys it on the fork through the canonical factory.
 */
export async function hookedSafe(
	owners: string[],
	module: string,
	saltNonce: string
): Promise<{
	address: string;
	genesis: { factory: string; singleton: string; saltNonce: string; initializer: string };
	deploy(): Promise<void>;
}> {
	const { execFileSync } = await import('node:child_process');
	const cast = (...args: string[]) => execFileSync('cast', args, { encoding: 'utf8' }).trim();
	const enable = cast('calldata', 'enableModules(address[])', `[${module}]`);
	const initializer = cast(
		'calldata',
		'setup(address[],uint256,address,bytes,address,address,uint256,address)',
		`[${owners.join(',')}]`,
		'1',
		MODULE_SETUP,
		enable,
		HANDLER_141,
		ZERO,
		'0',
		ZERO
	);
	const create = 'createProxyWithNonce(address,bytes,uint256)';
	const args = [SAFE_L2_141, initializer, saltNonce];
	const address = cast('call', FACTORY_141, `${create}(address)`, ...args, '--rpc-url', FORK_RPC);
	return {
		address,
		genesis: {
			factory: FACTORY_141,
			singleton: SAFE_L2_141,
			saltNonce: `0x${BigInt(saltNonce).toString(16)}`,
			initializer
		},
		async deploy() {
			await rpc('anvil_impersonateAccount', [DEPLOYER]);
			await setBalance(DEPLOYER, 10n ** 18n);
			cast(
				'send',
				FACTORY_141,
				create,
				...args,
				'--rpc-url',
				FORK_RPC,
				'--unlocked',
				'--from',
				DEPLOYER
			);
		}
	};
}

/** keccak256 of hex data, as cast computes it. */
export async function keccak(hex: string): Promise<string> {
	const { execFileSync } = await import('node:child_process');
	return execFileSync('cast', ['keccak', hex], { encoding: 'utf8' }).trim();
}

/** The registry (v4) on the fork: the address the wallet knows. */
export const REGISTRY = '0xF412388bF908E510066d3Be5c704bd9A327f945d';

/**
 * What the registry on the fork holds for `safe`, read straight from the
 * chain (`genesisOf`): its name and the block of its last rename, or `null`.
 */
export async function recordOnChain(
	safe: string
): Promise<{ name: string; renamedAt: number } | null> {
	const { execFileSync } = await import('node:child_process');
	const out = execFileSync(
		'cast',
		[
			'call',
			REGISTRY,
			'genesisOf(address)(bool,(address,address,uint64,uint256,bytes,bytes32,address,uint64,uint32,string))',
			safe,
			'--rpc-url',
			FORK_RPC
		],
		{ encoding: 'utf8' }
	).trim();
	if (!out.startsWith('true')) return null;
	const name = /, "((?:[^"\\]|\\.)*)"\)$/.exec(out)?.[1] ?? '';
	const renamedAt = Number(
		/^true\s+\(0x[0-9a-fA-F]{40}, 0x[0-9a-fA-F]{40}, (\d+)/.exec(out)?.[1] ?? 'NaN'
	);
	return { name, renamedAt };
}

/** The genesis service's answer for `safe` once it holds a landed record (it polls). */
export async function landedRecord(safe: string): Promise<{ genesis: { name?: string } }> {
	for (let i = 0; i < 60; i += 1) {
		const r = await fetch(`${GENESIS}/api/genesis/${safe}`);
		const body = (await r.json()) as {
			found?: boolean;
			pending?: boolean;
			genesis: { name?: string };
		};
		if (r.status === 200 && body.found && !body.pending) return body;
		await new Promise((done) => setTimeout(done, 1000));
	}
	throw new Error(`no landed record for ${safe}`);
}

/** A Safe transaction as the mailbox takes it (quantities hex). */
export interface WireSafeTx {
	to: string;
	value: string;
	data: string;
	operation: number;
	safeTxGas: string;
	baseGas: string;
	gasPrice: string;
	gasToken: string;
	refundReceiver: string;
	nonce: string;
}

/** A plain call from the multisig, at `nonce`. */
export function safeTx(to: string, data: string, nonce: number, value = '0x0'): WireSafeTx {
	return {
		to,
		value,
		data,
		operation: 0,
		safeTxGas: '0x0',
		baseGas: '0x0',
		gasPrice: '0x0',
		gasToken: ZERO,
		refundReceiver: ZERO,
		nonce: `0x${nonce.toString(16)}`
	};
}

/**
 * An outside owner's approval: anvil's unlocked `owner` signs the SafeTx's
 * EIP-712 data (`eth_signTypedData_v4`), as MetaMask would — on the fork
 * answering as `chainId`.
 */
export async function signSafeTx(
	owner: string,
	chainId: number,
	safe: string,
	tx: WireSafeTx
): Promise<{ owner: string; signature: string }> {
	const typed = {
		types: {
			EIP712Domain: [
				{ name: 'chainId', type: 'uint256' },
				{ name: 'verifyingContract', type: 'address' }
			],
			SafeTx: [
				{ name: 'to', type: 'address' },
				{ name: 'value', type: 'uint256' },
				{ name: 'data', type: 'bytes' },
				{ name: 'operation', type: 'uint8' },
				{ name: 'safeTxGas', type: 'uint256' },
				{ name: 'baseGas', type: 'uint256' },
				{ name: 'gasPrice', type: 'uint256' },
				{ name: 'gasToken', type: 'address' },
				{ name: 'refundReceiver', type: 'address' },
				{ name: 'nonce', type: 'uint256' }
			]
		},
		primaryType: 'SafeTx',
		domain: { chainId, verifyingContract: safe },
		message: tx
	};
	const signature = await rpc<string>(
		'eth_signTypedData_v4',
		[owner, JSON.stringify(typed)],
		chainId
	);
	return { owner, signature };
}

/** Hand a transaction and one approval of it to the genesis service's mailbox. */
export async function postApproval(
	chainId: number,
	safe: string,
	tx: WireSafeTx,
	approval: { owner: string; signature: string },
	genesis?: unknown
): Promise<{
	status: number;
	body: {
		status?: string;
		proposal?: {
			safeTxHash: string;
			approvals: { owner: string; signerIsGenesisOwner?: boolean; verified?: boolean }[];
		};
		refusal?: unknown;
	};
}> {
	const r = await fetch(`${GENESIS}/api/proposals`, {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({ chainId, safe, safeTx: tx, approval, genesis })
	});
	return { status: r.status, body: await r.json() };
}

/** `cast calldata`: a call's ABI encoding. */
export async function calldata(signature: string, ...args: string[]): Promise<string> {
	const { execFileSync } = await import('node:child_process');
	return execFileSync('cast', ['calldata', signature, ...args], { encoding: 'utf8' }).trim();
}

/** The genesis the service hands out for `safe` (recorded, or kept until it lands). */
export async function servedGenesis(safe: string): Promise<Record<string, unknown>> {
	for (let i = 0; i < 30; i += 1) {
		const r = await fetch(`${GENESIS}/api/genesis/${safe}`);
		const body = (await r.json()) as { found?: boolean; genesis?: Record<string, unknown> };
		if (r.status === 200 && body.found && body.genesis) return body.genesis;
		await new Promise((done) => setTimeout(done, 1000));
	}
	throw new Error(`the service has no genesis for ${safe}`);
}
