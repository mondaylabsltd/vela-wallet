// The answer goes to the Vela wallet on this device and nowhere else (spec 102
// R7) — checked on the PUBLISHED bytes, served the way the host serves them.
//
//   CHROME_BIN=… SB=… bun samples/answer-test.mjs                  the page these sources build
//   CHROME_BIN=… SB=… bun samples/answer-test.mjs --page=<sha256>   any version in dist/b/
//
// The attack, played for real. A phishing site opens the official page —
// `https://sign.getvela.app/b/<hash>/sign` — with an operation of its own
// choosing (it drains the account) and an answer address of its own
// (`cb=https://evil.example/steal`). The card describes that operation
// truthfully, on Vela's real domain; a person who slides hands a usable
// signature to evil.example. The same through postMessage (the site opens the
// page and asks), and through a frame (the site draws over the page).
//
// First the rule itself, in Node (R): the exact address, and every other
// channel and spelling refused. Then, for the page under test:
//   A. a signature answered to evil.example: no slide, no passkey prompt, the
//      card names evil.example, and evil.example receives NOTHING — not the
//      answer, not a refusal, not a beacon when the tab closes;
//   B. a signature asked over postMessage by evil.example: refused, and the
//      opener hears "refused" and never a signature;
//   C. a sign-in answered to evil.example: refused the same way;
//   D. a loopback answer address: refused — the desktop demo's exception is
//      not in the published bytes (and the harness page that carries it names
//      src/sign.html's scripts plus that one file, nothing else);
//   E. a frame: the host's headers stop it, and with no headers the page
//      itself refuses to run in one;
//   F. the real thing still works: answered to `velawallet://sign-result`,
//      the page signs, and the signature verifies under the account's key;
//   G. postMessage from Vela's web wallet (2026-10-29, widened 2026-10-30): a
//      multisig member's approval and a message are answered to
//      `wallet.getvela.app`; evil.example and a getvela.app subdomain are
//      refused;
//   H. postMessage from the Chrome extension (2026-10-30): a real unpacked
//      extension carrying the published manifest key (so the published id)
//      opens the page and is answered — a message, typed data, a transaction
//      — while one with another key (another id) is refused before any
//      passkey.
//
// Whenever a slide IS offered, this suite slides — what a person who trusted
// the card would do — so an old page shows what it would have given away.
//
// The host is played by this file: `sign.getvela.app` serves `dist/` with the
// rules in `dist/_headers` applied (Cloudflare Pages: every matching rule,
// same-name values joined), `getvela.app` serves this folder as files with no
// headers, and `evil.example` is the attacker.
import { spawn } from 'node:child_process';
import { createServer as createHttpServer } from 'node:http';
import { createServer as createHttpsServer } from 'node:https';
import { createHash, generateKeyPairSync, webcrypto } from 'node:crypto';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { extname, join, normalize } from 'node:path';
import { Page, b64url, loadPageLibs, makeChecks, root, sleep } from './test-kit.mjs';
import { build } from './build-single.mjs';

// Overridable, so two runs on one machine (two agents, two worktrees) do not
// fight over a port: ANSWER_PORT_BASE=9900 → CDP 9900, TLS 9901, loopback 9902.
const PORT_BASE = Number(process.env.ANSWER_PORT_BASE) || 0;
const CDP = PORT_BASE ? PORT_BASE : 9406;
const TLS_PORT = PORT_BASE ? PORT_BASE + 1 : 8453;
const LOOPBACK_PORT = PORT_BASE ? PORT_BASE + 2 : 8478;
const WALLET_CALLBACK = 'velawallet://sign-result';
const check = makeChecks();

const SB = process.env.SB;
if (!SB || !process.env.CHROME_BIN) throw new Error('set CHROME_BIN and SB (see HANDOVER.md)');

const DIST = join(root, 'dist');
const pageArg = process.argv.find((a) => a.startsWith('--page='));
const HASH = pageArg ? pageArg.slice('--page='.length) : build().hash;
const PUBLISHED = join(DIST, 'b', HASH, 'sign.html');
if (!/^[0-9a-f]{64}$/.test(HASH) || !existsSync(PUBLISHED)) {
  console.log(`FAILED: dist/b/${HASH}/sign.html is not there — run \`bun samples/build-single.mjs\``);
  process.exit(1);
}
const PAGE = `https://sign.getvela.app/b/${HASH}/sign`;
console.log(`page under test: ${PAGE}${pageArg ? '' : ' (what these sources build)'}\n`);

// --- the operation a phishing site would bring --------------------------------

const lib = loadPageLibs([
  'src/lib/i18n.js', 'src/lib/locales/en.js', 'src/lib/keccak.js', 'src/lib/identicon-features.js',
  'src/lib/identicon.js', 'src/lib/abi.js', 'src/lib/encode.js', 'src/lib/fee.js', 'src/lib/logos.js',
  'src/lib/catalog.js', 'src/lib/registry.js', 'src/lib/resolve.js', 'src/lib/signer.js', 'src/lib/ceremony.js',
  'src/lib/safeop.js', 'src/lib/multisig.js',
]);
const SAFE = '0x88cCA0f8B4E1F0dC0e7C4f9a2B3d5E6f7A8b6894';
const USDC = '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48';
const ATTACKER = '0x9A8b7C6d5E4F3a2B1c0D9e8F7a6B5c4D3e2F1a09';
const drain = lib.encode.call('transfer(address,uint256)', [ATTACKER, 7654321000n]);
const userOp = {
  sender: SAFE, nonce: '0x7', initCode: '0x',
  callData: lib.encode.call('executeUserOp(address,uint256,bytes,uint8)', [USDC, 0n, drain, 0]),
  verificationGasLimit: '300000', callGasLimit: '200000', preVerificationGas: '110000',
  maxFeePerGas: '0', maxPriorityFeePerGas: '0', paymasterAndData: '0x',
};
const intent = {
  method: 'eth_sendTransaction', origin: 'https://app.uniswap.org',
  params: [{ to: USDC, value: '0x0', data: drain }],
};

// The account's key, already in the browser's vault (`rpId` getvela.app — the
// page folds sign.getvela.app to it, as the apps' passkeys are).
const pair = await webcrypto.subtle.generateKey({ name: 'ECDSA', namedCurve: 'P-256' }, true, ['sign', 'verify']);
const pkcs8 = Buffer.from(await webcrypto.subtle.exportKey('pkcs8', pair.privateKey)).toString('base64');
const rawId = Buffer.from(webcrypto.getRandomValues(new Uint8Array(16)));
const credentialId = rawId.toString('base64url');
const context = { chainId: 1, account: SAFE, allowCredentials: [credentialId], operation: { userOp } };
// Fields a requester might put in its context to pass for the wallet. The page
// overrides them with what the channel says (sign.js `show`).
const spoof = { channel: 'url', callback: WALLET_CALLBACK, originVerified: true, requester: 'https://getvela.app' };
const fragment = (request, callback, token) =>
  `#i=${b64url(JSON.stringify(request))}` + (callback ? `&cb=${b64url(callback)}` : '') + `&t=${token}`;

