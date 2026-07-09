// FULL-STACK integration test: the REAL content.js + background.js + inpage.js
// wired together in a real Chromium page through a faked `browser` runtime + a
// faked native handler. Unlike provider.spec.ts (real inpage vs a MOCK bridge),
// this EXECUTES the real router, connect/sign sheets (shadow DOM), grant logic,
// read-proxy, and the return-poll — the code that was previously only reviewed.
//
// It collapses the three extension worlds into one page (fine for a logic test):
// content ↔ background talk over the fake browser.runtime; inpage ↔ content over
// real window.postMessage; native is a fake object the test drives to simulate
// the app returning a sign result.
import { test, expect } from '@playwright/test';
import { createServer, Server } from 'node:http';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const PKG = path.resolve(HERE, '..');
const ASSETS = path.resolve(PKG, '..', '..', 'targets', 'safari', 'assets');
const INPAGE = readFileSync(path.join(ASSETS, 'inpage.js'), 'utf8');
const CONTENT = readFileSync(path.join(ASSETS, 'content.js'), 'utf8');
const BACKGROUND = readFileSync(path.join(ASSETS, 'background.js'), 'utf8');
const DAPP_HTML = readFileSync(path.resolve(PKG, 'testdapp', 'index.html'), 'utf8');

const ADDR = '0x1111111111111111111111111111111111111111';

// Fake `browser` runtime + native. Wires content.runtime.sendMessage → background
// onMessage listener; storage.local in-memory; getURL('inpage.js') → a blob of the
// real inpage bundle so content's REAL injection path runs; sendNativeMessage →
// window.__native (getAccount / writeSignRequest / pollSignResult).
const FAKE_ENV = `
window.__native = {
  account: { address: '${ADDR}', name: 'Main', accounts: [{ name: 'Main', address: '${ADDR}' }],
             chainId: 1, chains: { '1': { name: 'Ethereum', rpcUrl: 'http://rpc.test/1', bundlerUrl: 'http://bundler.test/1' } } },
  signResults: {},   // rid -> { status, userOpHash }  (test sets this to simulate app return)
  writes: [],
  async handle(msg) {
    if (msg.type === 'getAccount') return { type: 'account', found: true, account: this.account };
    if (msg.type === 'writeSignRequest') { this.writes.push(msg); return { type: 'sign-req-ack', rid: msg.rid }; }
    if (msg.type === 'pollSignResult') {
      var r = this.signResults[msg.rid];
      return { type: 'sign-result', rid: msg.rid, found: !!r, result: r || {} };
    }
    return { type: 'error' };
  },
};
(function () {
  var listeners = [];
  var store = {};
  function clone(v){ return v === undefined ? undefined : JSON.parse(JSON.stringify(v)); }
  window.browser = {
    runtime: {
      getURL: function (p) { return URL.createObjectURL(new Blob([window.__INPAGE_SRC], { type: 'text/javascript' })); },
      onMessage: { addListener: function (fn) { listeners.push(fn); } },
      // content → background. Resolve with whatever a listener sendResponse's.
      sendMessage: function (msg) {
        return new Promise(function (resolve) {
          var done = false; var async = false;
          function sendResponse(r) { if (!done) { done = true; resolve(clone(r)); } }
          for (var i = 0; i < listeners.length; i++) {
            var ret = listeners[i](clone(msg), { id: 'test' }, sendResponse);
            if (ret === true) async = true;
          }
          if (!async && !done) resolve(undefined);
        });
      },
      // background → native
      sendNativeMessage: function (appId, msg) { return window.__native.handle(clone(msg)); },
    },
    storage: {
      local: {
        get: function (keys) {
          var out = {};
          if (keys === null || keys === undefined) { for (var k in store) out[k] = clone(store[k]); }
          else if (typeof keys === 'string') { if (k_in(keys)) out[keys] = clone(store[keys]); }
          else { for (var i = 0; i < keys.length; i++) if (k_in(keys[i])) out[keys[i]] = clone(store[keys[i]]); }
          function k_in(k){ return Object.prototype.hasOwnProperty.call(store, k); }
          return Promise.resolve(out);
        },
        set: function (obj) { for (var k in obj) store[k] = clone(obj[k]); return Promise.resolve(); },
        remove: function (k) { delete store[k]; return Promise.resolve(); },
      },
    },
  };
})();
`;

