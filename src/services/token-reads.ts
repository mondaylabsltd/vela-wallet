/**
 * Small on-chain ERC-20 reads for the signing sheet.
 *
 * Kept separate from token-metadata (symbol/decimals) because these are
 * point-in-time *state* reads (allowance) rather than immutable metadata, so
 * they must never be cached across a signing session.
 */
import { poolRpcCall } from '@/services/rpc-pool';

const ALLOWANCE_SELECTOR = '0xdd62ed3e'; // allowance(address owner,address spender)

function pad32(addr: string): string {
  return addr.toLowerCase().replace(/^0x/, '').padStart(64, '0');
}

/**
 * Read `allowance(owner, spender)` for an ERC-20. Returns null on any failure
 * (RPC down, non-token, reverts) so callers degrade gracefully. Used to show the
 * RESULTING total for `increaseAllowance` (current + increment) instead of a
 * bare, easily-misread increment.
 */
export async function readErc20Allowance(
  chainId: number,
  token: string,
  owner: string,
  spender: string,
): Promise<bigint | null> {
  if (!/^0x[0-9a-fA-F]{40}$/.test(token) || !/^0x[0-9a-fA-F]{40}$/.test(owner) || !/^0x[0-9a-fA-F]{40}$/.test(spender)) {
    return null;
  }
  const data = ALLOWANCE_SELECTOR + pad32(owner) + pad32(spender);
  try {
    const res = await poolRpcCall('eth_call', [{ to: token, data }, 'latest'], chainId);
    if (res?.error || typeof res?.result !== 'string' || res.result === '0x') return null;
    return BigInt(res.result);
  } catch {
    return null;
  }
}
