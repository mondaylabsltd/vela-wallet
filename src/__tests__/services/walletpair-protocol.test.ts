import {
  WalletPairSession,
  computeDappPairingCode,
  parsePairingUri,
  type SessionPersistence,
} from '@/services/walletpair-protocol';
import { chacha20poly1305 } from '@noble/ciphers/chacha.js';
import { x25519 } from '@noble/curves/ed25519.js';
import { hkdf } from '@noble/hashes/hkdf.js';
import { sha256 } from '@noble/hashes/sha2.js';

const CHANNEL = '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef';
const DAPP_SECRET = Uint8Array.from({ length: 32 }, (_, index) => index + 1);
const DAPP_PUBKEY = Buffer.from(x25519.getPublicKey(DAPP_SECRET)).toString('base64url');
const rfc3986 = (value: string) => encodeURIComponent(value).replace(/[!'()*]/g, (c) => `%${c.charCodeAt(0).toString(16).toUpperCase()}`);
const URI = 'walletpair:?' + [
  ['ch', CHANNEL], ['pubkey', DAPP_PUBKEY], ['relay', 'wss://relay.walletpair.org/v1'],
  ['name', 'Protocol Test dApp'], ['url', 'https://dapp.example'], ['icon', 'https://dapp.example/icon.png'],
].map(([key, value]) => `${key}=${rfc3986(value)}`).join('&');

class MockWebSocket {
  static last: MockWebSocket | null = null;
  readonly OPEN = 1;
  binaryType = 'blob';
  readyState = 1;
  onopen: (() => void) | null = null;
  onmessage: ((event: { data: string }) => void) | null = null;
  onerror: (() => void) | null = null;
  onclose: ((event: { code: number }) => void) | null = null;
  sent: string[] = [];

  constructor(readonly url: string) {
    MockWebSocket.last = this;
    queueMicrotask(() => this.onopen?.());
  }

  send(frame: string) { this.sent.push(frame); }
  close() {
    this.readyState = 3;
    this.onclose?.({ code: 1000 });
  }
  receive(data: unknown) { this.onmessage?.({ data: JSON.stringify(data) }); }
  receiveRaw(data: string) { this.onmessage?.({ data }); }
}

const utf8 = (value: string) => new TextEncoder().encode(value);
const concat = (...parts: Uint8Array[]) => {
  const output = new Uint8Array(parts.reduce((size, part) => size + part.length, 0));
  let offset = 0;
  for (const part of parts) { output.set(part, offset); offset += part.length; }
  return output;
};
const lp = (value: string) => {
  const bytes = utf8(value);
  return Uint8Array.of(bytes.length >>> 8, bytes.length & 0xff, ...bytes);
};
const u32 = (value: number) => Uint8Array.of(value >>> 24, value >>> 16 & 0xff, value >>> 8 & 0xff, value & 0xff);
const packString = (value: string) => {
  const bytes = utf8(value);
  return bytes.length <= 31
    ? Uint8Array.of(0xa0 | bytes.length, ...bytes)
    : Uint8Array.of(0xd9, bytes.length, ...bytes);
};
const dappToWalletKeys = (walletPubkey: string) => {
  const channel = Buffer.from(CHANNEL, 'hex');
  const shared = x25519.getSharedSecret(DAPP_SECRET, Buffer.from(walletPubkey, 'base64url'));
  const root = hkdf(sha256, shared, channel, utf8('walletpair-v1/root'), 32);
  const transcript = sha256(concat(utf8('walletpair-v1/transcript'), channel, lp(DAPP_PUBKEY), lp(walletPubkey)));
  const send = hkdf(sha256, root, transcript, utf8('walletpair-v1/dapp-to-wallet'), 32);
  const receive = hkdf(sha256, root, transcript, utf8('walletpair-v1/wallet-to-dapp'), 32);
  shared.fill(0); root.fill(0);
  return { channel, transcript, send, receive };
};
const aad = (channel: Uint8Array, transcript: Uint8Array, direction: 1 | 2, sequence: Uint8Array, caip2: string) =>
  concat(utf8('walletpair-v1/aead'), channel, transcript, Uint8Array.of(direction), sequence, lp(caip2));

describe('WalletPair v1 protocol', () => {
  const originalWebSocket = globalThis.WebSocket;

  beforeEach(() => {
    (globalThis as unknown as { WebSocket: typeof WebSocket }).WebSocket = MockWebSocket as unknown as typeof WebSocket;
    MockWebSocket.last = null;
  });

  afterEach(() => {
    (globalThis as unknown as { WebSocket: typeof WebSocket }).WebSocket = originalWebSocket;
  });

  it('strictly accepts the six canonical pairing fields and derives a four-digit code', () => {
    const parsed = parsePairingUri(URI);
    expect(parsed).toMatchObject({ ch: CHANNEL, name: 'Protocol Test dApp', relay: 'wss://relay.walletpair.org/v1' });
    expect(computeDappPairingCode(parsed)).toMatch(/^\d{4}$/);
    expect(() => parsePairingUri(`${URI}&name=Duplicate`)).toThrow(/duplicate/i);
    expect(() => parsePairingUri(URI.replace('icon=https', 'icon=http'))).toThrow(/unsupported scheme/i);
  });

  it('joins only after its own relay event and persists the v1 counter snapshot', async () => {
    let snapshot: string | null = null;
    const persistence: SessionPersistence = {
      save: async (next) => { snapshot = next; },
      load: async () => snapshot,
      clear: async () => { snapshot = null; },
    };
    const session = new WalletPairSession({
      meta: { name: 'Vela Wallet', url: 'https://getvela.app', icon: 'https://getvela.app/icon.png' },
      persistence,
    });
    const phases: string[] = [];
    session.on('phase', (phase) => phases.push(phase));

    session.prepareJoin(URI);
    const connect = session.confirmJoin();
    await Promise.resolve();
    const socket = MockWebSocket.last!;
    const params = new URL(socket.url).searchParams;
    expect(params.get('ch')).toBe(CHANNEL);
    expect(params.get('name')).toBe('Vela Wallet');
    socket.receive({
      type: 'channel_joined', ch: CHANNEL, name: 'Vela Wallet',
      url: 'https://getvela.app', icon: 'https://getvela.app/icon.png', pubkey: params.get('pubkey'),
    });
    await connect;

    expect(session.phase).toBe('connected');
    expect(phases).toEqual(['awaiting_confirmation', 'connected']);
    expect(JSON.parse(snapshot!)).toMatchObject({
      v: 1, role: 'wallet', sendSequence: 0, receiveSequence: -1,
      dapp: { name: 'Protocol Test dApp', ch: CHANNEL },
    });
    session.destroy();
    expect(snapshot).toBeNull();
  });

  it('restores the full dApp and wallet join identity for a reconnect', async () => {
    let snapshot: string | null = null;
    const persistence: SessionPersistence = {
      save: async (next) => { snapshot = next; },
      load: async () => snapshot,
      clear: async () => { snapshot = null; },
    };
    const meta = { name: 'Vela Wallet', url: 'https://getvela.app', icon: 'https://getvela.app/icon.png' };
    const first = new WalletPairSession({ meta, persistence });
    first.prepareJoin(URI);
    const joined = first.confirmJoin();
    await Promise.resolve();
    const initialSocket = MockWebSocket.last!;
    const walletPubkey = new URL(initialSocket.url).searchParams.get('pubkey')!;
    initialSocket.receive({
      type: 'channel_joined', ch: CHANNEL, name: meta.name, url: meta.url, icon: meta.icon, pubkey: walletPubkey,
    });
    await joined;

    const saved = snapshot!;
    const parsed = JSON.parse(saved);
    expect(parsed).toMatchObject({
      relayUrl: 'wss://relay.walletpair.org/v1',
      dapp: { ch: CHANNEL, pubkey: DAPP_PUBKEY, name: 'Protocol Test dApp', url: 'https://dapp.example' },
      wallet: { name: meta.name, url: meta.url, icon: meta.icon, pubkey: walletPubkey },
    });

    const restored = new WalletPairSession({ meta, persistence });
    expect(restored.restore(saved)).toBe(true);
    const reconnect = restored.reconnect();
    await Promise.resolve();
    const reconnectSocket = MockWebSocket.last!;
    const params = new URL(reconnectSocket.url).searchParams;
    expect(reconnectSocket.url.startsWith('wss://relay.walletpair.org/v1')).toBe(true);
    expect(params.get('ch')).toBe(CHANNEL);
    expect(params.get('pubkey')).toBe(walletPubkey);
    expect(params.get('name')).toBe(meta.name);
    expect(params.get('url')).toBe(meta.url);
    expect(params.get('icon')).toBe(meta.icon);
    reconnectSocket.receive({
      type: 'channel_joined', ch: CHANNEL, name: meta.name, url: meta.url, icon: meta.icon, pubkey: walletPubkey,
    });
    await reconnect;
    restored.destroy();
    expect(snapshot).toBeNull();
  });

  it('rejects snapshots whose persisted counter state is unsafe', async () => {
    const persistence: SessionPersistence = { save: async () => {}, load: async () => null, clear: async () => {} };
    const session = new WalletPairSession({
      meta: { name: 'Vela Wallet', url: 'https://getvela.app', icon: 'https://getvela.app/icon.png' },
      persistence,
    });
    expect(session.restore(JSON.stringify({ v: 1, role: 'wallet', sendSequence: -1 }))).toBe(false);
    expect(session.phase).toBe('idle');
  });

  it('opens a standard encrypted EIP-1193 request and replies on its authenticated chain', async () => {
    const persistence: SessionPersistence = { save: async () => {}, load: async () => null, clear: async () => {} };
    const session = new WalletPairSession({
      meta: { name: 'Vela Wallet', url: 'https://getvela.app', icon: 'https://getvela.app/icon.png' },
      persistence,
    });
    const requests: unknown[] = [];
    session.on('request', (request) => requests.push(request));
    session.prepareJoin(URI);
    const connected = session.confirmJoin();
    await Promise.resolve();
    const socket = MockWebSocket.last!;
    const walletPubkey = new URL(socket.url).searchParams.get('pubkey')!;
    socket.receive({
      type: 'channel_joined', ch: CHANNEL, name: 'Vela Wallet',
      url: 'https://getvela.app', icon: 'https://getvela.app/icon.png', pubkey: walletPubkey,
    });
    await connected;

    const { channel, transcript, send, receive } = dappToWalletKeys(walletPubkey);
    const requestPlaintext = concat(
      Uint8Array.of(0x83), packString('id'), packString('req-1'),
      packString('method'), packString('eth_chainId'), packString('params'), Uint8Array.of(0x90),
    );
    const sequence = u32(0);
    const sealed = chacha20poly1305(send, concat(new Uint8Array(8), sequence), aad(channel, transcript, 1, sequence, 'eip155:1')).encrypt(requestPlaintext);
    socket.receiveRaw(`${Buffer.from(concat(sequence, sealed)).toString('base64url')}@eip155:1`);
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(requests).toEqual([{ id: 'req-1', method: 'eth_chainId', params: [], caip2: 'eip155:1' }]);
    session.approve('req-1', '0x1');
    await new Promise((resolve) => setTimeout(resolve, 0));
    const frame = socket.sent[0]!;
    expect(frame).toMatch(/@eip155:1$/);
    const sealedResponse = Buffer.from(frame.slice(0, frame.indexOf('@')), 'base64url');
    const responseSequence = sealedResponse.subarray(0, 4);
    const responsePlaintext = chacha20poly1305(
      receive,
      concat(new Uint8Array(8), responseSequence),
      aad(channel, transcript, 2, responseSequence, 'eip155:1'),
    ).decrypt(sealedResponse.subarray(4));
    expect(Array.from(responsePlaintext)).toEqual(Array.from(concat(
      Uint8Array.of(0x82), packString('id'), packString('req-1'), packString('result'), packString('0x1'),
    )));
    session.destroy();
  });

  it('delivers an encrypted personal_sign request to the signing route', async () => {
    const persistence: SessionPersistence = { save: async () => {}, load: async () => null, clear: async () => {} };
    const session = new WalletPairSession({
      meta: { name: 'Vela Wallet', url: 'https://getvela.app', icon: 'https://getvela.app/icon.png' },
      persistence,
    });
    const requests: unknown[] = [];
    session.on('request', (request) => requests.push(request));
    session.prepareJoin(URI);
    const connected = session.confirmJoin();
    await Promise.resolve();
    const socket = MockWebSocket.last!;
    const walletPubkey = new URL(socket.url).searchParams.get('pubkey')!;
    socket.receive({
      type: 'channel_joined', ch: CHANNEL, name: 'Vela Wallet',
      url: 'https://getvela.app', icon: 'https://getvela.app/icon.png', pubkey: walletPubkey,
    });
    await connected;

    const { channel, transcript, send } = dappToWalletKeys(walletPubkey);
    const requestPlaintext = concat(
      Uint8Array.of(0x83), packString('id'), packString('sign-1'),
      packString('method'), packString('personal_sign'), packString('params'), Uint8Array.of(0x92),
      packString('0x48656c6c6f'), packString('0x1111111111111111111111111111111111111111'),
    );
    const sequence = u32(0);
    const sealed = chacha20poly1305(send, concat(new Uint8Array(8), sequence), aad(channel, transcript, 1, sequence, 'eip155:1')).encrypt(requestPlaintext);
    socket.receiveRaw(`${Buffer.from(concat(sequence, sealed)).toString('base64url')}@eip155:1`);
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(requests).toEqual([{
      id: 'sign-1', method: 'personal_sign',
      params: ['0x48656c6c6f', '0x1111111111111111111111111111111111111111'], caip2: 'eip155:1',
    }]);
    session.destroy();
  });
});
