/**
 * Safe's own transaction service, read straight from the wallet: where a
 * deployed Safe's creation is on record (factory, singleton, setup, salt
 * nonce) — its genesis, without the genesis service. Nothing it says is
 * believed: the core must derive the Safe's address from what it returns.
 */
import { checksumAddress } from '$lib/core/kernels';
import { poolRpcCall } from '$lib/services/rpc-pool';
import { genesisAddress, genesisFromCalldata, sameAddress, type Genesis } from './kernel';

/** `https://api.safe.global/tx-service/<prefix>` per chain (checked against each service). */
const PREFIX: Record<number, string> = {
	1: 'eth',
	10: 'oeth',
	50: 'xdc',
	56: 'bnb',
	100: 'gno',
	130: 'unichain',
	137: 'pol',
	143: 'monad',
	146: 'sonic',
	204: 'opbnb',
	324: 'zksync',
	480: 'wc',
	5000: 'mantle',
	8453: 'base',
	9745: 'plasma',
	10200: 'chi',
	42161: 'arb1',
	42220: 'celo',
	43111: 'hemi',
	43114: 'avax',
	57073: 'ink',
	59144: 'linea',
	80094: 'berachain',
	84532: 'basesep',
	534352: 'scr',
	747474: 'katana',
	11155111: 'sep'
};

export function hasTransactionService(chainId: number): boolean {
	return PREFIX[chainId] !== undefined;
}

export type Creation = { genesis: Genesis } | { unsupported: true } | null;

/**
 * The genesis Safe's transaction service records for `safe` on `chainId`.
 * Where its record lacks the salt nonce or the setup (a Safe made through a
 * 4337 bundle, say), the creating transaction is read from the chain and the
 * genesis found in its calldata. `{ unsupported }`: made with a factory or
 * singleton Vela does not know. `null`: nothing that derives `safe`. Throws
 * only when the service cannot be reached.
 */
export async function creationGenesis(chainId: number, safe: string): Promise<Creation> {
	const prefix = PREFIX[chainId];
	if (!prefix) return null;
	const url = `https://api.safe.global/tx-service/${prefix}/api/v1/safes/${checksumAddress(safe)}/creation/`;
	const response = await fetch(url, { headers: { accept: 'application/json' } });
	if (response.status === 404 || response.status === 422) return null;
	if (!response.ok) throw new Error(`transaction service: HTTP ${response.status}`);
	const c = (await response.json()) as {
		factoryAddress?: string;
		masterCopy?: string;
		setupData?: string;
		saltNonce?: string | null;
		transactionHash?: string;
	};
	const complete =
		c.factoryAddress && c.masterCopy && c.setupData && c.setupData !== '0x' && c.saltNonce != null;
	if (complete) {
		const genesis: Genesis = {
			factory: c.factoryAddress!,
			singleton: c.masterCopy!,
			saltNonce: `0x${BigInt(c.saltNonce!).toString(16)}`,
			initializer: c.setupData!
		};
		try {
			return sameAddress(genesisAddress(genesis), safe) ? { genesis } : null;
		} catch {
			return { unsupported: true };
		}
	}
	if (!c.transactionHash) return null;
	const tx = (await poolRpcCall('eth_getTransactionByHash', [c.transactionHash], chainId))
		.result as { input?: string } | null | undefined;
	if (!tx?.input) return null;
	const genesis = genesisFromCalldata(tx.input, safe);
	if (genesis) return { genesis };
	// A creation Vela cannot read: an unknown factory or singleton.
	return c.factoryAddress ? { unsupported: true } : null;
}
