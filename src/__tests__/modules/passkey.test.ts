/**
 * Tests for Passkey JS bridge interface.
 *
 * Tests the JS API surface, type contracts, error handling, and encoding
 * helpers. Native-side passkey operations require a real device and cannot
 * be exercised in a Node test runner.
 */

// Mock NativeModules — native module absent
jest.mock('react-native', () => ({
  NativeModules: { VelaPasskey: null },
  NativeEventEmitter: jest.fn().mockImplementation(() => ({
    addListener: jest.fn().mockReturnValue({ remove: jest.fn() }),
    removeAllListeners: jest.fn(),
  })),
  Platform: { OS: 'ios' },
}));

import {
  isSupported,
  MAX_USER_NAME_BYTES,
  register,
  authenticate,
  sign,
  encodeUserID,
  decodeUserName,
  decodeUserNameFromHandle,
  RELYING_PARTY,
  PasskeyErrorCode,
  type PasskeyRegistrationResult,
  type PasskeyAssertionResult,
} from '@/modules/passkey';

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

describe('Passkey constants', () => {
  test('RELYING_PARTY matches native implementations', () => {
    expect(RELYING_PARTY).toBe('getvela.app');
  });
});

// ---------------------------------------------------------------------------
// Error codes
// ---------------------------------------------------------------------------

describe('PasskeyErrorCode', () => {
  test('defines all required error codes', () => {
    expect(PasskeyErrorCode.CANCELLED).toBe('PASSKEY_CANCELLED');
    expect(PasskeyErrorCode.FAILED).toBe('PASSKEY_FAILED');
    expect(PasskeyErrorCode.NO_CREDENTIAL).toBe('PASSKEY_NO_CREDENTIAL');
    expect(PasskeyErrorCode.NOT_SUPPORTED).toBe('PASSKEY_NOT_SUPPORTED');
    expect(PasskeyErrorCode.NOT_AVAILABLE).toBe('PASSKEY_NOT_AVAILABLE');
    expect(PasskeyErrorCode.NOT_DISCOVERABLE).toBe('PASSKEY_NOT_DISCOVERABLE');
  });
});

// ---------------------------------------------------------------------------
// UserID encoding — pure JS, testable without native module
// ---------------------------------------------------------------------------

describe('encodeUserID', () => {
  test('encodes name with null byte separator and UUID suffix', () => {
    const encoded = encodeUserID('Alice');
    expect(encoded).toContain('Alice');
    expect(encoded).toContain('\0');
    // Should have name + \0 + UUID (36 chars)
    const parts = encoded.split('\0');
    expect(parts[0]).toBe('Alice');
    expect(parts[1].length).toBe(36); // UUID format
  });

  test('different calls produce different UUIDs', () => {
    const a = encodeUserID('Alice');
    const b = encodeUserID('Alice');
    expect(a).not.toBe(b); // UUID portion differs
  });

  test('handles empty name', () => {
    const encoded = encodeUserID('');
    expect(encoded.startsWith('\0')).toBe(true);
  });

  test('handles unicode names', () => {
    const encoded = encodeUserID('Vela');
    expect(encoded).toContain('Vela');
  });
});

describe('MAX_USER_NAME_BYTES', () => {
  test('a maximal name still fits WebAuthn\'s 64-byte user.id cap', () => {
    const maxAscii = 'x'.repeat(MAX_USER_NAME_BYTES);
    expect(new TextEncoder().encode(encodeUserID(maxAscii)).length).toBeLessThanOrEqual(64);
    // Multi-byte names count in BYTES, not characters: 9 CJK chars = 27 bytes.
    const maxCjk = '看'.repeat(Math.floor(MAX_USER_NAME_BYTES / 3));
    expect(new TextEncoder().encode(encodeUserID(maxCjk)).length).toBeLessThanOrEqual(64);
  });

  test('one byte over the limit exceeds the cap (documents the boundary)', () => {
    const oneOver = 'x'.repeat(MAX_USER_NAME_BYTES + 1);
    expect(new TextEncoder().encode(encodeUserID(oneOver)).length).toBeGreaterThan(64);
  });
});

