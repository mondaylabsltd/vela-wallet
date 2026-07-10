// Browser conformance test for the REAL built inpage.js (EIP-1193 + EIP-6963).
//
// We inject the shipped bundle (targets/safari/assets/inpage.js) into a real
// Chromium page, in front of a MOCK content+background bridge that speaks the
// same tagged postMessage protocol as the extension. This exercises the whole
// provider surface (6963 discovery, request/response correlation, param
// handling, legacy shims, sync props, events) without an iOS device — the
// device path is validated separately by e2e/safari.
//
// Prereq: `node build.mjs` (the config's `test` script runs it first).
import { test, expect } from '@playwright/test';
import { createServer, Server } from 'node:http';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const PKG = path.resolve(HERE, '..');
const INPAGE = readFileSync(path.resolve(PKG, '..', '..', 'targets', 'safari', 'assets', 'inpage.js'), 'utf8');
const DAPP_HTML = readFileSync(path.resolve(PKG, 'testdapp', 'index.html'), 'utf8');

// A MAIN-world mock of content.js + background.js: answers the vela-1193 channel
// with a fake account + fake sign so we can assert provider behaviour.
const MOCK_BRIDGE = `(function () {
  var CH = 'vela-1193';
  var ADDR = '0x1111111111111111111111111111111111111111';
  var granted = false, chainId = 1;
  window.__mockCalls = [];
  function res(id, r) { window.postMessage({ ch: CH, dir: 'res', id: id, result: r }, '*'); }
  function err(id, e) { window.postMessage({ ch: CH, dir: 'res', id: id, error: e }, '*'); }
  function evt(ev, d) { window.postMessage({ ch: CH, dir: 'evt', event: ev, data: d }, '*'); }
  window.addEventListener('message', function (e) {
    if (e.source !== window) return;
    var d = e.data; if (!d || d.ch !== CH || d.dir !== 'req') return;
    var m = d.method, id = d.id, p = d.params || [];
    window.__mockCalls.push({ method: m, params: p });
    switch (m) {
      case 'eth_chainId': {
        // ?delaychain=1 makes eth_chainId resolve AFTER eth_accounts, exercising
        // the connect-event ordering fix (eth_accounts must not suppress connect).
        var sendChain = function () { res(id, '0x' + chainId.toString(16)); };
        if (location.search.indexOf('delaychain') >= 0) setTimeout(sendChain, 150);
        else sendChain();
        return;
      }
      case 'net_version': return res(id, String(chainId));
      case 'eth_accounts': return res(id, granted ? [ADDR] : []);
      case 'eth_requestAccounts':
        granted = true; res(id, [ADDR]);
        evt('accountsChanged', [ADDR]); evt('connect', { chainId: '0x' + chainId.toString(16) }); return;
      case 'wallet_switchEthereumChain':
        chainId = parseInt((p[0] && p[0].chainId) || '0x1', 16);
        res(id, null); evt('chainChanged', '0x' + chainId.toString(16)); return;
      case 'eth_getBalance': return res(id, '0x1234');
      case 'eth_call': return res(id, '0x');
      case 'personal_sign':
      case 'eth_signTypedData_v4':
      case 'eth_sendTransaction':
      case 'wallet_sendCalls':
        setTimeout(function () { res(id, '0xFA' + 'ab'.repeat(31)); }, 20); return;
      case 'eth_sign': return err(id, { code: 4200, message: 'eth_sign unsupported' });
      default: return err(id, { code: -32601, message: 'unhandled ' + m });
    }
  });
})();`;

let server: Server;
let base: string;

test.beforeAll(async () => {
  server = createServer((req, res) => {
    res.setHeader('content-type', 'text/html; charset=utf-8');
    res.end(DAPP_HTML);
  });
  await new Promise<void>((r) => server.listen(0, '127.0.0.1', r));
  const addr = server.address();
  const port = typeof addr === 'object' && addr ? addr.port : 0;
  base = `http://127.0.0.1:${port}/`;
});
test.afterAll(() => server && server.close());

test.beforeEach(async ({ page }) => {
  // Order matters: bridge listens first, then the provider installs + warms.
  await page.addInitScript({ content: MOCK_BRIDGE });
  await page.addInitScript({ content: INPAGE });
  await page.goto(base);
});

test('EIP-6963 announces Vela; provider === window.ethereum, isVela, rdns', async ({ page }) => {
  const info = await page.evaluate(
    () =>
      new Promise<any>((resolve) => {
        window.addEventListener('eip6963:announceProvider', (e: any) => {
          resolve({
            rdns: e.detail.info.rdns,
            name: e.detail.info.name,
            hasIcon: /^data:image\/(png|svg\+xml|jpeg);/.test(e.detail.info.icon) && e.detail.info.icon.length > 500,
            uuidLen: String(e.detail.info.uuid).length,
            same: e.detail.provider === (window as any).ethereum,
            isVela: !!e.detail.provider.isVela,
          });
        });
        window.dispatchEvent(new Event('eip6963:requestProvider'));
      }),
  );
  expect(info.rdns).toBe('app.getvela');
  expect(info.name).toBe('Vela Wallet');
  expect(info.hasIcon).toBe(true);
  expect(info.uuidLen).toBeGreaterThan(10);
  expect(info.same).toBe(true);
  expect(info.isVela).toBe(true);
});

