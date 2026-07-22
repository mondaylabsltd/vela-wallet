// WalletPair v1 dApp peer for the concurrent-session device proof (F2/F3).
//
// This is intentionally self-contained: it follows protocols/{relay,
// encryption,ethereum}.md and does not import a package-level session client.
import WebSocket from 'ws';
import { chacha20poly1305 } from '@noble/ciphers/chacha.js';
import { x25519 } from '@noble/curves/ed25519.js';
import { hkdf } from '@noble/hashes/hkdf.js';
import { sha256 } from '@noble/hashes/sha2.js';

const RELAY = process.env.WP_RELAY || 'wss://relay.walletpair.org/v1';
const log = (...args) => console.error('[wp_peer]', ...args);
const out = (value) => process.stdout.write(JSON.stringify(value) + '\n');
const enc = new TextEncoder();
const dec = new TextDecoder('utf-8', { fatal: true });
const received = [];

const meta = {
  name: 'Vela Concurrency Peer',
  url: 'https://wp-peer.test',
  icon: 'https://wp-peer.test/icon.png',
};
const keys = x25519.keygen();
const ch = Buffer.from(crypto.getRandomValues(new Uint8Array(32))).toString('hex');
const pubkey = Buffer.from(keys.publicKey).toString('base64url');
const rfc3986 = (value) => encodeURIComponent(value).replace(/[!'()*]/g, (c) => `%${c.charCodeAt(0).toString(16).toUpperCase()}`);
const pairingUri = 'walletpair:?' + [
  ['ch', ch], ['pubkey', pubkey], ['relay', RELAY],
  ['name', meta.name], ['url', meta.url], ['icon', meta.icon],
].map(([key, value]) => `${key}=${rfc3986(value)}`).join('&');

const concat = (...parts) => {
  const result = new Uint8Array(parts.reduce((n, part) => n + part.length, 0));
  let at = 0;
  for (const part of parts) { result.set(part, at); at += part.length; }
  return result;
};
const lp = (value) => {
  const bytes = enc.encode(value);
  const prefix = new Uint8Array(2);
  new DataView(prefix.buffer).setUint16(0, bytes.length, false);
  return concat(prefix, bytes);
};
const u32 = (value) => {
  const bytes = new Uint8Array(4);
  new DataView(bytes.buffer).setUint32(0, value, false);
  return bytes;
};
const fingerprint = () => {
  const digest = sha256(concat(
    enc.encode('walletpair-v1-dapp-fingerprint'), Buffer.from(ch, 'hex'),
    lp(meta.name), lp(meta.url), lp(meta.icon), lp(pubkey),
  ));
  return String(new DataView(digest.buffer, digest.byteOffset, 4).getUint32(0, false) % 10_000).padStart(4, '0');
};

function readMessagePack(bytes) {
  let offset = 0;
  const take = (length) => {
    if (offset + length > bytes.length) throw new Error('truncated MessagePack');
    const part = bytes.subarray(offset, offset + length);
    offset += length;
    return part;
  };
  const str = (length) => dec.decode(take(length));
  const value = () => {
    const mark = take(1)[0];
    if (mark <= 0x7f) return mark;
    if (mark >= 0xe0) return mark - 0x100;
    if ((mark & 0xe0) === 0xa0) return str(mark & 0x1f);
    if ((mark & 0xf0) === 0x90) return Array.from({ length: mark & 0x0f }, value);
    if ((mark & 0xf0) === 0x80) return map(mark & 0x0f);
    if (mark === 0xc0) return null;
    if (mark === 0xc2) return false;
    if (mark === 0xc3) return true;
    if (mark === 0xcc) return take(1)[0];
    if (mark === 0xcd) { const p = take(2); return new DataView(p.buffer, p.byteOffset, 2).getUint16(0, false); }
    if (mark === 0xce) { const p = take(4); return new DataView(p.buffer, p.byteOffset, 4).getUint32(0, false); }
    if (mark === 0xd0) { const p = take(1); return new DataView(p.buffer, p.byteOffset, 1).getInt8(0); }
    if (mark === 0xd1) { const p = take(2); return new DataView(p.buffer, p.byteOffset, 2).getInt16(0, false); }
    if (mark === 0xd2) { const p = take(4); return new DataView(p.buffer, p.byteOffset, 4).getInt32(0, false); }
    if (mark === 0xd9) return str(take(1)[0]);
    if (mark === 0xda) { const p = take(2); return str(new DataView(p.buffer, p.byteOffset, 2).getUint16(0, false)); }
    if (mark === 0xdc) { const p = take(2); return Array.from({ length: new DataView(p.buffer, p.byteOffset, 2).getUint16(0, false) }, value); }
    if (mark === 0xde) { const p = take(2); return map(new DataView(p.buffer, p.byteOffset, 2).getUint16(0, false)); }
    throw new Error(`unsupported MessagePack marker 0x${mark.toString(16)}`);
  };
  const map = (length) => {
    const result = {};
    for (let i = 0; i < length; i++) result[value()] = value();
    return result;
  };
  const result = value();
  if (offset !== bytes.length) throw new Error('trailing MessagePack bytes');
  return result;
}

let phase = 'pairing';
let cipher = null;
let receiveSequence = -1;
const endpoint = new URL(RELAY);
endpoint.searchParams.set('ch', ch);
endpoint.searchParams.set('name', meta.name);
endpoint.searchParams.set('url', meta.url);
endpoint.searchParams.set('icon', meta.icon);
endpoint.searchParams.set('pubkey', pubkey);
const socket = new WebSocket(endpoint);

function deriveCipher(walletPubkey) {
  const shared = x25519.getSharedSecret(keys.secretKey, Buffer.from(walletPubkey, 'base64url'));
  const channelBytes = Buffer.from(ch, 'hex');
  const root = hkdf(sha256, shared, channelBytes, enc.encode('walletpair-v1/root'), 32);
  const transcript = sha256(concat(enc.encode('walletpair-v1/transcript'), channelBytes, lp(pubkey), lp(walletPubkey)));
  const receiveKey = hkdf(sha256, root, transcript, enc.encode('walletpair-v1/wallet-to-dapp'), 32);
  shared.fill(0); root.fill(0);
  return { transcript, receiveKey };
}

function openFrame(frame) {
  if (!cipher) return;
  const separator = frame.indexOf('@');
  if (separator < 1 || separator !== frame.lastIndexOf('@')) throw new Error('invalid encrypted frame');
  const sealed = Buffer.from(frame.slice(0, separator), 'base64url');
  const caip2 = frame.slice(separator + 1);
  if (!/^eip155:[1-9][0-9]*$/.test(caip2)) throw new Error('invalid chain context');
  const sequence = new DataView(sealed.buffer, sealed.byteOffset, 4).getUint32(0, false);
  if (sequence <= receiveSequence) throw new Error('replayed frame');
  const seq = u32(sequence);
  const nonce = concat(new Uint8Array(8), seq);
  const plaintext = chacha20poly1305(
    cipher.receiveKey,
    nonce,
    concat(enc.encode('walletpair-v1/aead'), Buffer.from(ch, 'hex'), cipher.transcript, Uint8Array.of(2), seq, lp(caip2)),
  ).decrypt(sealed.subarray(4));
  receiveSequence = sequence;
  const message = readMessagePack(plaintext);
  received.push({ chain: caip2, data: message });
  log('received', JSON.stringify(message).slice(0, 160));
}

socket.on('open', () => {
  log('pairing created; fingerprint', fingerprint());
  out({ type: 'uri', uri: pairingUri, fingerprint: fingerprint() });
});
socket.on('message', (data) => {
  const text = data.toString('utf8');
  try {
    const joined = JSON.parse(text);
    if (joined?.type === 'channel_joined') {
      if (joined.ch !== ch) return;
      if (joined.pubkey === pubkey) return;
      cipher = deriveCipher(joined.pubkey);
      phase = 'connected';
      log('wallet joined:', joined.name);
      out({ type: 'walletJoined', wallet: joined.name });
      return;
    }
  } catch { /* encrypted application frames are not JSON */ }
  try { openFrame(text); } catch (error) { log('frame rejected:', error.message); }
});
socket.on('close', () => { if (phase !== 'closed') phase = 'disconnected'; });
socket.on('error', (error) => { log('socket error', error.message); out({ type: 'error', error: error.message }); });

process.stdin.setEncoding('utf8');
let buffer = '';
process.stdin.on('data', (data) => {
  buffer += data;
  let newline;
  while ((newline = buffer.indexOf('\n')) >= 0) {
    const command = buffer.slice(0, newline).trim();
    buffer = buffer.slice(newline + 1);
    if (command === 'status') out({ type: 'status', phase, receivedCount: received.length, received });
    if (command === 'close') {
      phase = 'closed';
      socket.close(1000, 'test-done');
      out({ type: 'closed' });
      process.exit(0);
    }
  }
});

setInterval(() => {}, 1 << 30);
