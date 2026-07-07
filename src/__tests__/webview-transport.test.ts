// Off-device unit test for WebViewTransport — the in-app dApp browser transport.
//
// It imports only a type from @/services/dapp-transport, so no react-native chain
// loads and it runs headless (mirrors extension-bridge-transport.test.ts).

import { WebViewTransport, type WalletWebViewBridge } from '@/services/webview-transport';

function makeBridge() {
  return {
    respond: jest.fn<void, [string, unknown, { code: number; message: string } | null]>(),
    emitEvent: jest.fn<void, [string, unknown]>(),
  } satisfies WalletWebViewBridge & { respond: jest.Mock; emitEvent: jest.Mock };
}

test('connect() flips connected and emits connected with the transport name', async () => {
  const t = new WebViewTransport(makeBridge());
  const events: unknown[] = [];
  t.on('connected', (name) => events.push(['connected', name]));
  await t.connect();
  expect(t.connected).toBe(true);
  expect(events[0]).toEqual(['connected', 'In-App Browser']);
});

test('handleProviderRequest emits request with the NATIVE-stamped origin', async () => {
  const t = new WebViewTransport(makeBridge());
  const seen: unknown[] = [];
  t.on('request', (id, method, params, origin) => seen.push([id, method, params, origin]));
  await t.connect();
  t.handleProviderRequest('1', 'eth_chainId', [], 'https://app.uniswap.org', true);
  expect(seen[0]).toEqual(['1', 'eth_chainId', [], 'https://app.uniswap.org']);
});

test('requests before connect() are dropped', () => {
  const t = new WebViewTransport(makeBridge());
  const seen: unknown[] = [];
  t.on('request', (...a) => seen.push(a));
  t.handleProviderRequest('1', 'eth_chainId', [], 'https://x.io', true);
  expect(seen).toHaveLength(0);
});

test('iframe (non-main-frame) requests never reach signing — rejected 4100', async () => {
  const bridge = makeBridge();
  const t = new WebViewTransport(bridge);
  const seen: unknown[] = [];
  t.on('request', (...a) => seen.push(a));
  await t.connect();
  t.handleProviderRequest('1', 'eth_requestAccounts', [], 'https://evil.example', false);
  expect(seen).toHaveLength(0);
  expect(bridge.respond).toHaveBeenCalledWith('1', undefined, { code: 4100, message: 'Unauthorized frame' });
});

test('sendResponse(success) routes result to the bridge with null error', async () => {
  const bridge = makeBridge();
  const t = new WebViewTransport(bridge);
  await t.connect();
  t.handleProviderRequest('7', 'eth_chainId', [], 'https://x.io', true);
  t.sendResponse('7', '0x1');
  expect(bridge.respond).toHaveBeenCalledWith('7', '0x1', null);
});

test('sendResponse(error) routes the error with an undefined result', async () => {
  const bridge = makeBridge();
  const t = new WebViewTransport(bridge);
  await t.connect();
  t.handleProviderRequest('7', 'personal_sign', ['0x', '0x'], 'https://x.io', true);
  t.sendResponse('7', undefined, { code: 4001, message: 'User rejected' });
  expect(bridge.respond).toHaveBeenCalledWith('7', undefined, { code: 4001, message: 'User rejected' });
});

test('sendResponse is idempotent per id and ignores unknown ids', async () => {
  const bridge = makeBridge();
  const t = new WebViewTransport(bridge);
  await t.connect();
  t.handleProviderRequest('7', 'eth_chainId', [], 'https://x.io', true);
  t.sendResponse('7', '0x1');
  t.sendResponse('7', '0x2'); // duplicate — ignored
  t.sendResponse('nope', '0x9'); // never requested — ignored
  expect(bridge.respond).toHaveBeenCalledTimes(1);
  expect(bridge.respond).toHaveBeenCalledWith('7', '0x1', null);
});

test('pushWalletInfo emits accountsChanged + chainChanged (hex chainId)', () => {
  const bridge = makeBridge();
  const t = new WebViewTransport(bridge);
  t.pushWalletInfo({ address: '0xabc', chainId: 8453, name: 'A', accounts: [] });
  expect(bridge.emitEvent).toHaveBeenNthCalledWith(1, 'accountsChanged', ['0xabc']);
  expect(bridge.emitEvent).toHaveBeenNthCalledWith(2, 'chainChanged', '0x2105'); // 8453
});

test('fetchDAppInfo returns the current identity; setDAppInfo updates it', async () => {
  const t = new WebViewTransport(makeBridge(), { name: 'uniswap', url: 'https://app.uniswap.org' });
  expect(await t.fetchDAppInfo()).toEqual({ name: 'uniswap', url: 'https://app.uniswap.org' });
  t.setDAppInfo({ name: 'aave', url: 'https://app.aave.com' });
  expect(await t.fetchDAppInfo()).toEqual({ name: 'aave', url: 'https://app.aave.com' });
});

test('on() returns an unsubscribe that stops delivery', async () => {
  const t = new WebViewTransport(makeBridge());
  let hits = 0;
  const off = t.on('connected', () => (hits += 1));
  off();
  await t.connect();
  expect(hits).toBe(0);
});

test('settlePending settles all in-flight ids with the given (non-4001) code and clears them', async () => {
  const bridge = makeBridge();
  const t = new WebViewTransport(bridge);
  await t.connect();
  t.handleProviderRequest('1', 'eth_call', [], 'https://x.io', true);
  t.handleProviderRequest('2', 'personal_sign', ['0x', '0x'], 'https://x.io', true);
  t.settlePending({ code: 4900, message: 'Navigated' });
  expect(bridge.respond).toHaveBeenCalledWith('1', undefined, { code: 4900, message: 'Navigated' });
  expect(bridge.respond).toHaveBeenCalledWith('2', undefined, { code: 4900, message: 'Navigated' });
  // a late response for a now-settled id is dropped (no double-answer)
  bridge.respond.mockClear();
  t.sendResponse('1', '0xdead');
  expect(bridge.respond).not.toHaveBeenCalled();
});

test('disconnect() settles in-flight requests with 4900, never 4001', async () => {
  const bridge = makeBridge();
  const t = new WebViewTransport(bridge);
  await t.connect();
  t.handleProviderRequest('9', 'eth_sendTransaction', [{}], 'https://x.io', true);
  t.disconnect();
  expect(bridge.respond).toHaveBeenCalledWith('9', undefined, expect.objectContaining({ code: 4900 }));
});

test('disconnect() flips connected and emits disconnected once', () => {
  const t = new WebViewTransport(makeBridge());
  let count = 0;
  t.on('disconnected', () => (count += 1));
  void t.connect();
  t.disconnect();
  t.disconnect(); // no-op the second time
  expect(t.connected).toBe(false);
  expect(count).toBe(1);
});
