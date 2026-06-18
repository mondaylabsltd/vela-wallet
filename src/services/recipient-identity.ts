/**
 * Recipient identity resolution for the Send screen.
 *
 * Resolution priority:
 *   1. Passkey Index — Vela user lookup by walletRef
 *   2. Name services via on-chain RPC (no third-party API dependencies):
 *      - .bnb  (BSC, chainId 56)
 *      - .arb  (Arbitrum, chainId 42161)
 *      - Basenames (Base, chainId 8453) https://github.com/base/basenames
 *      - ENS   (Ethereum mainnet, chainId 1)
 *
 * Adding a new name service:
 *   Just add an entry to NAME_SERVICES below with the registry address and chainId.
 *   If it follows the ENS registry pattern (registry.resolver(node) → resolver.name(node)),
 *   it will work automatically.
 *
 * Only positive results are cached (AsyncStorage, 24h TTL).
 */

import AsyncStorage from '@react-native-async-storage/async-storage';
import { keccak256 } from './eth-crypto';
import { queryByWalletRef } from './public-key-index';
import { rpcCall } from './rpc-adapter';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type IdentitySource = 'passkey' | 'ens' | string;

export interface RecipientIdentity {
  /** Display name. */
  name: string;
  /** Source label for display (e.g. "ENS", "BNB", "Basename"). */
  source: IdentitySource;
}

// ---------------------------------------------------------------------------
// Name service registry — add new services here
// ---------------------------------------------------------------------------

interface NameServiceConfig {
  /** Human-readable label shown in UI. */
  label: string;
  /** Chain ID to send RPC calls to. */
  chainId: number;
  /** ENS-compatible Registry contract address on this chain. */
  registry: string;
  /**
   * Optional: ReverseRegistrar address for ENSIP-19 chains.
   * When set, we call `reverseRegistrar.node(address)` to get the chain-specific
   * reverse node instead of using the standard `namehash("<addr>.addr.reverse")`.
   */
  reverseRegistrar?: string;
}

const NAME_SERVICES: NameServiceConfig[] = [
  // SPACE ID name services (use their own SID registry per chain)
  { label: '.bnb',      chainId: 56,    registry: '0x08CEd32a7f3eeC915Ba84415e9C07a7286977956' },
  { label: '.arb',      chainId: 42161, registry: '0x4a067EE58e73ac5E4a43722E008DFdf65B2bF348' },
  { label: '.g',        chainId: 1625,  registry: '0x5dC881dDA4e4a8d312be3544AD13118D1a04Cb17' },
  // Basenames (Base chain) — ENSIP-19 uses chain-specific reverseNode
  // https://github.com/base/basenames
  { label: 'Basename',  chainId: 8453,  registry: '0xb94704422c2a1e396835a571837aa5ae53285a95', reverseRegistrar: '0x79ea96012eea67a83431f1701b3dff7e37f9e282' },
  // ENS on Ethereum mainnet
  { label: 'ENS',       chainId: 1,     registry: '0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e' },
];

// ---------------------------------------------------------------------------
// Cache (AsyncStorage, positive results only)
// ---------------------------------------------------------------------------

const CACHE_PREFIX = 'recipient_id:';
const CACHE_TTL = 24 * 60 * 60 * 1000; // 24 hours

interface CachedEntry {
  identity: RecipientIdentity;
  cachedAt: number;
}

async function getCache(address: string): Promise<RecipientIdentity | undefined> {
  try {
    const raw = await AsyncStorage.getItem(CACHE_PREFIX + address.toLowerCase());
    if (!raw) return undefined;
    const entry: CachedEntry = JSON.parse(raw);
    if (Date.now() - entry.cachedAt > CACHE_TTL) return undefined;
    return entry.identity;
  } catch {
    return undefined;
  }
}

async function setCache(address: string, identity: RecipientIdentity): Promise<void> {
  try {
    const entry: CachedEntry = { identity, cachedAt: Date.now() };
    await AsyncStorage.setItem(CACHE_PREFIX + address.toLowerCase(), JSON.stringify(entry));
  } catch { /* best-effort */ }
}

// ---------------------------------------------------------------------------
// ENS-compatible reverse resolution via raw RPC
// ---------------------------------------------------------------------------

/**
 * ENS namehash algorithm (EIP-137).
 * namehash("") = 0x00...00
 * namehash("eth") = keccak256(namehash("") + keccak256("eth"))
 */
function namehash(name: string): string {
  let node = new Uint8Array(32) as Uint8Array; // 0x00...00
  if (!name) return toHex(node);

  const labels = name.split('.');
  for (let i = labels.length - 1; i >= 0; i--) {
    const labelHash = keccak256(new TextEncoder().encode(labels[i]));
    const combined = new Uint8Array(64);
    combined.set(node, 0);
    combined.set(labelHash, 32);
    node = keccak256(combined);
  }
  return toHex(node);
}

function toHex(bytes: Uint8Array): string {
  let hex = '0x';
  for (const b of bytes) hex += b.toString(16).padStart(2, '0');
  return hex;
}

