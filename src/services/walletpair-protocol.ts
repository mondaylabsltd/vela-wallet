/**
 * Self-contained WalletPair v1 wallet-side protocol implementation.
 *
 * This follows the repository's public protocol documents directly:
 * protocols/{relay,encryption,ethereum}.md.  It deliberately has no dependency
 * on a package-level WalletPair session implementation.
 */

import { chacha20poly1305 } from '@noble/ciphers/chacha.js';
import { x25519 } from '@noble/curves/ed25519.js';
import { hkdf } from '@noble/hashes/hkdf.js';
import { sha256 } from '@noble/hashes/sha2.js';

const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder('utf-8', { fatal: true });
const MAX_PLAINTEXT_BYTES = 64 * 1024;
const MAX_NESTING_DEPTH = 64;
const MAX_SEQUENCE = 2 ** 31;
const MAX_SEALED_BYTES = 4 + MAX_PLAINTEXT_BYTES + 16;

const ROOT_INFO = utf8('walletpair-v1/root');
const TRANSCRIPT_DOMAIN = utf8('walletpair-v1/transcript');
const DAPP_TO_WALLET_INFO = utf8('walletpair-v1/dapp-to-wallet');
const WALLET_TO_DAPP_INFO = utf8('walletpair-v1/wallet-to-dapp');
const FINGERPRINT_DOMAIN = utf8('walletpair-v1-dapp-fingerprint');
const AEAD_DOMAIN = utf8('walletpair-v1/aead');

export type JsonValue = null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue };
export type WalletPairPhase = 'idle' | 'awaiting_confirmation' | 'connected' | 'disconnected' | 'closed';

export interface ParticipantMeta {
  name: string;
  url: string;
  icon: string;
}

export interface ParsedPairingUri {
  relay: string;
  ch: string;
  pubkey: string;
  name: string;
  url: string;
  icon: string;
}

export interface SessionPersistence {
  save(snapshot: string): Promise<void> | void;
  load(): Promise<string | null> | string | null;
  clear(): Promise<void> | void;
}

export interface EthereumRequest {
  id: string;
  method: string;
  params: any[] | Record<string, unknown>;
  caip2: string;
}

interface RelayIdentity extends ParticipantMeta {
  ch: string;
  pubkey: string;
}

interface ChannelJoined extends RelayIdentity {
  type: 'channel_joined';
}

interface SessionSnapshot {
  v: 1;
  role: 'wallet';
  relayUrl: string;
  dapp: RelayIdentity;
  wallet: ParticipantMeta & { pubkey: string };
  secretKey: string;
  sendSequence: number;
  receiveSequence: number;
}

interface CipherCounters {
  sendSequence: number;
  receiveSequence: number;
}

interface TrafficKeys {
  transcriptHash: Uint8Array;
  dappToWalletKey: Uint8Array;
  walletToDappKey: Uint8Array;
}

type Listener = (...args: any[]) => void;

function utf8(value: string): Uint8Array {
  return textEncoder.encode(value);
}

function utf8Length(value: string): number {
  return utf8(value).length;
}

function concatBytes(...parts: Uint8Array[]): Uint8Array {
  const output = new Uint8Array(parts.reduce((length, part) => length + part.length, 0));
  let offset = 0;
  for (const part of parts) {
    output.set(part, offset);
    offset += part.length;
  }
  return output;
}

function uint16be(value: number): Uint8Array {
  if (!Number.isInteger(value) || value < 0 || value > 0xffff) throw new RangeError('uint16 out of range');
  return Uint8Array.of(value >>> 8, value & 0xff);
}

function uint32be(value: number): Uint8Array {
  if (!Number.isInteger(value) || value < 0 || value > 0xffffffff) throw new RangeError('uint32 out of range');
  const output = new Uint8Array(4);
  new DataView(output.buffer).setUint32(0, value, false);
  return output;
}

function readUint32be(value: Uint8Array): number {
  if (value.length !== 4) throw new RangeError('uint32 requires four bytes');
  return new DataView(value.buffer, value.byteOffset, 4).getUint32(0, false);
}

function lp(value: string): Uint8Array {
  const bytes = utf8(value);
  if (bytes.length > 0xffff) throw new RangeError('length-prefixed value is too long');
  return concatBytes(uint16be(bytes.length), bytes);
}

function bytesToHex(value: Uint8Array): string {
  return Array.from(value, (byte) => byte.toString(16).padStart(2, '0')).join('');
}

function hexToBytes(value: string, expectedBytes?: number): Uint8Array {
  if (!/^(?:[0-9a-f]{2})+$/.test(value)) throw new TypeError('invalid canonical lowercase hex');
  if (expectedBytes !== undefined && value.length !== expectedBytes * 2) throw new RangeError('unexpected hex length');
  const output = new Uint8Array(value.length / 2);
  for (let index = 0; index < output.length; index++) {
    output[index] = Number.parseInt(value.slice(index * 2, index * 2 + 2), 16);
  }
  return output;
}

function bytesToBase64Url(value: Uint8Array): string {
  let binary = '';
  for (let offset = 0; offset < value.length; offset += 0x8000) {
    binary += String.fromCharCode(...value.subarray(offset, offset + 0x8000));
  }
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '');
}

