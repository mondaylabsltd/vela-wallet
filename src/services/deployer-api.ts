/**
 * Bundler / Deployer address management.
 *
 * In production, addresses are derived server-side from:
 *   passkeys publicKey + tag("bundler"|"deployer") + server secret → EOA
 *
 * Currently mocked — address derivation uses a placeholder algorithm.
 * Balance queries are REAL (on-chain eth_getBalance via rpcCall).
 */

import type { BundlerDeployerInfo, NetworkFundingStatus, FundingStatus } from '@/models/types';
import { rpcCall } from './rpc-adapter';
import { keccak256 } from './eth-crypto';
// Shared wei→ETH formatter (was a local 4-decimal copy). Aliased so call sites are
// unchanged; >= 1 ETH balances now render with the app-wide 3-decimal precision.
import { formatWeiToEth as formatWei } from './format-eth';

// ---------------------------------------------------------------------------
// Address derivation (mock)
// ---------------------------------------------------------------------------

/**
 * Get Bundler and Deployer EOA addresses for a wallet.
 * TODO: Replace with real API call to deployer service.
 */
export async function getAddresses(publicKeyHex: string): Promise<BundlerDeployerInfo> {
  // Mock: derive deterministic addresses from public key + tag
  const encoder = new TextEncoder();
  const bundlerHash = keccak256(
    new Uint8Array([...fromHexLight(publicKeyHex), ...encoder.encode('bundler')]),
  );
  const deployerHash = keccak256(
    new Uint8Array([...fromHexLight(publicKeyHex), ...encoder.encode('deployer')]),
  );

  const bundlerAddress = '0x' + toHexLight(bundlerHash).slice(0, 40);
  const deployerAddress = '0x' + toHexLight(deployerHash).slice(0, 40);

  return {
    walletAddress: '', // filled by caller
    bundlerAddress,
    deployerAddress,
  };
}

// ---------------------------------------------------------------------------
// Balance queries (real on-chain)
// ---------------------------------------------------------------------------

/** Balance thresholds in wei */
const LOW_THRESHOLD = BigInt('1000000000000000');    // 0.001 ETH
const ZERO_THRESHOLD = BigInt('100000000000000');     // 0.0001 ETH

function balanceToStatus(balanceWei: bigint): FundingStatus {
  if (balanceWei < ZERO_THRESHOLD) return 'not_funded';
  if (balanceWei < LOW_THRESHOLD) return 'low_balance';
  return 'funded';
}

async function getBalance(address: string, chainId: number): Promise<bigint> {
  try {
    const res = await rpcCall('eth_getBalance', [address, 'latest'], chainId);
    const hex = res.result as string | undefined;
    if (!hex) return 0n;
    return BigInt(hex);
  } catch {
    return 0n;
  }
}

/** Get funding status for a specific network. */
export async function getNetworkFunding(
  bundlerAddress: string,
  deployerAddress: string,
  chainId: number,
): Promise<NetworkFundingStatus> {
  const [bundlerWei, deployerWei] = await Promise.all([
    getBalance(bundlerAddress, chainId),
    getBalance(deployerAddress, chainId),
  ]);

  return {
    chainId,
    bundlerBalance: formatWei(bundlerWei),
    deployerBalance: formatWei(deployerWei),
    bundlerStatus: balanceToStatus(bundlerWei),
    deployerStatus: balanceToStatus(deployerWei),
  };
}

/** Get funding status for all networks in parallel. */
export async function getAllNetworkFunding(
  bundlerAddress: string,
  deployerAddress: string,
  chainIds: number[],
): Promise<NetworkFundingStatus[]> {
  const results = await Promise.allSettled(
    chainIds.map(chainId => getNetworkFunding(bundlerAddress, deployerAddress, chainId)),
  );

  return results.map((r, i) => {
    if (r.status === 'fulfilled') return r.value;
    return {
      chainId: chainIds[i],
      bundlerBalance: '0',
      deployerBalance: '0',
      bundlerStatus: 'not_funded' as FundingStatus,
      deployerStatus: 'not_funded' as FundingStatus,
    };
  });
}

// ---------------------------------------------------------------------------
// Contract deployment
// ---------------------------------------------------------------------------

export interface DeployRequest {
  chainId: number;
  rpcUrl: string;
  contractName: string;
  contractAddress: string;
  credentialId: string;
  signatureHex: string;
  challengeHex: string;
}

export interface DeployResult {
  txHash: string;
  status: 'pending' | 'confirmed' | 'failed';
}

/**
 * Estimated deployment gas per contract.
 * Based on actual bytecode sizes + CREATE2 overhead.
 * Larger contracts need more gas for deployment.
 */
const DEPLOY_GAS_ESTIMATES: Record<string, bigint> = {
  '0x29fcB43b46531BcA003ddC8FCB67FFE91900C762': 5_000_000n,  // Safe Singleton (~24KB)
  '0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67': 1_500_000n,  // Safe Proxy Factory (~3KB)
  '0x0000000071727De22E5E9d8BAf0edAc6f37da032': 6_000_000n,  // EntryPoint v0.7 (~16KB)
  '0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226': 3_000_000n,  // Safe 4337 Module (~8KB)
  '0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47': 500_000n,    // Safe Module Setup (~0.5KB)
  '0x94a4F6affBd8975951142c3999aEAB7ecee555c2': 1_500_000n,  // WebAuthn Signer (~3KB)
  '0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99': 2_500_000n,  // Fallback Handler (~5.6KB)
  '0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526': 500_000n,    // MultiSend (~0.6KB)
};
const DEFAULT_DEPLOY_GAS = 3_000_000n;

