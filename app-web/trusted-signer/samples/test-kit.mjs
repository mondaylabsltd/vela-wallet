// Shared plumbing for the Trusted Signer's Chrome tests (ceremony-test,
// hostile-test): Chrome + CDP, the page's own libraries in Node, and two
// stand-in wallets — one on the loopback WebSocket (the phones' channel), one
// in a tunnel room. Zero dependencies.
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { webcrypto } from 'node:crypto';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { accept } from './ws-lite.mjs';

export const root = join(dirname(fileURLToPath(import.meta.url)), '..');
export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
export const b64url = (bytes) => Buffer.from(bytes).toString('base64url');
export const unb64url = (text) => new Uint8Array(Buffer.from(text, 'base64url'));

/** The page's own libraries, loaded into this Node process as the page loads them. */
export function loadPageLibs(files) {
  globalThis.window = globalThis;
  Object.defineProperty(globalThis, 'crypto', { value: webcrypto, configurable: true });
  for (const file of files) (0, eval)(readFileSync(join(root, file), 'utf8'));
  return globalThis.VelaCS;
}

export function makeChecks() {
  const results = [];
  const check = (name, pass, detail) => {
    results.push(!!pass);
    console.log(`${pass ? 'PASS' : 'FAIL'}  ${name}${detail ? '  — ' + detail : ''}`);
    return !!pass;
  };
  check.results = results;
  check.summary = () => {
    const passed = results.filter(Boolean).length;
    console.log(`\n${passed}/${results.length} checks passed`);
    return passed === results.length;
  };
  return check;
}

// --- Chrome ---------------------------------------------------------------------

/**
 * The page on real hosts over TLS (a passkey's rpId check happens in the
 * browser process, so the page must really be on getvela.app), plus Chrome.
 * `hosts` are the names mapped to the TLS server besides getvela.app.
 */
/**
 * Every child this harness spawned, so an abnormal exit still takes them with
 * it.
 *
 * A run that throws used to leave its browser holding the debugging port, and
 * the NEXT run would then fail to open a tab — a failure with nothing to do
 * with the code under test, in a different place each time. Measured, twice.
 */
const spawned = new Set();
let reaperInstalled = false;

function reapOnExit() {
  if (reaperInstalled) return;
  reaperInstalled = true;
  const reap = () => {
    for (const child of spawned) {
      try { child.kill(); } catch { /* already gone */ }
    }
    spawned.clear();
  };
  process.on('exit', reap);
  for (const signal of ['SIGINT', 'SIGTERM']) {
    process.on(signal, () => { reap(); process.exit(1); });
  }
  process.on('uncaughtException', (error) => { reap(); throw error; });
}

export async function startBrowser({ cdp, tlsPort = 8443, hosts = [] }) {
  reapOnExit();
  const SB = process.env.SB;
  if (!SB || !process.env.CHROME_BIN) throw new Error('set CHROME_BIN and SB (see HANDOVER.md)');
  const tls = spawn('python3', [join(SB, 'tls-serve.py'), root, String(tlsPort), join(SB, 'cert.pem'), join(SB, 'key.pem')], { stdio: 'ignore' });
  const profile = mkdtempSync(join(tmpdir(), 'cs-'));
  const rules = ['getvela.app', ...hosts].map((h) => `MAP ${h}:443 127.0.0.1:${tlsPort}`).join(', ');
  const chrome = spawn(process.env.CHROME_BIN, [
    '--headless=new', `--remote-debugging-port=${cdp}`, `--user-data-dir=${profile}`,
    `--host-resolver-rules=${rules}`,
    '--ignore-certificate-errors', '--no-proxy-server',
    '--no-first-run', '--no-default-browser-check', 'about:blank',
  ], { stdio: 'ignore' });
  spawned.add(chrome);
  spawned.add(tls);
  for (let i = 0; i < 80; i++) {
    try { await (await fetch(`http://127.0.0.1:${cdp}/json/version`)).json(); break; } catch { await sleep(250); }
  }
  // …and wait for the PAGE SERVER too. Chrome being up says nothing about
  // python being up, and a navigation that arrives first gets
  // ERR_CONNECTION_REFUSED with no retry — a failure that looks like the page
  // is broken and is really a race in this harness. Measured: it fails perhaps
  // one run in three, on whichever sample happens to go first.
  let served = false;
  for (let i = 0; i < 80; i++) {
    try {
      await fetch(`https://127.0.0.1:${tlsPort}/src/sign.html`, { tls: { rejectUnauthorized: false } });
      served = true;
      break;
    } catch { await sleep(250); }
  }
  if (!served) throw new Error(`the page server never came up on ${tlsPort} — is another run holding it?`);
  await sleep(200);
  return {
    cdp,
    kill() {
      spawned.delete(chrome);
      spawned.delete(tls);
      chrome.kill();
      tls.kill();
      try { rmSync(profile, { recursive: true, force: true }); } catch { /* ignore */ }
    },
  };
}

