// "What you see is what you sign", enforced mechanically.
//
// Every field a requester controls is filled with a marker string. The sheet is
// then rendered for real, and the page's own text is searched:
//
//   · a marker that appears where it would be READ AS FACT fails the test;
//   · a fact that is missing — the recipient in the calldata, the amount, the
//     fee leg — also fails it.
//
// The point is that grit stops being something a human has to spot. Add a new
// display field and forget where it came from, and this goes red.
//
// The second half (spec 075) attacks the key ceremonies on the live page:
//
//   · a sign-in, a proof or a member proof whose challenge the requester tried
//     to supply — refused before any passkey prompt;
//   · a create (and a member proof) from a site that is not a Vela wallet,
//     even one that dresses its context up as an app channel;
//   · a tunnel requester whose key does not hash to the link's `rk` — the page
//     shows no code, sends nothing sealed, and leaves the room;
//   · a member proof whose registry answer is not the challenge for the
//     inputs on the card — refused, never signable.
//
//   CHROME_BIN=… SB=… node samples/hostile-test.mjs
import { spawn } from 'node:child_process';
import { webcrypto } from 'node:crypto';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..');
const SB = process.env.SB;
const PORT = 8443;
const CDP = 9398;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

globalThis.window = globalThis;
for (const file of ['lib/keccak.js', 'lib/abi.js', 'lib/encode.js']) {
  (0, eval)(readFileSync(join(root, file), 'utf8'));
}
const lib = globalThis.VelaCS;

import { Page, b64url, loopbackWallet, tunnelWallet, startBrowser } from './test-kit.mjs';
import { startRegistry } from './mock-registry.mjs';
import { startTunnel } from './mock-tunnel.mjs';

const results = [];
function check(name, pass, detail) {
  results.push(pass);
  console.log(`${pass ? 'PASS' : 'FAIL'}  ${name}${detail ? '  — ' + detail : ''}`);
}

// --- the poisoned request ----------------------------------------------------

const SAFE = '0x88cCA0f8B4E1F0dC0e7C4f9a2B3d5E6f7A8b6894';
const USDC = '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48';
const REAL_RECIPIENT = '0x9A8b7C6d5E4F3a2B1c0D9e8F7a6B5c4D3e2F1a09';
const RELAYER = '0x4d2C7a3B1e9F0a8b7C6d5E4f3A2b1C0d9E8f7A6b';
const MULTI_SEND = '0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526';

// The real payment, and the real fee leg, both inside the signed calldata.
const transferCall = lib.encode.call('transfer(address,uint256)', [REAL_RECIPIENT, 7654321000n]);
const feeCall = lib.encode.call('transfer(address,uint256)', [RELAYER, 420000n]);
const packed = lib.encode.packMultiSend([
  { to: USDC, value: 0, data: transferCall },
  { to: USDC, value: 0, data: feeCall },
]);
const userOp = {
  sender: SAFE,
  nonce: '0x7',
  initCode: '0x',
  callData: lib.encode.call('executeUserOp(address,uint256,bytes,uint8)', [
    MULTI_SEND, 0n, lib.encode.call('multiSend(bytes)', [packed]), 1,
  ]),
  // Zero, as they are on Vela: the fee is the leg above, not gas.
  verificationGasLimit: '0', callGasLimit: '0', preVerificationGas: '0',
  maxFeePerGas: '0', maxPriorityFeePerGas: '0', paymasterAndData: '0x',
};

