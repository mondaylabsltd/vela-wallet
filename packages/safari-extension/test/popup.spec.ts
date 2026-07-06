// Toolbar popup test: the REAL popup.js + background.js wired through the same
// faked `browser` runtime as integration.spec.ts (plus a `browser.tabs` stub).
// Proves the popup reads the app account cache, reflects the current site's grant,
// lists other connected sites, and that its disconnect actually revokes the grant.
import { test, expect } from '@playwright/test';
import { createServer, Server } from 'node:http';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const PKG = path.resolve(HERE, '..');
const ASSETS = path.resolve(PKG, '..', '..', 'targets', 'safari', 'assets');
const BACKGROUND = readFileSync(path.join(ASSETS, 'background.js'), 'utf8');
const POPUP_HTML = readFileSync(path.join(ASSETS, 'popup.html'), 'utf8');
const POPUP_JS = readFileSync(path.join(ASSETS, 'popup.js'), 'utf8');

const ADDR = '0x2222222222222222222222222222222222222222';
const CUR = 'https://app.uniswap.org'; // the active tab's origin (granted)
const OTHER = 'https://opensea.io'; // a second granted site

// Fake `browser`: runtime.sendMessage → background listener; in-memory storage
// pre-seeded with two grants; sendNativeMessage → getAccount; tabs stub for the
// active tab + a create() spy for the "open Vela" hand-off. UL attestation state
// (ulVerified/ulVerifiedAt on the account, an optional broken-veto in storage) is
// parameterized so each UL test can express exactly one state.
const mkEnv = (o: { ulVerified?: boolean; ulVerifiedAt?: number; brokenAt?: number } = {}) => `
window.__native = {
  account: { address: '${ADDR}', name: 'Main', accounts: [{ name: 'Main', address: '${ADDR}' }],
             ulVerified: ${!!o.ulVerified}, ulVerifiedAt: ${o.ulVerifiedAt || 0}, locale: 'zh',
             chainId: 1, chains: { '1': { name: 'Ethereum', rpcUrl: 'http://rpc.test/1', bundlerUrl: 'http://b.test/1' } } },
  async handle(msg) {
    if (msg.type === 'getAccount') return { type: 'account', found: true, account: this.account };
    return { type: 'error' };
  },
};
(function () {
  var listeners = [];
  var store = {
    'vela.perm.${CUR}':   { origin: '${CUR}',   address: '${ADDR}', chainId: 1, grantedAt: 2 },
    'vela.perm.${OTHER}': { origin: '${OTHER}', address: '${ADDR}', chainId: 1, grantedAt: 1 },
    ${o.brokenAt ? `'vela.ul.broken': { ts: ${o.brokenAt} },` : ''}
  };
  function clone(v){ return v === undefined ? undefined : JSON.parse(JSON.stringify(v)); }
  window.__opened = null;
  window.browser = {
    runtime: {
      onMessage: { addListener: function (fn) { listeners.push(fn); } },
      sendMessage: function (msg) {
        return new Promise(function (resolve) {
          var done = false, async = false;
          function sendResponse(r) { if (!done) { done = true; resolve(clone(r)); } }
          for (var i = 0; i < listeners.length; i++) { if (listeners[i](clone(msg), { id: 't' }, sendResponse) === true) async = true; }
          if (!async && !done) resolve(undefined);
        });
      },
      sendNativeMessage: function (_id, msg) { return window.__native.handle(clone(msg)); },
    },
    storage: { local: {
      get: function (keys) {
        var out = {};
        if (keys == null) { for (var k in store) out[k] = clone(store[k]); }
        else if (typeof keys === 'string') { if (store[keys] !== undefined) out[keys] = clone(store[keys]); }
        else { for (var i = 0; i < keys.length; i++) if (store[keys[i]] !== undefined) out[keys[i]] = clone(store[keys[i]]); }
        return Promise.resolve(out);
      },
      set: function (obj) { for (var k in obj) store[k] = clone(obj[k]); return Promise.resolve(); },
      remove: function (k) { delete store[k]; return Promise.resolve(); },
    } },
    tabs: {
      query: function () { return Promise.resolve([{ active: true, url: '${CUR}/swap?x=1' }]); },
      create: function (o) { window.__opened = o.url; return Promise.resolve({}); },
    },
  };
})();
`;

let server: Server;
let base: string;

