// End-to-end round-trip test for the IN-APP dApp browser provider stack, in
// headless Chromium (same engine as the Android WebView; iOS WKWebView differs
// only in the last native hop). The STAR is the REAL generated bundle
// (inpage.js + protocol.js + the webview-inject shim) — loaded from disk and
// injected at document-start, exactly as the native WalletWebView injects it.
//
// The host side (native bridge + WebViewTransport + connect-gate router) is
// mirrored inline here — those TS modules are unit-tested separately
// (webview-transport.test.ts, wallet-browser-router.test.ts); Playwright can't
// transpile cross-package .ts, and the point of THIS test is to prove the real
// injected JS round-trips a request through a native-style bridge in a real engine.
import { test, expect } from '@playwright/test';
import { createServer, type Server } from 'node:http';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const GEN = path.resolve(HERE, '..', '..', '..', 'src', 'modules', 'webview', 'injected-provider.generated.ts');
const INJECTED_PROVIDER_JS: string = (() => {
  const src = readFileSync(GEN, 'utf8');
  const m = src.match(/INJECTED_PROVIDER_JS = ("(?:[^"\\]|\\.)*");/s);
  if (!m) throw new Error('cannot extract INJECTED_PROVIDER_JS from ' + GEN);
  return JSON.parse(m[1]);
})();

const ADDR = '0x1111111111111111111111111111111111111111';

// Inline mirror of wallet-browser-router.classifyBrowserRequest (unit-tested there).
const CONNECT = new Set(['eth_requestAccounts', 'wallet_requestPermissions']);
const PERM = [{ parentCapability: 'eth_accounts' }];
function classify(method: string, granted: string[]): { kind: 'respond'; result: unknown } | { kind: 'consent' } | { kind: 'forward' } {
  if (method === 'eth_accounts') return { kind: 'respond', result: granted };
  if (method === 'wallet_getPermissions') return { kind: 'respond', result: granted.length ? PERM : [] };
  if (CONNECT.has(method)) {
    if (granted.length) return { kind: 'respond', result: method === 'wallet_requestPermissions' ? PERM : granted };
    return { kind: 'consent' };
  }
  return { kind: 'forward' };
}

const HTML = `<!doctype html><html><body><script>
  window.__events = []; window.__6963 = [];
  addEventListener('eip6963:announceProvider', e => window.__6963.push(e.detail.info.rdns));
  (function hook(){ if(!window.ethereum) return setTimeout(hook,5);
    window.ethereum.on('accountsChanged', a => window.__events.push(['accountsChanged', a]));
    window.ethereum.on('chainChanged',    c => window.__events.push(['chainChanged', c]));
    window.ethereum.on('disconnect',      () => window.__events.push(['disconnect']));
  })();
</script></body></html>`;

let server: Server;
let base: string;
test.beforeAll(async () => {
  server = createServer((_q, res) => { res.setHeader('content-type', 'text/html; charset=utf-8'); res.end(HTML); });
  await new Promise<void>((r) => server.listen(0, '127.0.0.1', r));
  const a = server.address();
  base = `http://127.0.0.1:${typeof a === 'object' && a ? a.port : 0}/`;
});
test.afterAll(() => server && server.close());

type Wallet = (id: string, method: string, params: unknown[]) => void;
const H = new WeakMap<object, { setWallet: (w: Wallet) => void; settlePending: (code: number, message: string) => void }>();

test.beforeEach(async ({ page }) => {
  const grants = new Map<string, string>();
  const pending = new Set<string>();
  let wallet: Wallet = () => {};

  // native → page (the injected shim owns the vela-1193 envelope)
  const respond = (id: string, result: unknown, error: { code: number; message: string } | null) => {
    if (!pending.delete(id)) return; // idempotent, like WebViewTransport
    void page.evaluate((a) => (window as any).__velaRespond(a.id, a.result, a.error), { id, result: result ?? null, error: error ?? null });
  };
  const respondLocal = (id: string, result: unknown) => {
    void page.evaluate((a) => (window as any).__velaRespond(a.id, a.result, null), { id, result: result ?? null });
  };
  const emit = (event: string, data: unknown) => {
    void page.evaluate((a) => (window as any).__velaEmit(a.event, a.data), { event, data });
  };

  wallet = (id, method) => {
    if (method === 'eth_chainId') return respond(id, '0x1', null);
    if (method === 'net_version') return respond(id, '1', null);
    if (method === 'eth_getBalance') return respond(id, '0x1234', null);
    if (method === 'personal_sign' || method.includes('signTypedData')) return respond(id, '0xSIGNED', null);
    if (method === 'eth_sendTransaction') return respond(id, '0xTX', null);
    // else: stays pending
  };

  H.set(page, {
    setWallet: (w) => { wallet = w; },
    settlePending: (code, message) => { for (const id of pending) void page.evaluate((a) => (window as any).__velaRespond(a.id, null, a.err), { id, err: { code, message } }); pending.clear(); },
  });

  // page → native: the BrowserScreen request handling (connect-gate + forward).
  await page.exposeFunction('__velaNativeReceive', async (raw: string) => {
    let msg: any;
    try { msg = JSON.parse(raw); } catch { return; }
    if (msg.dir !== 'req') return; // ignore 'ready'
    const origin = new URL(base).origin; // TRUSTED origin (native-stamped), not from msg
    const granted = grants.get(origin) ? [grants.get(origin)!] : [];
    const action = classify(msg.method, granted);
    if (action.kind === 'respond') { respondLocal(msg.id, action.result); return; }
    if (action.kind === 'consent') {
      grants.set(origin, ADDR); // simulate the user tapping Connect
      respondLocal(msg.id, msg.method === 'wallet_requestPermissions' ? PERM : [ADDR]);
      emit('accountsChanged', [ADDR]);
      emit('chainChanged', '0x1');
      return;
    }
    pending.add(msg.id);
    wallet(msg.id, msg.method, msg.params);
  });

  await page.addInitScript(() => { (window as any).velaBridge = { postMessage: (s: string) => (window as any).__velaNativeReceive(s) }; });
  await page.addInitScript({ content: INJECTED_PROVIDER_JS });
  await page.goto(base);
  await page.waitForFunction(() => !!(window as any).ethereum?.isVela, null, { timeout: 5000 });
});

