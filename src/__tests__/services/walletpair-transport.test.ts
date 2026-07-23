// End-to-end adapter contract: a standard WalletPair EIP-1193 account request
// must leave the encrypted relay as a direct provider result, never time out.

const mockStorage = new Map<string, string>();

jest.mock('react-native', () => ({
  AppState: { addEventListener: () => ({ remove: () => {} }) },
  Platform: { OS: 'ios' },
  NativeModules: {},
}));

jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: {
    setItem: jest.fn(async (key: string, value: string) => { mockStorage.set(key, value); }),
    getItem: jest.fn(async (key: string) => mockStorage.get(key) ?? null),
    removeItem: jest.fn(async (key: string) => { mockStorage.delete(key); }),
  },
}));

import { WalletPairTransport } from '@/services/walletpair-transport';
import { handleReadOnlyRPC } from '@/hooks/use-dapp-signing';
import { chacha20poly1305 } from '@noble/ciphers/chacha.js';
import { x25519 } from '@noble/curves/ed25519.js';
import { hkdf } from '@noble/hashes/hkdf.js';
import { sha256 } from '@noble/hashes/sha2.js';

const CHANNEL = '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef';
const DAPP_SECRET = Uint8Array.from({ length: 32 }, (_, index) => index + 1);
const DAPP_PUBKEY = Buffer.from(x25519.getPublicKey(DAPP_SECRET)).toString('base64url');
const ADDRESS = '0x1111111111111111111111111111111111111111';
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
  return bytes.length <= 31 ? Uint8Array.of(0xa0 | bytes.length, ...bytes) : Uint8Array.of(0xd9, bytes.length, ...bytes);
};
const rfc3986 = (value: string) => encodeURIComponent(value).replace(/[!'()*]/g, (c) => `%${c.charCodeAt(0).toString(16).toUpperCase()}`);
const URI = 'walletpair:?' + [
  ['ch', CHANNEL], ['pubkey', DAPP_PUBKEY], ['relay', 'wss://relay.walletpair.org/v1'],
  ['name', 'Protocol Test dApp'], ['url', 'https://dapp.example'], ['icon', 'https://dapp.example/icon.png'],
].map(([key, value]) => `${key}=${rfc3986(value)}`).join('&');

class MockWebSocket {
  static last: MockWebSocket | null = null;
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
  close() { this.readyState = 3; this.onclose?.({ code: 1000 }); }
  receive(data: unknown) { this.onmessage?.({ data: JSON.stringify(data) }); }
  receiveRaw(data: string) { this.onmessage?.({ data }); }
}

function dappKeys(walletPubkey: string) {
  const channel = Buffer.from(CHANNEL, 'hex');
  const shared = x25519.getSharedSecret(DAPP_SECRET, Buffer.from(walletPubkey, 'base64url'));
  const root = hkdf(sha256, shared, channel, utf8('walletpair-v1/root'), 32);
  const transcript = sha256(concat(utf8('walletpair-v1/transcript'), channel, lp(DAPP_PUBKEY), lp(walletPubkey)));
  const send = hkdf(sha256, root, transcript, utf8('walletpair-v1/dapp-to-wallet'), 32);
  const receive = hkdf(sha256, root, transcript, utf8('walletpair-v1/wallet-to-dapp'), 32);
  shared.fill(0); root.fill(0);
  return { channel, transcript, send, receive };
}

const aad = (channel: Uint8Array, transcript: Uint8Array, direction: 1 | 2, sequence: Uint8Array, caip2: string) =>
  concat(utf8('walletpair-v1/aead'), channel, transcript, Uint8Array.of(direction), sequence, lp(caip2));

describe('WalletPairTransport', () => {
  const originalWebSocket = globalThis.WebSocket;

  beforeEach(() => {
    mockStorage.clear();
    MockWebSocket.last = null;
    (globalThis as unknown as { WebSocket: typeof WebSocket }).WebSocket = MockWebSocket as unknown as typeof WebSocket;
  });

  afterEach(() => {
    (globalThis as unknown as { WebSocket: typeof WebSocket }).WebSocket = originalWebSocket;
  });

  it('returns eth_requestAccounts over the same authenticated chain frame', async () => {
    const { transport } = WalletPairTransport.prepare(URI);
    transport.on('request', (id, method, params, _origin, chainId) => {
      // This is the exact fast path DAppConnectionProvider uses for account
      // access, including the transport's encrypted response delivery.
      handleReadOnlyRPC(method, params, ADDRESS, chainId ?? 1).then((response) => {
        if (response.handled) transport.sendResponse(id, response.result);
      });
    });

    const connected = transport.connect();
    await Promise.resolve();
    const socket = MockWebSocket.last!;
    const walletPubkey = new URL(socket.url).searchParams.get('pubkey')!;
    socket.receive({
      type: 'channel_joined', ch: CHANNEL, name: 'Vela Wallet', url: 'https://getvela.app',
      icon: 'https://getvela.app/icon.png', pubkey: walletPubkey,
    });
    await connected;

    const { channel, transcript, send, receive } = dappKeys(walletPubkey);
    const sequence = u32(0);
    const request = concat(
      Uint8Array.of(0x83), packString('id'), packString('accounts-1'),
      packString('method'), packString('eth_requestAccounts'), packString('params'), Uint8Array.of(0x90),
    );
    const sealedRequest = chacha20poly1305(send, concat(new Uint8Array(8), sequence), aad(channel, transcript, 1, sequence, 'eip155:1')).encrypt(request);
    socket.receiveRaw(`${Buffer.from(concat(sequence, sealedRequest)).toString('base64url')}@eip155:1`);
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(socket.sent).toHaveLength(1);
    const frame = socket.sent[0]!;
    expect(frame).toMatch(/@eip155:1$/);
    const sealedResponse = Buffer.from(frame.slice(0, frame.indexOf('@')), 'base64url');
    const responseSequence = sealedResponse.subarray(0, 4);
    const plaintext = chacha20poly1305(
      receive,
      concat(new Uint8Array(8), responseSequence),
      aad(channel, transcript, 2, responseSequence, 'eip155:1'),
    ).decrypt(sealedResponse.subarray(4));
    expect(Array.from(plaintext)).toEqual(Array.from(concat(
      Uint8Array.of(0x82), packString('id'), packString('accounts-1'), packString('result'), Uint8Array.of(0x91), packString(ADDRESS),
    )));
    transport.disconnect();
  });
});
