/**
 * JSON-RPC adapter — public API for all RPC and bundler calls.
 *
 * Routes through the RPC pool which handles endpoint discovery,
 * latency-based sorting, load balancing, and automatic failover.
 *
 * RPC methods  → poolRpcCall     (public RPCs + ethereum-data built-ins)
 * Bundler methods → poolBundlerCall (user bundler + vela-relay.getvela.app)
 */

import { poolBundlerCall, poolRpcCall } from './rpc-pool';

// ---------------------------------------------------------------------------
// Method classification
// ---------------------------------------------------------------------------

/** ERC-4337 bundler methods (routed to bundler pool). */
const BUNDLER_METHODS = new Set([
  'eth_sendUserOperation',
  'eth_estimateUserOperationGas',
  'eth_getUserOperationReceipt',
  'eth_getUserOperationByHash',
  'pimlico_getUserOperationGasPrice',
]);

// ---------------------------------------------------------------------------
// Public API (same interface as before)
// ---------------------------------------------------------------------------

interface RPCResponse {
  jsonrpc: string;
  id: number;
  result?: any;
  error?: { code: number; message: string; data?: any };
}

/**
 * Make a JSON-RPC call with automatic load balancing and failover.
 *
 * Bundler methods are routed through the bundler endpoint pool.
 * Standard RPC methods are routed through the RPC endpoint pool.
 */
export async function rpcCall(
  method: string,
  params: any[],
  chainId: number,
): Promise<RPCResponse> {
  if (BUNDLER_METHODS.has(method)) {
    return poolBundlerCall(method, params, chainId);
  }
  return poolRpcCall(method, params, chainId);
}