let server: Server;
let base: string;

test.beforeAll(async () => {
  server = createServer((_req, res) => { res.setHeader('content-type', 'text/html; charset=utf-8'); res.end(DAPP_HTML); });
  await new Promise<void>((r) => server.listen(0, '127.0.0.1', r));
  const a = server.address();
  base = `http://127.0.0.1:${typeof a === 'object' && a ? a.port : 0}/`;
});
test.afterAll(() => server && server.close());

test.beforeEach(async ({ page }) => {
  // Intercept the read-proxy RPC so eth_getBalance is deterministic + offline.
  await page.route('http://rpc.test/**', (route) =>
    route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ jsonrpc: '2.0', id: 1, result: '0x1234' }) }),
  );
  // env (defines browser) + inpage src + background (registers listener) install
  // at document_start. content.js touches the DOM (injects inpage), so — unlike a
  // real Safari content script where documentElement already exists at
  // document_start — we inject it AFTER goto so the DOM is present; its real
  // injection path (append inpage script) then runs unchanged.
  await page.addInitScript({ content: `window.__INPAGE_SRC = ${JSON.stringify(INPAGE)};` });
  await page.addInitScript({ content: FAKE_ENV });
  await page.addInitScript({ content: BACKGROUND });
  await page.goto(base);
  await page.addScriptTag({ content: CONTENT });
  await page.waitForFunction(() => !!(window as any).ethereum?.isVela, null, { timeout: 5000 });
});

test('connect: real connect sheet renders, confirm grants, dApp gets [address] + accountsChanged', async ({ page }) => {
  await page.click('#btn-connect');
  // The REAL content.js connect sheet (open shadow root) appears.
  await page.waitForFunction(() => {
    const h = document.getElementById('vela-sheet-host') as any;
    return h && h.shadowRoot && h.shadowRoot.getElementById('cta');
  });
  // Confirm inside the shadow root.
  await page.evaluate(() => (document.getElementById('vela-sheet-host') as any).shadowRoot.getElementById('cta').click());
  await expect(page.locator('#result')).toContainText('eth_requestAccounts');
  const r = await page.evaluate(() => (window as any).__velaTestResult);
  expect(r.ok).toBe(true);
  expect(r.value).toEqual([ADDR]);
  await expect(page.locator('#events')).toContainText('accountsChanged');
});

test('state + read after connect: eth_accounts, eth_chainId, and a proxied eth_getBalance', async ({ page }) => {
  await page.click('#btn-connect');
  await page.waitForFunction(() => (document.getElementById('vela-sheet-host') as any)?.shadowRoot?.getElementById('cta'));
  await page.evaluate(() => (document.getElementById('vela-sheet-host') as any).shadowRoot.getElementById('cta').click());
  await expect(page.locator('#result')).toContainText('eth_requestAccounts');

  await page.click('#btn-accounts');
  await expect(page.locator('#result')).toContainText('"method": "eth_accounts"');
  expect(await page.evaluate(() => (window as any).__velaTestResult.value)).toEqual([ADDR]);

  await page.click('#btn-chainid');
  await expect(page.locator('#result')).toContainText('"method": "eth_chainId"');
  expect(await page.evaluate(() => (window as any).__velaTestResult.value)).toBe('0x1');

  await page.click('#btn-balance');
  await expect(page.locator('#result')).toContainText('"method": "eth_getBalance"');
  expect(await page.evaluate(() => (window as any).__velaTestResult.value)).toBe('0x1234'); // via real proxyRpc
});

test('eth_accounts on an UNGRANTED origin returns [] (never prompts)', async ({ page }) => {
  await page.click('#btn-accounts');
  await expect(page.locator('#result')).toContainText('"method": "eth_accounts"');
  expect(await page.evaluate(() => (window as any).__velaTestResult.value)).toEqual([]);
  // no sheet was shown
  expect(await page.evaluate(() => !!document.getElementById('vela-sheet-host'))).toBe(false);
});