function base64UrlToBytes(value: string, expectedBytes?: number): Uint8Array {
  if (!/^[A-Za-z0-9_-]+$/.test(value) || value.length % 4 === 1) throw new TypeError('invalid canonical base64url');
  let binary: string;
  try {
    binary = atob(value.replace(/-/g, '+').replace(/_/g, '/') + '='.repeat((4 - value.length % 4) % 4));
  } catch {
    throw new TypeError('invalid base64url');
  }
  const output = Uint8Array.from(binary, (character) => character.charCodeAt(0));
  if (bytesToBase64Url(output) !== value) throw new TypeError('non-canonical base64url');
  if (expectedBytes !== undefined && output.length !== expectedBytes) throw new RangeError('unexpected base64url length');
  return output;
}

function allZero(value: Uint8Array): boolean {
  let combined = 0;
  for (const byte of value) combined |= byte;
  return combined === 0;
}

function rfc3986Encode(value: string): string {
  return encodeURIComponent(value).replace(/[!'()*]/g, (character) => `%${character.charCodeAt(0).toString(16).toUpperCase()}`);
}

function validateChannelId(value: string): void {
  if (!/^[0-9a-f]{64}$/.test(value)) throw new TypeError('ch must be 64 lowercase hexadecimal characters');
  hexToBytes(value, 32);
}

function validatePublicKey(value: string): Uint8Array {
  const decoded = base64UrlToBytes(value, 32);
  if (allZero(decoded)) throw new TypeError('pubkey must not be all zero');
  return decoded;
}

function requireAbsoluteUrl(value: string, schemes: readonly string[], field: string): void {
  let parsed: URL;
  try {
    parsed = new URL(value);
  } catch {
    throw new TypeError(`${field} must be an absolute URL`);
  }
  if (!schemes.includes(parsed.protocol)) throw new TypeError(`${field} has an unsupported scheme`);
}

function validateParticipantMeta(meta: ParticipantMeta): void {
  const nameLength = utf8Length(meta.name);
  if (nameLength < 1 || nameLength > 128 || /\p{Cc}/u.test(meta.name)) {
    throw new TypeError('name must be 1-128 UTF-8 bytes without control characters');
  }
  if (utf8Length(meta.url) > 2048 || utf8Length(meta.icon) > 2048) throw new TypeError('metadata URL is too long');
  requireAbsoluteUrl(meta.url, ['http:', 'https:'], 'url');
  requireAbsoluteUrl(meta.icon, ['https:'], 'icon');
}

function validateRelayUrl(value: string): void {
  if (utf8Length(value) > 2048) throw new TypeError('relay URL is too long');
  requireAbsoluteUrl(value, ['ws:', 'wss:'], 'relay');
}

function validateIdentity(identity: RelayIdentity): void {
  validateChannelId(identity.ch);
  validateParticipantMeta(identity);
  validatePublicKey(identity.pubkey);
}

function buildRelayConnectionUrl(relayUrl: string, identity: RelayIdentity): string {
  validateRelayUrl(relayUrl);
  validateIdentity(identity);
  const url = new URL(relayUrl);
  url.hash = '';
  url.searchParams.set('ch', identity.ch);
  url.searchParams.set('name', identity.name);
  url.searchParams.set('url', identity.url);
  url.searchParams.set('icon', identity.icon);
  url.searchParams.set('pubkey', identity.pubkey);
  return url.toString();
}

/** Strictly parse the six-field QR URI required by encryption.md. */
export function parsePairingUri(value: string): ParsedPairingUri {
  if (!value.startsWith('walletpair:?') || value.includes('#')) throw new TypeError('invalid WalletPair pairing URI');
  const query = value.slice('walletpair:?'.length);
  const required = new Set(['ch', 'pubkey', 'relay', 'name', 'url', 'icon']);
  const fields = new Map<string, string>();
  if (!query) throw new TypeError('pairing URI query is empty');
  for (const part of query.split('&')) {
    const separator = part.indexOf('=');
    if (separator < 1) throw new TypeError('malformed pairing URI field');
    const key = part.slice(0, separator);
    const encoded = part.slice(separator + 1);
    if (!required.has(key) || fields.has(key)) throw new TypeError('pairing URI has a missing, duplicate, or unknown field');
    let decoded: string;
    try {
      decoded = decodeURIComponent(encoded);
    } catch {
      throw new TypeError('malformed percent encoding in pairing URI');
    }
    if (rfc3986Encode(decoded) !== encoded) throw new TypeError('pairing URI has non-canonical percent encoding');
    fields.set(key, decoded);
  }
  if (fields.size !== required.size) throw new TypeError('pairing URI must contain all six fields exactly once');
  const dapp: RelayIdentity = {
    ch: fields.get('ch')!,
    pubkey: fields.get('pubkey')!,
    name: fields.get('name')!,
    url: fields.get('url')!,
    icon: fields.get('icon')!,
  };
  validateIdentity(dapp);
  const relay = fields.get('relay')!;
  validateRelayUrl(relay);
  return { relay, ...dapp };
}

function parseChannelJoined(value: unknown): ChannelJoined | null {
  if (!isPlainRecord(value) || value.type !== 'channel_joined') return null;
  const expected = ['ch', 'icon', 'name', 'pubkey', 'type', 'url'];
  const keys = Object.keys(value).sort();
  if (keys.length !== expected.length || keys.some((key, index) => key !== expected[index])) return null;
  if (typeof value.ch !== 'string' || typeof value.name !== 'string' || typeof value.url !== 'string' || typeof value.icon !== 'string' || typeof value.pubkey !== 'string') return null;
  const joined: ChannelJoined = { type: 'channel_joined', ch: value.ch, name: value.name, url: value.url, icon: value.icon, pubkey: value.pubkey };
  try {
    validateIdentity(joined);
    return joined;
  } catch {
    return null;
  }
}

/** Four digit code displayed while the user compares dApp and wallet. */
export function computeDappPairingCode(parsed: ParsedPairingUri): string {
  const dapp: RelayIdentity = parsed;
  validateIdentity(dapp);
  const digest = sha256(concatBytes(
    FINGERPRINT_DOMAIN,
    hexToBytes(dapp.ch, 32),
    lp(dapp.name),
    lp(dapp.url),
    lp(dapp.icon),
    lp(dapp.pubkey),
  ));
  const number = new DataView(digest.buffer, digest.byteOffset, 4).getUint32(0, false) % 10_000;
  return number.toString().padStart(4, '0');
}

function validateCaip2(value: string): void {
  if (utf8Length(value) > 41 || !/^[-a-z0-9]{3,8}:[-_a-zA-Z0-9]{1,32}$/.test(value)) {
    throw new TypeError('invalid canonical CAIP-2 chain ID');
  }
  if (value.startsWith('eip155:') && !/^eip155:[1-9][0-9]*$/.test(value)) {
    throw new TypeError('invalid canonical EIP-155 CAIP-2 chain ID');
  }
}

function deriveTrafficKeys(channelId: string, localSecretKey: Uint8Array, dappPublicKey: string, walletPublicKey: string): TrafficKeys {
  validateChannelId(channelId);
  if (localSecretKey.length !== 32) throw new TypeError('X25519 private key must be 32 bytes');
  const dappKey = validatePublicKey(dappPublicKey);
  const walletKey = validatePublicKey(walletPublicKey);
  const ownKey = x25519.getPublicKey(localSecretKey);
  if (!equalBytes(ownKey, walletKey)) throw new TypeError('stored X25519 key does not match the wallet public key');
  let sharedSecret: Uint8Array | undefined;
  let rootKey: Uint8Array | undefined;
  try {
    sharedSecret = x25519.getSharedSecret(localSecretKey, dappKey);
    if (allZero(sharedSecret)) throw new TypeError('X25519 shared secret is all zero');
    const channelBytes = hexToBytes(channelId, 32);
    rootKey = hkdf(sha256, sharedSecret, channelBytes, ROOT_INFO, 32);
    const transcriptHash = sha256(concatBytes(
      TRANSCRIPT_DOMAIN,
      channelBytes,
      lp(dappPublicKey),
      lp(walletPublicKey),
    ));
    return {
      transcriptHash,
      dappToWalletKey: hkdf(sha256, rootKey, transcriptHash, DAPP_TO_WALLET_INFO, 32),
      walletToDappKey: hkdf(sha256, rootKey, transcriptHash, WALLET_TO_DAPP_INFO, 32),
    };
  } finally {
    sharedSecret?.fill(0);
    rootKey?.fill(0);
  }
}

function equalBytes(left: Uint8Array, right: Uint8Array): boolean {
  if (left.length !== right.length) return false;
  let difference = 0;
  for (let index = 0; index < left.length; index++) difference |= left[index]! ^ right[index]!;
  return difference === 0;
}

function nonce(sequence: Uint8Array): Uint8Array {
  return concatBytes(new Uint8Array(8), sequence);
}

function aad(channelId: Uint8Array, transcriptHash: Uint8Array, direction: 1 | 2, sequence: Uint8Array, caip2: string): Uint8Array {
  return concatBytes(AEAD_DOMAIN, channelId, transcriptHash, Uint8Array.of(direction), sequence, lp(caip2));
}

class ChannelCipher {
  private readonly channelBytes: Uint8Array;
  private readonly transcriptHash: Uint8Array;
  private readonly sendKey: Uint8Array;
  private readonly receiveKey: Uint8Array;
  private sendSequence: number;
  private receiveSequence: number;
  private destroyed = false;

  constructor(channelId: string, keys: TrafficKeys, counters: CipherCounters) {
    validateCounters(counters);
    this.channelBytes = hexToBytes(channelId, 32);
    this.transcriptHash = keys.transcriptHash.slice();
    this.sendKey = keys.walletToDappKey.slice();
    this.receiveKey = keys.dappToWalletKey.slice();
    this.sendSequence = counters.sendSequence;
    this.receiveSequence = counters.receiveSequence;
  }

  counters(): CipherCounters {
    return { sendSequence: this.sendSequence, receiveSequence: this.receiveSequence };
  }

  async seal(value: unknown, caip2: string, persist: () => Promise<void>): Promise<string> {
    this.assertUsable();
    validateCaip2(caip2);
    const plaintext = encodeJsonMessagePack(value);
    if (this.sendSequence >= MAX_SEQUENCE) throw new RangeError('channel send sequence is exhausted');
    const current = uint32be(this.sendSequence);
    this.sendSequence += 1;
    // The counter must be durable before a ciphertext can be produced for this nonce.
    await persist();
    const ciphertext = chacha20poly1305(this.sendKey, nonce(current), aad(this.channelBytes, this.transcriptHash, 2, current, caip2)).encrypt(plaintext);
    return `${bytesToBase64Url(concatBytes(current, ciphertext))}@${caip2}`;
  }

  async open(frame: string, persist: () => Promise<void>): Promise<{ value: JsonValue; caip2: string }> {
    this.assertUsable();
    const separator = frame.indexOf('@');
    if (separator <= 0 || separator !== frame.lastIndexOf('@') || separator === frame.length - 1) throw new TypeError('encrypted frame must contain exactly one separator');
    const sealed = base64UrlToBytes(frame.slice(0, separator));
    const caip2 = frame.slice(separator + 1);
    validateCaip2(caip2);
    if (sealed.length < 20 || sealed.length > MAX_SEALED_BYTES) throw new RangeError('encrypted frame has an invalid size');
    const sequence = sealed.subarray(0, 4);
    const sequenceNumber = readUint32be(sequence);
    if (sequenceNumber >= MAX_SEQUENCE || sequenceNumber <= this.receiveSequence) throw new RangeError('replayed or out-of-order encrypted frame');
    const plaintext = chacha20poly1305(this.receiveKey, nonce(sequence), aad(this.channelBytes, this.transcriptHash, 1, sequence, caip2)).decrypt(sealed.subarray(4));
    const value = decodeJsonMessagePack(plaintext);
    this.receiveSequence = sequenceNumber;
    await persist();
    return { value, caip2 };
  }

  destroy(): void {
    if (this.destroyed) return;
    this.destroyed = true;
    this.sendKey.fill(0);
    this.receiveKey.fill(0);
    this.transcriptHash.fill(0);
  }

  private assertUsable(): void {
    if (this.destroyed) throw new Error('WalletPair channel is closed');
  }
}

function validateCounters(counters: CipherCounters): void {
  if (!Number.isInteger(counters.sendSequence) || counters.sendSequence < 0 || counters.sendSequence > MAX_SEQUENCE) throw new TypeError('invalid persisted send sequence');
  if (!Number.isInteger(counters.receiveSequence) || counters.receiveSequence < -1 || counters.receiveSequence >= MAX_SEQUENCE) throw new TypeError('invalid persisted receive sequence');
}

function createWalletCipher(channelId: string, secretKey: Uint8Array, dappPublicKey: string, walletPublicKey: string, counters: CipherCounters): ChannelCipher {
  const keys = deriveTrafficKeys(channelId, secretKey, dappPublicKey, walletPublicKey);
  try {
    return new ChannelCipher(channelId, keys, counters);
  } finally {
    keys.transcriptHash.fill(0);
    keys.dappToWalletKey.fill(0);
    keys.walletToDappKey.fill(0);
  }
}

// MessagePack's JSON-only profile. Keeping this local avoids accepting binary,
// extension, duplicate-key, non-canonical-number, or trailing-value payloads.
function mpHeader(marker: number, value: number, width: 1 | 2 | 4): Uint8Array {
  const output = new Uint8Array(width + 1);
  output[0] = marker;
  const view = new DataView(output.buffer);
  if (width === 1) view.setUint8(1, value);
  if (width === 2) view.setUint16(1, value, false);
  if (width === 4) view.setUint32(1, value, false);
  return output;
}

function encodeInteger(value: number): Uint8Array {
  if (!Number.isSafeInteger(value)) throw new TypeError('JSON integer is outside the safe range');
  if (value >= 0) {
    if (value <= 0x7f) return Uint8Array.of(value);
    if (value <= 0xff) return mpHeader(0xcc, value, 1);
    if (value <= 0xffff) return mpHeader(0xcd, value, 2);
    if (value <= 0xffffffff) return mpHeader(0xce, value, 4);
    const output = new Uint8Array(9);
    output[0] = 0xcf;
    new DataView(output.buffer).setBigUint64(1, BigInt(value), false);
    return output;
  }
  if (value >= -32) return Uint8Array.of(0x100 + value);
  if (value >= -0x80) { const output = Uint8Array.of(0xd0, 0); new DataView(output.buffer).setInt8(1, value); return output; }
  if (value >= -0x8000) { const output = new Uint8Array(3); output[0] = 0xd1; new DataView(output.buffer).setInt16(1, value, false); return output; }
  if (value >= -0x80000000) { const output = new Uint8Array(5); output[0] = 0xd2; new DataView(output.buffer).setInt32(1, value, false); return output; }
  const output = new Uint8Array(9);
  output[0] = 0xd3;
  new DataView(output.buffer).setBigInt64(1, BigInt(value), false);
  return output;
}

function encodeString(value: string): Uint8Array {
  const bytes = utf8(value);
  const prefix = bytes.length <= 31 ? Uint8Array.of(0xa0 | bytes.length) : bytes.length <= 0xff ? mpHeader(0xd9, bytes.length, 1) : bytes.length <= 0xffff ? mpHeader(0xda, bytes.length, 2) : mpHeader(0xdb, bytes.length, 4);
  return concatBytes(prefix, bytes);
}

function encodeMessagePack(value: unknown, depth = 0, ancestors = new Set<object>()): Uint8Array {
  if (depth > MAX_NESTING_DEPTH) throw new RangeError('MessagePack nesting exceeds 64');
  if (value === null) return Uint8Array.of(0xc0);
  if (value === false) return Uint8Array.of(0xc2);
  if (value === true) return Uint8Array.of(0xc3);
  if (typeof value === 'string') return encodeString(value);
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) throw new TypeError('JSON numbers must be finite');
    if (Number.isInteger(value)) return encodeInteger(value);
    const output = new Uint8Array(9);
    output[0] = 0xcb;
    new DataView(output.buffer).setFloat64(1, value, false);
    return output;
  }
  if (!value || typeof value !== 'object') throw new TypeError('value is outside the JSON data model');
  if (ancestors.has(value)) throw new TypeError('cyclic values are not JSON');
  ancestors.add(value);
  try {
    if (Array.isArray(value)) {
      const prefix = value.length <= 15 ? Uint8Array.of(0x90 | value.length) : value.length <= 0xffff ? mpHeader(0xdc, value.length, 2) : mpHeader(0xdd, value.length, 4);
      return concatBytes(prefix, ...value.map((entry) => encodeMessagePack(entry, depth + 1, ancestors)));
    }
    const prototype = Object.getPrototypeOf(value);
    if (prototype !== Object.prototype && prototype !== null) throw new TypeError('only plain JSON objects are supported');
    const entries = Object.entries(value as Record<string, unknown>);
    const prefix = entries.length <= 15 ? Uint8Array.of(0x80 | entries.length) : entries.length <= 0xffff ? mpHeader(0xde, entries.length, 2) : mpHeader(0xdf, entries.length, 4);
    return concatBytes(prefix, ...entries.flatMap(([key, entry]) => [encodeString(key), encodeMessagePack(entry, depth + 1, ancestors)]));
  } finally {
    ancestors.delete(value);
  }
}