/** Safety multiplier: 2x buffer for gas price fluctuations. */
const GAS_BUFFER_MULTIPLIER = 2n;

/**
 * Estimate the required deployer balance for deploying missing contracts.
 *
 * Formula: sum(each contract's estimated gas) × gasPrice × 2 (buffer)
 */
export async function estimateDeployerCost(
  deployerAddress: string,
  chainId: number,
  missingContractAddresses: string[],
): Promise<{
  sufficient: boolean;
  balanceWei: bigint;
  balanceFormatted: string;
  requiredWei: bigint;
  requiredFormatted: string;
  shortfallWei: bigint;
  shortfallFormatted: string;
  gasPriceGwei: string;
}> {
  const [balanceWei, gasPrice] = await Promise.all([
    getBalance(deployerAddress, chainId),
    getGasPrice(chainId),
  ]);

  const totalGas = missingContractAddresses.reduce(
    (sum, addr) => sum + (DEPLOY_GAS_ESTIMATES[addr] ?? DEFAULT_DEPLOY_GAS), 0n,
  );
  const requiredWei = totalGas * gasPrice * GAS_BUFFER_MULTIPLIER;
  const shortfallWei = requiredWei > balanceWei ? requiredWei - balanceWei : 0n;

  return {
    sufficient: balanceWei >= requiredWei,
    balanceWei,
    balanceFormatted: formatWei(balanceWei),
    requiredWei,
    requiredFormatted: formatWei(requiredWei),
    shortfallWei,
    shortfallFormatted: formatWei(shortfallWei),
    gasPriceGwei: (Number(gasPrice) / 1e9).toFixed(1),
  };
}

async function getGasPrice(chainId: number): Promise<bigint> {
  try {
    const res = await rpcCall('eth_gasPrice', [], chainId);
    const hex = res.result as string | undefined;
    if (hex) return BigInt(hex);
  } catch {}
  return 50_000_000_000n; // fallback 50 gwei
}

/**
 * Build the challenge to sign for a deployment request.
 * challenge = keccak256(chainId + contractAddress + timestamp)
 */
export function buildDeployChallenge(chainId: number, contractAddress: string): { challengeHex: string; timestamp: number } {
  const timestamp = Math.floor(Date.now() / 1000);
  const encoder = new TextEncoder();
  const data = encoder.encode(`deploy:${chainId}:${contractAddress}:${timestamp}`);
  const hash = keccak256(data);
  return { challengeHex: toHexLight(hash), timestamp };
}

/**
 * Submit a contract deployment request to the deployer service.
 * TODO: Replace with real API call.
 *
 * Production flow:
 * 1. App signs challenge with passkey
 * 2. Sends to deployer service: { rpcUrl, chainId, contractAddress, signature, credentialId }
 * 3. Service verifies signature, uses deployer EOA to deploy contract on-chain
 * 4. Returns txHash
 */
export async function requestDeployment(req: DeployRequest): Promise<DeployResult> {
  // Mock: simulate a deployment with delay
  console.log(`[DeployerAPI] Mock deploy request:`, {
    chainId: req.chainId,
    contract: req.contractName,
    address: req.contractAddress,
    rpcUrl: req.rpcUrl,
  });

  await new Promise(r => setTimeout(r, 2000)); // Simulate network delay

  // TODO: Replace with real API call
  // const res = await fetch(`${DEPLOYER_SERVICE_URL}/api/deploy`, {
  //   method: 'POST',
  //   headers: { 'Content-Type': 'application/json' },
  //   body: JSON.stringify(req),
  // });
  // if (!res.ok) throw new Error('Deployment failed');
  // return res.json();

  return {
    txHash: '0x' + toHexLight(keccak256(new TextEncoder().encode(JSON.stringify(req)))).slice(0, 64),
    status: 'pending',
  };
}

/**
 * Poll for deployment transaction confirmation.
 */
export async function waitForDeployment(
  txHash: string,
  rpcUrl: string,
  timeoutMs: number = 60000,
): Promise<boolean> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    try {
      const controller = new AbortController();
      const t = setTimeout(() => controller.abort(), 10000);
      const res = await fetch(rpcUrl, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'eth_getTransactionReceipt', params: [txHash] }),
        signal: controller.signal,
      });
      clearTimeout(t);
      const json = await res.json();
      if (json.result?.status === '0x1') return true;
      if (json.result?.status === '0x0') return false;
    } catch {}
    await new Promise(r => setTimeout(r, 3000));
  }
  throw new Error('Deployment confirmation timed out');
}

// ---------------------------------------------------------------------------
// Hex helpers (lightweight, no import cycle)
// ---------------------------------------------------------------------------

function fromHexLight(hex: string): Uint8Array {
  const clean = hex.startsWith('0x') ? hex.slice(2) : hex;
  const bytes = new Uint8Array(clean.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = parseInt(clean.substr(i * 2, 2), 16);
  }
  return bytes;
}

function toHexLight(bytes: Uint8Array): string {
  return Array.from(bytes).map(b => b.toString(16).padStart(2, '0')).join('');
}