const hostile = {
  version: 1,
  cases: [{
    code: 'HOSTILE',
    title: { zh: '敌意上下文', en: 'Hostile context' },
    intent: {
      method: 'eth_sendTransaction',
      origin: 'https://app.uniswap.org',
      params: [{ to: USDC, value: '0x0', data: transferCall }],
    },
    context: {
      chainId: 1,
      account: SAFE,
      // Every one of these is the requester's to choose.
      chainName: 'POISONCHAIN',
      contacts: { [REAL_RECIPIENT.toLowerCase()]: { name: 'POISONCONTACT', kind: 'contact' } },
      seenAddresses: [REAL_RECIPIENT.toLowerCase()],
      fee: { kind: 'onchain', native: 'POISONFEE ETH', fiat: '≈ $POISONFIAT' },
      digest: '0xdeadbeef'.padEnd(66, '0'),
      signer: { name: 'Daily wallet' },   // allowed: it points at a passkey
      dapp: { name: 'Uniswap', tone: '#ff007a' },
      rates: { USDC: 1 },
      currency: '$',
      simulation: {
        rows: [{ symbol: 'USDC', delta: '-7,654.321' }],
        note: 'POISONSIM',
      },
      operation: { userOp, feeLegIndex: 1 },
    },
  }],
};

writeFileSync(join(root, 'samples/hostile-intents.json'), JSON.stringify(hostile, null, 2) + '\n');

// --- render it for real ------------------------------------------------------

const profile = mkdtempSync(join(tmpdir(), 'hostile-'));
const tls = spawn('python3', [join(SB, 'tls-serve.py'), root, String(PORT), join(SB, 'cert.pem'), join(SB, 'key.pem')], { stdio: 'ignore' });
const chrome = spawn(process.env.CHROME_BIN, [
  '--headless=new', `--remote-debugging-port=${CDP}`, `--user-data-dir=${profile}`,
  `--host-resolver-rules=MAP getvela.app:443 127.0.0.1:${PORT}`,
  '--ignore-certificate-errors', '--no-proxy-server',
  '--no-first-run', '--no-default-browser-check', 'about:blank',
], { stdio: 'ignore' });

try {
  for (let i = 0; i < 80; i++) {
    try { await (await fetch(`http://127.0.0.1:${CDP}/json/version`)).json(); break; } catch { await sleep(250); }
  }
  const url = 'https://getvela.app/gallery.html?lang=zh&src=samples/hostile-intents.json';
  const tab = await (await fetch(
    `http://127.0.0.1:${CDP}/json/new?${encodeURIComponent(url)}`, { method: 'PUT' },
  )).json();
  const ws = new WebSocket(tab.webSocketDebuggerUrl);
  await new Promise((resolve) => ws.addEventListener('open', resolve));
  let id = 0;
  const pending = new Map();
  ws.addEventListener('message', (event) => {
    const message = JSON.parse(event.data);
    if (pending.has(message.id)) { pending.get(message.id)(message.result); pending.delete(message.id); }
  });
  const send = (method, params = {}) => new Promise((resolve) => {
    const i = ++id; pending.set(i, resolve); ws.send(JSON.stringify({ id: i, method, params }));
  });
  await send('Runtime.enable');
  await sleep(2000);

  const ev = async (expression) => {
    const r = await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
    if (r.exceptionDetails) throw new Error(r.exceptionDetails.text);
    return r.result.value;
  };

  // Open every collapsed panel, so nothing hides behind a disclosure triangle.
  await ev(`[...document.querySelectorAll('details')].forEach(d => d.open = true);
            [...document.querySelectorAll('.identity')].forEach(b => b.click());
            document.body.textContent.length`);
  const text = await ev('document.body.innerText');
  const rendered = await ev("!!document.querySelector('.sheet')");
  if (!rendered) console.log('  page said:', String(await ev('document.body.innerText')).slice(0, 300));
  check('the hostile request still renders', rendered);

  // 1. markers that must never be read as fact
  const forbidden = {
    'a contact name for the recipient': 'POISONCONTACT',
    'a pre-rendered fee amount': 'POISONFEE',
    'a pre-rendered fiat total': 'POISONFIAT',
    'a chain name over a known chain id': 'POISONCHAIN',
  };
  for (const [what, marker] of Object.entries(forbidden)) {
    check(`never shown: ${what}`, !text.includes(marker));
  }

  // The simulation IS shown — it is often the only thing that can be shown —
  // but it must be labelled as the requester's, not as this page's finding.
  check('a supplied simulation is shown, and marked as supplied',
    text.includes('POISONSIM') && /模拟由请求方提供/.test(text));

  // 2. facts that must be present, and they come from the calldata
  check('the recipient from the calldata is shown', text.includes('0x9A8b') || text.includes('0x9a8b'));
  check('the amount from the calldata is shown', text.includes('7,654.321'));
  check('the fee leg amount is read out of the calldata', text.includes('0.42'));
  check('the fee is attributed to the requester, not asserted',
    /请求方的说法|requester/.test(text));
  check('the chain is named from its id', text.includes('Ethereum'));
  check('the signing account is shown as an address', text.includes('0x88cC'));
  check('the account name is shown (it points at a passkey)', text.includes('Daily wallet'));
  check('the full address is available to copy', text.includes('复制完整地址'));
  check('the self-reported origin is called out', /自述|self-reported/.test(text));
} catch (error) {
  console.log('FAILED: ' + error.message);
  process.exitCode = 1;
} finally {
  chrome.kill();
  tls.kill();
  try { rmSync(profile, { recursive: true, force: true }); } catch { /* ignore */ }
}

