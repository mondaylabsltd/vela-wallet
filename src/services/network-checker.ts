/**
 * Network compatibility checker — full gateway for adding networks.
 *
 * 1. Collects all available HTTPS RPCs for the chain
 * 2. Tests latency and picks the fastest one
 * 3. Checks all 9 required contracts via the best RPC
 * 4. Returns per-contract status + best RPC recommendation
 *
 * Only networks where ALL contracts are deployed can be added.
 */

import type { CompatibilityResult, ContractStatus } from '@/models/types';
import { fetchChainInfo } from './chain-registry';

// ---------------------------------------------------------------------------
// Required contracts (from safe-address.ts)
// ---------------------------------------------------------------------------

/** Order matches biubiu.tools Vela Wallet Chain Setup */
const REQUIRED_CONTRACTS: { name: string; address: string }[] = [
  { name: 'Deterministic Deployment Proxy', address: '0x4e59b44847b379578588920cA78FbF26c0B4956C' },
  { name: 'Safe Singleton Factory',         address: '0x914d7Fec6aaC8cd542e72Bca78B30650d45643d7' },
  { name: 'Multicall3',                     address: '0xcA11bde05977b3631167028862bE2a173976CA11' },
  { name: 'EntryPoint v0.7',                address: '0x0000000071727De22E5E9d8BAf0edAc6f37da032' },
  { name: 'Safe L2',                        address: '0x29fcB43b46531BcA003ddC8FCB67FFE91900C762' },
  { name: 'Safe Proxy Factory',             address: '0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67' },
  { name: 'Safe 4337 Module',               address: '0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226' },
  { name: 'Safe Module Setup',              address: '0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47' },
  { name: 'WebAuthn Signer',                address: '0x94a4F6affBd8975951142c3999aEAB7ecee555c2' },
  { name: 'Fallback Handler',               address: '0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99' },
  { name: 'MultiSend',                      address: '0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526' },
];

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Full compatibility check for adding a new network.
 *
 * @param rpcURLs - One or more HTTPS RPC URLs to test
 * @param chainId - Target chain ID
 */
export async function checkNetworkCompatibility(
  rpcURLs: string[],
  chainId: number,
): Promise<CompatibilityResult> {
  // 1. Collect all candidate RPCs
  const candidates = [...new Set(rpcURLs.filter(u => u && u.startsWith('https://')))];

  // Add RPCs from chain registry
  try {
    const info = await fetchChainInfo(chainId);
    if (info?.rpcUrl && !candidates.includes(info.rpcUrl)) {
      candidates.push(info.rpcUrl);
    }
  } catch {}

  if (candidates.length === 0) {
    return {
      chainId,
      compatible: false,
      contracts: REQUIRED_CONTRACTS.map(c => ({ ...c, deployed: false })),
      rpcFailed: true,
      error: 'No valid HTTPS RPC endpoints available',
    };
  }

  // 2. Test all RPCs in parallel, pick the fastest responding one
  console.log(`[NetworkChecker] Testing ${candidates.length} RPCs for chain ${chainId}`);
  const best = await pickFastestRpc(candidates, chainId);

  if (!best) {
    return {
      chainId,
      compatible: false,
      contracts: REQUIRED_CONTRACTS.map(c => ({ ...c, deployed: false })),
      rpcFailed: true,
      error: 'All RPC endpoints failed or timed out',
    };
  }

  console.log(`[NetworkChecker] Best RPC: ${best.url} (${best.latencyMs}ms)`);

  // 3. Check all required contracts + P256 precompile via the best RPC
  const [contracts, p256Available] = await Promise.all([
    checkAllContracts(best.url),
    checkP256Precompile(best.url),
  ]);

  const allDeployed = contracts.every(c => c.deployed);
  const missing = contracts.filter(c => !c.deployed);
  const compatible = allDeployed && p256Available;

  const issues: string[] = [];
  if (missing.length > 0) {
    issues.push(`${missing.length} contract${missing.length > 1 ? 's' : ''} not deployed: ${missing.map(c => c.name).join(', ')}`);
  }
  if (!p256Available) {
    issues.push('P256 precompile (RIP-7212) not available — passkey signatures will not work');
  }

  return {
    chainId,
    compatible,
    contracts,
    p256Available,
    bestRpcUrl: best.url,
    bestRpcLatency: best.latencyMs,
    error: issues.length > 0 ? issues.join('. ') : undefined,
  };
}

// ---------------------------------------------------------------------------
// RPC latency test
// ---------------------------------------------------------------------------