describe('decodeUserName', () => {
  test('extracts name before null byte', () => {
    const name = decodeUserName('Alice\0some-uuid-here');
    expect(name).toBe('Alice');
  });

  test('returns full string if no null byte', () => {
    const name = decodeUserName('Alice');
    expect(name).toBe('Alice');
  });

  test('handles empty name before null byte', () => {
    const name = decodeUserName('\0uuid');
    expect(name).toBe('');
  });

  test('roundtrips with encodeUserID', () => {
    const encoded = encodeUserID('Bob');
    const decoded = decodeUserName(encoded);
    expect(decoded).toBe('Bob');
  });
});

describe('decodeUserNameFromHandle', () => {
  /** Hex of the UTF-8 bytes of `s` — how user.id crosses the bridge. */
  const handleHex = (s: string): string =>
    Array.from(new TextEncoder().encode(s), (b) => b.toString(16).padStart(2, '0')).join('');

  test('roundtrips an ASCII name', () => {
    expect(decodeUserNameFromHandle(handleHex(encodeUserID('Alice')))).toBe('Alice');
  });

  test('roundtrips a Chinese name (regression: Latin-1 decode garbled UTF-8)', () => {
    // The exact failure from issue #1's follow-up: a passkey named 看看书
    // recovered as mojibake because UTF-8 bytes were read as Latin-1.
    expect(decodeUserNameFromHandle(handleHex(encodeUserID('看看书')))).toBe('看看书');
    expect(decodeUserNameFromHandle(handleHex(encodeUserID('日本語ウォレット')))).toBe('日本語ウォレット');
    expect(decodeUserNameFromHandle(handleHex(encodeUserID('família €')))).toBe('família €');
  });

  test('rejects a random (non-UTF-8) foreign handle instead of leaking mojibake', () => {
    // Shape of the real-world case: ~23 random bytes as user.id from a
    // credential the app did not mint.
    expect(decodeUserNameFromHandle('5249d650b5b4f8e9d6e747457131dc61163613ad6291f621')).toBeNull();
  });

  test('rejects valid UTF-8 that is not name\\0uuid shaped', () => {
    expect(decodeUserNameFromHandle(handleHex('just a plain string'))).toBeNull(); // no separator
    expect(decodeUserNameFromHandle(handleHex('Alice\u0000not-a-uuid'))).toBeNull(); // bad uuid
    expect(decodeUserNameFromHandle(handleHex(encodeUserID('')))).toBeNull(); // empty name
  });

  test('rejects control characters in the name', () => {
    const uuid = '12345678-1234-4123-8123-123456789abc';
    expect(decodeUserNameFromHandle(handleHex(`Bad\u0007Name\u0000${uuid}`))).toBeNull();
  });

  test('returns null for missing input', () => {
    expect(decodeUserNameFromHandle(undefined)).toBeNull();
    expect(decodeUserNameFromHandle('')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// API — native module absent (graceful failure)
// ---------------------------------------------------------------------------

describe('Passkey API (native module absent)', () => {
  test('isSupported returns false', async () => {
    const result = await isSupported();
    expect(result).toBe(false);
  });

  test('register rejects with NOT_AVAILABLE', async () => {
    await expect(register('Alice')).rejects.toMatchObject({
      code: PasskeyErrorCode.NOT_AVAILABLE,
    });
  });

  test('authenticate rejects with NOT_AVAILABLE', async () => {
    await expect(authenticate()).rejects.toMatchObject({
      code: PasskeyErrorCode.NOT_AVAILABLE,
    });
  });

  test('sign rejects with NOT_AVAILABLE', async () => {
    await expect(sign('deadbeef')).rejects.toMatchObject({
      code: PasskeyErrorCode.NOT_AVAILABLE,
    });
  });

  test('sign with credentialId rejects with NOT_AVAILABLE', async () => {
    await expect(sign('deadbeef', 'cred123')).rejects.toMatchObject({
      code: PasskeyErrorCode.NOT_AVAILABLE,
    });
  });
});

// ---------------------------------------------------------------------------
// API — with mocked native module
// ---------------------------------------------------------------------------

describe('Passkey API (native module present)', () => {
  const mockRegResult: PasskeyRegistrationResult = {
    credentialId: 'abc123',
    attestationObjectHex: 'a163666d74...',
    clientDataJSONHex: '7b22747970...',
  };

  const mockAssertResult: PasskeyAssertionResult = {
    credentialId: 'abc123',
    signatureHex: '3045022100...',
    authenticatorDataHex: '49960de5...',
    clientDataJSONHex: '7b22747970...',
    userIdHex: '416c696365...',
  };

  let Passkey: typeof import('@/modules/passkey');

  beforeEach(() => {
    jest.resetModules();
    jest.doMock('react-native', () => ({
      NativeModules: {
        VelaPasskey: {
          isSupported: jest.fn().mockResolvedValue(true),
          register: jest.fn().mockResolvedValue(mockRegResult),
          authenticate: jest.fn().mockResolvedValue(mockAssertResult),
          sign: jest.fn().mockResolvedValue(mockAssertResult),
        },
      },
      NativeEventEmitter: jest.fn().mockImplementation(() => ({
        addListener: jest.fn().mockReturnValue({ remove: jest.fn() }),
        removeAllListeners: jest.fn(),
      })),
      Platform: { OS: 'ios' },
    }));
    Passkey = require('@/modules/passkey');
  });

  test('isSupported returns true', async () => {
    expect(await Passkey.isSupported()).toBe(true);
  });

  test('register passes userName and returns result', async () => {
    const result = await Passkey.register('Alice');
    expect(result).toEqual(mockRegResult);

    const { NativeModules } = require('react-native');
    expect(NativeModules.VelaPasskey.register).toHaveBeenCalledWith('Alice');
  });

  test('authenticate returns assertion result', async () => {
    const result = await Passkey.authenticate();
    expect(result).toEqual(mockAssertResult);
  });

  test('sign passes challenge hex', async () => {
    const result = await Passkey.sign('deadbeef');
    expect(result).toEqual(mockAssertResult);

    const { NativeModules } = require('react-native');
    expect(NativeModules.VelaPasskey.sign).toHaveBeenCalledWith('deadbeef', null);
  });

  test('sign passes credentialId when provided', async () => {
    await Passkey.sign('deadbeef', 'cred123');

    const { NativeModules } = require('react-native');
    expect(NativeModules.VelaPasskey.sign).toHaveBeenCalledWith('deadbeef', 'cred123');
  });
});

// ---------------------------------------------------------------------------
// Web registration — discoverable-credential guard (issue #1)
// ---------------------------------------------------------------------------

describe('Passkey web registration (discoverable credential guard)', () => {
  let Passkey: typeof import('@/modules/passkey');
  let createMock: jest.Mock;

  /** Minimal PublicKeyCredential stand-in for navigator.credentials.create(). */
  function fakeCredential(extensionResults: Record<string, unknown> | null) {
    return {
      rawId: new Uint8Array([0x01, 0x02, 0x03]).buffer,
      response: {
        attestationObject: new Uint8Array([0x04, 0x05]).buffer,
        clientDataJSON: new Uint8Array([0x06]).buffer,
      },
      ...(extensionResults !== null
        ? { getClientExtensionResults: () => extensionResults }
        : {}),
    };
  }

  beforeEach(() => {
    jest.resetModules();
    jest.doMock('react-native', () => ({
      NativeModules: {},
      Platform: { OS: 'web' },
    }));
    createMock = jest.fn();
    Object.defineProperty(globalThis, 'window', {
      value: {
        PublicKeyCredential: function PublicKeyCredential() {},
        location: { hostname: 'getvela.app' },
      },
      configurable: true,
    });
    // Node ≥21 exposes navigator as a getter — defineProperty to override it
    Object.defineProperty(globalThis, 'navigator', {
      value: { credentials: { create: createMock } },
      configurable: true,
    });
    Passkey = require('@/modules/passkey');
  });

  afterEach(() => {
    delete (globalThis as Record<string, unknown>).window;
    delete (globalThis as Record<string, unknown>).navigator;
  });

  test('requests a discoverable credential per WebAuthn L1 AND L2 fields, plus credProps', async () => {
    createMock.mockResolvedValue(fakeCredential({ credProps: { rk: true } }));
    await Passkey.register('Alice');

    const publicKey = createMock.mock.calls[0][0].publicKey;
    expect(publicKey.authenticatorSelection.residentKey).toBe('required');
    expect(publicKey.authenticatorSelection.requireResidentKey).toBe(true);
    expect(publicKey.extensions).toEqual({ credProps: true });
  });

  test('requests ES256 (P-256) only — no RS256 fallback', async () => {
    // The on-chain verifier (RIP-7212 precompile) and two-signature pubkey
    // recovery are both P-256 ECDSA — an RS256 credential can never become a
    // working wallet, so offering it would only delay the failure past
    // create() and leave an orphan passkey behind. Android is ES256-only too.
    createMock.mockResolvedValue(fakeCredential({ credProps: { rk: true } }));
    await Passkey.register('Alice');

    const publicKey = createMock.mock.calls[0][0].publicKey;
    expect(publicKey.pubKeyCredParams).toEqual([{ type: 'public-key', alg: -7 }]);
  });

  test('rejects with NOT_DISCOVERABLE when the client reports rk: false', async () => {
    createMock.mockResolvedValue(fakeCredential({ credProps: { rk: false } }));
    await expect(Passkey.register('Alice')).rejects.toMatchObject({
      code: PasskeyErrorCode.NOT_DISCOVERABLE,
    });
  });

  test('resolves when the client reports rk: true', async () => {
    createMock.mockResolvedValue(fakeCredential({ credProps: { rk: true } }));
    const result = await Passkey.register('Alice');
    expect(result.credentialId).toBe('010203');
  });

  test('resolves when credProps is absent (client cannot say — benefit of the doubt)', async () => {
    createMock.mockResolvedValue(fakeCredential({}));
    await expect(Passkey.register('Alice')).resolves.toMatchObject({
      credentialId: '010203',
    });
  });

  test('resolves when getClientExtensionResults is missing entirely', async () => {
    createMock.mockResolvedValue(fakeCredential(null));
    await expect(Passkey.register('Alice')).resolves.toMatchObject({
      credentialId: '010203',
    });
  });
});

// ---------------------------------------------------------------------------
// Type shape (compile-time, verified at runtime for documentation)
// ---------------------------------------------------------------------------

describe('Type contracts', () => {
  test('PasskeyRegistrationResult shape', () => {
    const result: PasskeyRegistrationResult = {
      credentialId: 'hex',
      attestationObjectHex: 'hex',
      clientDataJSONHex: 'hex',
    };
    expect(result.credentialId).toBeDefined();
    expect(result.attestationObjectHex).toBeDefined();
    expect(result.clientDataJSONHex).toBeDefined();
  });

  test('PasskeyAssertionResult shape', () => {
    const result: PasskeyAssertionResult = {
      credentialId: 'hex',
      signatureHex: 'hex',
      authenticatorDataHex: 'hex',
      clientDataJSONHex: 'hex',
    };
    expect(result.credentialId).toBeDefined();
    expect(result.signatureHex).toBeDefined();
    expect(result.authenticatorDataHex).toBeDefined();
    expect(result.clientDataJSONHex).toBeDefined();
    // userIdHex is optional
    expect(result.userIdHex).toBeUndefined();
  });
});
