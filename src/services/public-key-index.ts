/**
 * Client for WebAuthn P256 Public Key Index API.
 *
 * The server stores public keys on Gnosis Chain. No signature/challenge
 * required — the server wallet signs transactions automatically.
 */

import { loadServiceEndpoints } from './storage';
import { DEFAULT_SERVICE_ENDPOINTS } from '@/models/types';

const FALLBACK_URL = DEFAULT_SERVICE_ENDPOINTS.passkeyIndexURL;

/** Cached base URL — refreshed from storage on each call to avoid stale config. */
let _cachedUrl: string | null = null;
let _cachedAt = 0;
const CACHE_TTL = 5_000; // 5s — re-read storage periodically in case user changes it

async function getBaseUrl(): Promise<string> {
  const now = Date.now();
  if (_cachedUrl && now - _cachedAt < CACHE_TTL) return _cachedUrl;
  try {
    const endpoints = await loadServiceEndpoints();
    _cachedUrl = endpoints.passkeyIndexURL?.trim().replace(/\/$/, '') || FALLBACK_URL;
  } catch {
    _cachedUrl = FALLBACK_URL;
  }
  _cachedAt = now;
  return _cachedUrl;
}

export interface PublicKeyRecord {
  rpId: string;
  credentialId: string;
  publicKey: string;
  name: string;
  initialCredentialId?: string;
  metadata?: string;
  createdAt: number;
}

interface CreateRequest {
  rpId: string;
  credentialId: string;
  publicKey: string;
  name: string;
  initialCredentialId?: string;
  metadata?: string;
}

/** Store a public key record. No signature needed — server signs on-chain tx. */
export async function createRecord(request: CreateRequest): Promise<PublicKeyRecord> {
  const baseUrl = await getBaseUrl();
  const response = await fetch(`${baseUrl}/api/create`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(request),
  });
  if (!response.ok) {
    const text = await response.text();
    throw new Error(`Create failed: ${response.status} ${text}`);
  }
  return response.json();
}

/** Query a public key by rpId and credentialId. */
export async function queryRecord(rpId: string, credentialId: string): Promise<PublicKeyRecord> {
  const baseUrl = await getBaseUrl();
  const url = `${baseUrl}/api/query?rpId=${encodeURIComponent(rpId)}&credentialId=${encodeURIComponent(credentialId)}`;
  const response = await fetch(url);
  if (!response.ok) throw new Error(`Query failed: ${response.status}`);
  return response.json();
}

/** Convert a 20-byte address to a bytes32 hex string (left-padded with zeros). */
function addressToBytes32(address: string): string {
  const stripped = address.toLowerCase().replace(/^0x/, '');
  return '0x' + stripped.padStart(64, '0');
}

/** Query a public key record by wallet address (walletRef). Returns null if not found. */
export async function queryByWalletRef(address: string): Promise<PublicKeyRecord | null> {
  const baseUrl = await getBaseUrl();
  const walletRef = addressToBytes32(address);
  const url = `${baseUrl}/api/query?walletRef=${encodeURIComponent(walletRef)}`;
  const response = await fetch(url);
  if (!response.ok) return null;
  return response.json();
}
