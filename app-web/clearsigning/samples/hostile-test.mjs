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
//   CHROME_BIN=… SB=… node samples/hostile-test.mjs
import { spawn } from 'node:child_process';
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
  const passed = results.filter(Boolean).length;
  console.log(`\n${passed}/${results.length} checks passed`);
  if (passed !== results.length) process.exitCode = 1;
}
