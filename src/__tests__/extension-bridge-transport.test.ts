// Fund-safety unit test for ExtensionBridgeTransport.writeResult — the FROZEN
// sign-result contract. Only 'submitted'|'rejected' may reach disk; every
// non-4001 error must write NOTHING (so the page falls to the recoverable 4900
// "check Vela", never a 4001 false-decline → double-spend).
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

const flush = () => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
  writeFile.mockClear();
  readFile.mockClear();
  readFile.mockResolvedValue(null);
});

function lastWrite() {
  if (!writeFile.mock.calls.length) return null;
  const [name, json] = writeFile.mock.calls[writeFile.mock.calls.length - 1];
  return { name, obj: JSON.parse(json) };
}

test('connect() reads sign-req and emits connected + request (rid is the id)', async () => {
  const rid = 'rid-1';
  readFile.mockResolvedValueOnce(
    JSON.stringify({ rid, method: 'personal_sign', params: ['0xdead', '0xabc'], origin: 'https://x.io', ts: 1, chainId: 8453 }),
  );
  const t = new ExtensionBridgeTransport(rid);
  const events: any[] = [];
  t.on('connected', (n) => events.push(['connected', n]));
  t.on('request', (id, method, params, origin) => events.push(['request', id, method, params, origin]));
  await t.connect();
  expect(t.connected).toBe(true);
  expect(t.requestChainId).toBe(8453); // F4: per-request chain surfaced
  expect(t.requestOrigin).toBe('https://x.io');
  expect(events[0]).toEqual(['connected', 'Safari Extension']);
  expect(events[1]).toEqual(['request', rid, 'personal_sign', ['0xdead', '0xabc'], 'https://x.io']);
});

test('connect() with no sign-req rejects + emits error (never a phantom sign)', async () => {
  readFile.mockResolvedValue(null); // never appears
  const t = new ExtensionBridgeTransport('rid-missing');
  let errMsg = '';
  t.on('error', (m) => (errMsg = m));
  await expect(t.connect()).rejects.toThrow();
  expect(errMsg).toMatch(/not found|expired/i);
  expect(t.connected).toBe(false);
});

test('success → writes { submitted, userOpHash: <result> } and disconnects after write', async () => {
  const t = new ExtensionBridgeTransport('rid-ok');
  let disconnected = false;
  t.on('disconnected', () => (disconnected = true));
  t.sendResponse('rid-ok', '0xSIGNATURE');
  await flush();
  const w = lastWrite();
  expect(w?.name).toBe('sign-result-rid-ok.json');
  expect(w?.obj).toMatchObject({ rid: 'rid-ok', status: 'submitted', userOpHash: '0xSIGNATURE' });
  expect(disconnected).toBe(true);
  expect(t.connected).toBe(false);
});

test('explicit reject (4001) → writes { rejected, userOpHash: "0x" }', async () => {
  const t = new ExtensionBridgeTransport('rid-rej');
  t.sendResponse('rid-rej', undefined, { code: 4001, message: 'User rejected' });
  await flush();
  expect(lastWrite()?.obj).toMatchObject({ status: 'rejected', userOpHash: '0x' });
});

test('NON-4001 errors write NOTHING (recoverable 4900, never a false-decline)', async () => {
  for (const code of [-32603, 4900, 4100, 4902, -32000]) {
    writeFile.mockClear();
    const t = new ExtensionBridgeTransport('rid-e' + code);
    let disconnected = false;
    t.on('disconnected', () => (disconnected = true));
    t.sendResponse('rid', undefined, { code, message: 'x' });
    await flush();
    expect(writeFile).not.toHaveBeenCalled(); // no file → page recovers via 4900
    expect(disconnected).toBe(true); // still settles the controller
  }
});

test('sendResponse is idempotent — a second call never double-writes', async () => {
  const t = new ExtensionBridgeTransport('rid-idem');
  t.sendResponse('rid-idem', '0xAAA');
  t.sendResponse('rid-idem', '0xBBB'); // ignored
  t.sendResponse('rid-idem', undefined, { code: 4001, message: 'x' }); // ignored
  await flush();
  expect(writeFile).toHaveBeenCalledTimes(1);
  expect(lastWrite()?.obj.userOpHash).toBe('0xAAA');
});

test('fetchDAppInfo derives per-request identity from the origin host', async () => {
  readFile.mockResolvedValueOnce(
    JSON.stringify({ rid: 'r', method: 'personal_sign', params: [], origin: 'https://app.uniswap.org', ts: 1 }),
  );
  const t = new ExtensionBridgeTransport('r');
  await t.connect();
  await expect(t.fetchDAppInfo()).resolves.toEqual({ name: 'app.uniswap.org', url: 'https://app.uniswap.org' });
});
