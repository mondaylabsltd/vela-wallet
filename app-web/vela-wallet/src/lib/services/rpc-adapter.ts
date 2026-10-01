// Ported from src/services/rpc-adapter.ts @ f9bcb278 — RN seams rewritten to the web modules; logic verbatim.
/**
 * JSON-RPC adapter — public API for all RPC and bundler calls.
 *
 * Routes through the RPC pool which handles endpoint discovery,
 * latency-based sorting, load balancing, and automatic failover.
 *
 * RPC methods  → poolRpcCall     (public RPCs + ethereum-data built-ins)
 * Bundler methods → poolBundlerCall (user bundler + vela-relay.getvela.app)
 */

import { receiptShouldStaySilent, relayShouldFail, submitShouldReject } from './fault-injection';
import { PoolFailedError, poolBundlerCall, poolRpcCall } from './rpc-pool';

// ---------------------------------------------------------------------------
// Method classification
// ---------------------------------------------------------------------------

/**
 * The relay's lifecycle-status method — `tx_tracker::USER_OP_STATUS_METHOD`
 * (spec 082 RA7). A TS constant because a module-level Set cannot wait for the
 * wasm; `rpc-pool-executor.test.ts` pins it to `userOpStatusMethod()`, so the
 * two spellings cannot drift the way `eth_getUserOperationStatus` did (G13).
 */
export const USER_OP_STATUS_METHOD = 'pimlico_getUserOperationStatus';

/** ERC-4337 bundler methods (routed to bundler pool). */
const BUNDLER_METHODS = new Set([
	'eth_sendUserOperation',
	'eth_estimateUserOperationGas',
	'eth_getUserOperationReceipt',
	'eth_getUserOperationByHash',
	// A receipt only exists once an op lands. Before that it is the only thing that
	// separates "still going" from "the relay refused it" — without this the wallet
	// polls a null receipt until timeout and reports a rejected op as pending.
	USER_OP_STATUS_METHOD,
	'pimlico_getUserOperationGasPrice'
]);

// ---------------------------------------------------------------------------
// Public API (same interface as before)
// ---------------------------------------------------------------------------

interface RPCResponse {
	jsonrpc: string;
	id: number;
	result?: unknown;
	error?: { code: number; message: string; data?: unknown };
}

/**
 * Make a JSON-RPC call with automatic load balancing and failover.
 *
 * Bundler methods are routed through the bundler endpoint pool.
 * Standard RPC methods are routed through the RPC endpoint pool.
 */
export async function rpcCall(
	method: string,
	params: unknown[],
	chainId: number
): Promise<RPCResponse> {
	if (BUNDLER_METHODS.has(method)) {
		// The relay faults (spec 026 T223) sit at this chokepoint, so every
		// caller — fee quote, submit, receipt poll — meets them identically.
		// Each is a shape the relay really produces, not an invented one.
		if (relayShouldFail(chainId)) {
			// Thrown before any byte leaves: the pool's "reached nobody".
			throw new PoolFailedError(`[fault] relay unreachable for chain ${chainId}`, {
				maybeDelivered: false,
				rateLimited: false
			});
		}
		if (method === 'eth_sendUserOperation' && submitShouldReject(chainId)) {
			return {
				jsonrpc: '2.0',
				id: 1,
				error: { code: -32521, message: '[fault] user operation reverted during simulation' }
			};
		}
		if (method === 'eth_getUserOperationReceipt' && receiptShouldStaySilent(chainId)) {
			// Accepted, never landed: the answer a busy chain really gives.
			return { jsonrpc: '2.0', id: 1, result: null };
		}
		return poolBundlerCall(method, params, chainId);
	}
	return poolRpcCall(method, params, chainId);
}