test.beforeAll(async () => {
  // Serve popup.html at / and popup.js at /popup.js so the real <script src> loads.
  server = createServer((req, res) => {
    if (req.url && req.url.startsWith('/popup.js')) {
      res.setHeader('content-type', 'text/javascript'); res.end(POPUP_JS);
    } else {
      res.setHeader('content-type', 'text/html; charset=utf-8'); res.end(POPUP_HTML);
    }
  });
  await new Promise<void>((r) => server.listen(0, '127.0.0.1', r));
  const a = server.address();
  base = `http://127.0.0.1:${typeof a === 'object' && a ? a.port : 0}/`;
});
test.afterAll(() => server && server.close());

test('popup renders account, current-site grant, other sites; disconnect revokes', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(String(e)));

  await page.addInitScript({ content: mkEnv() });
  await page.addInitScript({ content: BACKGROUND });
  await page.goto(base);

  // account card from the app cache
  await expect(page.locator('#account')).toContainText('Main');
  await expect(page.locator('#account')).toContainText('0x2222…2222');
  await expect(page.locator('#account')).toContainText('Ethereum');

  // current site (uniswap) is granted → 已连接 + a disconnect button
  await expect(page.locator('#site-status')).toContainText('app.uniswap.org');
  await expect(page.locator('#site-status')).toContainText('已连接');

  // the OTHER granted site is listed
  await expect(page.locator('#sites-title')).toContainText('其他已连接站点 (1)');
  await expect(page.locator('#sites')).toContainText('opensea.io');

  // UL not attested yet → the popup offers the self-test probe (not the verified badge)
  await expect(page.locator('#ul-status')).toContainText('测试一键签名');
  await expect(page.locator('#ul-status .ul-ok')).toHaveCount(0);

  // disconnect the current site → revoke → re-render → 未连接
  await page.locator('#site-status button.disconnect').click();
  await expect(page.locator('#site-status')).toContainText('未连接');

  // "open Vela" hands off via tabs.create(velawallet://)
  await page.locator('#open').click();
  expect(await page.evaluate(() => (window as any).__opened)).toBe('velawallet://');

  expect(errors).toEqual([]);
});

test('attested + no veto → verified badge + reachable re-test (not the probe prompt)', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript({ content: mkEnv({ ulVerified: true, ulVerifiedAt: 1000 }) });
  await page.addInitScript({ content: BACKGROUND });
  await page.goto(base);

  await expect(page.locator('#ul-status .ul-ok')).toContainText('一键签名已启用');
  await expect(page.locator('#ul-status button.ul-test')).toHaveCount(0);
  // A reachable re-test even when verified (so a device that later breaks isn't stuck).
  await expect(page.locator('#ul-status button.ul-reverify')).toContainText('重新测试');
  expect(errors).toEqual([]);
});

test('self-test probe stamps UL_PENDING + opens the UL self-test URL in a new tab', async ({ page }) => {
  await page.addInitScript({ content: mkEnv() });
  await page.addInitScript({ content: BACKGROUND });
  await page.goto(base);
  await page.locator('#ul-status button.ul-test').click();
  // openProbe stamps pending (async) then tabs.create — poll for the new-tab URL.
  await expect
    .poll(() => page.evaluate(() => (window as any).__opened))
    .toBe('https://getvela.app/sign?rid=ul-selftest');
  // pending stamped so a FAILED probe can re-veto via the getvela.app landing.
  expect(await page.evaluate(() => (window as any).browser.storage.local.get('vela.ul.pending')
    .then((r: any) => !!r['vela.ul.pending']))).toBe(true);
});

test('veto NEWER than attestation → broken UI (re-test prompt), no ✓ badge', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  // Attested at 1000, then broke at 2000 → the veto wins.
  await page.addInitScript({ content: mkEnv({ ulVerified: true, ulVerifiedAt: 1000, brokenAt: 2000 }) });
  await page.addInitScript({ content: BACKGROUND });
  await page.goto(base);
  await expect(page.locator('#ul-status')).toContainText('跳转失败');
  await expect(page.locator('#ul-status button.ul-test')).toContainText('重新测试一键签名');
  await expect(page.locator('#ul-status .ul-ok')).toHaveCount(0);
  expect(errors).toEqual([]);
});

test('re-attestation NEWER than the veto → verified again (race-free, no manual clear)', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  // Broke at 2000, then a successful probe re-attested at 3000 → attestation wins.
  await page.addInitScript({ content: mkEnv({ ulVerified: true, ulVerifiedAt: 3000, brokenAt: 2000 }) });
  await page.addInitScript({ content: BACKGROUND });
  await page.goto(base);
  await expect(page.locator('#ul-status .ul-ok')).toContainText('一键签名已启用');
  await expect(page.locator('#ul-status')).not.toContainText('跳转失败');
  expect(errors).toEqual([]);
});