function encodeJsonMessagePack(value: unknown): Uint8Array {
  const encoded = encodeMessagePack(value);
  if (encoded.length > MAX_PLAINTEXT_BYTES) throw new RangeError('MessagePack plaintext exceeds 64 KiB');
  return encoded;
}

class MessagePackReader {
  private offset = 0;
  constructor(private readonly bytes: Uint8Array) {}
  get remaining(): number { return this.bytes.length - this.offset; }
  private take(length: number): Uint8Array {
    if (!Number.isSafeInteger(length) || length < 0 || length > this.remaining) throw new RangeError('truncated MessagePack');
    const output = this.bytes.subarray(this.offset, this.offset + length);
    this.offset += length;
    return output;
  }
  private byte(): number { return this.take(1)[0]!; }
  private u16(): number { const bytes = this.take(2); return new DataView(bytes.buffer, bytes.byteOffset, 2).getUint16(0, false); }
  private u32(): number { const bytes = this.take(4); return new DataView(bytes.buffer, bytes.byteOffset, 4).getUint32(0, false); }
  private string(length: number): string { return textDecoder.decode(this.take(length)); }
  private array(length: number, depth: number): JsonValue[] {
    if (length > this.remaining) throw new RangeError('invalid MessagePack array length');
    return Array.from({ length }, () => this.value(depth + 1));
  }
  private map(length: number, depth: number): { [key: string]: JsonValue } {
    if (length > Math.floor(this.remaining / 2)) throw new RangeError('invalid MessagePack map length');
    const result: { [key: string]: JsonValue } = Object.create(null);
    const keys = new Set<string>();
    for (let index = 0; index < length; index++) {
      const key = this.value(depth + 1);
      if (typeof key !== 'string' || keys.has(key)) throw new TypeError('MessagePack map keys must be unique strings');
      keys.add(key);
      result[key] = this.value(depth + 1);
    }
    return result;
  }
  value(depth: number): JsonValue {
    if (depth > MAX_NESTING_DEPTH) throw new RangeError('MessagePack nesting exceeds 64');
    const marker = this.byte();
    if (marker <= 0x7f) return marker;
    if (marker >= 0xe0) return marker - 0x100;
    if ((marker & 0xe0) === 0xa0) return this.string(marker & 0x1f);
    if ((marker & 0xf0) === 0x90) return this.array(marker & 0x0f, depth);
    if ((marker & 0xf0) === 0x80) return this.map(marker & 0x0f, depth);
    if (marker === 0xc0) return null;
    if (marker === 0xc2) return false;
    if (marker === 0xc3) return true;
    if (marker === 0xcc) { const value = this.byte(); if (value <= 0x7f) throw new TypeError('non-shortest MessagePack integer'); return value; }
    if (marker === 0xcd) { const value = this.u16(); if (value <= 0xff) throw new TypeError('non-shortest MessagePack integer'); return value; }
    if (marker === 0xce) { const value = this.u32(); if (value <= 0xffff) throw new TypeError('non-shortest MessagePack integer'); return value; }
    if (marker === 0xcf) { const value = new DataView(this.take(8).buffer, this.bytes.byteOffset + this.offset - 8, 8).getBigUint64(0, false); if (value <= 0xffffffffn || value > BigInt(Number.MAX_SAFE_INTEGER)) throw new TypeError('invalid MessagePack uint64'); return Number(value); }
    if (marker === 0xd0) { const value = new DataView(this.take(1).buffer, this.bytes.byteOffset + this.offset - 1, 1).getInt8(0); if (value >= -32) throw new TypeError('non-shortest MessagePack integer'); return value; }
    if (marker === 0xd1) { const bytes = this.take(2); const value = new DataView(bytes.buffer, bytes.byteOffset, 2).getInt16(0, false); if (value >= -0x80) throw new TypeError('non-shortest MessagePack integer'); return value; }
    if (marker === 0xd2) { const bytes = this.take(4); const value = new DataView(bytes.buffer, bytes.byteOffset, 4).getInt32(0, false); if (value >= -0x8000) throw new TypeError('non-shortest MessagePack integer'); return value; }
    if (marker === 0xd3) { const bytes = this.take(8); const value = new DataView(bytes.buffer, bytes.byteOffset, 8).getBigInt64(0, false); if (value >= -0x80000000n || value < BigInt(Number.MIN_SAFE_INTEGER)) throw new TypeError('invalid MessagePack int64'); return Number(value); }
    if (marker === 0xcb) { const bytes = this.take(8); const value = new DataView(bytes.buffer, bytes.byteOffset, 8).getFloat64(0, false); if (!Number.isFinite(value) || Number.isInteger(value)) throw new TypeError('invalid MessagePack float64'); return value; }
    if (marker === 0xd9) return this.string(this.byte());
    if (marker === 0xda) return this.string(this.u16());
    if (marker === 0xdb) return this.string(this.u32());
    if (marker === 0xdc) return this.array(this.u16(), depth);
    if (marker === 0xdd) return this.array(this.u32(), depth);
    if (marker === 0xde) return this.map(this.u16(), depth);
    if (marker === 0xdf) return this.map(this.u32(), depth);
    throw new TypeError(`MessagePack type 0x${marker.toString(16)} is outside the JSON profile`);
  }
}

