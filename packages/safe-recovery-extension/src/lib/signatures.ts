import {
  encodeAbiParameters,
  getAddress,
  hashTypedData,
  hexToBytes,
  isAddress,
  numberToHex,
  size,
  type Hex,
} from 'viem';
import { SHARED_WEBAUTHN_OWNER } from './constants';
import { providerError } from './errors';
import type { SignRequestView, WebAuthnAssertion } from './types';

const P256_N = BigInt('0xFFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551');
const P256_HALF_N = P256_N >> 1n;
const REQUIRED_CLIENT_PREFIX = '{"type":"webauthn.get","challenge":"';

const SAFE_TX_FIELDS = [
  ['to', 'address'],
  ['value', 'uint256'],
  ['data', 'bytes'],
  ['operation', 'uint8'],
  ['safeTxGas', 'uint256'],
  ['baseGas', 'uint256'],
  ['gasPrice', 'uint256'],
  ['gasToken', 'address'],
  ['refundReceiver', 'address'],
  ['nonce', 'uint256'],
] as const;

export interface SafeTypedData {
  domain: { chainId?: string | number; verifyingContract?: string; [key: string]: unknown };
  types: Record<string, Array<{ name: string; type: string }>>;
  primaryType: string;
  message: Record<string, unknown>;
}

function bytesToHex(bytes: Uint8Array): Hex {
  let output = '0x';
  for (const byte of bytes) output += byte.toString(16).padStart(2, '0');
  return output as Hex;
}

function bytesToBigInt(bytes: Uint8Array): bigint {
  return BigInt(bytesToHex(bytes));
}

function bigIntTo32(value: bigint): Uint8Array {
  return hexToBytes(numberToHex(value, { size: 32 }));
}

export function parseDerP256Signature(signature: Hex): { r: bigint; s: bigint } {
  const der = hexToBytes(signature);
  if (der.length < 8 || der[0] !== 0x30 || der[1] !== der.length - 2) {
    throw providerError(-32602, 'Passkey returned an invalid DER signature.');
  }

  let offset = 2;
  const readInteger = (): Uint8Array => {
    if (der[offset] !== 0x02) throw providerError(-32602, 'Passkey DER integer is malformed.');
    const length = der[offset + 1];
    if (length === undefined || length === 0 || length > 33) {
      throw providerError(-32602, 'Passkey DER integer length is invalid.');
    }
    const start = offset + 2;
    const end = start + length;
    if (end > der.length) throw providerError(-32602, 'Passkey DER signature is truncated.');
    offset = end;
    let value = der.slice(start, end);
    if (value.length === 33 && value[0] === 0) value = value.slice(1);
    if (value.length > 32) throw providerError(-32602, 'Passkey DER integer is too large.');
    return value;
  };

  const rBytes = readInteger();
  const sBytes = readInteger();
  if (offset !== der.length) throw providerError(-32602, 'Passkey DER signature has trailing data.');
  const r = bytesToBigInt(rBytes);
  let s = bytesToBigInt(sBytes);
  if (r <= 0n || r >= P256_N || s <= 0n || s >= P256_N) {
    throw providerError(-32602, 'Passkey signature is outside the P-256 scalar range.');
  }
  if (s > P256_HALF_N) s = P256_N - s;
  return { r, s };
}

export function extractClientDataFields(clientDataJSONHex: Hex): Hex {
  const json = new TextDecoder().decode(hexToBytes(clientDataJSONHex));
  if (!json.startsWith(REQUIRED_CLIENT_PREFIX) || !json.endsWith('}')) {
    throw providerError(-32602, 'Passkey clientDataJSON is incompatible with the Safe WebAuthn contract.');
  }
  const match = json.match(/^\{"type":"webauthn\.get","challenge":"[A-Za-z0-9_-]{43}",(.*)\}$/);
  if (!match?.[1]) {
    throw providerError(-32602, 'Passkey clientDataJSON field order is incompatible with Safe.');
  }
  return bytesToHex(new TextEncoder().encode(match[1]));
}

function base64Url(bytes: Uint8Array): string {
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '');
}

function equalBytes(left: Uint8Array, right: Uint8Array): boolean {
  if (left.length !== right.length) return false;
  let difference = 0;
  for (let i = 0; i < left.length; i++) difference |= left[i]! ^ right[i]!;
  return difference === 0;
}

export async function validateAssertion(
  assertion: WebAuthnAssertion,
  challengeHex: Hex,
  rpId: string,
  extensionOrigin: string,
): Promise<void> {
  const clientDataBytes = hexToBytes(assertion.clientDataJSONHex);
  let client: { type?: unknown; challenge?: unknown; origin?: unknown; crossOrigin?: unknown };
  try {
    client = JSON.parse(new TextDecoder().decode(clientDataBytes)) as typeof client;
  } catch {
    throw providerError(-32602, 'Passkey clientDataJSON is malformed.');
  }
  if (client.type !== 'webauthn.get') throw providerError(-32602, 'Unexpected WebAuthn ceremony type.');
  if (client.challenge !== base64Url(hexToBytes(challengeHex))) {
    throw providerError(-32602, 'Passkey challenge does not match the Safe transaction hash.');
  }
  if (client.origin !== extensionOrigin || client.crossOrigin === true) {
    throw providerError(-32602, 'Passkey assertion came from an unexpected origin.');
  }

  const authData = hexToBytes(assertion.authenticatorDataHex);
  if (authData.length < 37) throw providerError(-32602, 'Passkey authenticatorData is too short.');
  const expectedRpHash = new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(rpId)));
  if (!equalBytes(authData.slice(0, 32), expectedRpHash)) {
    throw providerError(-32602, 'Passkey RP ID hash does not match the configured recovery domain.');
  }
  const flags = authData[32]!;
  if ((flags & 0x01) === 0 || (flags & 0x04) === 0) {
    throw providerError(-32602, 'Passkey did not prove user presence and verification.');
  }
  extractClientDataFields(assertion.clientDataJSONHex);
  parseDerP256Signature(assertion.signatureHex);
}

