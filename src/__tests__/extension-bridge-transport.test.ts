// Fund-safety unit tests for ExtensionBridgeTransport — the App Group sign mailbox.
//
// Covers:
//  • connect() reads the sign-req and emits connected + request (rid is the id).
//  • The FROZEN sign-result contract (writeResult): only 'submitted'|'rejected'
//    reach disk; every non-4001 error writes NOTHING (page → recoverable 4900,
//    never a 4001 false-decline → double-spend).
//  • ANTI-DOUBLE-SUBMIT: a rid that already has a result NEVER re-emits the request
//    (no second signing modal) — it replays the prior outcome (§12.5 gate d).
//  • Request-payload TTL (§12.1.4): a stale sign-req is refused, never signed.
//
// @/modules/app-group is mocked with a name-keyed in-memory container so a written
// result is readable back (exercising the real re-launch → replay path).

let files: Record<string, string> = {};
const writeFile = jest.fn(async (name: string, json: string): Promise<void> => { files[name] = json; });
const readFile = jest.fn(async (name: string): Promise<string | null> =>
  Object.prototype.hasOwnProperty.call(files, name) ? files[name] : null);
jest.mock('@/modules/app-group', () => ({
  writeFile: (name: string, json: string) => writeFile(name, json),
  readFile: (name: string) => readFile(name),
  isSupportedSync: () => true,
}));

import { ExtensionBridgeTransport } from '@/services/extension-bridge-transport';

const flush = () => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
  files = {};
  writeFile.mockClear();
  readFile.mockClear();
});

// NB: real Date.now() (not mocked) — readSignRequest's poll loop needs it to advance.
function putReq(rid: string, req: Record<string, unknown>) {
  files[`sign-req-${rid}.json`] = JSON.stringify({ rid, method: 'personal_sign', params: [], origin: 'https://x.io', ts: Date.now(), ...req });
}
function putResult(rid: string, res: Record<string, unknown>) {
  files[`sign-result-${rid}.json`] = JSON.stringify({ rid, ts: Date.now(), ...res });
}
function lastWrite() {
  if (!writeFile.mock.calls.length) return null;
  const [name, json] = writeFile.mock.calls[writeFile.mock.calls.length - 1];
  return { name, obj: JSON.parse(json) };
}

test('connect() reads sign-req and emits connected + request (rid is the id)', async () => {
  const rid = 'rid-1';
  putReq(rid, { method: 'personal_sign', params: ['0xdead', '0xabc'], origin: 'https://x.io', chainId: 8453, address: '0xAbC' });
  const t = new ExtensionBridgeTransport(rid);
  const events: any[] = [];
  t.on('connected', (n) => events.push(['connected', n]));
  t.on('request', (id, method, params, origin) => events.push(['request', id, method, params, origin]));
  await t.connect();
  expect(t.connected).toBe(true);
  expect(t.requestChainId).toBe(8453); // F4: per-request chain surfaced
  expect(t.requestOrigin).toBe('https://x.io');
  expect(t.requestAddress).toBe('0xAbC'); // §12.1.6: granted address surfaced
  expect(t.alreadySettled).toBe(false);
  expect(events[0]).toEqual(['connected', 'Safari Extension']);
  expect(events[1]).toEqual(['request', rid, 'personal_sign', ['0xdead', '0xabc'], 'https://x.io']);
});

test('connect() with no sign-req rejects + emits error (never a phantom sign)', async () => {
  const t = new ExtensionBridgeTransport('rid-missing');
  let errMsg = '';
  t.on('error', (m) => (errMsg = m));
  await expect(t.connect()).rejects.toThrow();
  expect(errMsg).toMatch(/not found|expired/i);
  expect(t.connected).toBe(false);
});

test('ANTI-DOUBLE-SUBMIT: connect() on a rid with an existing result replays it — NO request re-emitted', async () => {
  const rid = 'rid-done';
  putReq(rid, {}); // the (never-consumed) sign-req is still present…
  putResult(rid, { status: 'submitted', userOpHash: '0xHASH' }); // …but a result already exists
  const t = new ExtensionBridgeTransport(rid);
  const emitted: string[] = [];
  t.on('request', () => emitted.push('request'));
  await t.connect();
  expect(emitted).toEqual([]); // the signing modal is NEVER shown again (no re-sign)
  expect(t.alreadySettled).toBe(true);
  expect(t.outcome).toBe('submitted');
  // A late sendResponse can't rewrite the result either (already settled).
  writeFile.mockClear();
  t.sendResponse(rid, '0xOTHER');
  await flush();
  expect(writeFile).not.toHaveBeenCalled();
});

test('ANTI-DOUBLE-SUBMIT also short-circuits a prior REJECT (replays rejected, no re-sign)', async () => {
  const rid = 'rid-rej-done';
  putReq(rid, {});
  putResult(rid, { status: 'rejected', userOpHash: '0x' });
  const t = new ExtensionBridgeTransport(rid);
  const emitted: string[] = [];
  t.on('request', () => emitted.push('request'));
  await t.connect();
  expect(emitted).toEqual([]);
  expect(t.alreadySettled).toBe(true);
  expect(t.outcome).toBe('rejected');
});

test('request TTL (§12.1.4): a stale sign-req is refused, never signed', async () => {
  const rid = 'rid-stale';
  putReq(rid, { ts: Date.now() - 6 * 60 * 1000 }); // 6 min old > 5 min TTL
  const t = new ExtensionBridgeTransport(rid);
  const emitted: string[] = [];
  let errMsg = '';
  t.on('request', () => emitted.push('request'));
  t.on('error', (m) => (errMsg = m));
  await expect(t.connect()).rejects.toThrow(/expired/i);
  expect(emitted).toEqual([]);
  expect(errMsg).toMatch(/expired/i);
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
  putReq('r', { origin: 'https://app.uniswap.org' });
  const t = new ExtensionBridgeTransport('r');
  await t.connect();
  await expect(t.fetchDAppInfo()).resolves.toEqual({ name: 'app.uniswap.org', url: 'https://app.uniswap.org' });
});