function decodeJsonMessagePack(bytes: Uint8Array): JsonValue {
  if (bytes.length > MAX_PLAINTEXT_BYTES) throw new RangeError('MessagePack plaintext exceeds 64 KiB');
  const reader = new MessagePackReader(bytes);
  const value = reader.value(0);
  if (reader.remaining !== 0) throw new TypeError('trailing MessagePack bytes');
  return value;
}

function isPlainRecord(value: unknown): value is Record<string, any> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function validateRequestId(value: string): void {
  if (!/^[\x20-\x7e]{1,128}$/.test(value)) throw new TypeError('request id must be 1-128 printable ASCII bytes');
}

function parseEthereumRequest(value: JsonValue, caip2: string): EthereumRequest {
  if (!isPlainRecord(value)) throw new TypeError('invalid Ethereum request');
  const record = value as Record<string, any>;
  if (typeof record.id !== 'string' || typeof record.method !== 'string') throw new TypeError('invalid Ethereum request');
  if ('result' in record || 'error' in record || 'event' in record) throw new TypeError('ambiguous Ethereum message shape');
  validateRequestId(record.id);
  const methodLength = utf8Length(record.method);
  if (methodLength < 1 || methodLength > 128) throw new TypeError('method must be 1-128 UTF-8 bytes');
  const params = record.params === undefined ? [] : record.params;
  if (!Array.isArray(params) && !isPlainRecord(params)) throw new TypeError('EIP-1193 params must be an array or object');
  return { id: record.id, method: record.method, params, caip2 };
}

