// End-to-end for the browser-side channels: a real intent goes in, a real
// WebAuthn assertion comes out, over the real transports.
//
// The passkey ceremony runs against a CDP virtual authenticator, so this needs
// no fingerprint reader — but everything else is the shipped code path:
// intake → resolve → render → drag → digest → navigator.credentials → respond.
//
//   CHROME_BIN=… SB=… node samples/channels-test.mjs
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { createServer as createTcpServer, connect as tcpConnect } from 'node:net';
import { createHash, webcrypto } from 'node:crypto';
import { mkdtempSync, readFileSync, rmSync, readdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..');
const SB = process.env.SB;
const TLS_PORT = 8443;
const LOOPBACK_PORT = 8477;
const CDP = 9390;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const b64url = (s) => Buffer.from(s).toString('base64url');

// The page's own encoders, so the fixtures below are built the same way the
// wallet would build them.
globalThis.window = globalThis;
for (const file of ['src/lib/keccak.js', 'src/lib/abi.js', 'src/lib/encode.js']) {
  (0, eval)(readFileSync(join(root, file), 'utf8'));
}
const lib = globalThis.VelaCS;

// vela-core, compiled — the reference the page's digest must agree with.
const core = await import(join(root, '..', '..', 'rust/pkg-web/vela_core.js'));
// The committed build, whatever its content hash is this week.
const wasmDir = join(root, '..', '..', 'assets/wasm');
const wasmFile = readdirSync(wasmDir).find((name) => /^vela_core_bg\..*\.wasm$/.test(name));
core.initSync({ module: readFileSync(join(wasmDir, wasmFile)) });

function expectedSafeOpHash(op, chainId) {
  const bytes = core.attestSafeOpHash(JSON.stringify({
    sender: op.sender, nonce: op.nonce,
    init_code_hex: op.initCode, call_data_hex: op.callData,
    verification_gas_limit: op.verificationGasLimit,
    call_gas_limit: op.callGasLimit,
    pre_verification_gas: op.preVerificationGas,
    max_fee_per_gas: op.maxFeePerGas,
    max_priority_fee_per_gas: op.maxPriorityFeePerGas,
    paymaster_and_data_hex: op.paymasterAndData,
  }), '', BigInt(chainId));
  return '0x' + Buffer.from(bytes).toString('hex');
}

const results = [];
function check(name, pass, detail) {
  results.push(pass);
  console.log(`${pass ? 'PASS' : 'FAIL'}  ${name}${detail ? '  — ' + detail : ''}`);
}

// The native app's side of the loopback callback.
let callbackHit = null;
const loopback = createServer((request, response) => {
  const url = new URL(request.url, `http://127.0.0.1:${LOOPBACK_PORT}`);
  // The navigation also asks for /favicon.ico; only the callback path counts.
  if (url.pathname === '/vela') callbackHit = url;
  response.writeHead(200, { 'content-type': 'text/html' });
  response.end('<p>you can close this tab</p>');
});
loopback.listen(LOOPBACK_PORT, '127.0.0.1');

const profile = mkdtempSync(join(tmpdir(), 'channels-'));
const tls = spawn('python3', [join(SB, 'tls-serve.py'), root, String(TLS_PORT), join(SB, 'cert.pem'), join(SB, 'key.pem')], { stdio: 'ignore' });
const chrome = spawn(process.env.CHROME_BIN, [
  '--headless=new', `--remote-debugging-port=${CDP}`,
  `--user-data-dir=${profile}`,
  `--host-resolver-rules=MAP getvela.app:443 127.0.0.1:${TLS_PORT}`,
  '--ignore-certificate-errors', '--no-proxy-server',
  '--enable-unsafe-extension-debugging',
  `--disable-extensions-except=${root}`, `--load-extension=${root}`,
  '--no-first-run', '--no-default-browser-check', 'about:blank',
], { stdio: 'ignore' });

class Page {
  constructor(ws) {
    this.ws = ws;
    this.id = 0;
    this.pending = new Map();
    ws.addEventListener('message', (event) => {
      const message = JSON.parse(event.data);
      if (message.id && this.pending.has(message.id)) {
        const { resolve, reject } = this.pending.get(message.id);
        this.pending.delete(message.id);
        message.error ? reject(new Error(message.error.message)) : resolve(message.result);
      }
    });
  }

  static async open(url) {
    // fetch() drops everything after '#' — encode the whole target URL, or the
    // intent in the fragment never reaches the browser.
    const tab = await (await fetch(
      `http://127.0.0.1:${CDP}/json/new?${encodeURIComponent(url)}`, { method: 'PUT' },
    )).json();
    const ws = new WebSocket(tab.webSocketDebuggerUrl);
    await new Promise((resolve) => ws.addEventListener('open', resolve));
    const page = new Page(ws);
    page.targetId = tab.id;
    await page.send('Runtime.enable');
    return page;
  }

  send(method, params = {}) {
    const id = ++this.id;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.ws.send(JSON.stringify({ id, method, params }));
    });
  }

  async ev(expression, userGesture = false) {
    const r = await this.send('Runtime.evaluate', {
      expression, returnByValue: true, awaitPromise: true, userGesture,
    });
    // send() already unwraps the CDP envelope, so this is the method result:
    // { result: RemoteObject, exceptionDetails? }
    if (r.exceptionDetails) throw new Error(r.exceptionDetails.text);
    return r.result.value;
  }

  // A virtual authenticator only exists for the target that added it — and the
  // signing page will NOT create a key, so one has to be there already. This is
  // the automated stand-in for "this person enrolled last week".
  async addAuthenticator(rpId = 'getvela.app') {
    await this.send('WebAuthn.enable');
    const { authenticatorId } = await this.send('WebAuthn.addVirtualAuthenticator', {
      options: {
        protocol: 'ctap2', transport: 'internal',
        hasResidentKey: true, hasUserVerification: true,
        isUserVerified: true, automaticPresenceSimulation: true,
      },
    });
    const pair = await webcrypto.subtle.generateKey(
      { name: 'ECDSA', namedCurve: 'P-256' }, true, ['sign', 'verify'],
    );
    const pkcs8 = Buffer.from(await webcrypto.subtle.exportKey('pkcs8', pair.privateKey));
    const rawId = Buffer.from(webcrypto.getRandomValues(new Uint8Array(16)));
    await this.send('WebAuthn.addCredential', {
      authenticatorId,
      credential: {
        credentialId: rawId.toString('base64'),
        isResidentCredential: true,
        rpId,
        privateKey: pkcs8.toString('base64'),
        userHandle: Buffer.from('vela-test').toString('base64'),
        signCount: 0,
      },
    });
    const publicKey = Buffer.from(await webcrypto.subtle.exportKey('raw', pair.publicKey));
    return {
      authenticatorId,
      credentialId: rawId.toString('base64url'),
      // What the wallet stores for this key: hex id, uncompressed SEC1 point.
      walletKey: { credentialId: rawId.toString('hex'), publicKeyHex: publicKey.toString('hex') },
    };
  }
}