export class Page {
  constructor(ws, cdp) {
    this.ws = ws;
    this.cdp = cdp;
    this.id = 0;
    this.pending = new Map();
    this.console = [];
    ws.addEventListener('message', (event) => {
      const message = JSON.parse(event.data);
      if (message.id && this.pending.has(message.id)) {
        const { resolve, reject } = this.pending.get(message.id);
        this.pending.delete(message.id);
        message.error ? reject(new Error(message.error.message)) : resolve(message.result);
      } else if (message.method === 'Runtime.exceptionThrown') {
        this.console.push('exception: ' + JSON.stringify(message.params.exceptionDetails).slice(0, 300));
      } else if (message.method === 'Log.entryAdded' && message.params.entry.level === 'error') {
        // With the URL: "Failed to load resource: 404" names nothing on its
        // own, and a suite that reports it cannot be acted on.
        const entry = message.params.entry;
        this.console.push(entry.text + (entry.url ? ' <' + entry.url + '>' : ''));
      }
    });
  }

  static async attach(webSocketDebuggerUrl, cdp) {
    const ws = new WebSocket(webSocketDebuggerUrl);
    await new Promise((resolve) => ws.addEventListener('open', resolve));
    const page = new Page(ws, cdp);
    await page.send('Runtime.enable');
    await page.send('Log.enable');
    return page;
  }

  static async open(cdp, url = 'about:blank') {
    // fetch() drops everything after '#' — encode the whole target URL.
    const tab = await (await fetch(`http://127.0.0.1:${cdp}/json/new?${encodeURIComponent(url)}`, { method: 'PUT' })).json();
    const page = await Page.attach(tab.webSocketDebuggerUrl, cdp);
    page.targetId = tab.id;
    return page;
  }

  static async find(cdp, test, timeoutMs = 8000) {
    const end = Date.now() + timeoutMs;
    while (Date.now() < end) {
      const targets = await (await fetch(`http://127.0.0.1:${cdp}/json/list`)).json();
      const found = targets.find((t) => t.type === 'page' && test(t.url));
      if (found) {
        const page = await Page.attach(found.webSocketDebuggerUrl, cdp);
        page.targetId = found.id;
        return page;
      }
      await sleep(150);
    }
    return null;
  }

  send(method, params = {}) {
    const id = ++this.id;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.ws.send(JSON.stringify({ id, method, params }));
    });
  }

  async ev(expression, userGesture = false) {
    const r = await this.send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true, userGesture });
    if (r.exceptionDetails) throw new Error(r.exceptionDetails.text + ' ' + (r.exceptionDetails.exception && r.exceptionDetails.exception.description));
    return r.result.value;
  }

  async navigate(url) {
    await this.send('Page.navigate', { url });
  }

  async waitFor(expression, timeoutMs = 10000) {
    const end = Date.now() + timeoutMs;
    while (Date.now() < end) {
      try { if (await this.ev(expression)) return true; } catch { /* navigating */ }
      await sleep(120);
    }
    return false;
  }

  text() {
    return this.ev("document.body.innerText");
  }

  status() {
    return this.ev("document.getElementById('status') && document.getElementById('status').textContent");
  }

  /** A virtual platform authenticator on this target, empty unless a key is injected. */
  async addAuthenticator() {
    await this.send('WebAuthn.enable');
    const { authenticatorId } = await this.send('WebAuthn.addVirtualAuthenticator', {
      options: {
        protocol: 'ctap2', transport: 'internal',
        hasResidentKey: true, hasUserVerification: true,
        isUserVerified: true, automaticPresenceSimulation: true,
      },
    });
    this.authenticatorId = authenticatorId;
    return authenticatorId;
  }

  async close() {
    try { await fetch(`http://127.0.0.1:${this.cdp}/json/close/${this.targetId}`); } catch { /* gone */ }
  }
}

// --- the loopback wallet (the phones' channel, 071 §2 / 075 §1.5) ----------

/**
 * A wallet listening on 127.0.0.1 for the page's WebSocket, as the Android and
 * iOS shells do: only the signer page's Origin is let in, the page must say
 * hello with the token, then the wallet sends intents one after another.
 */