function isSnapshot(value: unknown): value is SessionSnapshot {
  if (!isPlainRecord(value) || value.v !== 1 || value.role !== 'wallet') return false;
  return typeof value.relayUrl === 'string' && typeof value.secretKey === 'string' && typeof value.sendSequence === 'number' && typeof value.receiveSequence === 'number' && isPlainRecord(value.dapp) && isPlainRecord(value.wallet);
}

function asIdentity(value: Record<string, any>): RelayIdentity {
  if (typeof value.ch !== 'string' || typeof value.pubkey !== 'string' || typeof value.name !== 'string' || typeof value.url !== 'string' || typeof value.icon !== 'string') throw new TypeError('invalid saved dApp identity');
  const identity: RelayIdentity = { ch: value.ch, pubkey: value.pubkey, name: value.name, url: value.url, icon: value.icon };
  validateIdentity(identity);
  return identity;
}

function asWalletMeta(value: Record<string, any>): ParticipantMeta & { pubkey: string } {
  if (typeof value.pubkey !== 'string' || typeof value.name !== 'string' || typeof value.url !== 'string' || typeof value.icon !== 'string') throw new TypeError('invalid saved wallet identity');
  const meta = { name: value.name, url: value.url, icon: value.icon, pubkey: value.pubkey };
  validateParticipantMeta(meta);
  validatePublicKey(meta.pubkey);
  return meta;
}

