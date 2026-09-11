/**
 * Shared plumbing for the multi-passkey Safe e2e: wasm loading, key
 * management, and the constants every step agrees on.
 *
 * Keys are throwaway TEST keys persisted to state/keys.json (gitignored) so
 * the compute → fund → deploy steps operate on the same wallet across runs.
 * Never point this at keys that guard real funds.
 */
import { readFileSync, writeFileSync, existsSync, mkdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { p256 } from '@noble/curves/p256';

export const REPO = join(dirname(fileURLToPath(import.meta.url)), '..', '..', '..', '..');
export const STATE_DIR = join(dirname(fileURLToPath(import.meta.url)), '..', 'state');
/** Override with STATE_FILE to operate an archived wallet (e.g. sweep funds
 * out of a previous keyset) without touching the active one. */
export const KEYS_FILE = process.env.STATE_FILE
  ? join(dirname(fileURLToPath(import.meta.url)), '..', process.env.STATE_FILE)
  : join(STATE_DIR, 'keys.json');

export const GNOSIS_RPC = process.env.GNOSIS_RPC ?? 'https://rpc.gnosischain.com';
export const CHAIN_ID = 100n;

// eslint-disable-next-line @typescript-eslint/no-require-imports
export const wasm = await (async () => {
  const mod = await import(join(REPO, 'rust', 'pkg-web', 'vela_core.js'));
  const { WASM_URL } = await import(join(REPO, 'rust', 'pkg-web', 'vela_core_wasm_url.js'));
  mod.initSync({ module: readFileSync(join(REPO, 'assets', 'wasm', WASM_URL.replace(/^\//, ''))) });
  return mod;
})();

export interface StoredKey {
  /** 32-byte hex, no 0x */
  privateKey: string;
  /** 32-byte hex each, no 0x */
  x: string;
  y: string;
}

export interface WalletState {
  keys: StoredKey[];
  safeAddress: string;
  saltNonce: string;
  setupData: string;
  initCodeHash: string;
}

export const hexToBytes = (hex: string): Uint8Array =>
  Uint8Array.from(Buffer.from(hex.replace(/^0x/, ''), 'hex'));
export const bytesToHex = (b: Uint8Array): string => Buffer.from(b).toString('hex');

export function generateKeys(count: number): StoredKey[] {
  return Array.from({ length: count }, () => {
    const priv = p256.utils.randomPrivateKey();
    const pub = p256.getPublicKey(priv, false); // 04 ‖ x ‖ y
    return {
      privateKey: bytesToHex(priv),
      x: bytesToHex(pub.slice(1, 33)),
      y: bytesToHex(pub.slice(33, 65)),
    };
  });
}

/** Flat 64-byte x‖y blocks — the computeSafeAddressMulti input convention. */
export function packKeys(keys: StoredKey[]): Uint8Array {
  const flat = new Uint8Array(keys.length * 64);
  keys.forEach((k, i) => {
    flat.set(hexToBytes(k.x), i * 64);
    flat.set(hexToBytes(k.y), i * 64 + 32);
  });
  return flat;
}

export function saveState(state: WalletState): void {
  mkdirSync(STATE_DIR, { recursive: true });
  writeFileSync(KEYS_FILE, JSON.stringify(state, null, 2));
}

export function loadState(): WalletState {
  if (!existsSync(KEYS_FILE)) {
    throw new Error(`no ${KEYS_FILE} — run \`bun run generate\` first`);
  }
  return JSON.parse(readFileSync(KEYS_FILE, 'utf8')) as WalletState;
}

export async function rpc(method: string, params: unknown[]): Promise<unknown> {
  const res = await fetch(GNOSIS_RPC, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
  });
  const body = (await res.json()) as { result?: unknown; error?: { message: string } };
  if (body.error) throw new Error(`${method}: ${body.error.message}`);
  return body.result;
}

export const RELAY_URL = process.env.RELAY_URL ?? 'https://vela-relay.getvela.app';

export async function relayCall(method: string, params: unknown[]): Promise<unknown> {
  const res = await fetch(`${RELAY_URL}/${CHAIN_ID}`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
  });
  const body = (await res.json()) as {
    result?: unknown;
    error?: { code: number; message: string };
  };
  if (body.error) throw new Error(`${method}: [${body.error.code}] ${body.error.message}`);
  return body.result;
}

/** Poll eth_getUserOperationReceipt until mined; exits the process on result. */
export async function awaitReceipt(userOpHash: string): Promise<never> {
  for (let i = 0; i < 45; i++) {
    await new Promise((r) => setTimeout(r, 4000));
    const receipt = (await relayCall('eth_getUserOperationReceipt', [userOpHash])) as {
      success: boolean;
      receipt?: { transactionHash: string };
    } | null;
    if (receipt) {
      console.log(`mined:  success=${receipt.success} tx=${receipt.receipt?.transactionHash}`);
      console.log(`        https://gnosisscan.io/tx/${receipt.receipt?.transactionHash}`);
      process.exit(receipt.success ? 0 : 1);
    }
    console.log(`…waiting (${(i + 1) * 4}s)`);
  }
  throw new Error('timed out waiting for receipt — check eth_getUserOperationStatus manually');
}