async function waitFor(page, expression, timeoutMs = 8000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await page.ev(expression)) return true;
    await sleep(150);
  }
  return false;
}

try {
  for (let i = 0; i < 80; i++) {
    try { await (await fetch(`http://127.0.0.1:${CDP}/json/version`)).json(); break; } catch { await sleep(250); }
  }
  await sleep(1000);

  const fixtures = JSON.parse(readFileSync(join(root, 'samples/intents.json'), 'utf8'));
  // A message intent: its digest is derivable here, so it can actually be signed.
  const login = fixtures.cases.find((c) => c.code === 'CS19');
  const transfer = fixtures.cases.find((c) => c.code === 'CS1');

  // === 1. URL fragment in, loopback callback out ============================
  {
    const payload = JSON.stringify({ intent: login.intent, context: login.context });
    const url = 'https://getvela.app/src/sign.html?ch=url&lang=en#i=' + b64url(payload) +
      '&cb=' + b64url(`http://127.0.0.1:${LOOPBACK_PORT}/vela`) + '&t=tok-123';
    const page = await Page.open(url);
    await page.addAuthenticator();
    await sleep(1200);

    const ready = await waitFor(page, '!!window.__slider');
    if (!ready) {
      console.log('  status text :', await page.ev("document.getElementById('status').textContent"));
      console.log('  sheet html  :', String(await page.ev("document.getElementById('sheet-slot').innerHTML")).slice(0, 200));
      console.log('  libs loaded :', await page.ev("Object.keys(window.VelaCS || {}).join(',')"));
    }
    check('url: intent decoded and sheet rendered', ready,
      await page.ev("document.querySelector('.intent-label') && document.querySelector('.intent-label').textContent"));
    check('url: the one-time token is scrubbed from history', !(await page.ev('location.hash')).includes('tok-123'));

    await page.ev('window.__slider.__confirm()', true);
    const gotCallback = await (async () => {
      for (let i = 0; i < 60; i++) { if (callbackHit) return true; await sleep(200); }
      return false;
    })();
    check('url: the loopback callback was called', gotCallback, callbackHit ? callbackHit.pathname : '—');
    if (callbackHit) {
      check('url: the callback carries the one-time token back', callbackHit.searchParams.get('t') === 'tok-123');
      const result = JSON.parse(Buffer.from(callbackHit.searchParams.get('result'), 'base64url').toString());
      check('url: a real assertion came back', !!result.signature && result.signature.length === 130,
        `digest ${String(result.digest).slice(0, 14)}… (${result.digestKind})`);
      check('url: user verification was required and met', result.userVerified === true);
    }
  }

  // === 2. postMessage, same browser =========================================
  {
    callbackHit = null;
    const opener = await Page.open('https://getvela.app/samples/dapp-sim.html?lang=en');
    await sleep(600);
    await opener.ev('window.__openSigner()', true);
    await sleep(1200);

    const targets = await (await fetch(`http://127.0.0.1:${CDP}/json/list`)).json();
    const popup = targets.find((t) => t.type === 'page' && t.url.includes('sign.html?ch=post'));
    check('post: the signing window opened', !!popup);
    if (popup) {
      const ws = new WebSocket(popup.webSocketDebuggerUrl);
      await new Promise((resolve) => ws.addEventListener('open', resolve));
      const signer = new Page(ws);
      await signer.send('Runtime.enable');
      await signer.addAuthenticator();

      const ready = await waitFor(signer, '!!window.__slider');
      check('post: intent arrived over postMessage and rendered', ready);
      check('post: the requester origin is treated as VERIFIED',
        !(await signer.ev("[...document.querySelectorAll('.warning')].some(w => /self-reported/.test(w.textContent))")));

      await signer.ev('window.__slider.__confirm()', true);
      const answered = await waitFor(opener, '!!window.__answer', 12000);
      check('post: the result reached the opener', answered,
        answered ? String(await opener.ev('window.__answer.signature')).slice(0, 18) + '…' : '—');
    }
  }

  // === 3. a transaction must be refused, not signed =========================
  {
    const payload = JSON.stringify({ intent: transfer.intent, context: transfer.context });
    const page = await Page.open('https://getvela.app/src/sign.html?ch=url&lang=en#i=' + b64url(payload));
    await page.addAuthenticator();
    await sleep(1200);
    const refused = await page.ev("document.querySelector('.slide').classList.contains('slide-off')");
    const why = await page.ev("[...document.querySelectorAll('.warning-text')].map(n => n.textContent).join(' | ')");
    check('refusal: a transaction is not signed with a guessed digest', refused, why.slice(0, 90) + '…');
  }
  // === 3b. a transaction WITH its assembled operation =======================
  {
    callbackHit = null;
    const USDC = '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48';
    const ALICE = '0xaF5e8917831Ef08A64e18b2Cde9f8f5d32c7b3e1';
    const SAFE = '0x88cCA0f8B4E1F0dC0e7C4f9a2B3d5E6f7A8b6894';
    const transferCall = lib.encode.call('transfer(address,uint256)', [ALICE, 1000000000n]);
    const userOp = {
      sender: SAFE,
      nonce: '0x7',
      initCode: '0x',
      callData: lib.encode.call('executeUserOp(address,uint256,bytes,uint8)', [USDC, 0n, transferCall, 0]),
      verificationGasLimit: '300000',
      callGasLimit: '200000',
      preVerificationGas: '110000',
      maxFeePerGas: '1500000007',
      maxPriorityFeePerGas: '1500000000',
      paymasterAndData: '0x',
    };
    const context = {
      chainName: 'Ethereum', chainId: 1, account: SAFE,
      signer: { name: 'Vela', letter: 'V' },
      fee: { kind: 'onchain', native: '~0.0021 ETH', fiat: '≈ $5.40' },
      operation: { userOp },
    };
    const intent = {
      method: 'eth_sendTransaction', origin: 'https://app.uniswap.org',
      params: [{ to: USDC, value: '0x0', data: transferCall }],
    };

    const payload = JSON.stringify({ intent, context });
    const page = await Page.open('https://getvela.app/src/sign.html?ch=url&lang=en#i=' + b64url(payload) +
      '&cb=' + b64url(`http://127.0.0.1:${LOOPBACK_PORT}/vela`) + '&t=tx-1');
    await page.addAuthenticator();
    await sleep(1200);

    const ready = await waitFor(page, '!!window.__slider');
    check('tx: an operation-backed transaction can be signed', ready,
      await page.ev("document.querySelector('.sentence') && document.querySelector('.sentence').textContent"));
    check('tx: the sheet names the SafeOp digest it will sign',
      /SafeOp/.test(String(await page.ev("document.querySelector('.tech-body') && document.querySelector('.tech-body').textContent"))));

    await page.ev('window.__slider.__confirm()', true);
    const got = await (async () => {
      for (let i = 0; i < 60; i++) { if (callbackHit) return true; await sleep(200); }
      return false;
    })();
    check('tx: the signature came back', got);
    if (callbackHit) {
      const result = JSON.parse(Buffer.from(callbackHit.searchParams.get('result'), 'base64url').toString());
      check('tx: it signed the SafeOp digest', result.digestKind === 'SafeOp', result.digest.slice(0, 20) + '…');
      // The whole point: the digest must equal what the core computes.
      check('tx: that digest matches vela-core', result.digest === expectedSafeOpHash(userOp, 1),
        expectedSafeOpHash(userOp, 1).slice(0, 20) + '…');
    }
  }

  // === 3c. the operation does not contain what the site asked for ===========
  {
    callbackHit = null;
    const USDC = '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48';
    const ALICE = '0xaF5e8917831Ef08A64e18b2Cde9f8f5d32c7b3e1';
    const ATTACKER = '0x9A8b7C6d5E4F3a2B1c0D9e8F7a6B5c4D3e2F1a09';
    const SAFE = '0x88cCA0f8B4E1F0dC0e7C4f9a2B3d5E6f7A8b6894';
    const asked = lib.encode.call('transfer(address,uint256)', [ALICE, 1000000000n]);
    const swapped = lib.encode.call('transfer(address,uint256)', [ATTACKER, 1000000000n]);
    const context = {
      chainName: 'Ethereum', chainId: 1, account: SAFE,
      operation: {
        userOp: {
          sender: SAFE, nonce: '0x7', initCode: '0x',
          callData: lib.encode.call('executeUserOp(address,uint256,bytes,uint8)', [USDC, 0n, swapped, 0]),
          verificationGasLimit: '300000', callGasLimit: '200000', preVerificationGas: '110000',
          maxFeePerGas: '1', maxPriorityFeePerGas: '1', paymasterAndData: '0x',
        },
      },
    };
    const intent = {
      method: 'eth_sendTransaction', origin: 'https://app.uniswap.org',
      params: [{ to: USDC, value: '0x0', data: asked }],
    };
    const page = await Page.open('https://getvela.app/src/sign.html?ch=url&lang=en#i=' +
      b64url(JSON.stringify({ intent, context })));
    await sleep(1400);

    const refused = await page.ev("document.querySelector('.slide').classList.contains('slide-off')");
    const warnings = String(await page.ev("[...document.querySelectorAll('.warning-text')].map(n => n.textContent).join(' | ')"));
    check('tamper: a swapped recipient inside the operation is caught', refused && /altered during assembly/.test(warnings));
    check('tamper: the sheet shows the operation\'s OWN recipient, not the requested one',
      /0x9A8b|0x9a8b/.test(String(await page.ev("document.body.textContent"))));
  }

  // === 3d. the same tamper, in a real Vela operation: the call AND the fee ==
  //
  // Every operation the wallet builds is a MultiSend of the calls plus the
  // fee leg, so the refusal above must survive a batch — it once did not:
  // reading the legs overwrote it and the slide came back (desktop e2e).
  {
    const USDC = '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48';
    const ALICE = '0xaF5e8917831Ef08A64e18b2Cde9f8f5d32c7b3e1';
    const ATTACKER = '0x9A8b7C6d5E4F3a2B1c0D9e8F7a6B5c4D3e2F1a09';
    const RELAYER = '0xee2cca98ecbff34663591a925968fa4db5a1f0dd';
    const SAFE = '0x88cCA0f8B4E1F0dC0e7C4f9a2B3d5E6f7A8b6894';
    const asked = lib.encode.call('transfer(address,uint256)', [ALICE, 1000000000n]);
    const swapped = lib.encode.call('transfer(address,uint256)', [ATTACKER, 1000000000n]);
    const leg = (to, value, data) => {
      const body = data.replace(/^0x/, '');
      return '00' + to.replace(/^0x/, '').toLowerCase() +
        value.toString(16).padStart(64, '0') + (body.length / 2).toString(16).padStart(64, '0') + body;
    };
    const packed = '0x' + leg(USDC, 0n, swapped) + leg(RELAYER, 10n ** 16n, '0x');
    const batch = lib.encode.call('multiSend(bytes)', [packed]);
    const context = {
      chainName: 'Ethereum', chainId: 1, account: SAFE,
      operation: {
        feeLegIndex: 1,
        userOp: {
          sender: SAFE, nonce: '0x7', initCode: '0x',
          callData: lib.encode.call('executeUserOp(address,uint256,bytes,uint8)', [lib.safeop ? lib.safeop.MULTI_SEND : '0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526', 0n, batch, 1]),
          verificationGasLimit: '300000', callGasLimit: '200000', preVerificationGas: '110000',
          maxFeePerGas: '1', maxPriorityFeePerGas: '1', paymasterAndData: '0x',
        },
      },
    };
    const intent = {
      method: 'wallet_sendCalls', origin: 'https://app.uniswap.org',
      params: [{ version: '1.0', chainId: '0x1', from: SAFE, calls: [{ to: USDC, value: '0x0', data: asked }] }],
    };
    const page = await Page.open('https://getvela.app/src/sign.html?ch=url&lang=en#i=' +
      b64url(JSON.stringify({ intent, context })));
    await sleep(1400);
    const refused = await page.ev("document.querySelector('.slide').classList.contains('slide-off')");
    const warnings = String(await page.ev("[...document.querySelectorAll('.warning-text')].map(n => n.textContent).join(' | ')"));
    check('tamper: caught in a two-leg operation (the call and the fee) too', refused && /altered during assembly/.test(warnings),
      warnings.slice(0, 80));
  }

  // === 5 and 4 · the WebSocket and the extension: not driven from here ====
  //
  // The phones' loopback WebSocket used to be exercised here, with the app's
  // side being vela-core's own connection over wasm. Spec 075 cut the Clear
  // Signer from the WEB wallet, and with it the wasm export this harness
  // drove (`TrustedSignerWs`) — the phones reach that code through UniFFI, not
  // wasm. It is tested where it runs: Android's `TrustedSignerChannelTest` and
  // `TrustedSignerCeremonyTest`, against the same core.
  //
  // The extension channel is gone outright (owner, 2026-09-23): the Clear
  // Signer is reached only by the Android, iOS and desktop clients, in this
  // device's own browser, so the page is a page and not also an MV3
  // extension.
} catch (error) {
  console.log('FAILED: ' + error.message);
  process.exitCode = 1;
} finally {
  chrome.kill();
  tls.kill();
  loopback.close();
  try { rmSync(profile, { recursive: true, force: true }); } catch { /* ignore */ }
  const passed = results.filter(Boolean).length;
  console.log(`\n${passed}/${results.length} checks passed`);
  if (passed !== results.length) process.exitCode = 1;
}
