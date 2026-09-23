// lib/transport/secure.js against vela-core's own session, byte for byte.
//
// The vectors are written by the Rust side (`clear_signer::secure`,
// rust/crates/vela-core/tests/clear-signer/secure-session.json): fixed secrets and
// nonces for both ends, the code and `rk` they must agree on, and four sealed
// messages per case. With the same secrets and nonces injected, the page's
// session must reproduce every one of them — and refuse what Rust refuses.
//
// Runs the shipped file in Node, then (when CHROME_BIN is set) the same checks
// again inside Chrome, which is where the page actually runs.
//
//   node samples/secure-vectors.mjs
import { spawn } from 'node:child_process';
import { webcrypto } from 'node:crypto';
import { createServer } from 'node:http';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..');
const repo = join(root, '..', '..');
// Where vela-core keeps them; the first spelling is where they lived before the
// conformance corpus (tests/vectors/) was reserved for the shared suites.
const VECTORS = [
  'rust/crates/vela-core/tests/clear-signer/secure-session.json',
  'rust/crates/vela-core/tests/vectors/secure-session.json',
].map((path) => join(repo, path)).find((path) => existsSync(path));
if (!VECTORS) throw new Error('no secure-session.json under rust/crates/vela-core/tests/');
const CDP = 9391;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const vectors = JSON.parse(readFileSync(VECTORS, 'utf8'));
const secureSource = readFileSync(join(root, 'lib/transport/secure.js'), 'utf8');

// The checks, as a function that runs unchanged in Node and in the browser:
// it only needs `VelaCS.transport.secure` and the vectors.
async function runChecks(secure, vectors) {
  const out = [];
  const check = (name, pass, detail) => out.push({ name, pass: !!pass, detail: detail || '' });
  const hex = (h) => new Uint8Array(h.match(/../g).map((x) => parseInt(x, 16)));
  const toHex = (b) => Array.from(b).map((x) => x.toString(16).padStart(2, '0')).join('');
  const enc = (s) => new TextEncoder().encode(s);
  const dec = (b) => new TextDecoder().decode(b);
  const sameObject = (a, b) => {
    const keys = Object.keys(a).sort();
    return JSON.stringify(keys) === JSON.stringify(Object.keys(b).sort()) &&
      keys.every((k) => a[k] === b[k]);
  };
  const codeOf = async (promise) => {
    try { await promise; return 'accepted'; } catch (e) { return e.code || e.message; }
  };

  for (const c of vectors.cases) {
    const label = c.label === 'vela-ble/1' ? 'ble' : 'tunnel';
    const tail = (m) => (label === 'ble' ? m.msgId : undefined);

    // Both ends, from the vector's randomness. The signer imports its bare
    // secret (the public key is derived); the requester imports secret + key.
    const signer = await secure.handshake({
      role: 'signer', secret: hex(c.signer.secretHex), nonce: hex(c.signer.nonceHex),
    });
    const requester = await secure.handshake({
      role: 'requester', secret: hex(c.requester.secretHex),
      publicKey: hex(c.requester.publicKeyHex), nonce: hex(c.requester.nonceHex),
    });
    check(`${c.name}: the public key derived from the secret is Rust's`,
      toHex(signer.publicKey) === c.signer.publicKeyHex);
    check(`${c.name}: signer hello = Rust's`, sameObject(signer.hello(), JSON.parse(c.signer.hello)));
    check(`${c.name}: requester hello = Rust's`,
      sameObject(requester.hello(c.requester.app), JSON.parse(c.requester.hello)));
    check(`${c.name}: rk = Rust's`, (await secure.fingerprint(requester.publicKey)) === c.rk, c.rk);

    // The page checks rk; the wallet does not.
    const page = await signer.complete(c.requester.hello, label, c.rk);
    const wallet = await requester.complete(c.signer.hello, label);
    check(`${c.name}: the page's code = Rust's`, page.code === c.code, `${page.code} / ${c.code}`);
    check(`${c.name}: the wallet's code = Rust's`, wallet.code === c.code);

    // Every message: seal on its sender to the same bytes, open on the other.
    let sealedAll = true;
    let openedAll = true;
    for (const m of c.messages) {
      const [from, to] = m.from === 'signer' ? [page, wallet] : [wallet, page];
      const sealed = await from.seal(enc(m.plaintext), tail(m));
      if (toHex(sealed) !== m.sealedHex) sealedAll = false;
      const opened = await to.open(hex(m.sealedHex), tail(m)).then(dec, () => null);
      if (opened !== m.plaintext) openedAll = false;
    }
    check(`${c.name}: every message seals to Rust's bytes`, sealedAll, `${c.messages.length} messages`);
    check(`${c.name}: every Rust message opens`, openedAll);

    // What Rust refuses, the page refuses.
    const firstFromWallet = c.messages.find((m) => m.from === 'requester');
    check(`${c.name}: a replayed message is refused`,
      (await codeOf(page.open(hex(firstFromWallet.sealedHex), tail(firstFromWallet)))) === 'replayed');

    const fresh = await (await secure.handshake({
      role: 'signer', secret: hex(c.signer.secretHex), nonce: hex(c.signer.nonceHex),
    })).complete(c.requester.hello, label, c.rk);
    const tampered = hex(firstFromWallet.sealedHex);
    tampered[tampered.length - 1] ^= 1;
    check(`${c.name}: a tampered message is refused`,
      (await codeOf(fresh.open(tampered, tail(firstFromWallet)))) === 'unreadable');
    const ownDirection = c.messages.find((m) => m.from === 'signer');
    check(`${c.name}: a message in our own direction is refused`,
      (await codeOf(fresh.open(hex(ownDirection.sealedHex), tail(ownDirection)))) === 'replayed');
    if (label === 'tunnel') {
      // The tunnel's AAD ends with the counter: the same bytes under another
      // label (a BLE session) must not open.
      const other = await (await secure.handshake({
        role: 'signer', secret: hex(c.signer.secretHex), nonce: hex(c.signer.nonceHex),
      })).complete(c.requester.hello, 'ble', c.rk);
      check(`${c.name}: a tunnel message does not open in a BLE session`,
        (await codeOf(other.open(hex(firstFromWallet.sealedHex), 1))) === 'unreadable');
    }

    // The pairing check: a hello whose key does not hash to rk.
    const stranger = await (await secure.handshake({
      role: 'signer', secret: hex(c.signer.secretHex), nonce: hex(c.signer.nonceHex),
    })).complete(c.requester.hello, label, c.rk === 'AAAAAAAAAAAAAAAAAAAAAA' ? 'BBBBBBBBBBBBBBBBBBBBBB' : 'AAAAAAAAAAAAAAAAAAAAAA')
      .then(() => 'accepted', (e) => e.code);
    check(`${c.name}: a requester whose key does not hash to rk is refused`, stranger === 'foreign_peer');
    const sameRole = await signer.complete(c.signer.hello, label).then(() => 'accepted', (e) => e.code);
    check(`${c.name}: a peer in our own role is refused`, sameRole === 'wrong_role');
  }
  return out;
}