// Function selectors
const RESOLVER_SELECTOR = '0x0178b8bf'; // resolver(bytes32)
const NAME_SELECTOR = '0x691f3431';     // name(bytes32)
const NODE_SELECTOR = '0xbffbe61c';     // node(address) — ENSIP-19 ReverseRegistrar

/**
 * Reverse-resolve an address using an ENS-compatible registry.
 *
 * Standard flow: registry.resolver(namehash(addr.addr.reverse)) → resolver.name(node)
 * ENSIP-19 flow: reverseRegistrar.node(addr) → registry.resolver(node) → resolver.name(node)
 */
async function reverseResolveRegistry(
  address: string,
  config: NameServiceConfig,
): Promise<string | null> {
  try {
    let reverseNode: string;

    if (config.reverseRegistrar) {
      // ENSIP-19: call reverseRegistrar.node(address) to get chain-specific reverse node
      const addrPadded = '000000000000000000000000' + address.toLowerCase().slice(2);
      const nodeCalldata = NODE_SELECTOR + addrPadded;
      const nodeRes = await rpcCall(
        'eth_call',
        [{ to: config.reverseRegistrar, data: nodeCalldata }, 'latest'],
        config.chainId,
      );
      if (nodeRes.error || !nodeRes.result || nodeRes.result === '0x' || (nodeRes.result as string).length < 66) return null;
      reverseNode = nodeRes.result as string;
    } else {
      // Standard: namehash("<addr>.addr.reverse")
      const addr = address.toLowerCase().slice(2);
      reverseNode = namehash(`${addr}.addr.reverse`);
    }

    // Step 1: registry.resolver(node) → address
    const resolverCalldata = RESOLVER_SELECTOR + reverseNode.slice(2);
    const resolverRes = await rpcCall(
      'eth_call',
      [{ to: config.registry, data: resolverCalldata }, 'latest'],
      config.chainId,
    );
    if (resolverRes.error || !resolverRes.result || resolverRes.result === '0x') return null;

    const resolverAddr = '0x' + (resolverRes.result as string).slice(26);
    // Check resolver is not zero address
    if (/^0x0+$/.test(resolverAddr)) return null;

    // Step 2: resolver.name(node) → string
    const nameCalldata = NAME_SELECTOR + reverseNode.slice(2);
    const nameRes = await rpcCall(
      'eth_call',
      [{ to: resolverAddr, data: nameCalldata }, 'latest'],
      config.chainId,
    );
    if (nameRes.error || !nameRes.result || nameRes.result === '0x') return null;

    const name = decodeString(nameRes.result as string);
    if (!name || name.length === 0) return null;
    return name;
  } catch {
    return null;
  }
}

/**
 * Decode a Solidity string return value from ABI-encoded hex.
 * ABI: offset (32 bytes) + length (32 bytes) + data (padded)
 */
function decodeString(hex: string): string | null {
  try {
    const data = hex.startsWith('0x') ? hex.slice(2) : hex;
    if (data.length < 128) return null;

    // offset to string data
    const offset = parseInt(data.slice(0, 64), 16) * 2;
    // string length
    const strLen = parseInt(data.slice(offset, offset + 64), 16);
    if (strLen === 0 || strLen > 256) return null;
    // string bytes
    const strHex = data.slice(offset + 64, offset + 64 + strLen * 2);
    const bytes = new Uint8Array(strLen);
    for (let i = 0; i < strLen; i++) {
      bytes[i] = parseInt(strHex.slice(i * 2, i * 2 + 2), 16);
    }
    return new TextDecoder().decode(bytes);
  } catch {
    return null;
  }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Resolve a recipient address to a display identity.
 *
 *   1. Local cache
 *   2. Passkey Index (Vela user)
 *   3. Name services in order: .bnb → .arb → Basename → ENS
 */
export async function resolveRecipientIdentity(address: string): Promise<RecipientIdentity | null> {
  if (!/^0x[0-9a-fA-F]{40}$/.test(address)) return null;
  // The zero address is a mint/burn counterparty (e.g. EIP-7708 native events),
  // not a real recipient — it has no identity and would 404 the passkey index.
  if (/^0x0{40}$/.test(address)) return null;

  // 1. Cache
  const cached = await getCache(address);
  if (cached !== undefined) return cached;

  // 2. Passkey Index
  try {
    const record = await queryByWalletRef(address);
    if (record?.name) {
      const identity: RecipientIdentity = { name: record.name, source: 'passkey' };
      await setCache(address, identity);
      return identity;
    }
  } catch { /* continue */ }

  // 3. Name services — query all in parallel, return first match by priority
  const results = await Promise.allSettled(
    NAME_SERVICES.map((config) => reverseResolveRegistry(address, config)),
  );

  for (let i = 0; i < results.length; i++) {
    const r = results[i];
    if (r.status === 'fulfilled' && r.value) {
      const identity: RecipientIdentity = { name: r.value, source: NAME_SERVICES[i].label };
      await setCache(address, identity);
      return identity;
    }
  }

  return null;
}