/** Wallet side of the documented, relay-agnostic WebSocket protocol. */
export class WalletPairSession {
  phase: WalletPairPhase = 'idle';
  private readonly meta: ParticipantMeta;
  private readonly persistence: SessionPersistence;
  private readonly connectTimeout: number;
  private readonly listeners = new Map<string, Set<Listener>>();
  private relayUrl = '';
  private dapp: RelayIdentity | null = null;
  private wallet: (ParticipantMeta & { pubkey: string }) | null = null;
  private secretKey: Uint8Array | null = null;
  private cipher: ChannelCipher | null = null;
  private socket: WebSocket | null = null;
  private ownJoinReceived = false;
  private intentionalClose = false;
  private sendTail: Promise<void> = Promise.resolve();
  private receiveTail: Promise<void> = Promise.resolve();
  private readonly requestChains = new Map<string, string>();

  constructor(options: { meta: ParticipantMeta; persistence: SessionPersistence; connectTimeout?: number }) {
    validateParticipantMeta(options.meta);
    this.meta = { ...options.meta };
    this.persistence = options.persistence;
    this.connectTimeout = options.connectTimeout ?? 30_000;
  }

  on(event: string, listener: Listener): this {
    if (!this.listeners.has(event)) this.listeners.set(event, new Set());
    this.listeners.get(event)!.add(listener);
    return this;
  }