test('sign loop: hand-off sheet → launch writes sign-req → app returns submitted → dApp promise resolves', async ({ page }) => {
  // connect first (sign requires a grant)
  await page.click('#btn-connect');
  await page.waitForFunction(() => (document.getElementById('vela-sheet-host') as any)?.shadowRoot?.getElementById('cta'));
  await page.evaluate(() => (document.getElementById('vela-sheet-host') as any).shadowRoot.getElementById('cta').click());
  await expect(page.locator('#result')).toContainText('eth_requestAccounts');

  // personal_sign → the REAL sign hand-off sheet (CTA is a button; onSignLaunch
  // sets location.href=velawallet:// which is an inert no-op scheme in Chromium).
  await page.click('#btn-sign');
  await page.waitForFunction(() => (document.getElementById('vela-sheet-host') as any)?.shadowRoot?.getElementById('cta'));
  await page.evaluate(() => (document.getElementById('vela-sheet-host') as any).shadowRoot.getElementById('cta').click());
  // The real content.js relayed the sign request to (fake) native — the rid is authoritative there.
  await page.waitForFunction(() => (window as any).__native.writes.length > 0);
  const rid = await page.evaluate(() => { const w = (window as any).__native.writes; return w[w.length - 1].rid; });

  // Simulate the app finishing: write the frozen submitted result, then return focus.
  await page.evaluate((r) => { (window as any).__native.signResults[r] = { status: 'submitted', userOpHash: '0xFAdeadbeef' }; }, rid);
  await page.evaluate(() => document.dispatchEvent(new Event('visibilitychange')));

  await expect(page.locator('#result')).toContainText('personal_sign');
  const r = await page.evaluate(() => (window as any).__velaTestResult);
  expect(r.ok).toBe(true);
  expect(r.value).toBe('0xFAdeadbeef'); // the dApp promise resolved with the returned hash
});

test('sign reject: app returns rejected → dApp gets 4001 (only on a real reject)', async ({ page }) => {
  await page.click('#btn-connect');
  await page.waitForFunction(() => (document.getElementById('vela-sheet-host') as any)?.shadowRoot?.getElementById('cta'));
  await page.evaluate(() => (document.getElementById('vela-sheet-host') as any).shadowRoot.getElementById('cta').click());
  await expect(page.locator('#result')).toContainText('eth_requestAccounts');

  await page.click('#btn-sign');
  await page.waitForFunction(() => (document.getElementById('vela-sheet-host') as any)?.shadowRoot?.getElementById('cta'));
  await page.evaluate(() => (document.getElementById('vela-sheet-host') as any).shadowRoot.getElementById('cta').click());
  await page.waitForFunction(() => (window as any).__native.writes.length > 0);
  const rid = await page.evaluate(() => { const w = (window as any).__native.writes; return w[w.length - 1].rid; });

  await page.evaluate((r) => { (window as any).__native.signResults[r] = { status: 'rejected', userOpHash: '0x' }; }, rid);
  await page.evaluate(() => document.dispatchEvent(new Event('visibilitychange')));

  await expect(page.locator('#result')).toContainText('personal_sign');
  const r = await page.evaluate(() => (window as any).__velaTestResult);
  expect(r.ok).toBe(false);
  expect(r.code).toBe(4001); // 4001 ONLY because native said rejected
});

test('sign-req carries the granted address (§12.1.6 — app signs from the connected account)', async ({ page }) => {
  await page.click('#btn-connect');
  await page.waitForFunction(() => (document.getElementById('vela-sheet-host') as any)?.shadowRoot?.getElementById('cta'));
  await page.evaluate(() => (document.getElementById('vela-sheet-host') as any).shadowRoot.getElementById('cta').click());
  await expect(page.locator('#result')).toContainText('eth_requestAccounts');

  await page.click('#btn-sign');
  await page.waitForFunction(() => (document.getElementById('vela-sheet-host') as any)?.shadowRoot?.getElementById('cta'));
  await page.evaluate(() => (document.getElementById('vela-sheet-host') as any).shadowRoot.getElementById('cta').click());
  await page.waitForFunction(() => (window as any).__native.writes.length > 0);
  const req = await page.evaluate(() => { const w = (window as any).__native.writes; return w[w.length - 1].request; });
  expect((req.address || '').toLowerCase()).toBe(ADDR.toLowerCase()); // the granted account rides the sign-req
});