async function req(page: any, args: any) {
  return page.evaluate(async (a: any) => {
    try { return { ok: true, value: await (window as any).ethereum.request(a) }; }
    catch (e: any) { return { ok: false, code: e.code, message: e.message }; }
  }, args);
}

test('real bundle installs window.ethereum with EIP-6963 (rdns app.getvela)', async ({ page }) => {
  await page.evaluate(() => window.dispatchEvent(new Event('eip6963:requestProvider')));
  const info = await page.evaluate(() => ({ isVela: !!(window as any).ethereum?.isVela, rdns: (window as any).__6963 }));
  expect(info.isVela).toBe(true);
  expect(info.rdns).toContain('app.getvela');
});

test('eth_chainId round-trips: inpage → shim → (native) → back', async ({ page }) => {
  expect(await req(page, { method: 'eth_chainId' })).toEqual({ ok: true, value: '0x1' });
});

test('eth_accounts on an ungranted origin returns [] and never prompts', async ({ page }) => {
  expect(await req(page, { method: 'eth_accounts' })).toEqual({ ok: true, value: [] });
});

test('eth_requestAccounts: ungranted → consent → [ADDR] + accountsChanged reaches the page', async ({ page }) => {
  expect(await req(page, { method: 'eth_requestAccounts' })).toEqual({ ok: true, value: [ADDR] });
  await page.waitForFunction(() => ((window as any).__events || []).some((e: any) => e[0] === 'accountsChanged'));
  const ev = await page.evaluate(() => (window as any).__events.find((e: any) => e[0] === 'accountsChanged'));
  expect(ev[1]).toEqual([ADDR]);
});

test('after connect, eth_accounts returns the granted address', async ({ page }) => {
  await req(page, { method: 'eth_requestAccounts' });
  expect(await req(page, { method: 'eth_accounts' })).toEqual({ ok: true, value: [ADDR] });
});

test('personal_sign forwards and resolves with the signature', async ({ page }) => {
  await req(page, { method: 'eth_requestAccounts' });
  expect(await req(page, { method: 'personal_sign', params: ['0xdeadbeef', ADDR] })).toEqual({ ok: true, value: '0xSIGNED' });
});

test('a wallet reject reaches the dApp as 4001', async ({ page }) => {
  H.get(page)!.setWallet((id) => void page.evaluate((a) => (window as any).__velaRespond(a.id, null, a.err), { id, err: { code: 4001, message: 'User rejected the request' } }));
  await req(page, { method: 'eth_requestAccounts' });
  const v = await req(page, { method: 'personal_sign', params: ['0xdeadbeef', ADDR] });
  expect(v.ok).toBe(false);
  expect(v.code).toBe(4001);
});

test('settle-on-navigation rejects an in-flight request with 4900 (never 4001)', async ({ page }) => {
  H.get(page)!.setWallet(() => { /* never responds */ });
  await req(page, { method: 'eth_requestAccounts' });
  await page.evaluate(() => {
    (window as any).__p = (window as any).ethereum
      .request({ method: 'personal_sign', params: ['0xdeadbeef', '0x1'] })
      .then((v: any) => ({ ok: true, value: v }), (e: any) => ({ ok: false, code: e.code }));
  });
  await page.waitForTimeout(150);
  H.get(page)!.settlePending(4900, 'Navigated away');
  const v = await page.evaluate(() => (window as any).__p);
  expect(v.ok).toBe(false);
  expect(v.code).toBe(4900);
});
