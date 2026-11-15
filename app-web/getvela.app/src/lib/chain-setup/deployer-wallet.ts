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
import type { Address } from '../evm/hash';
import { toQuantity, type Hex } from '../evm/hex';
import { generatePrivateKey, privateKeyToAddress, signLegacyTx, txHash } from '../evm/tx';
import { loadEvm } from '../evm/wasm';
import { getBalance, getGasPrice, rpcCall, sendRaw } from './rpc';

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
	const key: DeployerKey = { privateKey, address: privateKeyToAddress(privateKey) };
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

export interface SendArgs {
	to: `0x${string}`;
	data?: Hex;
	value?: bigint;
	/** A floor; the on-chain estimate ×1.2 wins when it is higher. */
	gasFloor?: bigint;
}

export interface RawSend {
	to: Address;
	data?: Hex;
	value?: bigint;
	gas: bigint;
	/** Defaults to the node's `eth_gasPrice`. */
	gasPrice?: bigint;
}

/**
 * Sign locally and broadcast one legacy (EIP-155) transaction. The RPC is
 * asked only for facts it cannot fake past the signature — the chain id is
 * checked against the one we sign for, so a node for the wrong chain is
 * refused before anything is signed — and the hash returned is the one we
 * computed, not the node's word for it.
 */
export async function sendLegacy(
	rpcUrl: string,
	chainId: number,
	privateKey: Hex,
	tx: RawSend
): Promise<Hex> {
	await loadEvm();
	const from = privateKeyToAddress(privateKey);
	const [answeredChain, nonce, gasPrice] = await Promise.all([
		rpcCall<string>(rpcUrl, 'eth_chainId'),
		rpcCall<string>(rpcUrl, 'eth_getTransactionCount', [from, 'pending']),
		tx.gasPrice === undefined ? getGasPrice(rpcUrl) : Promise.resolve(tx.gasPrice)
	]);
	if (Number(BigInt(answeredChain)) !== chainId) {
		throw new Error(`the RPC answers for chain ${BigInt(answeredChain)}, not ${chainId}`);
	}
	const raw = signLegacyTx(
		{
			chainId,
			nonce: BigInt(nonce),
			gasPrice,
			gas: tx.gas,
			to: tx.to,
			value: tx.value ?? 0n,
			data: tx.data ?? '0x'
		},
		privateKey
	);
	const hash = txHash(raw);
	const answered = await sendRaw(rpcUrl, raw);
	if (typeof answered === 'string' && answered.toLowerCase() !== hash) {
		throw new Error(`the RPC reported hash ${answered} for a transaction whose hash is ${hash}`);
	}
	return hash;
}

/** Estimate, pad by 20%, sign locally, broadcast. Returns the hash. */
export async function sendFromDeployer(
	rpcUrl: string,
	chainId: number,
	key: DeployerKey,
	args: SendArgs
): Promise<`0x${string}`> {
	let gas = args.gasFloor ?? 0n;
	let estimateError: unknown = null;
	try {
		const est = BigInt(
			await rpcCall<string>(rpcUrl, 'eth_estimateGas', [
				{
					from: key.address,
					to: args.to,
					data: args.data ?? '0x',
					value: args.value ? toQuantity(args.value) : undefined
				}
			])
		);
		const padded = (est * 120n) / 100n;
		if (padded > gas) gas = padded;
	} catch (e) {
		/* keep the floor; the node will still refuse a truly wrong one */
		estimateError = e;
	}
	if (gas === 0n) throw estimateError ?? new Error('no gas limit');
	return sendLegacy(rpcUrl, chainId, key.privateKey, {
		to: args.to,
		data: args.data,
		value: args.value ?? 0n,
		gas
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
	// Bid twice the going price (room for a base-fee tick). A legacy
	// transaction costs exactly gas × gasPrice, so this sweeps to zero.
	const bid = gasPrice * 2n;
	const fee = 21_000n * bid;
	if (balance <= fee) return null;
	return sendLegacy(rpcUrl, chainId, key.privateKey, {
		to,
		value: balance - fee,
		gas: 21_000n,
		gasPrice: bid
	});
}