/** Does this callback's `result=` carry a valid signature by the account's key? */
async function usable(resultParam) {
  try {
    const result = JSON.parse(Buffer.from(resultParam, 'base64url').toString('utf8'));
    const hex = (h) => Buffer.from(String(h).replace(/^0x/, ''), 'hex');
    const clientData = hex(result.clientDataJSON);
    const hash = Buffer.from(await webcrypto.subtle.digest('SHA-256', clientData));
    const valid = await webcrypto.subtle.verify({ name: 'ECDSA', hash: 'SHA-256' }, pair.publicKey,
      hex(result.signature), Buffer.concat([hex(result.authenticatorData), hash]));
    const challenge = JSON.parse(clientData.toString('utf8')).challenge;
    return { valid, digest: result.digest, kind: result.digestKind, challenge, overDigest: !!result.digest && challenge === hex(result.digest).toString('base64url') };
  } catch (error) {
    return { valid: false, error: error.message };
  }
}

// --- the host, the attacker, and a loopback listener -------------------------

/** `dist/_headers`, as Cloudflare Pages applies it. */
function headerRules() {
  const rules = [];
  let current = null;
  for (const line of readFileSync(join(DIST, '_headers'), 'utf8').split('\n')) {
    if (!line.trim() || line.trim().startsWith('#')) continue;
    if (!/^\s/.test(line)) {
      current = { pattern: new RegExp('^' + line.trim().replace(/[.+?^${}()|[\]\\]/g, '\\$&').replace(/\*/g, '.*') + '$'), headers: [] };
      rules.push(current);
    } else if (current) {
      const at = line.indexOf(':');
      current.headers.push([line.slice(0, at).trim(), line.slice(at + 1).trim()]);
    }
  }
  return (path) => {
    const out = {};
    for (const rule of rules) {
      if (!rule.pattern.test(path)) continue;
      for (const [name, value] of rule.headers) out[name] = out[name] ? `${out[name]}, ${value}` : value;
    }
    return out;
  };
}
const headersFor = headerRules();

const TYPES = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8', '.css': 'text/css; charset=utf-8', '.json': 'application/json' };
function serveFile(res, base, path, extra = {}) {
  const file = normalize(join(base, decodeURIComponent(path)));
  if (!file.startsWith(base) || !existsSync(file) || statSync(file).isDirectory()) {
    res.writeHead(404).end();
    return;
  }
  res.writeHead(200, { 'content-type': TYPES[extname(file)] || 'application/octet-stream', ...extra });
  res.end(readFileSync(file));
}

/** Everything the attacker's host was sent, other than its own pages. */
const stolen = [];
const loopbackHits = [];

const attackerOpener = (signer) => `<!doctype html><meta charset="utf-8"><p>evil.example</p><script>
  window.__answers = [];
  window.__go = function () {
    var child = window.open(${JSON.stringify(signer)}, 'vela-signer', 'width=460,height=760');
    window.addEventListener('message', function (event) {
      var data = event.data || {};
      if (data.vela === 'ready') {
        child.postMessage({ vela: 'intent', id: 'drain-1', intent: ${JSON.stringify(intent)}, context: ${JSON.stringify(context)} }, event.origin);
      } else if (data.vela === 'result' || data.vela === 'error') {
        window.__answers.push(data);
      }
    });
    return true;
  };
</script>`;

/**
 * A window that opens the page and hands it `request` over postMessage — the
 * web wallet's way (2026-10-29), or an attacker's copy of it. Whatever comes
 * back is kept in `window.__answers`.
 */
const genericOpener = (signer, request, who) => `<!doctype html><meta charset="utf-8"><p>${who}</p><script>
  window.__answers = [];
  window.__go = function () {
    var child = window.open(${JSON.stringify(signer)}, 'vela-signer', 'width=460,height=760');
    window.addEventListener('message', function (event) {
      var data = event.data || {};
      if (event.source !== child) return;
      if (data.vela === 'ready') {
        child.postMessage(Object.assign({ vela: 'intent', id: 'ms-1' }, ${JSON.stringify(request)}), event.origin);
      } else if (data.vela === 'result' || data.vela === 'error') {
        window.__answers.push(data);
      }
    });
    // The wallet's goodbye (round 7): 'done' once it took the answer,
    // 'withdrawn' when it gave up on the request.
    window.__bye = function (reason) {
      child.postMessage({ vela: 'bye', v: 1, reason: reason }, new URL(${JSON.stringify(signer)}).origin);
      return true;
    };
    window.__childClosed = function () { return child.closed; };
    return true;
  };
</script>`;

const framing = (src) => `<!doctype html><meta charset="utf-8"><p>a page with a frame</p>
<iframe id="f" src="${src.replace(/"/g, '&quot;')}" style="width:420px;height:720px"></iframe>`;

const tls = { key: readFileSync(join(SB, 'key.pem')), cert: readFileSync(join(SB, 'cert.pem')) };
const host = createHttpsServer(tls, (req, res) => {
  const name = String(req.headers.host || '').split(':')[0];
  const url = new URL(req.url, `https://${name}`);
  if (url.pathname === '/__frame') {
    res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' }).end(framing(url.searchParams.get('src') || ''));
    return;
  }
  if ((name === 'evil.example' || name === 'wallet.getvela.app' || name === 'staging.getvela.app') &&
    url.pathname === '/open-request') {
    const request = JSON.parse(Buffer.from(url.searchParams.get('req') || '', 'base64url').toString('utf8'));
    res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' })
      .end(genericOpener(url.searchParams.get('signer'), request, name));
    return;
  }
  if (name === 'evil.example') {
    if (url.pathname === '/open') {
      res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' }).end(attackerOpener(url.searchParams.get('signer')));
      return;
    }
    if (url.pathname !== '/favicon.ico') stolen.push({ method: req.method, url: req.url });
    res.writeHead(200, { 'content-type': 'text/plain' }).end('thanks');
    return;
  }
  if (name === 'sign.getvela.app') {
    // Pretty URLs, as Pages serves them: `/b/<hash>/sign` is `sign.html`.
    const path = url.pathname.endsWith('/sign') ? `${url.pathname}.html` : url.pathname;
    serveFile(res, DIST, path, headersFor(url.pathname));
    return;
  }
  serveFile(res, root, url.pathname);
});
await new Promise((r) => host.listen(TLS_PORT, '127.0.0.1', r));

const loopback = createHttpServer((req, res) => {
  if (!req.url.startsWith('/favicon')) loopbackHits.push({ method: req.method, url: req.url });
  res.writeHead(200).end('ok');
});
await new Promise((r) => loopback.listen(LOOPBACK_PORT, '127.0.0.1', r));

// --- the browser ----------------------------------------------------------------