test('dead-poll floor: no result on return → sheet shows recoverable "check Vela", ring never hangs, no false 4001', async ({ page }) => {
  await page.click('#btn-connect');
  await page.waitForFunction(() => (document.getElementById('vela-sheet-host') as any)?.shadowRoot?.getElementById('cta'));
  await page.evaluate(() => (document.getElementById('vela-sheet-host') as any).shadowRoot.getElementById('cta').click());
  await expect(page.locator('#result')).toContainText('eth_requestAccounts');

  await page.click('#btn-sign');
  await page.waitForFunction(() => (document.getElementById('vela-sheet-host') as any)?.shadowRoot?.getElementById('cta'));
  await page.evaluate(() => (document.getElementById('vela-sheet-host') as any).shadowRoot.getElementById('cta').click());
  await page.waitForFunction(() => (window as any).__native.writes.length > 0);

  // The app NEVER writes a result (evicted worker / lost hand-off). Return focus and
  // let the bounded poll cycle run — the sheet must swap the ring for a recoverable
  // "check Vela" affordance (dataset.state='checking'), NOT hang the ring, NOT 4001.
  await page.evaluate(() => document.dispatchEvent(new Event('visibilitychange')));
  await page.waitForFunction(
    () => (document.getElementById('vela-sheet-host') as any)?.shadowRoot?.getElementById('sheet')?.dataset.state === 'checking',
    undefined,
    { timeout: 15_000 },
  );
  // A "return to Vela" re-open affordance exists (never a dead end), and NO result
  // reached the dApp — the promise is still pending (no false decline).
  const hasReopen = await page.evaluate(() => !!(document.getElementById('vela-sheet-host') as any).shadowRoot.getElementById('reopen'));
  expect(hasReopen).toBe(true);
  const settled = await page.evaluate(() => (window as any).__velaTestResult && (window as any).__velaTestResult.method === 'personal_sign');
  expect(settled).toBeFalsy(); // still pending — never resolved, never a 4001
});

test('eth_signTransaction is refused by the real router (never proxied to RPC)', async ({ page }) => {
  const r = await page.evaluate(
    (addr) =>
      (window as any).ethereum
        .request({ method: 'eth_signTransaction', params: [{ from: addr, to: addr }] })
        .then(() => ({ ok: true }))
        .catch((e: any) => ({ ok: false, code: e.code })),
    ADDR,
  );
  expect(r.ok).toBe(false);
  expect(r.code).toBe(4200); // UNSUPPORTED_METHOD — allowlist, no fail-open
});

const PERM_PREFIX = 'vela.perm.';

