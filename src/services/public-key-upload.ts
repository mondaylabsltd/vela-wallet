/**
 * Uploads passkey public keys to the index server for cross-device recovery.
 *
 * Flow: createRecord → verify (no signature needed, server signs on-chain tx)
 */
import * as PublicKeyIndex from './public-key-index';
import { RELYING_PARTY } from '@/modules/passkey';
import { fromHex } from './hex';
import { loadPendingUploads, removePendingUpload } from './storage';

// ---------------------------------------------------------------------------
// Safe WebAuthn Compatibility Validation
// ---------------------------------------------------------------------------

/**
 * Error thrown when the passkey provider is not compatible with Safe contracts.
 * This is NOT retryable — the device/provider cannot be used.
 */
export class PasskeyIncompatibleError extends Error {
  readonly reason: string;
  constructor(reason: string) {
    super(`Passkey provider incompatible: ${reason}`);
    this.name = 'PasskeyIncompatibleError';
    this.reason = reason;
  }
}

/**
 * Validate the registration (webauthn.create) clientDataJSON for Safe compatibility.
 *
 * If the provider outputs wrong field order for create, it will do the same for get.
 * This lets us reject incompatible providers BEFORE saving anything.
 *
 * Expected: {"type":"webauthn.create","challenge":"<base64url>", ...}
 */
export function validateCreateClientData(clientDataJSONHex: string): void {
  const clientDataBytes = fromHex(clientDataJSONHex);
  const clientDataJSON = new TextDecoder().decode(clientDataBytes);

  const requiredPrefix = '{"type":"webauthn.create","challenge":"';
  if (!clientDataJSON.startsWith(requiredPrefix)) {
    const actualStart = clientDataJSON.slice(0, 80);
    console.error('[SafeCompat] CREATE clientDataJSON prefix mismatch');
    console.error('[SafeCompat] Expected prefix:', requiredPrefix);
    console.error('[SafeCompat] Actual start:', actualStart);
    throw new PasskeyIncompatibleError(
      'Your device\'s passkey provider produces an incompatible response format. ' +
      'The clientDataJSON field order does not match Safe contract requirements. ' +
      'Please try a different passkey provider or device.\n\n' +
      'Got: ' + actualStart,
    );
  }

  if (!clientDataJSON.endsWith('}')) {
    throw new PasskeyIncompatibleError('clientDataJSON does not end with }');
  }

  console.log('[SafeCompat] CREATE clientDataJSON format OK');
}

// ---------------------------------------------------------------------------
// Upload
// ---------------------------------------------------------------------------

/**
 * Upload a public key to the index server, then verify it was stored.
 * No signature needed — server signs on-chain tx automatically.
 */
export async function uploadPublicKey(params: {
  credentialId: string;
  publicKeyHex: string;
  name: string;
}): Promise<void> {
  const { credentialId, publicKeyHex, name } = params;

  console.log('[PublicKeyUpload] Starting upload for:', name);

  // 1. Upload to server (no challenge/signature needed)
  await PublicKeyIndex.createRecord({
    rpId: RELYING_PARTY,
    credentialId,
    publicKey: publicKeyHex,
    name,
  });
  console.log('[PublicKeyUpload] Upload SUCCESS for:', name);

  // 2. Verify: query server to confirm the record exists
  const record = await PublicKeyIndex.queryRecord(RELYING_PARTY, credentialId);
  if (record.publicKey !== publicKeyHex) {
    throw new Error('Server verification failed: public key mismatch');
  }
  console.log('[PublicKeyUpload] Verified on server for:', name);

  // 3. Remove from pending uploads
  await removePendingUpload(credentialId);
}

/**
 * Retry all pending public key uploads.
 * No biometric needed — safe to call silently on app launch.
 */
export async function retryPendingUploads(): Promise<{
  succeeded: number;
  failed: number;
}> {
  const pending = await loadPendingUploads();
  if (pending.length === 0) return { succeeded: 0, failed: 0 };
  console.log('[PublicKeyUpload] Retrying', pending.length, 'pending uploads');

  let succeeded = 0;
  let failed = 0;

  for (const upload of pending) {
    try {
      await uploadPublicKey({
        credentialId: upload.id,
        publicKeyHex: upload.publicKeyHex,
        name: upload.name,
      });
      succeeded++;
    } catch (err) {
      failed++;
      console.error('[PublicKeyUpload] Retry FAILED for', upload.name, ':', err instanceof Error ? err.message : String(err));
    }
  }

  console.log('[PublicKeyUpload] Retry complete:', succeeded, 'succeeded,', failed, 'failed');
  return { succeeded, failed };
}