// === the key ceremonies under attack (075) =====================================

// Counts every passkey prompt the page opens: a refusal must come before one.
const COUNT_WEBAUTHN = `(() => {
  window.__webauthnCalls = 0;
  const c = navigator.credentials;
  const get = c.get.bind(c), create = c.create.bind(c);
  c.get = (o) => { window.__webauthnCalls++; return get(o); };
  c.create = (o) => { window.__webauthnCalls++; return create(o); };
})();`;

const SIGNER = 'https://getvela.app/sign.html';
const SAFE_OP_HASH = '0x' + 'a1'.repeat(32); // stands for any 32 bytes a Safe would accept
const ns075 = globalThis.VelaCS;
for (const file of ['lib/signer.js', 'lib/ceremony.js', 'lib/transport/secure.js']) {
  (0, eval)(readFileSync(join(root, file), 'utf8'));
}
Object.defineProperty(globalThis, 'crypto', { value: webcrypto, configurable: true });

const browser = await startBrowser({ cdp: 9397, tlsPort: 8447, hosts: ['evil.test'] });
const registry = await startRegistry({ ns: ns075 });
const tunnel = await startTunnel();
try {
  const page = await Page.open(9397, 'about:blank');
  await page.send('Page.enable');
  await page.send('Page.addScriptToEvaluateOnNewDocument', { source: COUNT_WEBAUTHN });
  await page.addAuthenticator();

  // --- 1. challenges the requester tried to supply, over the wallet's own channel
  {
    const wallet = loopbackWallet({ token: 'tok-hostile' });
    await wallet.listening;
    await page.navigate(`${SIGNER}?ch=ws&lang=en#p=${wallet.port}&t=tok-hostile`);
    await wallet.hello;
    const credentialId = b64url(new Uint8Array(16).fill(7));
    const attempts = [
      ['a sign-in carrying params[0].challenge', { method: 'vela_signIn', params: [{ challenge: b64url(Buffer.from(SAFE_OP_HASH.slice(2), 'hex')) }], origin: '' }, {}],
      ['a sign-in carrying context.challenge', { method: 'vela_signIn', params: [{}], origin: '' }, { challenge: SAFE_OP_HASH }],
      ['a sign-in carrying a digest', { method: 'vela_signIn', params: [{ digest: SAFE_OP_HASH }], origin: '' }, {}],
      ['a proof with the bytes as a second parameter', { method: 'vela_proof', params: [{ credentialId, purpose: 'verify' }, SAFE_OP_HASH], origin: '' }, {}],
      ['a member proof carrying its own challenge', { method: 'vela_memberProof', params: [{ credentialId, publicKey: '04' + '11'.repeat(64), groupPublicKey: '04' + '22'.repeat(64), registry: registry.url, challenge: SAFE_OP_HASH }], origin: '' }, {}],
    ];
    const before = registry.requests.length;
    let n = 0;
    for (const [what, intent, context] of attempts) {
      const answer = await wallet.request('h' + (++n), intent, context);
      await page.waitFor("window.__velaState.phase === 'refused'");
      const text = await page.text();
      check(`supplied challenge: ${what} — refused, and the page says why`,
        answer.t === 'error' && answer.code === 'refused' && /tried to supply the challenge/.test(text) &&
        !(await page.ev('!!window.__slider')));
    }
    check('supplied challenge: not one passkey prompt was opened', (await page.ev('window.__webauthnCalls')) === 0);
    check('supplied challenge: the registry was never even asked', registry.requests.length === before);

    // --- 4. the registry's answer is not the challenge for the inputs shown
    const inputs = { credentialId, publicKey: '04' + '11'.repeat(64), groupPublicKey: '04' + '22'.repeat(64), attestation: '', registry: registry.url };
    for (const [lie, what] of [
      ['otherGroup', 'the challenge for another group key'],
      ['otherKey', 'the challenge for another member key'],
      ['safeOp', '32 bytes that are no registry challenge (a Safe digest)'],
      ['binding', 'the right challenge beside a wrong binding'],
    ]) {
      registry.state.lie = lie;
      const answer = await wallet.request('m' + (++n), { method: 'vela_memberProof', params: [inputs], origin: '' }, { walletName: 'Mine' });
      await page.waitFor("window.__velaState.phase === 'refused'");
      check(`member proof: a registry answering ${what} is refused`,
        answer.t === 'error' && answer.code === 'refused' &&
        /not the one these keys give/.test(await page.text()) && !(await page.ev('!!window.__slider')));
    }
    registry.state.lie = 'down';
    const down = await wallet.request('m' + (++n), { method: 'vela_memberProof', params: [inputs], origin: '' }, {});
    check('member proof: a registry that does not answer is "unavailable", never signed',
      down.t === 'error' && down.code === 'unavailable' && /did not answer/.test(await page.text()));
    registry.state.lie = null;
    check('member proof: still not one passkey prompt', (await page.ev('window.__webauthnCalls')) === 0);
    wallet.bye();
    await wallet.stop();
  }

  // --- 2. a create from a site that is not a Vela wallet
  {
    const signerUrl = encodeURIComponent(`${SIGNER}?ch=post&lang=en`);
    const evil = await Page.open(9397, `https://evil.test/samples/wallet-sim.html?signer=${signerUrl}`);
    await evil.waitFor('!!window.__open');
    await evil.ev('window.__open().then(() => true)', true);
    const popup = await Page.find(9397, (u) => u.includes('sign.html?ch=post'));
    await popup.addAuthenticator();
    // It even dresses its context up as the wallet's own app channel.
    await evil.ev(`window.__request('e1', { method: 'vela_createPasskey', params: [{ name: 'Evil' }], origin: '' },
      { walletName: 'Your Vela wallet', channel: 'ws', originVerified: true, requester: 'https://getvela.app' }); true`);
    await evil.waitFor('window.__answers.length === 1', 10000);
    const refusedCreate = await evil.ev('window.__answers[0]');
    const text = await popup.text();
    check('create from evil.test: refused, and the site hears "refused"',
      refusedCreate && refusedCreate.vela === 'error' && refusedCreate.code === 'refused');
    check('create from evil.test: the card says only a Vela wallet may ask, and names the real site',
      /Only a Vela wallet may ask this page to create a key/.test(text) && text.includes('evil.test'));
    const made = await popup.send('WebAuthn.getCredentials', { authenticatorId: popup.authenticatorId });
    check('create from evil.test: no key exists afterwards', made.credentials.length === 0 && !(await popup.ev('!!window.__slider')));

    await evil.ev(`window.__request('e2', { method: 'vela_memberProof', params: [{ credentialId: '${b64url(new Uint8Array(16))}', publicKey: '04${'11'.repeat(64)}', groupPublicKey: '04${'22'.repeat(64)}', registry: '${registry.url}' }], origin: '' }, {}); true`);
    await evil.waitFor('window.__answers.length === 2', 10000);
    check('member proof from evil.test: refused as well', (await evil.ev('window.__answers[1].code')) === 'refused' &&
      /registry confirmation/.test(await popup.text()));

    await evil.ev(`window.__request('e3', { method: 'vela_signIn', params: [{}], origin: '' }, {}); true`);
    await popup.waitFor("window.__velaState.phase === 'card' && window.__velaState.kind === 'signIn'");
    check('a sign-in from evil.test is shown — with a warning that no Vela wallet is asking',
      /did not come from a Vela wallet/.test(await popup.text()));
    await popup.close();
    await evil.close();

    // Over the URL fragment any site can open the page: never a wallet channel.
    const payload = Buffer.from(JSON.stringify({
      intent: { method: 'vela_createPasskey', params: [{ name: 'X' }], origin: '' },
      context: { channel: 'ws', walletName: 'X' },
    })).toString('base64url');
    const viaUrl = await Page.open(9397, `${SIGNER}?ch=url&lang=en#i=${payload}`);
    await viaUrl.waitFor("window.__velaState && window.__velaState.phase === 'refused'");
    check('create over a URL fragment: refused, whatever its context claims',
      /Only a Vela wallet may ask/.test(await viaUrl.text()) &&
      (await viaUrl.ev("document.querySelector('.slide').classList.contains('slide-off')")));
    await viaUrl.close();
  }

  // --- 3. a tunnel requester whose key does not match the link
  {
    const room = b64url(webcrypto.getRandomValues(new Uint8Array(16)));
    const someoneElse = await ns075.transport.secure.handshake({ role: 'requester' });
    const linkRk = await ns075.transport.secure.fingerprint(someoneElse.publicKey);
    const impostor = await tunnelWallet({ ns: ns075, tunnelUrl: tunnel.url, room, rk: linkRk });
    await impostor.connect();
    await page.navigate(`${SIGNER}?ch=relay&lang=en&s=h#relay=${encodeURIComponent(tunnel.url)}&room=${room}&rk=${linkRk}&v=1`);
    const ended = await page.waitFor("window.__velaState.phase === 'ended'", 10000);
    const text = await page.text();
    check('wrong rk: the page refuses the requester and says so',
      ended && /not the wallet that made this link/.test(text), (await page.status()) || '');
    check('wrong rk: no code is ever shown', !(await page.ev('window.__velaState.code')) &&
      !(await page.ev("!!document.querySelector('.pairing-code')")));
    const fromPage = tunnel.tap.filter((f) => f.room === room && f.from === 'signer');
    check('wrong rk: the page sent its hello and nothing sealed', fromPage.length === 1 && !fromPage[0].isBinary);
    check('wrong rk: the page left the room', await (async () => {
      for (let i = 0; i < 40; i++) {
        if (impostor.tunnelFrames.includes('left')) return true;
        await new Promise((r) => setTimeout(r, 100));
      }
      return false;
    })());
    impostor.leave();
  }
  const errors = page.console.filter((line) => !/favicon|ERR_NAME_NOT_RESOLVED|Failed to load resource/.test(line));
  check('the page threw nothing along the way', errors.length === 0, errors.slice(0, 2).join(' | '));
} catch (error) {
  console.log('FAILED: ' + (error.stack || error.message));
  results.push(false);
} finally {
  browser.kill();
  await registry.close();
  await tunnel.close();
  const passed = results.filter(Boolean).length;
  console.log(`\n${passed}/${results.length} checks passed`);
  process.exit(passed === results.length ? 0 : 1);
}