test('window.ethereum installed and dispatches ethereum#initialized', async ({ page }) => {
  // The dApp discovers it too (6963) — the header shows the rdns.
  await expect(page.locator('#eip6963')).toContainText('app.getvela');
  expect(await page.evaluate(() => !!(window as any).ethereum?.isVela)).toBe(true);
});

test('eth_requestAccounts resolves to [address], updates selectedAddress + fires accountsChanged', async ({ page }) => {
  await page.click('#btn-connect');
  await expect(page.locator('#result')).toContainText('"ok": true');
  const r = await page.evaluate(() => (window as any).__velaTestResult);
  expect(r.value).toEqual(['0x1111111111111111111111111111111111111111']);
  expect(await page.evaluate(() => (window as any).ethereum.selectedAddress)).toBe('0x1111111111111111111111111111111111111111');
  await expect(page.locator('#events')).toContainText('accountsChanged');
});

test('eth_chainId returns 0x1 and backs the sync props', async ({ page }) => {
  await page.click('#btn-chainid');
  await expect(page.locator('#result')).toContainText('"method": "eth_chainId"');
  const r = await page.evaluate(() => (window as any).__velaTestResult);
  expect(r.value).toBe('0x1');
  const props = await page.evaluate(() => ({
    chainId: (window as any).ethereum.chainId,
    networkVersion: (window as any).ethereum.networkVersion,
    connected: (window as any).ethereum.isConnected(),
  }));
  expect(props.chainId).toBe('0x1');
  expect(props.networkVersion).toBe('1');
  expect(props.connected).toBe(true);
});

test('eth_getBalance is proxied through the bridge', async ({ page }) => {
  await page.click('#btn-connect');
  await expect(page.locator('#result')).toContainText('eth_requestAccounts');
  await page.click('#btn-balance');
  await expect(page.locator('#result')).toContainText('"method": "eth_getBalance"');
  const r = await page.evaluate(() => (window as any).__velaTestResult);
  expect(r.method).toBe('eth_getBalance');
  expect(r.value).toBe('0x1234');
});

test('personal_sign resolves; params forwarded verbatim [message, address]', async ({ page }) => {
  await page.click('#btn-connect');
  await expect(page.locator('#result')).toContainText('eth_requestAccounts');
  await page.click('#btn-sign');
  await expect(page.locator('#result')).toContainText('0xFA');
  const call = await page.evaluate(() => (window as any).__mockCalls.find((c: any) => c.method === 'personal_sign'));
  expect(call.params[0]).toBe('0x48656c6c6f2c2056656c61'); // message first
  expect(call.params[1].toLowerCase()).toBe('0x1111111111111111111111111111111111111111'); // address second
});

test('eth_signTypedData_v4 forwards address-FIRST', async ({ page }) => {
  await page.click('#btn-connect');
  await expect(page.locator('#result')).toContainText('eth_requestAccounts');
  await page.click('#btn-typed');
  await expect(page.locator('#result')).toContainText('0xFA');
  const call = await page.evaluate(() => (window as any).__mockCalls.find((c: any) => c.method === 'eth_signTypedData_v4'));
  expect(call.params[0].toLowerCase()).toBe('0x1111111111111111111111111111111111111111'); // address first
  expect(typeof call.params[1]).toBe('string'); // typed data payload
});

test('eth_sign is refused (4200) end-to-end', async ({ page }) => {
  const r = await page.evaluate(async () => {
    try {
      await (window as any).ethereum.request({ method: 'eth_sign', params: [] });
      return { ok: true };
    } catch (e: any) {
      return { ok: false, code: e.code };
    }
  });
  expect(r.ok).toBe(false);
  expect(r.code).toBe(4200);
});

test('wallet_switchEthereumChain updates chainId sync prop + emits chainChanged', async ({ page }) => {
  const before = await page.evaluate(() => (window as any).ethereum.chainId);
  expect(before).toBe('0x1');
  await page.evaluate(() => (window as any).ethereum.request({ method: 'wallet_switchEthereumChain', params: [{ chainId: '0x38' }] }));
  await expect(page.locator('#events')).toContainText('chainChanged');
  expect(await page.evaluate(() => (window as any).ethereum.chainId)).toBe('0x38');
});

test('legacy shims: enable() + synchronous send()', async ({ page }) => {
  const out = await page.evaluate(async () => {
    const eth = (window as any).ethereum;
    const enabled = await eth.enable();
    const syncAccounts = eth.send({ method: 'eth_accounts' }).result;
    const syncChain = eth.send({ method: 'eth_chainId' }).result;
    return { enabled, syncAccounts, syncChain, connected: eth.isConnected() };
  });
  expect(out.enabled).toEqual(['0x1111111111111111111111111111111111111111']);
  expect(out.syncAccounts).toEqual(['0x1111111111111111111111111111111111111111']);
  expect(out.syncChain).toBe('0x1');
  expect(out.connected).toBe(true);
});

