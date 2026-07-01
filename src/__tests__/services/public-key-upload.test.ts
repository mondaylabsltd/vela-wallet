/**
 * Tests for public-key-upload — cross-device recovery sync (US 1.2).
 *
 * The critical invariant: the server VERIFY (queryRecord) is the source of truth,
 * not the create call. So the pending entry is cleared (and success reported) iff
 * the key is confirmed present on the server — never on an unconfirmed result, and
 * never faked. This is what keeps a wallet from looking "synced" locally while it's
 * actually unrecoverable on other devices. See CreateWalletScreen's don't-persist-
 * until-synced flow ([[project_store_launch_readiness]]).
 */

// Mock the network/storage/native deps so the four-quadrant logic runs offline.
jest.mock('@/modules/passkey', () => ({ getRelyingPartyId: () => 'getvela.app' }));
jest.mock('@/services/public-key-index', () => ({
  createRecord: jest.fn(),
  queryRecord: jest.fn(),
}));
jest.mock('@/services/storage', () => ({
  loadPendingUploads: jest.fn(),
  removePendingUpload: jest.fn(async () => {}),
}));

import {
  uploadPublicKey,
  validateCreateClientData,
  retryPendingUploads,
  PasskeyIncompatibleError,
} from '@/services/public-key-upload';
import * as PublicKeyIndex from '@/services/public-key-index';
import { loadPendingUploads, removePendingUpload } from '@/services/storage';
import { toHex } from '@/services/hex';

const createRecord = PublicKeyIndex.createRecord as jest.Mock;
const queryRecord = PublicKeyIndex.queryRecord as jest.Mock;

const PK = '04' + 'ab'.repeat(64); // uncompressed P-256 pubkey (shape only)
const PARAMS = { credentialId: 'cred-1', publicKeyHex: PK, name: 'Alice' };

function record(publicKey = PK): PublicKeyIndex.PublicKeyRecord {
  return { rpId: 'getvela.app', credentialId: 'cred-1', publicKey, name: 'Alice', createdAt: 0 };
}

beforeEach(() => jest.clearAllMocks());

describe('uploadPublicKey — verify is the source of truth', () => {
  test('create OK + verify matches → success, pending cleared', async () => {
    createRecord.mockResolvedValue(record());
    queryRecord.mockResolvedValue(record());
    await expect(uploadPublicKey(PARAMS)).resolves.toBeUndefined();
    expect(removePendingUpload).toHaveBeenCalledWith('cred-1');
  });

  test('create FAILS but verify confirms (already-exists / timeout-but-wrote) → success', async () => {
    createRecord.mockRejectedValue(new Error('503 under load'));
    queryRecord.mockResolvedValue(record());
    await expect(uploadPublicKey(PARAMS)).resolves.toBeUndefined();
    expect(removePendingUpload).toHaveBeenCalledWith('cred-1');
  });

  test('create OK but verify THROWS → stays pending, no fake success', async () => {
    createRecord.mockResolvedValue(record());
    queryRecord.mockRejectedValue(new Error('verify timeout'));
    await expect(uploadPublicKey(PARAMS)).rejects.toThrow('verify timeout');
    expect(removePendingUpload).not.toHaveBeenCalled();
  });

  test('create FAILS and verify THROWS → surfaces the CREATE error, stays pending', async () => {
    createRecord.mockRejectedValue(new Error('genuine 4xx'));
    queryRecord.mockRejectedValue(new Error('query 404'));
    // `throw createError ?? verifyErr` → the create error is preferred.
    await expect(uploadPublicKey(PARAMS)).rejects.toThrow('genuine 4xx');
    expect(removePendingUpload).not.toHaveBeenCalled();
  });

  test('verify returns a MISMATCHED key → throws, never clears pending', async () => {
    createRecord.mockResolvedValue(record());
    queryRecord.mockResolvedValue(record('04' + 'cd'.repeat(64)));
    await expect(uploadPublicKey(PARAMS)).rejects.toThrow(/public key mismatch/i);
    expect(removePendingUpload).not.toHaveBeenCalled();
  });
});

describe('validateCreateClientData — reject incompatible providers before saving', () => {
  const hexOf = (s: string) => toHex(new TextEncoder().encode(s));

  test('correct webauthn.create field order → no throw', () => {
    expect(() =>
      validateCreateClientData(hexOf('{"type":"webauthn.create","challenge":"abc","origin":"x"}')),
    ).not.toThrow();
  });

  test('wrong field order (challenge before type) → PasskeyIncompatibleError', () => {
    expect(() =>
      validateCreateClientData(hexOf('{"challenge":"abc","type":"webauthn.create"}')),
    ).toThrow(PasskeyIncompatibleError);
  });

  test('does not end with } → PasskeyIncompatibleError', () => {
    expect(() =>
      validateCreateClientData(hexOf('{"type":"webauthn.create","challenge":"abc"')),
    ).toThrow(PasskeyIncompatibleError);
  });
});

describe('retryPendingUploads', () => {
  test('no pending → {succeeded:0, failed:0}, no work', async () => {
    (loadPendingUploads as jest.Mock).mockResolvedValue([]);
    await expect(retryPendingUploads()).resolves.toEqual({ succeeded: 0, failed: 0 });
    expect(createRecord).not.toHaveBeenCalled();
  });

  test('counts successes and failures independently (one keeps retrying)', async () => {
    (loadPendingUploads as jest.Mock).mockResolvedValue([
      { id: 'a', publicKeyHex: PK, name: 'A' },
      { id: 'b', publicKeyHex: PK, name: 'B' },
    ]);
    // a: create+verify OK → success. b: both fail → stays failed.
    createRecord.mockResolvedValueOnce(record()).mockRejectedValueOnce(new Error('x'));
    queryRecord.mockResolvedValueOnce(record()).mockRejectedValueOnce(new Error('y'));
    await expect(retryPendingUploads()).resolves.toEqual({ succeeded: 1, failed: 1 });
    expect(removePendingUpload).toHaveBeenCalledTimes(1);
    expect(removePendingUpload).toHaveBeenCalledWith('a');
  });
});
