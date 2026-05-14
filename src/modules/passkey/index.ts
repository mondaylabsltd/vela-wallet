/**
 * Passkey (WebAuthn) module — unified API across iOS, Android, and Web.
 *
 * - iOS:     ASAuthorization (via VelaPasskey native module)
 * - Android: Credential Manager (via VelaPasskey native module)
 * - Web:     navigator.credentials (WebAuthn API, no native module needed)
 *
 * Design principles:
 *   - All data crosses as hex strings (no base64 ambiguity)
 *   - Errors carry a typed code so callers can branch (cancelled vs. failed)
 *   - The module is stateless — no cached credentials or sessions
 */

import { NativeModules, Platform } from 'react-native';

const { VelaPasskey } = NativeModules;
const isWeb = Platform.OS === 'web';

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

export const RELYING_PARTY_NATIVE = 'getvela.app';

/**
 * Get the relying party ID for WebAuthn.
 *
 * Native: always `getvela.app`.
 * Web: extract the registrable domain from hostname so passkeys work
 *      across subdomains (e.g. `wallet.getvela.app` → `getvela.app`).
 *      For localhost / IP addresses, use hostname as-is.
 */
export function getRelyingPartyId(): string {
  if (isWeb && typeof window !== 'undefined') {
    // When the WebAuthn proxy extension is installed it sets this global
    // so that rpId is consistent across WebAuthn calls AND server queries.
    const proxyRpId = (window as any).__VELA_WEBAUTHN_PROXY_RPID__;
    if (proxyRpId) return proxyRpId;

    const hostname = window.location.hostname;
    // localhost or IP address — use as-is
    if (hostname === 'localhost' || /^\d+\.\d+\.\d+\.\d+$/.test(hostname) || hostname === '127.0.0.1') {
      return hostname;
    }
    // Only extract registrable domain for *.getvela.app subdomains.
    // Other deployment hosts (e.g. pages.dev, github.io) use the
    // native rpId so passkeys stay consistent across environments.
    if (hostname.endsWith('.' + RELYING_PARTY_NATIVE)) {
      return RELYING_PARTY_NATIVE;
    }
    return hostname === RELYING_PARTY_NATIVE ? hostname : RELYING_PARTY_NATIVE;
  }
  return RELYING_PARTY_NATIVE;
}

/** @deprecated Use getRelyingPartyId() — kept for backward compat in imports */
export const RELYING_PARTY = RELYING_PARTY_NATIVE;

// ---------------------------------------------------------------------------
// Error model
// ---------------------------------------------------------------------------

export const PasskeyErrorCode = {
  CANCELLED: 'PASSKEY_CANCELLED',
  FAILED: 'PASSKEY_FAILED',
  NO_CREDENTIAL: 'PASSKEY_NO_CREDENTIAL',
  NOT_SUPPORTED: 'PASSKEY_NOT_SUPPORTED',
  NOT_AVAILABLE: 'PASSKEY_NOT_AVAILABLE',
} as const;

export type PasskeyErrorCode = (typeof PasskeyErrorCode)[keyof typeof PasskeyErrorCode];

export class PasskeyError extends Error {
  readonly code: PasskeyErrorCode;
  constructor(code: PasskeyErrorCode, message: string) {
    super(message);
    this.name = 'PasskeyError';
    this.code = code;
  }
}

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

export interface PasskeyRegistrationResult {
  credentialId: string;
  attestationObjectHex: string;
  clientDataJSONHex: string;
}

export interface PasskeyAssertionResult {
  credentialId: string;
  signatureHex: string;
  authenticatorDataHex: string;
  clientDataJSONHex: string;
  userIdHex?: string;
}

// ---------------------------------------------------------------------------
// UserID helpers
// ---------------------------------------------------------------------------

export function encodeUserID(name: string): string {
  return `${name}\0${generateUUID()}`;
}