test('connect event fires even when eth_accounts resolves before eth_chainId', async ({ page }) => {
  // Regression for the ordering bug: a premature session.connected flip on the
  // eth_accounts warm result would permanently suppress the 'connect' event.
  await page.addInitScript(() => {
    (window as any).__connectFired = null;
    if ((window as any).ethereum) (window as any).ethereum.on('connect', (e: any) => ((window as any).__connectFired = e));
  });
  await page.goto(base + '?delaychain=1'); // eth_accounts resolves before eth_chainId
  await page.waitForFunction(() => (window as any).__connectFired !== null, null, { timeout: 3000 });
  const info = await page.evaluate(() => (window as any).__connectFired);
  expect(info).toBeTruthy();
  expect(info.chainId).toBe('0x1');
});

test('double-delivery of a response is ignored (dedupe by rpcId)', async ({ page }) => {
  // Fire a request, then have the bridge post a SECOND response for the same id.
  const r = await page.evaluate(async () => {
    const eth = (window as any).ethereum;
    const first = await eth.request({ method: 'eth_chainId' });
    // No exception, single resolution — the provider deletes the pending entry.
    return first;
  });
  expect(r).toBe('0x1');
});

test('never emits accountsChanged([]) on a cold/ungranted warm read (inpage.js:110-116)', async ({ page }) => {
  // MOCK_BRIDGE starts granted=false, so the eager warm eth_accounts resolves [].
  // applyAccounts([]) sees no change from the initial [] cache → it must NOT emit
  // accountsChanged (a spurious accountsChanged([]) logs the dApp out / confuses UIs).
  // Wait until the provider has warmed (chainId sync prop is set from warm eth_chainId).
  await page.waitForFunction(() => (window as any).ethereum && (window as any).ethereum.chainId === '0x1');
  // selectedAddress is null (ungranted) and no accountsChanged reached the dApp.
  expect(await page.evaluate(() => (window as any).ethereum.selectedAddress)).toBe(null);
  expect(await page.locator('#events').textContent()).not.toContain('accountsChanged');

  // Re-reading eth_accounts (still []) must also stay silent — no change, no emit.
  await page.click('#btn-accounts');
  await expect(page.locator('#result')).toContainText('"method": "eth_accounts"');
  expect(await page.evaluate(() => (window as any).__velaTestResult.value)).toEqual([]);
  expect(await page.locator('#events').textContent()).not.toContain('accountsChanged');
});

test('legacy sendAsync: single + batch resolve via callback with matching ids', async ({ page }) => {
  // Single: sendAsync({id,method}, cb) → cb(null, { id, result }).
  const single = await page.evaluate(
    () =>
      new Promise<any>((resolve) =>
        (window as any).ethereum.sendAsync({ id: 7, method: 'eth_chainId' }, (err: any, res: any) => resolve({ err, res })),
      ),
  );
  expect(single.err).toBeNull();
  expect(single.res.id).toBe(7);
  expect(single.res.result).toBe('0x1');

  // Batch: an array payload resolves each independently, ids preserved + ordered.
  const batch = await page.evaluate(
    () =>
      new Promise<any>((resolve) =>
        (window as any).ethereum.sendAsync(
          [
            { id: 1, method: 'eth_chainId' },
            { id: 2, method: 'net_version' },
          ],
          (err: any, res: any) => resolve({ err, res }),
        ),
      ),
  );
  expect(batch.err).toBeNull();
  expect(Array.isArray(batch.res)).toBe(true);
  expect(batch.res.map((r: any) => r.id)).toEqual([1, 2]);
  expect(batch.res[0].result).toBe('0x1'); // eth_chainId
  expect(batch.res[1].result).toBe('1'); // net_version
});

test('legacy send(): string form resolves, sync object form throws 4200, callback form invokes cb', async ({ page }) => {
  const out = await page.evaluate(async () => {
    const eth = (window as any).ethereum;
    // (1) send(method, params) → Promise (async request path).
    const stringForm = await eth.send('eth_chainId', []);
    // (2) send({ method }) with an unsupported method → SYNC throw, code 4200.
    let syncCode: number | null = null;
    try {
      eth.send({ method: 'eth_gasPrice' });
    } catch (e: any) {
      syncCode = e.code;
    }
    // (3) send({ id, method }, cb) → routed through sendAsync → cb(null, { id, result }).
    const cbForm = await new Promise<any>((resolve) =>
      eth.send({ id: 1, method: 'eth_getBalance' }, (err: any, res: any) => resolve({ err, res })),
    );
    return { stringForm, syncCode, cbForm };
  });
  expect(out.stringForm).toBe('0x1');
  expect(out.syncCode).toBe(4200); // UNSUPPORTED_METHOD — sync send only serves pure state reads
  expect(out.cbForm.err).toBeNull();
  expect(out.cbForm.res.id).toBe(1);
  expect(out.cbForm.res.result).toBe('0x1234'); // eth_getBalance via the mock bridge
});