export function loopbackWallet({ origin = 'https://getvela.app', token }) {
  const wallet = {
    token,
    log: [],          // every message the page sent
    conn: null,
    closed: null,
    helloOk: false,
    port: 0,
    waiters: [],
  };
  let resolveHello;
  wallet.hello = new Promise((r) => { resolveHello = r; });
  const server = createServer((req, res) => res.writeHead(404).end());
  server.on('upgrade', (req, socket, head) => {
    if (req.headers.origin !== origin) {
      socket.end('HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n');
      return;
    }
    const conn = accept(req, socket, head);
    if (!conn) return;
    conn.on('message', (data) => {
      let message = null;
      try { message = JSON.parse(String(data)); } catch { return; }
      wallet.log.push(message);
      if (message.t === 'hello') {
        if (message.token !== token) { conn.close(1008, 'token'); return; }
        wallet.conn = conn;
        wallet.helloOk = true;
        resolveHello(true);
        return;
      }
      const waiter = wallet.waiters.find((w) => w.match(message));
      if (waiter) {
        wallet.waiters.splice(wallet.waiters.indexOf(waiter), 1);
        waiter.resolve(message);
      }
    });
    conn.on('close', (code) => { if (conn === wallet.conn) wallet.closed = code; });
  });
  wallet.listening = new Promise((r) => server.listen(0, '127.0.0.1', () => {
    wallet.port = server.address().port;
    r(wallet.port);
  }));
  wallet.expect = (match, timeoutMs = 15000) => new Promise((resolve, reject) => {
    const already = wallet.log.find((m) => m.t !== 'hello' && match(m) && !m.__taken);
    if (already) { already.__taken = true; resolve(already); return; }
    const timer = setTimeout(() => reject(new Error('the page did not answer in time')), timeoutMs);
    wallet.waiters.push({ match, resolve: (m) => { clearTimeout(timer); m.__taken = true; resolve(m); } });
  });
  /** Send one intent and wait for the page's answer to it. */
  wallet.send = (object) => wallet.conn.send(JSON.stringify(object));
  wallet.request = (id, intent, context = {}) => {
    wallet.send({ v: 1, t: 'intent', id, intent, context });
    return wallet.expect((m) => m.id === id && (m.t === 'result' || m.t === 'error'), 30000);
  };
  wallet.bye = () => wallet.send({ v: 1, t: 'bye', reason: 'done' });
  wallet.drop = () => wallet.conn && wallet.conn.close(1000, '');
  wallet.stop = () => new Promise((done) => { server.close(() => done()); server.closeAllConnections?.(); });
  return wallet;
}

// --- the tunnel wallet (075 tunnel.md) ----------------------------------------------


// --- checking what came back ------------------------------------------------------

export const fromHex = (text) => new Uint8Array(Buffer.from(String(text).replace(/^0x/, ''), 'hex'));

// WebAuthn signs DER; WebCrypto verifies r‖s.
export function derToRaw(der) {
  let offset = 2;
  if (der[1] & 0x80) offset = 2 + (der[1] & 0x7f);
  const out = new Uint8Array(64);
  for (let half = 0; half < 2; half++) {
    const length = der[offset + 1];
    let value = der.subarray(offset + 2, offset + 2 + length);
    while (value.length > 32 && value[0] === 0) value = value.subarray(1);
    out.set(value, half * 32 + (32 - value.length));
    offset += 2 + length;
  }
  return out;
}

/** Does this answer's signature verify under the key `{x, y}` (0x-hex)? */
export async function verifies(assertion, key) {
  const raw = new Uint8Array([4, ...fromHex(key.x), ...fromHex(key.y)]);
  const publicKey = await webcrypto.subtle.importKey('raw', raw, { name: 'ECDSA', namedCurve: 'P-256' }, false, ['verify']);
  const clientData = fromHex(assertion.clientDataJSON);
  const hash = new Uint8Array(await webcrypto.subtle.digest('SHA-256', clientData));
  const signed = new Uint8Array([...fromHex(assertion.authenticatorData), ...hash]);
  return webcrypto.subtle.verify({ name: 'ECDSA', hash: 'SHA-256' }, publicKey, derToRaw(fromHex(assertion.signatureDer)), signed);
}

export function clientData(answer) {
  return JSON.parse(Buffer.from(fromHex(answer.clientDataJSON)).toString('utf8'));
}

/** Lowercase hex without 0x — the encoding pinned for every byte field (075 §1.4). */
export const isBareHex = (value) => typeof value === 'string' && /^([0-9a-f]{2})+$/.test(value);
