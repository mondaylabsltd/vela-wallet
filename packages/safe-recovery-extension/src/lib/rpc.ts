import { isHex } from 'viem';
import { MAX_RPC_RESPONSE_BYTES, RPC_TIMEOUT_MS } from './constants';
import { providerError } from './errors';

let rpcId = 0;

export function normalizeRpcUrl(raw: string): string {
  let url: URL;
  try {
    url = new URL(raw.trim());
  } catch {
    throw providerError(-32602, 'RPC URL is invalid.');
  }

  const local = url.hostname === 'localhost' || url.hostname === '127.0.0.1';
  if (url.protocol !== 'https:' && !(local && url.protocol === 'http:')) {
    throw providerError(-32602, 'RPC must use HTTPS (HTTP is allowed only for localhost).');
  }
  if (url.username || url.password) {
    throw providerError(-32602, 'RPC URLs with embedded username/password are not allowed.');
  }
  if (raw.length > 2_048) {
    throw providerError(-32602, 'RPC URL is too long.');
  }
  return url.toString().replace(/\/$/, '');
}

export function permissionPatternForUrl(raw: string): string {
  const url = new URL(normalizeRpcUrl(raw));
  return `${url.protocol}//${url.host}/*`;
}

export async function rpcCallAt<T = unknown>(rpcUrl: string, method: string, params: unknown = []): Promise<T> {
  const url = normalizeRpcUrl(rpcUrl);
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), RPC_TIMEOUT_MS);

  let response: Response;
  try {
    response = await fetch(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ jsonrpc: '2.0', id: ++rpcId, method, params: params ?? [] }),
      signal: controller.signal,
    });
  } catch (error) {
    if ((error as Error)?.name === 'AbortError') {
      throw providerError(-32603, `RPC timed out while calling ${method}.`);
    }
    throw providerError(-32603, `RPC unavailable: ${(error as Error)?.message ?? 'network error'}`);
  } finally {
    clearTimeout(timer);
  }

  if (!response.ok) {
    throw providerError(-32603, `RPC HTTP ${response.status}: ${response.statusText}`);
  }
  const length = Number(response.headers.get('content-length') ?? '0');
  if (length > MAX_RPC_RESPONSE_BYTES) {
    throw providerError(-32603, 'RPC response is too large.');
  }
  const text = await response.text();
  if (text.length > MAX_RPC_RESPONSE_BYTES) {
    throw providerError(-32603, 'RPC response is too large.');
  }

  let json: { result?: T; error?: { code?: number; message?: string; data?: unknown } };
  try {
    json = JSON.parse(text) as typeof json;
  } catch {
    throw providerError(-32603, 'RPC returned malformed JSON.');
  }
  if (json.error) {
    throw providerError(json.error.code ?? -32603, json.error.message ?? 'RPC error', json.error.data);
  }
  return json.result as T;
}

export async function verifyRpcChain(rpcUrl: string, expectedChainId: number): Promise<void> {
  const result = await rpcCallAt<string>(rpcUrl, 'eth_chainId', []);
  if (!isHex(result)) throw providerError(-32603, 'RPC returned an invalid chain ID.');
  const actual = Number.parseInt(result, 16);
  if (actual !== expectedChainId) {
    throw providerError(4902, `RPC is chain ${actual}, expected chain ${expectedChainId}.`);
  }
}