export function decodeUserName(userID: string): string {
  const idx = userID.indexOf('\0');
  return idx === -1 ? userID : userID.slice(0, idx);
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

export async function isSupported(): Promise<boolean> {
  if (isWeb) {
    return typeof window !== 'undefined' &&
      !!window.PublicKeyCredential &&
      typeof navigator.credentials?.create === 'function';
  }
  if (!VelaPasskey) return false;
  try { return await VelaPasskey.isSupported(); } catch { return false; }
}

export async function register(userName: string): Promise<PasskeyRegistrationResult> {
  if (isWeb) return webRegister(userName);
  assertNativeAvailable();
  try { return await VelaPasskey.register(userName); }
  catch (err) { throw normalizeError(err); }
}

export async function authenticate(): Promise<PasskeyAssertionResult> {
  if (isWeb) return webAuthenticate();
  assertNativeAvailable();
  try { return await VelaPasskey.authenticate(); }
  catch (err) { throw normalizeError(err); }
}

export async function sign(
  challengeHex: string,
  credentialId?: string | null,
): Promise<PasskeyAssertionResult> {
  if (isWeb) return webSign(challengeHex, credentialId ?? null);
  assertNativeAvailable();
  try { return await VelaPasskey.sign(challengeHex, credentialId ?? null); }
  catch (err) { throw normalizeError(err); }
}

// ---------------------------------------------------------------------------
// Web implementation (WebAuthn API)
// ---------------------------------------------------------------------------

async function webRegister(userName: string): Promise<PasskeyRegistrationResult> {
  assertWebSupported();
  const userId = new TextEncoder().encode(encodeUserID(userName));
  const challenge = crypto.getRandomValues(new Uint8Array(32));

  try {
    const credential = await navigator.credentials.create({
      publicKey: {
        rp: { id: getRelyingPartyId(), name: 'Vela Wallet' },
        user: {
          id: userId,
          name: userName,
          displayName: userName,
        },
        challenge,
        pubKeyCredParams: [
          { type: 'public-key', alg: -7 },  // ES256 (P-256) — preferred
          { type: 'public-key', alg: -257 }, // RS256 — fallback for compatibility
        ],
        authenticatorSelection: {
          authenticatorAttachment: 'platform',
          residentKey: 'required',
          userVerification: 'required',
        },
        attestation: 'direct',
      },
    }) as PublicKeyCredential | null;

    if (!credential) {
      throw new PasskeyError(PasskeyErrorCode.NO_CREDENTIAL, 'No credential returned');
    }

    const response = credential.response as AuthenticatorAttestationResponse;
    return {
      credentialId: bufToHex(credential.rawId),
      attestationObjectHex: bufToHex(response.attestationObject),
      clientDataJSONHex: bufToHex(response.clientDataJSON),
    };
  } catch (err) {
    throw normalizeWebError(err);
  }
}

async function webAuthenticate(): Promise<PasskeyAssertionResult> {
  assertWebSupported();
  const challenge = crypto.getRandomValues(new Uint8Array(32));

  try {
    const credential = await navigator.credentials.get({
      publicKey: {
        challenge,
        rpId: getRelyingPartyId(),
        userVerification: 'required',
      },
    }) as PublicKeyCredential | null;

    if (!credential) {
      throw new PasskeyError(PasskeyErrorCode.NO_CREDENTIAL, 'No credential returned');
    }

    return parseAssertionResponse(credential);
  } catch (err) {
    throw normalizeWebError(err);
  }
}

async function webSign(
  challengeHex: string,
  credentialId: string | null,
): Promise<PasskeyAssertionResult> {
  assertWebSupported();
  const challenge = hexToBuf(challengeHex);

  const options: PublicKeyCredentialRequestOptions = {
    challenge: challenge as BufferSource,
    rpId: getRelyingPartyId(),
    userVerification: 'required',
  };

  if (credentialId) {
    options.allowCredentials = [{
      type: 'public-key',
      id: hexToBuf(credentialId) as BufferSource,
    }];
  }

  try {
    const credential = await navigator.credentials.get({
      publicKey: options,
    }) as PublicKeyCredential | null;

    if (!credential) {
      throw new PasskeyError(PasskeyErrorCode.NO_CREDENTIAL, 'No credential returned');
    }

    return parseAssertionResponse(credential);
  } catch (err) {
    throw normalizeWebError(err);
  }
}

function parseAssertionResponse(credential: PublicKeyCredential): PasskeyAssertionResult {
  const response = credential.response as AuthenticatorAssertionResponse;
  const result: PasskeyAssertionResult = {
    credentialId: bufToHex(credential.rawId),
    signatureHex: bufToHex(response.signature),
    authenticatorDataHex: bufToHex(response.authenticatorData),
    clientDataJSONHex: bufToHex(response.clientDataJSON),
  };
  if (response.userHandle && response.userHandle.byteLength > 0) {
    result.userIdHex = bufToHex(response.userHandle);
  }
  return result;
}

function assertWebSupported(): void {
  if (typeof window === 'undefined' || !window.PublicKeyCredential) {
    throw new PasskeyError(
      PasskeyErrorCode.NOT_SUPPORTED,
      'WebAuthn is not supported in this browser.',
    );
  }
}

function normalizeWebError(err: unknown): PasskeyError {
  if (err instanceof PasskeyError) return err;
  const e = err as DOMException;
  if (e?.name === 'NotAllowedError' || e?.name === 'AbortError') {
    return new PasskeyError(PasskeyErrorCode.CANCELLED, 'User cancelled the operation');
  }
  if (e?.name === 'InvalidStateError') {
    return new PasskeyError(PasskeyErrorCode.FAILED, 'Credential already exists');
  }
  return new PasskeyError(
    PasskeyErrorCode.FAILED,
    e?.message ?? 'Unknown WebAuthn error',
  );
}

// ---------------------------------------------------------------------------
// Native helpers
// ---------------------------------------------------------------------------

function assertNativeAvailable(): void {
  if (!VelaPasskey) {
    throw new PasskeyError(
      PasskeyErrorCode.NOT_AVAILABLE,
      'VelaPasskey native module is not available.',
    );
  }
}

function normalizeError(err: unknown): PasskeyError {
  if (err instanceof PasskeyError) return err;
  const raw = err as { code?: string; message?: string };
  const code = mapNativeCode(raw.code);
  return new PasskeyError(code, raw.message ?? 'Unknown passkey error');
}

function mapNativeCode(code?: string): PasskeyErrorCode {
  switch (code) {
    case 'PASSKEY_CANCELLED': return PasskeyErrorCode.CANCELLED;
    case 'PASSKEY_NO_CREDENTIAL': return PasskeyErrorCode.NO_CREDENTIAL;
    case 'PASSKEY_NOT_SUPPORTED': return PasskeyErrorCode.NOT_SUPPORTED;
    default: return PasskeyErrorCode.FAILED;
  }
}

// ---------------------------------------------------------------------------
// Encoding helpers
// ---------------------------------------------------------------------------

function bufToHex(buf: ArrayBuffer): string {
  const bytes = new Uint8Array(buf);
  let hex = '';
  for (let i = 0; i < bytes.length; i++) {
    hex += bytes[i].toString(16).padStart(2, '0');
  }
  return hex;
}

function hexToBuf(hex: string): Uint8Array {
  const clean = hex.startsWith('0x') ? hex.slice(2) : hex;
  const arr = new Uint8Array(clean.length / 2);
  for (let i = 0; i < arr.length; i++) {
    arr[i] = parseInt(clean.slice(i * 2, i * 2 + 2), 16);
  }
  return arr;
}

function generateUUID(): string {
  const hex = '0123456789abcdef';
  let uuid = '';
  for (let i = 0; i < 36; i++) {
    if (i === 8 || i === 13 || i === 18 || i === 23) uuid += '-';
    else if (i === 14) uuid += '4';
    else if (i === 19) uuid += hex[(Math.random() * 4) | 8];
    else uuid += hex[(Math.random() * 16) | 0];
  }
  return uuid;
}