// §12.4 grant re-validation (background.js validGrantedAddress:88-97). A stored grant
// must SURVIVE a cold-cache miss (evicted worker / getAccount not-found) but be dropped
// once the cache is PRESENT and no longer lists the granted address.
test('cold-cache miss KEEPS a prior grant: eth_accounts + wallet_getPermissions still list the address', async ({ page }) => {
  const origin = await page.evaluate(() => location.origin);
  // Seed a prior grant for this origin directly into the extension's storage.local.
  await page.evaluate(
    ([key, addr]) => (window as any).browser.storage.local.set({ [key]: { origin: location.origin, address: addr, chainId: 1 } }),
    [PERM_PREFIX + origin, ADDR],
  );
  // Simulate a cold account cache: native getAccount now reports not-found.
  await page.evaluate(() => {
    (window as any).__native.handle = async function (msg: any) {
      if (msg.type === 'getAccount') return { type: 'account', found: false };
      if (msg.type === 'writeSignRequest') { (window as any).__native.writes.push(msg); return { type: 'sign-req-ack', rid: msg.rid }; }
      if (msg.type === 'pollSignResult') { const r = (window as any).__native.signResults[msg.rid]; return { type: 'sign-result', rid: msg.rid, found: !!r, result: r || {} }; }
      return { type: 'error' };
    };
  });
  // The background memos the last good account for ACCT_TTL (4s); flush any in-flight
  // warm fetch, then wait out the memo so the not-found path actually yields cache=null.
  await page.click('#btn-chainid');
  await expect(page.locator('#result')).toContainText('"method": "eth_chainId"');
  await page.waitForTimeout(4200);

  await page.click('#btn-accounts');
  await expect(page.locator('#result')).toContainText('"method": "eth_accounts"');
  // Grant NOT dropped on a cold miss — the dApp stays connected.
  expect(await page.evaluate(() => (window as any).__velaTestResult.value)).toEqual([ADDR]);

  const perms = await page.evaluate(() =>
    (window as any).ethereum.request({ method: 'wallet_getPermissions' }),
  );
  const accountsCaveat = perms
    .find((p: any) => p.parentCapability === 'eth_accounts')
    ?.caveats?.find((c: any) => c.type === 'restrictReturnedAccounts');
  expect(accountsCaveat.value).toEqual([ADDR]);
});

test('since-removed granted account IS dropped: eth_accounts returns [] once the cache no longer owns it', async ({ page }) => {
  const origin = await page.evaluate(() => location.origin);
  const OTHER = '0x2222222222222222222222222222222222222222';
  await page.evaluate(
    ([key, addr]) => (window as any).browser.storage.local.set({ [key]: { origin: location.origin, address: addr, chainId: 1 } }),
    [PERM_PREFIX + origin, ADDR],
  );
  // The app now owns a DIFFERENT account — the granted ADDR is gone. (The background
  // memo holds the same live account object, so this mutation is reflected at once.)
  await page.evaluate((other) => { (window as any).__native.account.accounts = [{ name: 'Other', address: other }]; }, OTHER);

  await page.click('#btn-accounts');
  await expect(page.locator('#result')).toContainText('"method": "eth_accounts"');
  expect(await page.evaluate(() => (window as any).__velaTestResult.value)).toEqual([]);
});

test('sign WITHOUT a prior connect is refused 4100; no sign-req is written (content.js:531-534)', async ({ page }) => {
  await page.click('#btn-sign'); // never connected → no grant for this origin
  // Wait for the SETTLED result deterministically (avoids a parallel-load race where
  // #result shows the method label before the rejection is recorded).
  await page.waitForFunction(
    () => { const r = (window as any).__velaTestResult; return r && r.method === 'personal_sign' && r.ok === false; },
    null,
    { timeout: 5000 },
  );
  const r = await page.evaluate(() => (window as any).__velaTestResult);
  expect(r.ok).toBe(false);
  expect(r.code).toBe(4100); // UNAUTHORIZED — must call eth_requestAccounts first
  // Refused BEFORE any launch — nothing was handed to native.
  expect(await page.evaluate(() => (window as any).__native.writes.length)).toBe(0);
  // No hand-off sheet was shown either.
  expect(await page.evaluate(() => !!document.getElementById('vela-sheet-host'))).toBe(false);
});