interface RpcTestResult {
  url: string;
  latencyMs: number;
}

async function testRpcLatency(url: string): Promise<RpcTestResult | null> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 10000);
  const start = Date.now();

  try {
    const res = await fetch(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'eth_chainId', params: [] }),
      signal: controller.signal,
    });
    clearTimeout(timeout);
    if (!res.ok) return null;
    const json = await res.json();
    if (!json.result) return null;
    return { url, latencyMs: Date.now() - start };
  } catch {
    clearTimeout(timeout);
    return null;
  }
}

async function pickFastestRpc(urls: string[], chainId: number): Promise<RpcTestResult | null> {
  const results = await Promise.allSettled(urls.map(u => testRpcLatency(u)));
  const valid: RpcTestResult[] = [];
  for (const r of results) {
    if (r.status === 'fulfilled' && r.value) valid.push(r.value);
  }
  if (valid.length === 0) return null;
  valid.sort((a, b) => a.latencyMs - b.latencyMs);
  return valid[0];
}

// ---------------------------------------------------------------------------
// Contract deployment check
// ---------------------------------------------------------------------------

async function checkAllContracts(rpcUrl: string): Promise<ContractStatus[]> {
  // Batch all eth_getCode calls in parallel
  const results = await Promise.allSettled(
    REQUIRED_CONTRACTS.map(async (contract) => {
      const deployed = await checkCode(rpcUrl, contract.address);
      return { ...contract, deployed };
    }),
  );

  return results.map((r, i) => {
    if (r.status === 'fulfilled') return r.value;
    return { ...REQUIRED_CONTRACTS[i], deployed: false };
  });
}

// ---------------------------------------------------------------------------
// RIP-7212 P256 precompile check
// ---------------------------------------------------------------------------

const P256_PRECOMPILE = '0x0000000000000000000000000000000000000100';

/** sha256("test") signed with a known P-256 key — 160 bytes input */
const VALID_P256_CALL =
  '0x' +
  '9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08' +
  '7bf0e18d07660f15994adce5c3836d7bd6167cdb5726f631098f433ebe0be9c0' +
  '3936edbe5c791477e714e58244afb690b9b88b833ff4acdf0fbd1b28bf0b1182' +
  '3be8cbcb3f590087711ae5ed74b9cd06a88058d0bbe700b5f0ec5a1bfac15592' +
  'f989ef9bfaae0fee03c36625e88eae99806a879d813411f876e7e03a2ffd8314';

async function checkP256Precompile(rpcUrl: string): Promise<boolean> {
  // Strategy 1: eth_call with a valid P256 signature (include gas for zkSync compat)
  const callResult = await rpcCall(rpcUrl, 'eth_call', [
    { to: P256_PRECOMPILE, data: VALID_P256_CALL, gas: '0x100000' },
    'latest',
  ]);
  if (callResult) {
    const result = callResult as string;
    if (result !== '0x' && result.length >= 66) {
      try {
        if (BigInt(result) === 1n) return true;
      } catch {}
    }
  }

  // Strategy 2: fallback — check if code exists at the precompile address.
  // Some chains (e.g. zkSync) support P256 but eth_call to precompile may
  // behave unexpectedly. If there is code deployed at 0x100, it is the
  // RIP-7212 precompile.
  const hasCode = await checkCode(rpcUrl, P256_PRECOMPILE);
  return hasCode;
}

// ---------------------------------------------------------------------------
// Generic RPC helper
// ---------------------------------------------------------------------------

async function rpcCall(rpcUrl: string, method: string, params: unknown[]): Promise<unknown | null> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 10000);

  try {
    const res = await fetch(rpcUrl, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
      signal: controller.signal,
    });
    clearTimeout(timeout);
    if (!res.ok) return null;
    const json = await res.json();
    if (json.error) return null;
    return json.result ?? null;
  } catch {
    clearTimeout(timeout);
    return null;
  }
}

// ---------------------------------------------------------------------------
// Contract deployment check (eth_getCode)
// ---------------------------------------------------------------------------

async function checkCode(rpcUrl: string, address: string): Promise<boolean> {
  const code = await rpcCall(rpcUrl, 'eth_getCode', [address, 'latest']) as string | null;
  if (!code || code === '0x' || code === '0x0') return false;
  // Some chains (e.g. zkSync) may return a short bytecode hash instead of full
  // bytecode. Any non-empty response beyond "0x" / "0x0" indicates deployment.
  return code.length > 2;
}