const profile = mkdtempSync(join(tmpdir(), 'answer-'));
const rules = ['sign.getvela.app', 'getvela.app', 'evil.example', 'wallet.getvela.app', 'staging.getvela.app']
  .map((h) => `MAP ${h}:443 127.0.0.1:${TLS_PORT}`).join(', ');

// H: two unpacked extensions with one page that opens the signer and hands
// it a request — one carrying the wallet extension's published manifest key
// (so Chrome gives it the published id), one with a key of its own.
const idOf = (keyB64) => [...createHash('sha256').update(Buffer.from(keyB64, 'base64')).digest('hex').slice(0, 32)]
  .map((c) => String.fromCharCode(97 + parseInt(c, 16))).join('');
const PUBLISHED_KEY = JSON.parse(readFileSync(join(root, '..', 'vela-wallet', 'extension', 'manifest.json'), 'utf8')).key;
const OTHER_KEY = generateKeyPairSync('rsa', { modulusLength: 2048 }).publicKey
  .export({ type: 'spki', format: 'der' }).toString('base64');
const EXTENSIONS = { published: { key: PUBLISHED_KEY }, other: { key: OTHER_KEY } };
const OPENER_JS = `window.__answers = [];
window.__go = function (signer, request) {
  var child = window.open(signer, 'vela-signer', 'width=460,height=760');
  window.addEventListener('message', function (event) {
    var data = event.data || {};
    if (event.source !== child) return;
    if (data.vela === 'ready') child.postMessage(Object.assign({ vela: 'intent', id: 'ext-1' }, request), event.origin);
    else if (data.vela === 'result' || data.vela === 'error') window.__answers.push(data);
  });
  return true;
};`;
for (const [name, ext] of Object.entries(EXTENSIONS)) {
  ext.dir = join(profile, `ext-${name}`);
  mkdirSync(ext.dir, { recursive: true });
  writeFileSync(join(ext.dir, 'manifest.json'), JSON.stringify({ manifest_version: 3, name: `opener-${name}`, version: '1', key: ext.key }));
  writeFileSync(join(ext.dir, 'opener.html'), '<!doctype html><meta charset="utf-8"><p>extension</p><script src="opener.js"></script>');
  writeFileSync(join(ext.dir, 'opener.js'), OPENER_JS);
  ext.id = idOf(ext.key);
}
const chrome = spawn(process.env.CHROME_BIN, [
  '--headless=new', `--remote-debugging-port=${CDP}`, `--user-data-dir=${profile}`,
  `--host-resolver-rules=${rules}`, '--ignore-certificate-errors', '--no-proxy-server',
  `--load-extension=${EXTENSIONS.published.dir},${EXTENSIONS.other.dir}`,
  '--no-first-run', '--no-default-browser-check', 'about:blank',
], { stdio: 'ignore' });
for (let i = 0; i < 80; i++) {
  try { await (await fetch(`http://127.0.0.1:${CDP}/json/version`)).json(); break; } catch { await sleep(250); }
}

// Counts every passkey prompt the page raises: a refusal must come before one.
const COUNT_WEBAUTHN = `(() => {
  window.__webauthnCalls = 0;
  const c = navigator.credentials;
  const get = c.get.bind(c), create = c.create.bind(c);
  c.get = (o) => { window.__webauthnCalls++; return get(o); };
  c.create = (o) => { window.__webauthnCalls++; return create(o); };
})();`;

/** The account's key in this tab's vault; returns how to read its use count. */
async function addKey(page) {
  const authenticatorId = await page.addAuthenticator();
  await page.send('WebAuthn.addCredential', {
    authenticatorId,
    credential: {
      credentialId: rawId.toString('base64'), isResidentCredential: true, rpId: 'getvela.app',
      privateKey: pkcs8, userHandle: Buffer.from('vela-answer-test').toString('base64'), signCount: 0,
    },
  });
  return async () => {
    const { credentials } = await page.send('WebAuthn.getCredentials', { authenticatorId });
    return credentials.reduce((n, c) => n + c.signCount, 0);
  };
}

/** A tab that counts passkey prompts and records every navigation it asks for. */
async function tab(url) {
  const page = await Page.open(CDP, 'about:blank');
  page.navigations = [];
  page.ws.addEventListener('message', (event) => {
    const message = JSON.parse(event.data);
    if (message.method === 'Page.frameRequestedNavigation') page.navigations.push(message.params.url);
  });
  await page.send('Page.enable');
  await page.send('Page.addScriptToEvaluateOnNewDocument', { source: COUNT_WEBAUTHN });
  page.uses = await addKey(page);
  await page.navigate(url);
  await page.waitFor("window.__velaState && ['card', 'refused', 'answered', 'framed'].includes(window.__velaState.phase)");
  return page;
}

/** What a person who trusted the card would do: slide, if a slide is offered. */
async function slideIfOffered(page) {
  const offered = await page.ev('!!window.__slider');
  if (offered) await page.ev('window.__slider.__confirm()', true);
  return offered;
}

async function describeStolen(entries) {
  const lines = [];
  for (const entry of entries) {
    const result = new URL(entry.url, 'https://evil.example').searchParams.get('result');
    if (!result) { lines.push(`${entry.method} ${entry.url.slice(0, 60)}`); continue; }
    const sig = await usable(result);
    lines.push(`${entry.method} ${entry.url.slice(0, 48)}… — ` + (sig.valid
      ? `a VALID signature by the account's key over ${sig.kind} ${String(sig.digest).slice(0, 18)}…`
      : `not a valid signature (${sig.error || 'bad'})`));
  }
  return lines.join(' | ');
}