export function buildSafeContractSignature(assertion: WebAuthnAssertion, dynamicOffset = 65): Hex {
  if (!Number.isSafeInteger(dynamicOffset) || dynamicOffset < 65) {
    throw providerError(-32602, 'Safe contract signature offset is invalid.');
  }
  const { r, s } = parseDerP256Signature(assertion.signatureHex);
  const clientDataFields = extractClientDataFields(assertion.clientDataJSONHex);
  const dynamicData = encodeAbiParameters(
    [
      { name: 'authenticatorData', type: 'bytes' },
      { name: 'clientDataFields', type: 'bytes' },
      { name: 'sigR', type: 'uint256' },
      { name: 'sigS', type: 'uint256' },
    ],
    [assertion.authenticatorDataHex, clientDataFields, r, s],
  );

  // Protocol Kit assumes an injected wallet returns an EOA signature and reads
  // the final byte as `v`. Standard ABI encoding ends in zero padding for the
  // WebAuthn clientDataFields used here, so Protocol Kit changes that padding
  // byte from 0x00 to 0x1b without changing the encoded length or fields.
  // Do not append a compatibility byte: Safe's WebAuthn.castSignature enforces
  // an exact upper bound on the standard ABI size and rejects even one extra
  // byte. `bytes` and `string` have identical ABI wire encoding here.
  const dynamicLength = size(dynamicData);
  const signer = SHARED_WEBAUTHN_OWNER.slice(2).toLowerCase().padStart(64, '0');
  const offset = numberToHex(dynamicOffset, { size: 32 }).slice(2);
  const length = numberToHex(dynamicLength, { size: 32 }).slice(2);
  return `0x${signer}${offset}00${length}${dynamicData.slice(2)}` as Hex;
}

export function safeContractSignaturePayload(signature: Hex): Hex {
  const bytes = hexToBytes(signature);
  if (bytes.length < 97 || bytes[64] !== 0) {
    throw providerError(-32602, 'Safe contract signature wrapper is malformed.');
  }
  const length = bytesToBigInt(bytes.slice(65, 97));
  if (length === 0n || length > BigInt(bytes.length - 97)) {
    throw providerError(-32602, 'Safe contract signature payload length is invalid.');
  }
  return bytesToHex(bytes.slice(97, 97 + Number(length)));
}

function parseChainId(value: unknown): number {
  if (typeof value !== 'number' && typeof value !== 'string') return 0;
  const parsed = typeof value === 'string' && value.startsWith('0x')
    ? Number.parseInt(value, 16)
    : Number(value);
  return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : 0;
}

export function parseAndValidateSafeTypedData(raw: unknown, selectedChainId: number): SafeTypedData {
  let typedData: SafeTypedData;
  try {
    typedData = (typeof raw === 'string' ? JSON.parse(raw) : raw) as SafeTypedData;
  } catch {
    throw providerError(-32602, 'Typed data is not valid JSON.');
  }
  if (!typedData || typedData.primaryType !== 'SafeTx') {
    throw providerError(4200, 'Recovery mode signs SafeTx typed data only.');
  }
  const fields = typedData.types?.SafeTx;
  const validFields = fields?.length === SAFE_TX_FIELDS.length && fields.every((field, index) => {
    const expected = SAFE_TX_FIELDS[index];
    return expected && field.name === expected[0] && field.type === expected[1];
  });
  if (!validFields) throw providerError(-32602, 'SafeTx type definition is not canonical.');

  const safeAddress = typedData.domain?.verifyingContract;
  if (!safeAddress || !isAddress(safeAddress)) throw providerError(-32602, 'SafeTx has no valid Safe address.');
  const chainId = parseChainId(typedData.domain.chainId);
  if (chainId !== selectedChainId) {
    throw providerError(4901, `SafeTx is for chain ${chainId}, but the recovery provider is on ${selectedChainId}.`);
  }
  if (!isAddress(String(typedData.message?.to ?? ''))) {
    throw providerError(-32602, 'SafeTx destination is invalid.');
  }
  return typedData;
}

export function hashSafeTypedData(typedData: SafeTypedData): Hex {
  return hashTypedData(typedData as Parameters<typeof hashTypedData>[0]);
}

export function signRequestView(
  typedData: SafeTypedData,
  challengeHex: Hex,
  requestId: string,
  rpId: string,
  credentialId: string | undefined,
  chainName: string,
): SignRequestView {
  const data = String(typedData.message.data ?? '0x');
  return {
    requestId,
    rpId,
    challengeHex,
    credentialId,
    chainId: parseChainId(typedData.domain.chainId),
    chainName,
    safeAddress: getAddress(typedData.domain.verifyingContract!),
    to: getAddress(String(typedData.message.to)),
    value: String(typedData.message.value ?? '0'),
    operation: Number(typedData.message.operation ?? 0),
    nonce: String(typedData.message.nonce ?? '0'),
    dataSelector: data.length >= 10 ? data.slice(0, 10) : '0x',
  };
}

export function rawP256Scalars(signature: Hex): Hex {
  const { r, s } = parseDerP256Signature(signature);
  const raw = new Uint8Array(64);
  raw.set(bigIntTo32(r), 0);
  raw.set(bigIntTo32(s), 32);
  return bytesToHex(raw);
}