const results = [];
function report(where, list) {
  for (const r of list) {
    results.push(r.pass);
    console.log(`${r.pass ? 'PASS' : 'FAIL'}  [${where}] ${r.name}${r.detail ? '  — ' + r.detail : ''}`);
  }
}

// --- Node ----------------------------------------------------------------------

globalThis.window = globalThis;
Object.defineProperty(globalThis, 'crypto', { value: webcrypto, configurable: true });
globalThis.btoa = globalThis.btoa || ((s) => Buffer.from(s, 'binary').toString('base64'));
globalThis.atob = globalThis.atob || ((s) => Buffer.from(s, 'base64').toString('binary'));
(0, eval)(secureSource);
report('node', await runChecks(globalThis.VelaCS.transport.secure, vectors));

// --- Chrome ----------------------------------------------------------------------

if (process.env.CHROME_BIN) {
  // http://127.0.0.1 is a secure context, so WebCrypto is all there.
  const server = createServer((req, res) => {
    if (req.url === '/lib/transport/secure.js') {
      res.writeHead(200, { 'content-type': 'text/javascript' }).end(secureSource);
    } else {
      res.writeHead(200, { 'content-type': 'text/html' })
        .end('<!doctype html><script src="/lib/transport/secure.js"></script>');
    }
  });
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  const profile = mkdtempSync(join(tmpdir(), 'vectors-'));
  const chrome = spawn(process.env.CHROME_BIN, [
    '--headless=new', `--remote-debugging-port=${CDP}`, `--user-data-dir=${profile}`,
    '--no-proxy-server', '--no-first-run', '--no-default-browser-check', 'about:blank',
  ], { stdio: 'ignore' });
  try {
    for (let i = 0; i < 80; i++) {
      try { await (await fetch(`http://127.0.0.1:${CDP}/json/version`)).json(); break; } catch { await sleep(250); }
    }
    const url = `http://127.0.0.1:${server.address().port}/`;
    const tab = await (await fetch(`http://127.0.0.1:${CDP}/json/new?${encodeURIComponent(url)}`, { method: 'PUT' })).json();
    const ws = new WebSocket(tab.webSocketDebuggerUrl);
    await new Promise((r) => ws.addEventListener('open', r));
    let id = 0;
    const pending = new Map();
    ws.addEventListener('message', (event) => {
      const m = JSON.parse(event.data);
      if (pending.has(m.id)) { pending.get(m.id)(m.result); pending.delete(m.id); }
    });
    const send = (method, params = {}) => new Promise((resolve) => {
      const i = ++id; pending.set(i, resolve); ws.send(JSON.stringify({ id: i, method, params }));
    });
    await sleep(800);
    const evaluated = await send('Runtime.evaluate', {
      expression: `(${runChecks.toString()})(window.VelaCS.transport.secure, ${JSON.stringify(vectors)})`,
      awaitPromise: true, returnByValue: true,
    });
    if (evaluated.exceptionDetails) throw new Error(JSON.stringify(evaluated.exceptionDetails).slice(0, 400));
    report('chrome', evaluated.result.value);
  } catch (error) {
    console.log('FAILED (chrome): ' + error.message);
    results.push(false);
  } finally {
    chrome.kill();
    server.close();
    try { rmSync(profile, { recursive: true, force: true }); } catch { /* ignore */ }
  }
} else {
  console.log('(CHROME_BIN not set: the in-browser pass is skipped)');
}

const passed = results.filter(Boolean).length;
console.log(`\n${passed}/${results.length} checks passed`);
process.exitCode = passed === results.length ? 0 : 1;