  /** Parse and pin the dApp QR data before the user approves the pairing code. */
  prepareJoin(uri: string): string {
    if (this.phase !== 'idle' && this.phase !== 'closed') throw new Error('WalletPair session is already prepared');
    const parsed = parsePairingUri(uri.trim());
    this.releaseSecrets();
    const keyPair = x25519.keygen();
    const wallet = { ...this.meta, pubkey: bytesToBase64Url(keyPair.publicKey) };
    this.relayUrl = parsed.relay;
    this.dapp = { ch: parsed.ch, pubkey: parsed.pubkey, name: parsed.name, url: parsed.url, icon: parsed.icon };
    this.wallet = wallet;
    this.secretKey = keyPair.secretKey.slice();
    this.cipher = createWalletCipher(parsed.ch, this.secretKey, parsed.pubkey, wallet.pubkey, { sendSequence: 0, receiveSequence: -1 });
    this.intentionalClose = false;
    this.requestChains.clear();
    this.setPhase('awaiting_confirmation');
    return computeDappPairingCode(parsed);
  }

  /** Join only after the user has compared and accepted the four-digit code. */
  async confirmJoin(): Promise<void> {
    if (this.phase !== 'awaiting_confirmation') throw new Error('WalletPair pairing has not been approved');
    await this.connect();
  }

  async reconnect(): Promise<void> {
    if (this.phase !== 'disconnected') throw new Error('WalletPair session is not disconnected');
    this.intentionalClose = false;
    await this.connect();
  }

  restore(snapshot: string): boolean {
    try {
      const parsed = JSON.parse(snapshot) as unknown;
      if (!isSnapshot(parsed)) throw new TypeError('unsupported WalletPair session snapshot');
      validateRelayUrl(parsed.relayUrl);
      const dapp = asIdentity(parsed.dapp);
      const wallet = asWalletMeta(parsed.wallet);
      const secretKey = base64UrlToBytes(parsed.secretKey, 32);
      if (!equalBytes(x25519.getPublicKey(secretKey), validatePublicKey(wallet.pubkey))) throw new TypeError('stored X25519 key pair does not match');
      const counters = { sendSequence: parsed.sendSequence, receiveSequence: parsed.receiveSequence };
      validateCounters(counters);
      this.releaseSecrets();
      this.relayUrl = parsed.relayUrl;
      this.dapp = dapp;
      this.wallet = wallet;
      this.secretKey = secretKey;
      this.cipher = createWalletCipher(dapp.ch, secretKey, dapp.pubkey, wallet.pubkey, counters);
      this.intentionalClose = false;
      this.ownJoinReceived = false;
      this.requestChains.clear();
      this.setPhase('disconnected');
      return true;
    } catch {
      this.releaseSecrets();
      this.relayUrl = '';
      this.dapp = null;
      this.wallet = null;
      this.phase = 'idle';
      return false;
    }
  }

  approve(id: string, result: unknown): void {
    this.respond(id, { id, result });
  }

  reject(id: string, code: number | string, message: string): void {
    const numericCode = typeof code === 'string' ? Number(code) : code;
    if (!Number.isInteger(numericCode)) throw new TypeError('ProviderRpcError code must be an integer');
    this.respond(id, { id, error: { code: numericCode, message } });
  }

  pushEvent(event: 'connect' | 'disconnect' | 'chainChanged' | 'accountsChanged' | 'message', data: unknown, caip2: string): void {
    this.enqueueSend({ event, data }, caip2).catch((error) => this.emit('error', asError(error)));
  }

  ping(): boolean {
    return this.socket?.readyState === 1;
  }

  destroy(): void {
    if (this.phase === 'closed') return;
    this.intentionalClose = true;
    this.detachSocket('normal');
    this.releaseSecrets();
    this.requestChains.clear();
    this.setPhase('closed');
    Promise.resolve(this.persistence.clear()).catch(() => {});
  }

  serialize(): string {
    if (!this.dapp || !this.wallet || !this.secretKey || !this.cipher) throw new Error('WalletPair session has not been prepared');
    const snapshot: SessionSnapshot = {
      v: 1,
      role: 'wallet',
      relayUrl: this.relayUrl,
      dapp: { ...this.dapp },
      wallet: { ...this.wallet },
      secretKey: bytesToBase64Url(this.secretKey),
      ...this.cipher.counters(),
    };
    return JSON.stringify(snapshot);
  }