test('wallet_switchEthereumChain: unknown chain → 4902; known chain → chainChanged + chainId updates (background.js:177-188)', async ({ page }) => {
  // Add a SECOND chain the app owns so we can switch to a genuinely different one.
  await page.evaluate(() => {
    (window as any).__native.account.chains['137'] = { name: 'Polygon', rpcUrl: 'http://rpc.test/137', bundlerUrl: 'http://bundler.test/137' };
  });
  // Connect first (connect's fresh cache read refreshes the background memo to include 137).
  await page.click('#btn-connect');
  await page.waitForFunction(() => (document.getElementById('vela-sheet-host') as any)?.shadowRoot?.getElementById('cta'));
  await page.evaluate(() => (document.getElementById('vela-sheet-host') as any).shadowRoot.getElementById('cta').click());
  await expect(page.locator('#result')).toContainText('eth_requestAccounts');
  expect(await page.evaluate(() => (window as any).ethereum.chainId)).toBe('0x1');

  // (a) Switch to a chain the app does NOT own → 4902 CHAIN_NOT_ADDED.
  const unknown = await page.evaluate(() =>
    (window as any).ethereum
      .request({ method: 'wallet_switchEthereumChain', params: [{ chainId: '0x38' }] }) // 56 — not owned
      .then(() => ({ ok: true }))
      .catch((e: any) => ({ ok: false, code: e.code })),
  );
  expect(unknown.ok).toBe(false);
  expect(unknown.code).toBe(4902);
  expect(await page.evaluate(() => (window as any).ethereum.chainId)).toBe('0x1'); // unchanged

  // (b) Switch to a chain the app DOES own (137) → chainChanged + sync prop updates.
  await page.evaluate(() =>
    (window as any).ethereum.request({ method: 'wallet_switchEthereumChain', params: [{ chainId: '0x89' }] }),
  );
  await expect(page.locator('#events')).toContainText('chainChanged');
  await page.waitForFunction(() => (window as any).ethereum.chainId === '0x89');
  await page.click('#btn-chainid');
  await expect(page.locator('#result')).toContainText('"method": "eth_chainId"');
  expect(await page.evaluate(() => (window as any).__velaTestResult.value)).toBe('0x89');
});

test('sign ceiling: an unresolved sign past the ~3min floor settles 4900 (NOT 4001) + TIMEOUT mirror (content.js:719-729)', async ({ page }) => {
  // connect first (sign requires a grant)
  await page.click('#btn-connect');
  await page.waitForFunction(() => (document.getElementById('vela-sheet-host') as any)?.shadowRoot?.getElementById('cta'));
  await page.evaluate(() => (document.getElementById('vela-sheet-host') as any).shadowRoot.getElementById('cta').click());
  await expect(page.locator('#result')).toContainText('eth_requestAccounts');

  // Launch the sign → durable mirror + signMap entry created; app never returns a result.
  await page.click('#btn-sign');
  await page.waitForFunction(() => (document.getElementById('vela-sheet-host') as any)?.shadowRoot?.getElementById('cta'));
  await page.evaluate(() => (document.getElementById('vela-sheet-host') as any).shadowRoot.getElementById('cta').click());
  await page.waitForFunction(() => (window as any).__native.writes.length > 0);
  const rid = await page.evaluate(() => { const w = (window as any).__native.writes; return w[w.length - 1].rid; });

  // Backdate the durable mirror's arm timestamps beyond the ~3min ceiling (no fake
  // clock): serviceRid reads launchedAt || pendingStoredAt as firstArm. __native
  // stays empty (no result), so after the bounded poll the ceiling branch fires.
  await page.evaluate(async (r) => {
    const key = 'vela.sign.' + r;
    const cur = (await (window as any).browser.storage.local.get(key))[key] || {};
    const past = Date.now() - 4 * 60 * 1000;
    await (window as any).browser.storage.local.set({ [key]: { ...cur, launchedAt: past, pendingStoredAt: past } });
  }, rid);

  // Return focus → the poll runs, finds nothing, and the ceiling settles the promise.
  await page.evaluate(() => document.dispatchEvent(new Event('visibilitychange')));
  await page.waitForFunction(
    () => (window as any).__velaTestResult && (window as any).__velaTestResult.method === 'personal_sign',
    undefined,
    { timeout: 15_000 },
  );
  const r = await page.evaluate(() => (window as any).__velaTestResult);
  expect(r.ok).toBe(false);
  expect(r.code).toBe(4900); // UNKNOWN_PENDING — a stuck-but-maybe-submitted tx is NEVER a clean 4001
  // The durable mirror records the terminal timeout (check Vela Activity).
  const mirror = await page.evaluate(async (rr) => {
    const key = 'vela.sign.' + rr;
    return (await (window as any).browser.storage.local.get(key))[key];
  }, rid);
  expect(mirror.state).toBe('TIMEOUT');
  expect(mirror.resolveCode).toBe(4900);
});
