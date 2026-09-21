// A desktop app asking for a signature — the whole loop, on one machine, with
// no server anywhere.
//
//   node samples/desktop-demo.mjs             一笔 ERC-20 转账
//   node samples/desktop-demo.mjs --message   一条 SIWE 登录消息
//   node samples/desktop-demo.mjs --tamper    组装时被换掉收款方（应当拒签）
//   node samples/desktop-demo.mjs --auto      无人值守（用 CHROME_BIN + 虚拟认证器）
//
// What it does, in the order a real native app would:
//   1. assemble the operation and a one-time token
//   2. listen on a loopback port for the answer
//   3. open the default browser at the signing page (localhost is a valid
//      rpId and a secure context, so the passkey ceremony is real)
//   4. verify what comes back — the signature, the challenge binding, and that
//      the digest is the one vela-core would have computed
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { webcrypto } from 'node:crypto';
import { mkdtempSync, readFileSync, rmSync, statSync, writeFileSync, readdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, extname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..');
const repo = join(root, '..', '..');

const flags = new Set(process.argv.slice(2));
const MESSAGE_MODE = flags.has('--message');
const TAMPER = flags.has('--tamper');
const AUTO = flags.has('--auto');

const PAGE_PORT = Number(process.env.PAGE_PORT || 8099);
const CALLBACK_PORT = Number(process.env.CALLBACK_PORT || 8477);
const CDP_PORT = 9395;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const b64url = (s) => Buffer.from(s).toString('base64url');
const unb64url = (s) => new Uint8Array(Buffer.from(s, 'base64url'));
const hex = (b) => '0x' + Buffer.from(b).toString('hex');

// --- the page's own encoders, and vela-core for the reference digest --------

globalThis.window = globalThis;
Object.defineProperty(globalThis, 'crypto', { value: webcrypto, configurable: true });
for (const file of ['lib/keccak.js', 'lib/abi.js', 'lib/encode.js']) {
  (0, eval)(readFileSync(join(root, file), 'utf8'));
}
const lib = globalThis.VelaCS;

const core = await import(join(repo, 'rust/pkg-web/vela_core.js'));
// The committed build, whatever its content hash is this week.
const wasmDir = join(repo, 'assets/wasm');
const wasmFile = readdirSync(wasmDir).find((name) => /^vela_core_bg\..*\.wasm$/.test(name));
core.initSync({ module: readFileSync(join(wasmDir, wasmFile)) });

// --- 1. assemble the request ------------------------------------------------

const SAFE = '0x88cCA0f8B4E1F0dC0e7C4f9a2B3d5E6f7A8b6894';
const USDC = '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48';
const ALICE = '0xaF5e8917831Ef08A64e18b2Cde9f8f5d32c7b3e1';
const ATTACKER = '0x9A8b7C6d5E4F3a2B1c0D9e8F7a6B5c4D3e2F1a09';
const RELAYER = '0x4d2C7a3B1e9F0a8b7C6d5E4f3A2b1C0d9E8f7A6b';
const MULTI_SEND = '0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526';
const CHAIN_ID = 1;
const token = 'demo-' + Buffer.from(webcrypto.getRandomValues(new Uint8Array(9))).toString('hex');

function transactionRequest() {
  const asked = lib.encode.call('transfer(address,uint256)', [ALICE, 1000000000n]);
  // The tamper case swaps the recipient AFTER the site asked — exactly the
  // attack the sheet exists to catch.
  const executed = TAMPER
    ? lib.encode.call('transfer(address,uint256)', [ATTACKER, 1000000000n])
    : asked;

  // Vela pays IN BAND: the fee is an extra leg inside the calldata, so the
  // operation is a MultiSend of [the transfer, the fee].
  const feeLeg = lib.encode.call('transfer(address,uint256)', [RELAYER, 420000n]);
  const packed = lib.encode.packMultiSend([
    { to: USDC, value: 0, data: executed },
    { to: USDC, value: 0, data: feeLeg },
  ]);
  const userOp = {
    sender: SAFE,
    nonce: '0x7',
    initCode: '0x',
    callData: lib.encode.call('executeUserOp(address,uint256,bytes,uint8)', [
      MULTI_SEND, 0n, lib.encode.call('multiSend(bytes)', [packed]), 1,
    ]),
    verificationGasLimit: '300000',
    callGasLimit: '200000',
    preVerificationGas: '110000',
    maxFeePerGas: '1500000007',
    maxPriorityFeePerGas: '1500000000',
    paymasterAndData: '0x',
  };

  return {
    intent: {
      method: 'eth_sendTransaction',
      origin: 'https://app.uniswap.org',
      params: [{ to: USDC, value: '0x0', data: asked }],
    },
    context: {
      chainId: CHAIN_ID,
      chainName: 'Ethereum',
      account: SAFE,
      signer: { name: 'Vela Desktop', letter: 'V' },
      dapp: { name: 'Uniswap', tone: '#ff007a' },
      fee: { kind: 'onchain', native: '~0.0021 ETH', fiat: '≈ $5.40' },
      seenAddresses: [ALICE.toLowerCase(), USDC.toLowerCase()],
      rates: { ETH: 3400, USDC: 1 },
      currency: '$',
      // Which leg the requester CALLS the fee. The amount and recipient are
      // still read from the calldata; only the label comes from here.
      operation: { userOp, feeLegIndex: 1 },
    },
    userOp,
  };
}

function messageRequest() {
  const siwe = 'localhost wants you to sign in with your Ethereum account:\n' +
    SAFE + '\n\nSign in to the Vela desktop demo.\n\n' +
    'URI: http://localhost\nVersion: 1\nChain ID: 1\nNonce: ' + token.slice(5, 13) + '\n' +
    'Issued At: ' + new Date().toISOString();
  return {
    intent: {
      method: 'personal_sign',
      origin: 'http://localhost:' + PAGE_PORT,
      params: ['0x' + Buffer.from(siwe, 'utf8').toString('hex'), SAFE],
    },
    context: {
      chainId: CHAIN_ID,
      chainName: 'Ethereum',
      account: SAFE,
      signer: { name: 'Vela Desktop', letter: 'V' },
    },
    userOp: null,
  };
}

const request = MESSAGE_MODE ? messageRequest() : transactionRequest();

// --- 2. the loopback listener (this is the whole "server", and it is ours) ---

let answered = null;
let enrolled = null;   // { credentialId, publicKey } — set by the enrolment hop
const callback = createServer((incoming, response) => {
  const url = new URL(incoming.url, `http://127.0.0.1:${CALLBACK_PORT}`);
  if (url.pathname === '/enrolled') {
    enrolled = {
      credentialId: url.searchParams.get('credentialId'),
      publicKey: url.searchParams.get('publicKey'),
    };
    response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
    response.end('<meta charset="utf-8"><p style="font:16px system-ui;padding:40px">' +
      '钥匙已登记，可以关掉这个标签页了。</p>');
    return;
  }
  if (url.pathname === '/vela') {
    answered = url;
    response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
    response.end('<meta charset="utf-8"><p style="font:16px system-ui;padding:40px">' +
      '签名已回到桌面应用，可以关掉这个标签页了。</p>');
    return;
  }
  response.writeHead(404).end();
});
callback.listen(CALLBACK_PORT, '127.0.0.1');

// A static server for the page itself. In production this is getvela.app; on a
// developer's machine localhost is a perfectly good relying party.
const TYPES = {
  '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8', '.json': 'application/json; charset=utf-8',
  '.svg': 'image/svg+xml', '.png': 'image/png',
};
const pageServer = createServer((incoming, response) => {
  const path = normalize(join(root, decodeURIComponent(incoming.url.split('?')[0].split('#')[0])));
  if (!path.startsWith(root)) {
    response.writeHead(403).end();
    return;
  }
  try {
    if (statSync(path).isDirectory()) throw new Error('directory');
    response.writeHead(200, { 'content-type': TYPES[extname(path)] || 'application/octet-stream' });
    response.end(readFileSync(path));
  } catch {
    response.writeHead(404).end('not found');
  }
});
pageServer.listen(PAGE_PORT, '127.0.0.1');

// --- 3. make sure a key EXISTS, then hand the request to the browser -------
//
// Signing never creates a key: a new key is a new account, so a signing page
// that enrols would hand back a signature from a wallet nobody owns. Enrolment
// is therefore its own page, its own ceremony, and its own step here.

const KEY_FILE = join(tmpdir(), `vela-demo-key-${PAGE_PORT}.json`);

let chrome = null;
let profile = null;
let authenticatorId = null;
let cdp = null;

if (AUTO) {
  profile = mkdtempSync(join(tmpdir(), 'demo-'));
  chrome = spawn(process.env.CHROME_BIN, [
    '--headless=new', `--remote-debugging-port=${CDP_PORT}`, `--user-data-dir=${profile}`,
    '--no-first-run', '--no-default-browser-check', 'about:blank',
  ], { stdio: 'ignore' });
  for (let i = 0; i < 80; i++) {
    try { await (await fetch(`http://127.0.0.1:${CDP_PORT}/json/version`)).json(); break; } catch { await sleep(250); }
  }
}

const key = await ensureKey();
console.log('  钥匙     :', key.credentialId.slice(0, 16) + '… (' + key.source + ')');
request.context.allowCredentials = [key.credentialId];

const payload = JSON.stringify({ intent: request.intent, context: request.context });
const signUrl = `http://localhost:${PAGE_PORT}/sign.html?ch=url&lang=zh` +
  `#i=${b64url(payload)}&cb=${b64url(`http://127.0.0.1:${CALLBACK_PORT}/vela`)}&t=${token}`;

console.log('');
console.log('  桌面应用 → 浏览器签名页');
console.log('  意图     :', MESSAGE_MODE ? 'personal_sign（SIWE 登录）' : 'eth_sendTransaction（1,000 USDC）');
if (TAMPER) console.log('  注意     : 组装时把收款方换掉了 —— 签名页应当拒签');
console.log('  回调     : http://127.0.0.1:' + CALLBACK_PORT + '/vela');
console.log('  一次性码 :', token);
console.log('  URL 长度 :', signUrl.length, '字节');
console.log('');

if (AUTO) await driveHeadless(signUrl);
else openInBrowser(signUrl, '正在打开默认浏览器签名…');

// --- 4. wait, then check what came back -------------------------------------

const deadline = Date.now() + (AUTO ? 60000 : 180000);
while (!answered && Date.now() < deadline) await sleep(200);

if (!answered) {
  console.log('  超时：签名页没有回调。');
  await shutdown(1);
}

console.log('  回调收到 :', answered.pathname + '?' + [...answered.searchParams.keys()].join('&'));

const checks = [];
function check(name, pass, detail) {
  checks.push(pass);
  console.log(`  ${pass ? '✔' : '✘'} ${name}${detail ? '  — ' + detail : ''}`);
}

check('一次性码原样回传', answered.searchParams.get('t') === token);

if (answered.searchParams.get('error')) {
  console.log('');
  console.log('  签名页拒签：', answered.searchParams.get('error'));
  const code = answered.searchParams.get('error');
  console.log(code === 'refused'
    ? '  理由：签名页的规则拒绝了它（不是人点的拒绝）。'
    : '  理由：人没有滑动确认。');
  console.log(TAMPER
    ? '  这正是预期结果 —— 被换掉收款方的操作没有被签。'
    : '');
  await shutdown(TAMPER && code === 'refused' ? 0 : 1);
}

const result = JSON.parse(Buffer.from(answered.searchParams.get('result'), 'base64url').toString());

// 4a. the digest is the one the core would have computed
if (request.userOp) {
  const reference = hex(core.attestSafeOpHash(JSON.stringify({
    sender: request.userOp.sender, nonce: request.userOp.nonce,
    init_code_hex: request.userOp.initCode, call_data_hex: request.userOp.callData,
    verification_gas_limit: request.userOp.verificationGasLimit,
    call_gas_limit: request.userOp.callGasLimit,
    pre_verification_gas: request.userOp.preVerificationGas,
    max_fee_per_gas: request.userOp.maxFeePerGas,
    max_priority_fee_per_gas: request.userOp.maxPriorityFeePerGas,
    paymaster_and_data_hex: request.userOp.paymasterAndData,
  }), '', BigInt(CHAIN_ID)));
  check('摘要 = vela-core 算出的 SafeOp 哈希', result.digest === reference, reference.slice(0, 22) + '…');
}

// 4b. the authenticator signed OUR digest, not something else
const authenticatorData = Buffer.from(result.authenticatorData.slice(2), 'hex');
const clientDataBytes = Buffer.from(result.clientDataJSON.slice(2), 'hex');
const clientData = JSON.parse(clientDataBytes.toString('utf8'));

check('clientData.type 是 webauthn.get', clientData.type === 'webauthn.get');
check('challenge 就是这一份摘要',
  clientData.challenge === Buffer.from(result.digest.slice(2), 'hex').toString('base64url'),
  clientData.challenge.slice(0, 18) + '…');

// 4c. the signature actually verifies against the public key the page returned
const clientHash = new Uint8Array(await webcrypto.subtle.digest('SHA-256', clientDataBytes));
const signed = Buffer.concat([authenticatorData, Buffer.from(clientHash)]);
// Verified against the public key the REQUESTER already knows for this
// account — not one the page reported back, which would be circular.
const verifyKey = await webcrypto.subtle.importKey('spki', unb64url(key.publicKey),
  { name: 'ECDSA', namedCurve: 'P-256' }, false, ['verify']);
const valid = await webcrypto.subtle.verify({ name: 'ECDSA', hash: 'SHA-256' }, verifyKey,
  Buffer.from(result.signature.slice(2), 'hex'), signed);
check('签名对账户已知的公钥验证通过（P-256）', valid);
check('回答的正是被允许的那把钥匙', result.credentialId === key.credentialId);
check('用户验证位已置位（UV）', (authenticatorData[32] & 0x04) !== 0);

const rpIdHash = new Uint8Array(await webcrypto.subtle.digest('SHA-256', Buffer.from('localhost')));
check('rpIdHash = sha256("localhost")',
  Buffer.from(authenticatorData.subarray(0, 32)).equals(Buffer.from(rpIdHash)));

console.log('');
const passed = checks.filter(Boolean).length;
console.log(`  ${passed}/${checks.length} 项通过`);
await shutdown(passed === checks.length ? 0 : 1);

// --- plumbing ----------------------------------------------------------------

function openInBrowser(url, note) {
  const opener = process.platform === 'darwin' ? 'open'
    : process.platform === 'win32' ? 'start' : 'xdg-open';
  console.log('  ' + note);
  spawn(opener, [url], { stdio: 'ignore', detached: true, shell: process.platform === 'win32' });
}

/**
 * A key must already exist before anything is signed.
 *
 *   --auto : inject one into the virtual authenticator — the automated stand-in
 *            for "this person enrolled last week".
 *   manual : reuse the one from a previous run, or run the enrolment page once.
 */
async function ensureKey() {
  if (AUTO) {
    const pair = await webcrypto.subtle.generateKey(
      { name: 'ECDSA', namedCurve: 'P-256' }, true, ['sign', 'verify'],
    );
    const pkcs8 = Buffer.from(await webcrypto.subtle.exportKey('pkcs8', pair.privateKey));
    const spki = Buffer.from(await webcrypto.subtle.exportKey('spki', pair.publicKey));
    const rawId = Buffer.from(webcrypto.getRandomValues(new Uint8Array(16)));

    const version = await (await fetch(`http://127.0.0.1:${CDP_PORT}/json/version`)).json();
    cdp = await openCdp(version.webSocketDebuggerUrl);
    // The authenticator has to live on the page's own target, so it is created
    // later in driveHeadless; keep the material for then.
    return {
      credentialId: rawId.toString('base64url'),
      publicKey: spki.toString('base64url'),
      privateKeyPkcs8: pkcs8.toString('base64'),
      rawIdBase64: rawId.toString('base64'),
      source: '注入虚拟认证器',
    };
  }

  try {
    const saved = JSON.parse(readFileSync(KEY_FILE, 'utf8'));
    if (saved.credentialId && saved.publicKey) return { ...saved, source: '沿用上次注册的' };
  } catch { /* first run */ }

  const enrolUrl = `http://localhost:${PAGE_PORT}/samples/enrol.html` +
    `#cb=${b64url(`http://127.0.0.1:${CALLBACK_PORT}/enrolled`)}&t=${token}`;
  openInBrowser(enrolUrl, '这台设备还没有钥匙 —— 先打开注册页创建一把（这一步和签名是分开的）。');
  const until = Date.now() + 180000;
  while (!enrolled && Date.now() < until) await sleep(200);
  if (!enrolled) {
    console.log('  超时：没有完成注册。');
    await shutdown(1);
  }
  writeFileSync(KEY_FILE, JSON.stringify(enrolled));
  return { ...enrolled, source: '刚刚注册' };
}

async function openCdp(url) {
  const ws = new WebSocket(url);
  await new Promise((resolve) => ws.addEventListener('open', resolve));
  let id = 0;
  const pending = new Map();
  ws.addEventListener('message', (event) => {
    const message = JSON.parse(event.data);
    if (pending.has(message.id)) { pending.get(message.id)(message.result); pending.delete(message.id); }
  });
  return {
    send: (method, params = {}) => new Promise((resolve) => {
      const i = ++id; pending.set(i, resolve); ws.send(JSON.stringify({ id: i, method, params }));
    }),
  };
}

async function driveHeadless(url) {
  for (let i = 0; i < 80; i++) {
    try { await (await fetch(`http://127.0.0.1:${CDP_PORT}/json/version`)).json(); break; } catch { await sleep(250); }
  }
  const tab = await (await fetch(
    `http://127.0.0.1:${CDP_PORT}/json/new?${encodeURIComponent(url)}`, { method: 'PUT' },
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
  await send('WebAuthn.enable');
  const authenticator = await send('WebAuthn.addVirtualAuthenticator', {
    options: {
      protocol: 'ctap2', transport: 'internal', hasResidentKey: true,
      hasUserVerification: true, isUserVerified: true, automaticPresenceSimulation: true,
    },
  });
  authenticatorId = authenticator.authenticatorId;
  // The key already exists — the page will not, and must not, create one.
  await send('WebAuthn.addCredential', {
    authenticatorId,
    credential: {
      credentialId: key.rawIdBase64,
      isResidentCredential: true,
      rpId: 'localhost',
      privateKey: key.privateKeyPkcs8,
      userHandle: Buffer.from('vela-demo').toString('base64'),
      signCount: 0,
    },
  });
  console.log('  （--auto：钥匙已注入虚拟认证器，正在替你拖动滑条）');
  for (let i = 0; i < 40; i++) {
    const ready = await send('Runtime.evaluate', { expression: '!!window.__slider', returnByValue: true });
    if (ready.result && ready.result.value) break;
    await sleep(200);
  }
  await send('Runtime.evaluate', { expression: 'window.__slider.__confirm()', userGesture: true });

  // A refused request is answered by CLOSING the sheet — that is the product's
  // only reject affordance. Unattended, close the tab to stand in for the
  // person walking away.
  const refused = await send('Runtime.evaluate', {
    expression: '!!window.__refused',
    returnByValue: true,
  });
  if (refused.result && refused.result.value) {
    await sleep(400);
    await fetch(`http://127.0.0.1:${CDP_PORT}/json/close/${tab.id}`);
  }
}

async function shutdown(code) {
  callback.close();
  pageServer.close();
  if (chrome) chrome.kill();
  if (profile) { try { rmSync(profile, { recursive: true, force: true }); } catch { /* ignore */ } }
  await sleep(50);
  process.exit(code);
}
