// Concurrent-session proof (the two-slot raison d'être — critic findings F2/F3/F4).
//
// The provider can hold a live WalletPair session (durable slot) AND a Safari-extension
// sign (transient slot) AT THE SAME TIME. This test proves that under concurrency the
// two NEVER answer each other: each response goes to the transport that OWNS its request
// (bound per-request, never a shared ref), each carries its own chain + dApp identity,
// and the extension transport settles only on its OWN response.
//
// It drives the REAL ExtensionBridgeTransport (so we prove it writes ONLY its own
// sign-result file) plus a WalletPair stand-in, through the SAME routing seam the
// provider uses (src/models/dapp-request-routing.ts, wired into dapp-connection.tsx).
//
// @/modules/app-group is mocked so the react-native import chain never loads.

const writeFile = jest.fn(async (_name: string, _json: string): Promise<void> => {});
const readFile = jest.fn(async (_name: string): Promise<string | null> => null);
jest.mock('@/modules/app-group', () => ({
  writeFile: (name: string, json: string) => writeFile(name, json),
  readFile: (name: string) => readFile(name),
  isSupportedSync: () => true,
}));

import { ExtensionBridgeTransport } from '@/services/extension-bridge-transport';
import { responseTransport, requestChainId, requestDApp } from '@/models/dapp-request-routing';

const flush = () => new Promise((r) => setTimeout(r, 0));

/** A stand-in for a LIVE WalletPair session (the durable transportRef). Records every
 *  response it is asked to send — a leak/mis-route would show up in `sent`. */
function mockWalletPair() {
  return {
    name: 'WalletPair',
    connected: true,
    sent: [] as { id: string; result?: unknown; error?: unknown }[],
    sendResponse(id: string, result?: unknown, error?: unknown) {
      this.sent.push({ id, result, error });
    },
    // unused surface (present so it conforms structurally to DAppTransport)
    connect: async () => {},
    disconnect: jest.fn(),
    pushWalletInfo: () => {},
    fetchDAppInfo: async () => null,
    on: () => {},
  };
}

/** What handleIncoming stamps for a Safari-extension sign (per-request owner + chain +
 *  identity). An ordinary WalletPair request carries none of these. */
function extRequest(ext: unknown, rid: string) {
  return { id: rid, method: 'personal_sign', params: [], origin: 'https://app.uniswap.org',
           __transport: ext, __chainId: 8453, __dapp: { name: 'app.uniswap.org', url: 'https://app.uniswap.org' } } as any;
}
function wpRequest(id: string) {
  return { id, method: 'personal_sign', params: [], origin: 'https://wp-dapp.io' } as any;
}

beforeEach(() => {
  writeFile.mockClear();
  readFile.mockClear();
  readFile.mockResolvedValue(null);
});

test('F2: an extension sign is answered on the EXTENSION transport, never the live WalletPair socket', async () => {
  const wp = mockWalletPair();                       // durable slot (live WP session)
  const ext = new ExtensionBridgeTransport('rid-ext'); // transient slot
  const reqExt = extRequest(ext, 'rid-ext');

  // The provider routes an extension response through responseTransport(req, durable).
  expect(responseTransport(reqExt, wp as any)).toBe(ext);   // owner is the ext transport, NOT wp
  responseTransport(reqExt, wp as any)!.sendResponse('rid-ext', '0xEXTSIG');
  await flush();

  // The extension signature landed in the extension's OWN result file...
  expect(writeFile).toHaveBeenCalledTimes(1);
  const [name, json] = writeFile.mock.calls[0];
  expect(name).toBe('sign-result-rid-ext.json');
  expect(JSON.parse(json)).toMatchObject({ status: 'submitted', userOpHash: '0xEXTSIG' });
  // ...and NOTHING went over the WalletPair socket (no leak to the wrong origin — F2).
  expect(wp.sent).toHaveLength(0);
});