// === R. the rule, in Node =======================================================
{
  const signature = { method: 'personal_sign', params: ['0x68656c6c6f', SAFE], origin: 'https://app.example' };
  const r7 = (ctx) => {
    const view = lib.resolve(signature, { account: SAFE, chainId: 1, ...ctx });
    const w = view.warnings.find((x) => x.key === 'refuse.answerElsewhere' || x.key === 'refuse.answerNotToWallet');
    return { refused: view.refuse === true, key: w && w.key, to: w && w.params && w.params.to };
  };
  const ok = r7({ channel: 'url', callback: WALLET_CALLBACK });
  check('R. velawallet://sign-result on the url channel: the answer reaches the wallet', !ok.refused && !ok.key);
  for (const [callback, to] of [
    ['https://evil.example/steal', 'evil.example'],
    ['http://127.0.0.1:8477/vela', '127.0.0.1:8477'],
    ['velawallet://open?url=https%3A%2F%2Fevil.example%2F', 'velawallet://open'],
    ['velawallet://sign-result?x=1', 'velawallet://sign-result'],
    ['VELAWALLET://sign-result', 'velawallet://sign-result'],
    ['evilwallet://sign-result', 'evilwallet://sign-result'],
  ]) {
    const got = r7({ channel: 'url', callback });
    check(`R. ${callback}: refused, naming ${to}`, got.refused && got.key === 'refuse.answerElsewhere' && got.to === to,
      JSON.stringify(got));
  }
  for (const [what, ctx] of [
    ['no answer address', { channel: 'url' }],
    ['the loopback socket', { channel: 'ws', callback: WALLET_CALLBACK }],
    ['no channel at all', { callback: WALLET_CALLBACK }],
  ]) {
    const got = r7(ctx);
    check(`R. ${what}: refused`, got.refused && got.key === 'refuse.answerNotToWallet', JSON.stringify(got));
  }
  // postMessage (2026-10-30): answered only to Vela's own wallets, by the
  // exact origin the browser reports — the web wallet and the published
  // extension — and nothing broader.
  for (const opener of ['https://wallet.getvela.app', 'https://getvela.app', 'chrome-extension://bjbdmnmpgcfkocfcfdocopkioacojkhl']) {
    const got = r7({ channel: 'post', originVerified: true, requester: opener });
    check(`R. postMessage from ${opener}: the answer reaches the wallet`, !got.refused && !got.key, JSON.stringify(got));
  }
  for (const [opener, to] of [
    ['https://staging.getvela.app', 'staging.getvela.app'],
    ['https://evil.example', 'evil.example'],
    ['chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 'chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'],
    ['chrome-extension://bjbdmnmpgcfkocfcfdocopkioacojkhl.evil', 'chrome-extension://bjbdmnmpgcfkocfcfdocopkioacojkhl.evil'],
    ['http://localhost:5173', 'localhost:5173'],
  ]) {
    const got = r7({ channel: 'post', originVerified: true, requester: opener });
    check(`R. postMessage from ${opener}: refused, naming the opener`,
      got.refused && got.key === 'refuse.answerElsewhere' && got.to === to, JSON.stringify(got));
  }
  const unverified = r7({ channel: 'post', originVerified: false, requester: 'https://wallet.getvela.app' });
  check('R. postMessage the browser did not vouch for: refused', unverified.refused);
  const ethSign = lib.resolve({ method: 'eth_sign', params: [SAFE, '0x' + 'ab'.repeat(32)], origin: 'https://app.example' },
    { account: SAFE, chainId: 1, channel: 'post', originVerified: true, requester: 'https://wallet.getvela.app' });
  check('R. eth_sign, even from the web wallet: refused', ethSign.refuse === true);
  // The site a wallet relays is the wallet's word: drawn as a claim.
  const relayed = lib.resolve(signature, { account: SAFE, chainId: 1, channel: 'post', originVerified: true,
    requester: 'chrome-extension://bjbdmnmpgcfkocfcfdocopkioacojkhl' });
  check('R. the site an extension relays is drawn as the wallet\'s word, never as verified',
    relayed.dapp.originVerified === false && relayed.warnings.some((w) => w.key === 'warn.relayedOrigin'),
    JSON.stringify(relayed.dapp));

  const signIn = (ctx) => lib.resolve({ method: 'vela_signIn', params: [{}], origin: '' }, { rpId: 'getvela.app', ...ctx });
  const home = signIn({ channel: 'url', callback: WALLET_CALLBACK });
  const away = signIn({ channel: 'url', callback: 'https://evil.example/steal' });
  check('R. a sign-in on the url channel: offered when it answers the wallet, refused when it does not',
    !home.refuse && away.refuse === true && away.warnings.some((w) => w.key === 'refuse.ceremonyNotForWallet'));

  // Safari (native AUDIT §6.5): the extension's origin is made up per launch,
  // so the opener is answered when the browser's origin for it is EXACTLY one
  // the extension attested inside this window (lib/intake.js
  // `hearAttestations`) — a channel fact, never the requester's context.
  const SAFARI = 'safari-web-extension://3F2504E0-4F89-11D3-9A0C-0305E82C3301';
  const OTHER_SAFARI = 'safari-web-extension://9C1B6B2A-0000-4000-8000-000000000001';
  const safari = (requester, attestedExtensions) =>
    r7({ channel: 'post', originVerified: true, requester, attestedExtensions });
  check('R. Safari: the attested extension that opened the page is answered',
    !safari(SAFARI, [SAFARI]).refused);
  check('R. Safari: one of several attested origins is answered (the opener must still be one)',
    !safari(SAFARI, [OTHER_SAFARI, SAFARI]).refused);
  for (const [what, requester, attested] of [
    ['an extension that did not attest', SAFARI, []],
    ['an extension other than the one that attested', OTHER_SAFARI, [SAFARI]],
    ['no attestation at all', SAFARI, undefined],
    ['an attestation that is not a list', SAFARI, SAFARI],
    ['a Safari origin with a path', `${SAFARI}/x`, [`${SAFARI}/x`]],
    ['a Safari origin that is not a UUID', 'safari-web-extension://vela', ['safari-web-extension://vela']],
    ['a website "attested" as an extension', 'https://evil.example', ['https://evil.example']],
  ]) {
    const got = safari(requester, attested);
    check(`R. Safari: ${what} — refused`, got.refused && got.key === 'refuse.answerElsewhere', JSON.stringify(got));
  }
  // sign.js `show` writes the channel's list over any `attestedExtensions` a
  // requester put in its context — asserted on the published bytes below (S).
  check('R. Safari: unverified postMessage stays refused even when attested',
    r7({ channel: 'post', originVerified: false, requester: SAFARI, attestedExtensions: [SAFARI] }).refused);
  const safariEthSign = lib.resolve({ method: 'eth_sign', params: [SAFE, '0x' + 'ab'.repeat(32)], origin: 'https://app.example' },
    { account: SAFE, chainId: 1, channel: 'post', originVerified: true, requester: SAFARI, attestedExtensions: [SAFARI] });
  check('R. Safari: eth_sign from the attested extension — refused', safariEthSign.refuse === true);
  const safariRelayed = lib.resolve(signature, { account: SAFE, chainId: 1, channel: 'post', originVerified: true,
    requester: SAFARI, attestedExtensions: [SAFARI] });
  check('R. Safari: the site the extension relays is the wallet\'s word',
    safariRelayed.dapp.originVerified === false && safariRelayed.warnings.some((w) => w.key === 'warn.relayedOrigin'));
  const safariSignIn = signIn({ channel: 'post', originVerified: true, requester: SAFARI, attestedExtensions: [SAFARI] });
  const strangerSignIn = signIn({ channel: 'post', originVerified: true, requester: OTHER_SAFARI, attestedExtensions: [SAFARI] });
  check('R. Safari: the attested extension is a wallet for a key ceremony; another extension is not',
    lib.resolve.walletRequester({ channel: 'post', originVerified: true, requester: SAFARI, attestedExtensions: [SAFARI] }) &&
    !lib.resolve.walletRequester({ channel: 'post', originVerified: true, requester: OTHER_SAFARI, attestedExtensions: [SAFARI] }) &&
    !safariSignIn.refuse && !strangerSignIn.warnings.some((w) => w.key === 'refuse.answerElsewhere'),
    JSON.stringify(strangerSignIn.warnings.map((w) => w.key)));
  // Chrome and the web wallet stay exact: an attestation never widens them.
  check('R. Chrome: an attestation does not admit another Chrome extension id',
    r7({ channel: 'post', originVerified: true, requester: 'chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
      attestedExtensions: ['chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'] }).refused);
}

try {
  // === A. a signature answered to the attacker =================================
  //
  // Its context even claims to be the wallet's own channel; the page takes
  // the channel's word over the requester's.
  {
    const before = stolen.length;
    const page = await tab(`${PAGE}?ch=url&lang=en${fragment({ intent, context: { ...context, ...spoof } }, 'https://evil.example/steal', 'tok-a')}`);
    const slid = await slideIfOffered(page);
    await sleep(slid ? 3000 : 600);
    check('A. answered to evil.example: no slide is offered', !slid);
    const said = String(await page.ev("[...document.querySelectorAll('.warning-text')].map((n) => n.textContent).join(' | ') || document.body.innerText"));
    check('A. the card says it will not sign, and names where the answer would have gone',
      /sends a signature only to the Vela wallet/.test(said) && said.includes('evil.example'),
      said.replace(/\n/g, ' / ').slice(0, 170));
    check('A. no passkey prompt was raised', (await page.ev('window.__webauthnCalls')) === 0 && (await page.uses()) === 0,
      `prompts=${await page.ev('window.__webauthnCalls')} key uses=${await page.uses()}`);
    await page.close();
    await sleep(1200); // a closing tab's beacon would have arrived by now
    const got = stolen.slice(before);
    check('A. evil.example received nothing — no answer, no refusal, not even when the tab closed',
      got.length === 0, got.length ? await describeStolen(got) : '');
  }

  // === B. the same operation, asked over postMessage ============================
  {
    const opener = await Page.open(CDP, `https://evil.example/open?signer=${encodeURIComponent(`${PAGE}?ch=post&lang=en`)}`);
    await opener.waitFor('!!window.__go');
    await opener.ev('window.__go()', true);
    const popup = await Page.find(CDP, (u) => u.includes('/sign?ch=post'));
    const uses = await addKey(popup);
    await popup.waitFor("window.__velaState && ['card', 'refused'].includes(window.__velaState.phase)");
    const slid = await slideIfOffered(popup);
    await opener.waitFor('window.__answers.length > 0', slid ? 8000 : 4000);
    const answers = await opener.ev('window.__answers');
    check('B. asked over postMessage by evil.example: no slide is offered', !slid);
    check('B. the opener heard "refused", and never a signature',
      answers.length === 1 && answers[0].vela === 'error' && answers[0].code === 'refused',
      answers.map((a) => a.vela === 'result'
        ? `evil.example got a signature over ${a.result.digestKind} ${String(a.result.digest).slice(0, 18)}…`
        : `${a.vela}:${a.code}`).join(' | '));
    check('B. no passkey prompt was raised', (await uses()) === 0, `key uses=${await uses()}`);
    await popup.close();
    await opener.close();
  }

  // === C. a sign-in answered to the attacker ====================================
  {
    const before = stolen.length;
    const signIn = { intent: { method: 'vela_signIn', params: [{}], origin: '' }, context: { walletName: 'Mine' } };
    const page = await tab(`${PAGE}?ch=url&lang=en${fragment(signIn, 'https://evil.example/steal', 'tok-c')}`);
    const slid = await slideIfOffered(page);
    await sleep(slid ? 3000 : 600);
    check('C. a sign-in answered to evil.example: refused before any prompt',
      !slid && (await page.ev('window.__webauthnCalls')) === 0);
    await page.close();
    await sleep(1200);
    const got = stolen.slice(before);
    check('C. and evil.example received nothing', got.length === 0, got.map((e) => `${e.method} ${e.url.slice(0, 60)}`).join(' | '));
  }

  // === D. a loopback answer address ===========================================
  {
    const before = loopbackHits.length;
    const page = await tab(`${PAGE}?ch=url&lang=en${fragment({ intent, context }, `http://127.0.0.1:${LOOPBACK_PORT}/vela`, 'tok-d')}`);
    const slid = await slideIfOffered(page);
    await sleep(slid ? 3000 : 600);
    await page.close();
    await sleep(1200);
    check('D. a loopback answer address is refused by the published page, and nothing reaches it',
      !slid && loopbackHits.length === before, `slid=${slid} hits=${loopbackHits.length - before}`);

    const html = readFileSync(PUBLISHED, 'utf8');
    const harnessCode = existsSync(join(root, 'samples/loopback-answer.js'));
    check('D. the published bytes do not contain the harness exception',
      harnessCode && !html.includes('loopback-answer') && !html.includes('var strict = ns.resolve.answersToWallet'));

    // The harness page is src/sign.html plus that one file, so what the demo
    // tests is the real page and nothing else.
    const scripts = (file) => [...readFileSync(join(root, file), 'utf8').matchAll(/<script\s+src="([^"]+)"\s*><\/script>/g)].map((m) => m[1]);
    const harnessPage = join(root, 'samples/loopback-sign.html');
    const expected = scripts('src/sign.html').map((s) => `../src/${s}`);
    expected.splice(expected.indexOf('../src/sign.js'), 0, 'loopback-answer.js');
    const actual = existsSync(harnessPage) ? scripts('samples/loopback-sign.html') : [];
    check('D. the harness page names src/sign.html\'s scripts plus loopback-answer.js, nothing else',
      JSON.stringify(actual) === JSON.stringify(expected), existsSync(harnessPage) ? '' : 'samples/loopback-sign.html is missing');

    // …and the harness page does answer its loopback listener, or the demo
    // would be testing nothing.
    if (existsSync(harnessPage)) {
      const harness = await tab(`https://getvela.app/samples/loopback-sign.html?ch=url&lang=en${fragment({ intent, context }, `http://127.0.0.1:${LOOPBACK_PORT}/vela`, 'tok-h')}`);
      const offered = await slideIfOffered(harness);
      for (let i = 0; i < 40 && loopbackHits.length === before; i++) await sleep(200);
      const hit = loopbackHits.slice(before).find((h) => h.url.startsWith('/vela?t=tok-h&result='));
      check('D. the harness page (test only) answers its loopback listener',
        offered && !!hit && (await usable(new URL(hit.url, 'http://x').searchParams.get('result'))).valid);
      await harness.close();
    }
  }

  // === E. frames ================================================================
  {
    const signer = `${PAGE}?ch=url&lang=en${fragment({ intent, context }, WALLET_CALLBACK, 'tok-e')}`;
    const frame = await Page.open(CDP, `https://evil.example/__frame?src=${encodeURIComponent(signer)}`);
    await sleep(2500);
    // A cross-origin frame is a target of its own. DevTools lists it under the
    // URL it was asked for even when the browser refused to show it, so look
    // inside: a refused frame holds the browser's error page.
    const targets = await (await fetch(`http://127.0.0.1:${CDP}/json/list`)).json();
    const iframe = targets.find((t) => t.type === 'iframe' && t.url.startsWith('https://sign.getvela.app/'));
    let shown = '(no frame target)';
    if (iframe && iframe.webSocketDebuggerUrl) {
      const child = await Page.attach(iframe.webSocketDebuggerUrl, CDP);
      shown = await child.ev("location.href + ' · ' + (window.__velaState ? window.__velaState.phase : '-')");
    }
    const headers = headersFor(`/b/${HASH}/sign`);
    check('E. the host sends frame-ancestors \'none\' and X-Frame-Options: DENY for the page',
      /frame-ancestors 'none'/.test(headers['Content-Security-Policy'] || '') && headers['X-Frame-Options'] === 'DENY',
      JSON.stringify(headers));
    check('E. so evil.example cannot frame it: the frame holds the browser\'s error page, not the signer',
      !!iframe && shown.startsWith('chrome-error://'), shown.slice(0, 100));
    await frame.close();

    // A host that sends no headers (a copy served from anywhere): the page
    // itself refuses. Same origin here, so the frame can be read.
    const bare = `/dist/b/${HASH}/sign.html?ch=url&lang=en${fragment({ intent, context }, WALLET_CALLBACK, 'tok-f')}`;
    const parent = await Page.open(CDP, `https://getvela.app/__frame?src=${encodeURIComponent(bare)}`);
    await parent.waitFor("(() => { const w = document.getElementById('f').contentWindow; return !!(w && w.__velaState && w.__velaState.phase !== 'starting'); })()");
    await sleep(800);
    const inside = await parent.ev(`(() => {
      const w = document.getElementById('f').contentWindow;
      return { phase: w.__velaState.phase, received: w.__velaState.received,
        sheet: !!w.document.querySelector('.sheet'), slider: !!w.__slider,
        status: (w.document.getElementById('status') || {}).textContent || '' };
    })()`);
    check('E. with no headers, the page refuses to run inside a frame: no request read, no card, no slide',
      inside.phase === 'framed' && inside.received === 0 && !inside.sheet && !inside.slider,
      JSON.stringify(inside).slice(0, 160));
    await parent.close();
  }

  // === F. the real thing still works ===========================================
  {
    const before = stolen.length;
    const page = await tab(`${PAGE}?ch=url&lang=en${fragment({ intent, context }, WALLET_CALLBACK, 'tok-ok')}`);
    const offered = await slideIfOffered(page);
    for (let i = 0; i < 40 && !page.navigations.some((u) => u.startsWith(WALLET_CALLBACK)); i++) await sleep(200);
    const answer = page.navigations.find((u) => u.startsWith(`${WALLET_CALLBACK}?t=tok-ok&result=`));
    const sig = answer ? await usable(new URL(answer).searchParams.get('result')) : { valid: false };
    check('F. answered to velawallet://sign-result, the slide is offered and the page answers there',
      offered && !!answer, answer ? answer.slice(0, 60) + '…' : page.navigations.join(' | ').slice(0, 120));
    check('F. and the answer is the account\'s signature over the SafeOp the card showed',
      sig.valid && sig.kind === 'SafeOp' && sig.overDigest, `${sig.kind} ${String(sig.digest).slice(0, 18)}…`);
    check('F. evil.example heard nothing along the way', stolen.length === before);
    await page.close();
  }

  // === G. a multisig member's approval over postMessage (2026-10-29) =============
  //
  // The web wallet opens the page and hands it the SafeTx by postMessage; the
  // page answers to the opener only when the BROWSER reports the opener as
  // Vela's web wallet, and only for this one request.
  {
    const MULTISIG = '0x4c3FF6Bc7d1a4b7f4e0e93B4C61b8b1a7e5dF2A9';
    const safeTx = {
      to: ATTACKER, value: '0xde0b6b3a7640000', data: '0x', operation: 0, safeTxGas: '0x0', baseGas: '0x0',
      gasPrice: '0x0', gasToken: '0x0000000000000000000000000000000000000000',
      refundReceiver: '0x0000000000000000000000000000000000000000', nonce: '0x3',
    };
    const parsed = lib.multisig.parseTx(safeTx).tx;
    const hashes = lib.multisig.hashes(100, MULTISIG, parsed);
    const approvalIntent = (claimed) => ({
      method: 'vela_multisigApprove', origin: '',
      params: [{ chainId: 100, safe: MULTISIG, member: SAFE, safeTx, safeTxHash: claimed }],
    });
    const msContext = { chainId: 100, account: SAFE, allowCredentials: [credentialId], multisig: { name: 'Team fund' } };
    const expected = Buffer.from(lib.multisig.approvalChallenge(100, SAFE, hashes.safeTxHash)).toString('base64url');

    async function askOver(who, request) {
      const req = Buffer.from(JSON.stringify(request)).toString('base64url');
      const opener = await Page.open(CDP,
        `https://${who}/open-request?signer=${encodeURIComponent(`${PAGE}?ch=post&lang=en`)}&req=${req}`);
      await opener.waitFor('!!window.__go');
      await opener.ev('window.__go()', true);
      const popup = await Page.find(CDP, (u) => u.includes('/sign?ch=post'));
      const uses = await addKey(popup);
      await popup.waitFor("window.__velaState && ['card', 'refused'].includes(window.__velaState.phase)");
      const text = await popup.ev('document.body.innerText');
      const slid = await slideIfOffered(popup);
      await opener.waitFor('window.__answers.length > 0', slid ? 10000 : 4000).catch(() => {});
      const answers = await opener.ev('window.__answers');
      const used = await uses();
      await popup.close();
      await opener.close();
      return { slid, answers, used, text };
    }

    const stranger = await askOver('evil.example', { intent: approvalIntent(hashes.safeTxHash), context: msContext });
    check('G. a multisig approval asked by evil.example over postMessage: no button, no passkey, "refused"',
      !stranger.slid && stranger.used === 0 && stranger.answers.length === 1 &&
        stranger.answers[0].vela === 'error' && stranger.answers[0].code === 'refused',
      JSON.stringify(stranger.answers).slice(0, 160));

    const wallet = await askOver('wallet.getvela.app', { intent: approvalIntent(hashes.safeTxHash), context: msContext });
    const answer = wallet.answers.find((a) => a.vela === 'result');
    const clientData = answer
      ? JSON.parse(Buffer.from(String(answer.result.clientDataJSON).replace(/^0x/, ''), 'hex').toString('utf8'))
      : {};
    const sig = answer
      ? await usable(Buffer.from(JSON.stringify(answer.result)).toString('base64url'))
      : { valid: false };
    check('G. asked by the web wallet: the card shows the multisig, its name as the app\'s, the three hashes',
      wallet.text.includes('Team fund') && wallet.text.includes('named by the app') &&
        wallet.text.includes(hashes.safeTxHash) && wallet.text.includes(hashes.domainHash) &&
        wallet.text.includes(hashes.messageHash), wallet.text.slice(0, 200));
    check('G. and the approval goes back to the web wallet, signed by the member\'s key',
      wallet.slid && !!answer && sig.valid, JSON.stringify(wallet.answers).slice(0, 160));
    check('G. over exactly the approval bytes: SafeMessage(safeTxHash) under the member\'s domain',
      clientData.challenge === expected, `${clientData.challenge} vs ${expected}`);

    const forged = '0x' + 'ab'.repeat(32);
    const lied = await askOver('wallet.getvela.app', { intent: approvalIntent(forged), context: msContext });
    check('G. the wallet\'s fields do not make the hash it claims: refused loudly, no passkey, "refused"',
      !lied.slid && lied.used === 0 && lied.text.includes(forged) && lied.text.includes(hashes.safeTxHash) &&
        lied.answers.length === 1 && lied.answers[0].code === 'refused', lied.text.slice(0, 200));

    // 2026-10-30: the page, everywhere — a message the web wallet asks is
    // answered to it, over the SafeMessage this page derived.
    const messageRequest = {
      intent: { method: 'personal_sign', params: ['0x68656c6c6f', SAFE], origin: '' },
      context: { chainId: 1, account: SAFE, allowCredentials: [credentialId] },
    };
    const message = await askOver('wallet.getvela.app', messageRequest);
    const signed = message.answers.find((a) => a.vela === 'result');
    const messageSig = signed ? await usable(Buffer.from(JSON.stringify(signed.result)).toString('base64url')) : { valid: false };
    check('G. a message the web wallet asks over postMessage: answered to it, by the account\'s key, over the digest the page derived',
      message.slid && message.used === 1 && messageSig.valid && messageSig.overDigest,
      JSON.stringify(message.answers).slice(0, 160));
    const sub = await askOver('staging.getvela.app', messageRequest);
    check('G. the same asked by a getvela.app subdomain: refused, no passkey',
      !sub.slid && sub.used === 0 && sub.answers.length === 1 && sub.answers[0].code === 'refused',
      JSON.stringify(sub.answers).slice(0, 160));

    // Round 7 (r7-5): the wallet gives up on the request while the page shows
    // it — the page stops offering to sign, and says why.
    {
      const req = Buffer.from(JSON.stringify(messageRequest)).toString('base64url');
      const opener = await Page.open(CDP,
        `https://wallet.getvela.app/open-request?signer=${encodeURIComponent(`${PAGE}?ch=post&lang=en`)}&req=${req}`);
      await opener.waitFor('!!window.__go');
      await opener.ev('window.__go()', true);
      const popup = await Page.find(CDP, (u) => u.includes('/sign?ch=post'));
      const uses = await addKey(popup);
      await popup.waitFor("window.__velaState && window.__velaState.phase === 'card'");
      await opener.ev("window.__bye('withdrawn')", true);
      await popup.waitFor("window.__velaState.phase === 'ended' || window.__velaState.phase === 'withdrawn'").catch(() => {});
      const text = await popup.ev('document.body.innerText');
      const offered = await popup.ev("!!document.querySelector('.confirm:not([disabled])')");
      check('G. the wallet withdraws the request: the page says so and offers nothing to sign (r7-5)',
        text.includes('Withdrawn') && text.includes('stopped waiting for this request') && !offered && (await uses()) === 0,
        text.slice(0, 200));
      await popup.close();
      await opener.close();
    }

    // Round 7 (S4): answered and said goodbye — the page says it is done once,
    // then closes itself (each extension signature used to leave a tab open).
    {
      const req = Buffer.from(JSON.stringify(messageRequest)).toString('base64url');
      const opener = await Page.open(CDP,
        `https://wallet.getvela.app/open-request?signer=${encodeURIComponent(`${PAGE}?ch=post&lang=en`)}&req=${req}`);
      await opener.waitFor('!!window.__go');
      await opener.ev('window.__go()', true);
      const popup = await Page.find(CDP, (u) => u.includes('/sign?ch=post'));
      await addKey(popup);
      await popup.waitFor("window.__velaState && window.__velaState.phase === 'card'");
      const slid = await slideIfOffered(popup);
      await opener.waitFor('window.__answers.length > 0', 10000).catch(() => {});
      await opener.ev("window.__bye('done')", true);
      await popup.waitFor("window.__velaState.phase === 'ended'").catch(() => {});
      const text = await popup.ev('document.body.innerText').catch(() => '');
      const said = text.split('Done — the wallet has what it asked for').length - 1;
      await opener.waitFor('window.__childClosed()', 6000).catch(() => {});
      const closed = await opener.ev('window.__childClosed()');
      check('G. answered, the wallet says goodbye: "Done" said once, then the page closes itself (S4)',
        slid && said === 1 && closed === true, JSON.stringify({ slid, said, closed, text: text.slice(0, 160) }));
      await popup.close().catch(() => {});
      await opener.close();
    }
  }

  // === H. the Chrome extension over postMessage (2026-10-30) ====================
  //
  // The extension's request window or side panel opens the page with
  // `window.open` and hands it the dApp's request. The browser reports the
  // opener as `chrome-extension://<id>`: the published id is answered, any
  // other is not.
  {
    async function askFromExtension(which, request) {
      const ext = EXTENSIONS[which];
      const opener = await Page.open(CDP, `chrome-extension://${ext.id}/opener.html`);
      await opener.waitFor('!!window.__go');
      await opener.ev(`window.__go(${JSON.stringify(`${PAGE}?ch=post&lang=en`)}, ${JSON.stringify(request)})`, true);
      const popup = await Page.find(CDP, (u) => u.includes('/sign?ch=post'));
      const uses = await addKey(popup);
      await popup.waitFor("window.__velaState && ['card', 'refused'].includes(window.__velaState.phase)");
      const text = await popup.ev('document.body.innerText');
      const slid = await slideIfOffered(popup);
      await opener.waitFor('window.__answers.length > 0', slid ? 10000 : 4000).catch(() => {});
      const answers = await opener.ev('window.__answers');
      const used = await uses();
      await popup.close();
      await opener.close();
      const result = answers.find((a) => a.vela === 'result');
      const sig = result ? await usable(Buffer.from(JSON.stringify(result.result)).toString('base64url')) : { valid: false };
      return { slid, answers, used, text, sig };
    }
    check('H. the published extension id is the one its manifest key derives',
      EXTENSIONS.published.id === 'bjbdmnmpgcfkocfcfdocopkioacojkhl', EXTENSIONS.published.id);

    const site = 'https://app.example';
    const typed = JSON.stringify({
      types: {
        EIP712Domain: [{ name: 'name', type: 'string' }, { name: 'chainId', type: 'uint256' }],
        Mail: [{ name: 'contents', type: 'string' }],
      },
      primaryType: 'Mail', domain: { name: 'Ether Mail', chainId: 1 }, message: { contents: 'Hello from a dApp' },
    });
    const cases = [
      ['personal_sign', { intent: { method: 'personal_sign', params: ['0x68656c6c6f', SAFE], origin: site },
        context: { chainId: 1, account: SAFE, allowCredentials: [credentialId] } }],
      ['eth_signTypedData_v4', { intent: { method: 'eth_signTypedData_v4', params: [SAFE, typed], origin: site },
        context: { chainId: 1, account: SAFE, allowCredentials: [credentialId] } }],
      ['eth_sendTransaction', { intent, context }],
    ];
    for (const [method, request] of cases) {
      const got = await askFromExtension('published', request);
      check(`H. ${method} from the published extension: signed and answered to it, by the account's key over the page's digest`,
        got.slid && got.used === 1 && got.sig.valid && got.sig.overDigest,
        `${got.sig.kind || '-'} ${JSON.stringify(got.answers).slice(0, 120)}`);
      if (method === 'personal_sign') {
        check('H. the site the extension relays is drawn as its word ("claimed"), not as verified',
          /app\.example/.test(got.text) && got.text.includes("the wallet's word"), got.text.slice(0, 200));
      }
    }
    const other = await askFromExtension('other', cases[0][1]);
    check('H. the same from an extension with another id: refused before any passkey, nothing signed',
      !other.slid && other.used === 0 && other.answers.length === 1 && other.answers[0].code === 'refused',
      JSON.stringify(other.answers).slice(0, 160));
    check('H. and the card names the extension the answer would have gone to',
      other.text.includes(`chrome-extension://${EXTENSIONS.other.id}`), other.text.slice(0, 200));

    // === S. Safari: no opener, the extension's content script relays =========
    //
    // Measured on iOS Safari 26: a page an extension page opens has no
    // `window.opener`. Vela's content script on the page
    // (`app-web/vela-wallet/extension/safari/attest.js`) attests the
    // extension's origin inside the page and carries the hand-off. Played
    // here by a script in an ISOLATED world on the page's own tab (CDP
    // `worldName`) — which is what a content script is: code in this window,
    // not in the page's world. What it heard is left on the DOM for the test.
    const SAFARI = 'safari-web-extension://3f2504e0-4f89-11d3-9a0c-0305e82c3301';
    const OTHER_SAFARI = 'safari-web-extension://9c1b6b2a-0000-4000-8000-000000000001';
    async function viaSafariRelay({ attest, from, request }) {
      const relay = `(() => {
        const heard = [];
        const keep = () => document.documentElement && document.documentElement.setAttribute('data-relayed', JSON.stringify(heard));
        const say = () => ${attest ? `window.postMessage({ vela: 'extension', origin: ${JSON.stringify(attest)} }, location.origin)` : 'undefined'};
        window.addEventListener('message', (event) => {
          if (event.source !== window) return;
          const data = event.data || {};
          if (data.vela === 'extension?') { say(); return; }
          if (data.relay !== 'to-extension') return;
          heard.push(data); keep();
          if (data.vela === 'ready') {
            window.postMessage(Object.assign({ vela: 'intent', v: 1, id: 'safari-1', relay: 'to-page', from: ${JSON.stringify(from)} },
              ${JSON.stringify(request)}), location.origin);
          }
        });
        say();
      })();`;
      const page = await Page.open(CDP, 'about:blank');
      await page.send('Page.enable');
      await page.send('Page.addScriptToEvaluateOnNewDocument', { source: COUNT_WEBAUTHN });
      await page.send('Page.addScriptToEvaluateOnNewDocument', { source: relay, worldName: 'vela-safari-extension' });
      const uses = await addKey(page);
      await page.navigate(`${PAGE}?ch=post&lang=en`);
      await page.waitFor("window.__velaState && ['card', 'refused', 'answered', 'waiting', 'error'].includes(window.__velaState.phase)", 8000).catch(() => {});
      await sleep(1500);
      const phase = await page.ev('window.__velaState && window.__velaState.phase');
      const text = await page.ev('document.body.innerText');
      const slid = await slideIfOffered(page);
      if (slid) await sleep(3000);
      const relayed = JSON.parse((await page.ev("document.documentElement.getAttribute('data-relayed')")) || '[]');
      const used = await uses();
      await page.close();
      const result = relayed.find((m) => m.vela === 'result');
      const sig = result ? await usable(Buffer.from(JSON.stringify(result.result)).toString('base64url')) : { valid: false };
      return { phase, text, slid, relayed, used, sig };
    }
    const safariSign = cases[0][1];
    const ok = await viaSafariRelay({ attest: SAFARI, from: SAFARI, request: safariSign });
    check('S. Safari relay: the page says ready to the extension in its window, not to an opener',
      ok.relayed.length > 0 && ok.relayed[0].vela === 'ready', JSON.stringify(ok.relayed).slice(0, 160));
    check('S. Safari relay: the attested extension is answered — signed by the account\'s key, back through the relay',
      ok.slid && ok.used === 1 && ok.sig.valid && ok.sig.overDigest,
      `${ok.phase} ${JSON.stringify(ok.relayed).slice(0, 160)}`);
    check('S. Safari relay: the relayed site is drawn as the wallet\'s word',
      ok.text.includes("the wallet's word"), ok.text.slice(0, 160));
    const stranger = await viaSafariRelay({ attest: SAFARI, from: OTHER_SAFARI, request: safariSign });
    check('S. Safari relay: an intent naming an extension that did not attest is not heard — no card, nothing signed',
      !stranger.slid && stranger.used === 0 && !stranger.relayed.some((m) => m.vela === 'result') && stranger.phase !== 'card',
      `${stranger.phase} ${JSON.stringify(stranger.relayed).slice(0, 120)}`);
    const unattested = await viaSafariRelay({ attest: null, from: SAFARI, request: safariSign });
    check('S. Safari relay: no attestation in the window — the page says nobody asked, offers nothing',
      !unattested.slid && unattested.used === 0 && unattested.relayed.length === 0 && unattested.phase !== 'card',
      `${unattested.phase} ${unattested.text.slice(0, 120)}`);
    const ethSignRelay = await viaSafariRelay({ attest: SAFARI, from: SAFARI,
      request: { intent: { method: 'eth_sign', params: [SAFE, '0x' + 'ab'.repeat(32)], origin: site }, context: safariSign.context } });
    check('S. Safari relay: eth_sign from the attested extension — refused, and the refusal travels back',
      !ethSignRelay.slid && ethSignRelay.used === 0 &&
        ethSignRelay.relayed.some((m) => m.vela === 'error' && m.code === 'refused'),
      JSON.stringify(ethSignRelay.relayed).slice(0, 160));
  }
} catch (error) {
  console.log('FAILED: ' + (error.stack || error.message));
  check.results.push(false);
} finally {
  chrome.kill();
  host.close();
  loopback.close();
  try { rmSync(profile, { recursive: true, force: true }); } catch { /* ignore */ }
}
process.exit(check.summary() ? 0 : 1);
