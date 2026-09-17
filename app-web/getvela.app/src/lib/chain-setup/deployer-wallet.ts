/**
 * A throwaway key, generated in this browser, for the CREATE2 steps.
 *
 * Why a throwaway rather than "connect your wallet": the steps are plain
 * factory calls that any funded account can make, the chain being set up
 * usually has no wallet integration yet (that is the point), and a person
 * setting up a chain is comfortable sending a little gas to an address. The
 * key lives in localStorage, per chain, so a reload does not strand the funds;
 * it can be exported, and the leftover swept back when the work is done.
 *
 * It is spending money only in the sense of gas. It never holds anything a
 * person did not just send it for this purpose, and the page says so.
 */
import { createWalletClient, http, type Chain, type Hex } from 'viem';
import { generatePrivateKey, privateKeyToAccount } from 'viem/accounts';
import { getBalance, getGasPrice, rpcCall } from './rpc';

const STORAGE_KEY = 'getvela.chain-setup.deployer';

export interface DeployerKey {
	privateKey: Hex;
	address: `0x${string}`;
}

type Store = Record<string, DeployerKey>;

function readStore(): Store {
	try {
		const raw = globalThis.localStorage?.getItem(STORAGE_KEY);
		return raw ? (JSON.parse(raw) as Store) : {};
	} catch {
		return {};
	}
}
function writeStore(s: Store): void {
	try {
		globalThis.localStorage?.setItem(STORAGE_KEY, JSON.stringify(s));
	} catch {
		/* storage unavailable — the key still works for this page load */
	}
}

export function loadDeployer(chainId: number): DeployerKey | null {
	const k = readStore()[String(chainId)];
	return k?.privateKey && k?.address ? k : null;
}

export function getOrCreateDeployer(chainId: number): DeployerKey {
	const existing = loadDeployer(chainId);
	if (existing) return existing;
	const privateKey = generatePrivateKey();
	const key: DeployerKey = { privateKey, address: privateKeyToAccount(privateKey).address };
	const s = readStore();
	s[String(chainId)] = key;
	writeStore(s);
	return key;
}

export function forgetDeployer(chainId: number): void {
	const s = readStore();
	delete s[String(chainId)];
	writeStore(s);
}

/** The text a person saves. Plain, dated, and honest about what the key is. */
export function deployerExportText(chainId: number, key: DeployerKey): string {
	return [
		'getvela.app — chain setup deployer key',
		`chain id: ${chainId}`,
		`address:  ${key.address}`,
		`key:      ${key.privateKey}`,
		'',
		'This key was generated in your browser to deploy Vela’s contracts on this chain.',
		'Anyone holding it can spend whatever is at the address above. Sweep the leftover',
		'back to yourself when you are done, then you can discard this file.',
		`saved: ${new Date().toISOString()}`
	].join('\n');
}

function minimalChain(chainId: number, rpcUrl: string): Chain {
	return {
		id: chainId,
		name: `chain ${chainId}`,
		nativeCurrency: { name: 'coin', symbol: 'COIN', decimals: 18 },
		rpcUrls: { default: { http: [rpcUrl] } }
	};
}

export interface SendArgs {
	to: `0x${string}`;
	data?: Hex;
	value?: bigint;
	/** A floor; the on-chain estimate ×1.2 wins when it is higher. */
	gasFloor?: bigint;
}

/** Estimate, pad by 20%, sign locally, broadcast. Returns the hash. */
export async function sendFromDeployer(
	rpcUrl: string,
	chainId: number,
	key: DeployerKey,
	args: SendArgs
): Promise<`0x${string}`> {
	const account = privateKeyToAccount(key.privateKey);
	let gas = args.gasFloor ?? 0n;
	try {
		const est = BigInt(
			await rpcCall<string>(rpcUrl, 'eth_estimateGas', [
				{
					from: account.address,
					to: args.to,
					data: args.data ?? '0x',
					value: args.value ? `0x${args.value.toString(16)}` : undefined
				}
			])
		);
		const padded = (est * 120n) / 100n;
		if (padded > gas) gas = padded;
	} catch {
		/* keep the floor; the node will still refuse a truly wrong one */
	}
	const client = createWalletClient({
		account,
		chain: minimalChain(chainId, rpcUrl),
		transport: http(rpcUrl)
	});
	return client.sendTransaction({
		to: args.to,
		data: args.data,
		value: args.value ?? 0n,
		gas: gas || undefined
	});
}

/** Send everything but the gas for the sweep itself back to `to`. `null` = nothing worth sweeping. */
export async function sweepDeployer(
	rpcUrl: string,
	chainId: number,
	key: DeployerKey,
	to: `0x${string}`
): Promise<`0x${string}` | null> {
	const [balance, gasPrice] = await Promise.all([
		getBalance(rpcUrl, key.address),
		getGasPrice(rpcUrl)
	]);
	const fee = 21_000n * gasPrice * 2n; // room for a base-fee tick
	if (balance <= fee) return null;
	const account = privateKeyToAccount(key.privateKey);
	const client = createWalletClient({
		account,
		chain: minimalChain(chainId, rpcUrl),
		transport: http(rpcUrl)
	});
	return client.sendTransaction({ to, value: balance - fee, gas: 21_000n });
}
