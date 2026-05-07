/**
 * Client for WebAuthn P256 Public Key Index API.
 *
 * The server stores public keys on Gnosis Chain. No signature/challenge
 * required — the server wallet signs transactions automatically.
 */

const BASE_URL = 'https://webauthnp256-publickey-index.biubiu.tools';

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
  const response = await fetch(`${BASE_URL}/api/create`, {
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
  const url = `${BASE_URL}/api/query?rpId=${encodeURIComponent(rpId)}&credentialId=${encodeURIComponent(credentialId)}`;
  const response = await fetch(url);
  if (!response.ok) throw new Error(`Query failed: ${response.status}`);
  return response.json();
}