  private async connect(): Promise<void> {
    if (!this.dapp || !this.wallet || !this.secretKey || !this.cipher) throw new Error('WalletPair session has not been prepared');
    this.detachSocket('replaced');
    this.ownJoinReceived = false;
    const socket = new WebSocket(buildRelayConnectionUrl(this.relayUrl, { ch: this.dapp.ch, pubkey: this.wallet.pubkey, ...this.meta }));
    socket.binaryType = 'arraybuffer';
    this.socket = socket;

    await new Promise<void>((resolve, reject) => {
      let settled = false;
      const settle = (error?: Error) => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        if (error) reject(error); else resolve();
      };
      const timer = setTimeout(() => {
        if (socket === this.socket) socket.close();
        settle(new Error('WalletPair relay connection timed out'));
      }, this.connectTimeout);
      socket.onopen = () => { /* wait for our authenticated channel_joined frame */ };
      socket.onerror = () => settle(new Error('WalletPair relay WebSocket failed'));
      socket.onclose = (event) => {
        if (!this.ownJoinReceived) settle(new Error(`WalletPair relay WebSocket closed (${event.code})`));
        this.handleSocketClose(socket);
      };
      socket.onmessage = (event) => {
        if (socket !== this.socket || typeof event.data !== 'string') return;
        this.receiveTail = this.receiveTail
          .then(() => this.handleTextFrame(event.data, socket, () => settle()))
          .catch((error) => this.emit('error', asError(error)));
      };
    });
  }

  private async handleTextFrame(text: string, socket: WebSocket, settled: () => void): Promise<void> {
    let parsed: unknown;
    try { parsed = JSON.parse(text); } catch { parsed = undefined; }
    const joined = parseChannelJoined(parsed);
    if (joined) {
      if (joined.ch === this.dapp?.ch && joined.pubkey === this.wallet?.pubkey) {
        this.ownJoinReceived = true;
        await this.persistCounters();
        this.setPhase('connected');
        settled();
      }
      return;
    }
    if (socket !== this.socket || !this.ownJoinReceived || this.phase !== 'connected' || !this.cipher) return;
    let opened: { value: JsonValue; caip2: string };
    try {
      opened = await this.cipher.open(text, () => this.persistCounters());
    } catch {
      // Extra relay participants and malformed/replayed ciphertext are ignored.
      return;
    }
    if (!opened.caip2.startsWith('eip155:')) return;
    try {
      const request = parseEthereumRequest(opened.value, opened.caip2);
      if (this.requestChains.has(request.id) || this.requestChains.size >= 1024) throw new TypeError('duplicate or excessive outstanding request');
      this.requestChains.set(request.id, request.caip2);
      this.emit('request', request);
    } catch (error) {
      const record = isPlainRecord(opened.value) ? opened.value as Record<string, any> : null;
      const recoverableId = record && typeof record.id === 'string' && /^[\x20-\x7e]{1,128}$/.test(record.id) ? record.id : null;
      if (recoverableId) this.enqueueSend({ id: recoverableId, error: { code: -32600, message: 'Invalid request' } }, opened.caip2).catch(() => {});
      this.emit('protocolError', asError(error));
    }
  }

  private respond(id: string, value: unknown): void {
    const caip2 = this.requestChains.get(id);
    if (!caip2) throw new Error(`WalletPair request ${id} is not outstanding`);
    // Validate synchronously before forgetting the request ID. Otherwise an
    // accidental non-JSON value (for example `[undefined]` before state has
    // hydrated) fails only inside the async send queue and leaves the dApp to
    // time out with no possible retry/error response.
    encodeJsonMessagePack(value);
    this.requestChains.delete(id);
    this.enqueueSend(value, caip2).catch((error) => this.emit('error', asError(error)));
  }

  private enqueueSend(value: unknown, caip2: string): Promise<void> {
    const task = this.sendTail.then(async () => {
      if (this.phase !== 'connected' || !this.socket || this.socket.readyState !== 1 || !this.cipher) throw new Error('WalletPair channel is disconnected');
      const frame = await this.cipher.seal(value, caip2, () => this.persistCounters());
      if (!this.socket || this.socket.readyState !== 1) throw new Error('WalletPair channel disconnected before send');
      this.socket.send(frame);
    });
    this.sendTail = task.catch(() => {});
    return task;
  }

  private async persistCounters(): Promise<void> {
    try {
      await this.persistence.save(this.serialize());
    } catch (error) {
      // Continuing after a failed counter write risks a nonce reuse after restart.
      this.abandonUnsafeSession();
      throw error;
    }
  }

  private abandonUnsafeSession(): void {
    this.intentionalClose = true;
    this.detachSocket('counter persistence failed');
    this.releaseSecrets();
    this.requestChains.clear();
    this.setPhase('closed');
    Promise.resolve(this.persistence.clear()).catch(() => {});
  }

  private handleSocketClose(socket: WebSocket): void {
    if (socket !== this.socket) return;
    this.socket = null;
    this.ownJoinReceived = false;
    this.requestChains.clear();
    if (!this.intentionalClose && this.phase !== 'closed') this.setPhase('disconnected');
  }

  private detachSocket(reason: string): void {
    const socket = this.socket;
    this.socket = null;
    this.ownJoinReceived = false;
    if (!socket) return;
    socket.onopen = null;
    socket.onmessage = null;
    socket.onerror = null;
    socket.onclose = null;
    if (socket.readyState === 0 || socket.readyState === 1) socket.close(1000, reason);
  }

  private releaseSecrets(): void {
    this.cipher?.destroy();
    this.cipher = null;
    this.secretKey?.fill(0);
    this.secretKey = null;
  }

  private setPhase(phase: WalletPairPhase): void {
    if (this.phase === phase) return;
    this.phase = phase;
    this.emit('phase', phase);
  }

  private emit(event: string, ...args: unknown[]): void {
    for (const listener of this.listeners.get(event) ?? []) {
      try { listener(...args); } catch { /* listeners must not break cryptographic state */ }
    }
  }
}

function asError(value: unknown): Error {
  return value instanceof Error ? value : new Error(String(value));
}