test('F2: a WalletPair request is answered on the WalletPair socket, never the extension file (no mis-settle)', async () => {
  const wp = mockWalletPair();
  new ExtensionBridgeTransport('rid-ext'); // an ext transport is also installed
  const reqWP = wpRequest('wp-1');         // ordinary request → carries no __transport

  expect(responseTransport(reqWP, wp as any)).toBe(wp);     // owner is the durable WP transport
  responseTransport(reqWP, wp as any)!.sendResponse('wp-1', '0xWPRESULT');
  await flush();

  expect(wp.sent).toEqual([{ id: 'wp-1', result: '0xWPRESULT', error: undefined }]);
  expect(writeFile).not.toHaveBeenCalled(); // the extension's disk channel is untouched
});

test('F2: both live at once — each answered on its own channel, neither crosses', async () => {
  const wp = mockWalletPair();
  const ext = new ExtensionBridgeTransport('rid-ext');
  const reqExt = extRequest(ext, 'rid-ext');
  const reqWP = wpRequest('wp-1');

  // Interleave the two responses.
  responseTransport(reqWP, wp as any)!.sendResponse('wp-1', '0xWP');
  responseTransport(reqExt, wp as any)!.sendResponse('rid-ext', '0xEXT');
  await flush();

  // WalletPair got ONLY its own; extension file got ONLY its own. No cross-answer.
  expect(wp.sent).toEqual([{ id: 'wp-1', result: '0xWP', error: undefined }]);
  expect(writeFile).toHaveBeenCalledTimes(1);
  expect(writeFile.mock.calls[0][0]).toBe('sign-result-rid-ext.json');
  expect(JSON.parse(writeFile.mock.calls[0][1])).toMatchObject({ userOpHash: '0xEXT' });
});

test('F4: the extension signs against the ORIGIN’s granted chain, not the WalletPair global chain', () => {
  const reqExt = extRequest({}, 'rid-ext');       // stamped __chainId: 8453 (Base)
  const reqWP = wpRequest('wp-1');                // no __chainId
  const GLOBAL_WP_CHAIN = 1;                       // the concurrent WalletPair session's chain
  expect(requestChainId(reqExt, GLOBAL_WP_CHAIN)).toBe(8453);  // per-request wins
  expect(requestChainId(reqWP, GLOBAL_WP_CHAIN)).toBe(1);      // ordinary → global
});

test('F3: the extension sheet uses the extension origin, not the concurrent WalletPair dApp identity', () => {
  const wpDApp = { name: 'wp-dapp.io', url: 'https://wp-dapp.io' };
  const reqExt = extRequest({}, 'rid-ext');
  const reqWP = wpRequest('wp-1');
  expect(requestDApp(reqExt, wpDApp)).toEqual({ name: 'app.uniswap.org', url: 'https://app.uniswap.org' });
  expect(requestDApp(reqWP, wpDApp)).toBe(wpDApp);
});

test('the extension transport settles ONLY on its own response — a concurrent WalletPair reply never settles it', async () => {
  readFile.mockResolvedValueOnce(
    JSON.stringify({ rid: 'rid-ext', method: 'personal_sign', params: [], origin: 'https://app.uniswap.org', ts: 1 }),
  );
  const wp = mockWalletPair();
  const ext = new ExtensionBridgeTransport('rid-ext');
  let extDisconnected = false;
  ext.on('disconnected', () => (extDisconnected = true));
  await ext.connect();
  expect(ext.connected).toBe(true);

  // A WalletPair response resolves — routed to wp, it must NOT touch or settle ext.
  responseTransport(wpRequest('wp-1'), wp as any)!.sendResponse('wp-1', '0xWP');
  await flush();
  expect(ext.connected).toBe(true);   // still awaiting ITS own sign
  expect(extDisconnected).toBe(false);

  // The extension's own response settles it (and only it).
  ext.sendResponse('rid-ext', '0xEXT');
  await flush();
  expect(ext.connected).toBe(false);
  expect(extDisconnected).toBe(true);
});
