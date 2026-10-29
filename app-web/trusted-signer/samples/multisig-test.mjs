// A multisig member's approval on this page (`vela_multisigApprove`), checked
// two ways:
//
//  1. against vela-core's SHIPPED wasm, case by case — the safeTxHash, the
//     domain and message hashes, the approval bytes the member's passkey
//     signs, and the reading (`review`: what the SafeTx does, or why Vela
//     refuses it). The page's copy (lib/multisig.js) is transcribed from the
//     Rust; two readings agreeing is the only reason to believe either. The
//     cases are built by the core itself where it can build them
//     (`transfersTx`, `callsTx`, `memberChangeTx`, `cancelTx`, `renameTx`),
//     so a change to how the core encodes a transaction reaches this test;
//  2. through the page's own pipeline (resolve → digest → render) on the
//     hostile cases: a SafeTx whose fields do not make the hash the app
//     claims, hidden characters in the names the app gives, a huge payload,
//     an opener that is not the web wallet, and every refusal in all fifteen
//     languages.
//
//   node samples/multisig-test.mjs
import { readFileSync, readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { installFakeDom, loadPageLibs, makeChecks } from './test-kit.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const repo = join(here, '..', '..', '..');

const doc = installFakeDom();
const LOCALES = ['en', 'zh', 'zh-TW', 'zh-HK', 'ja', 'ko', 'vi', 'id', 'tr', 'es-MX', 'pt-BR', 'fr', 'de', 'ru', 'it'];
const ns = loadPageLibs([
  'src/lib/i18n.js',
  ...LOCALES.map((l) => `src/lib/locales/${l}.js`),
  'src/lib/catalog.js',
  'src/lib/keccak.js',
  'src/lib/identicon-features.js',
  'src/lib/identicon.js',
  'src/lib/abi.js',
  'src/lib/encode.js',
  'src/lib/fee.js',
  'src/lib/logos.js',
  'src/lib/registry.js',
  'src/lib/resolve.js',
  'src/lib/render.js',
  'src/lib/safeop.js',
  'src/lib/multisig.js',
  'src/lib/digest.js',
]);
const check = makeChecks();

// --- vela-core, as compiled for the web ---------------------------------------

const core = await import(join(repo, 'rust/pkg-web/vela_core.js'));
const wasmDir = join(repo, 'assets/wasm');
const wasmFile = readdirSync(wasmDir).find((name) => /^vela_core_bg\..*\.wasm$/.test(name));
core.initSync({ module: readFileSync(join(wasmDir, wasmFile)) });
const ms = (method, args) => JSON.parse(core.multisigCall(method, JSON.stringify(args)));
/** JSON with every object's keys in order: the core's serde and the page differ only in key order. */
const canonical = (value) => JSON.stringify(value, (_, v) => (v && typeof v === 'object' && !Array.isArray(v)
  ? Object.fromEntries(Object.keys(v).sort().map((k) => [k, v[k]])) : v));

// --- fixtures ----------------------------------------------------------------

const CHAIN = 5042002; // Arc Testnet
const SAFE = '0x4c3fF6bC7D1A4b7f4e0E93B4C61b8b1a7E5dF2a9';
const MEMBER = '0x8F1E0bC5D9aA3F6e2B7c4D1a9E8f3B2c5A6d7E01';
const OTHER = '0x2B5AD5c4795c026514f8317c7a215E218DcCD6cF';
const THIRD = '0x6813Eb9362372EEF6200f3b1dbC3f819671cBA69';
const USDC = '0x3600000000000000000000000000000000000000';
const MSCO = '0x9641d764fc13c8B624c04430C7356C1C7C8102e2';
const REGISTRY = '0xF412388bF908E510066d3Be5c704bd9A327f945d';
const ZERO = '0x0000000000000000000000000000000000000000';

const num = (n) => BigInt(n).toString(16).padStart(64, '0');
const word = (hex) => hex.replace(/^0x/, '').toLowerCase().padStart(64, '0');
const sel = (sig) => ns.keccak.selector(sig).slice(2);
const bytesArg = (hexBody) => num(hexBody.length / 2) + hexBody.padEnd(Math.ceil(hexBody.length / 64) * 64, '0');
const packed = (legs) => legs.map(({ op = 0, to, value = 0, data = '0x' }) => {
  const body = data.replace(/^0x/, '');
  return op.toString(16).padStart(2, '0') + to.replace(/^0x/, '').toLowerCase() + num(value) + num(body.length / 2) + body;
}).join('');
const multiSend = (legs) => '0x' + sel('multiSend(bytes)') + num(32) + bytesArg(packed(legs));

function tx(fields) {
  return {
    to: OTHER, value: '0x0', data: '0x', operation: 0, safeTxGas: '0x0', baseGas: '0x0',
    gasPrice: '0x0', gasToken: ZERO, refundReceiver: ZERO, nonce: '0x7', ...fields,
  };
}

const transfer = '0x' + sel('transfer(address,uint256)') + word(THIRD) + num(250_000_000n);
const members = { owners: [MEMBER, OTHER], threshold: 2 };
const memberTx = (change) => {
  const verdict = ms('memberChangeTx', { multisig: SAFE, state: members, change, nonce: '0x7' });
  if (!verdict.tx) throw new Error('the core refused ' + JSON.stringify(change) + ': ' + JSON.stringify(verdict));
  return verdict.tx;
};

const CASES = [
  ['native transfer (core transfersTx)', ms('transfersTx', { transfers: [{ to: THIRD, amount: '1500000000000000000' }], nonce: '0x7' })],
  ['ERC-20 transfer (core transfersTx)', ms('transfersTx', { transfers: [{ token: USDC, to: THIRD, amount: '250000000' }], nonce: '0x7' })],
  ['two transfers as one batch (core transfersTx)', ms('transfersTx', {
    transfers: [{ to: THIRD, amount: '1' }, { token: USDC, to: OTHER, amount: '2' }], nonce: '0x8',
  })],
  ['a dApp call (core callsTx)', ms('callsTx', { calls: [{ to: THIRD, value: '0x0', data: '0x12345678' + num(9) }], nonce: '0x7' })],
  ['two dApp calls (core callsTx)', ms('callsTx', {
    calls: [{ to: THIRD, value: '0x5', data: '0x' }, { to: USDC, value: '0x0', data: transfer }], nonce: '0x9',
  })],
  ['add a member (core memberChangeTx)', memberTx({ kind: 'add', owner: THIRD, threshold: 2 })],
  ['remove a member (core memberChangeTx)', memberTx({ kind: 'remove', owner: OTHER, threshold: 1 })],
  ['replace a member (core memberChangeTx)', memberTx({ kind: 'replace', oldOwner: OTHER, newOwner: THIRD })],
  ['change the threshold (core memberChangeTx)', memberTx({ kind: 'threshold', threshold: 1 })],
  ['reject: a cancellation (core cancelTx)', ms('cancelTx', { multisig: SAFE, nonce: '0x7' })],
  ['rename (core renameTx)', ms('renameTx', { name: 'Team fund 🚀', nonce: '0x7' })],
  ['rename to a name with RLO', tx({
    to: REGISTRY, data: '0x' + sel('rename(string)') + num(32) + bytesArg(Buffer.from('Team‮dnuf').toString('hex')),
  })],
  ['coins to the registry', tx({ to: REGISTRY, value: '0x1', data: '0x' })],
  ['ERC-20 transfer with dirty padding → a call', tx({
    to: USDC, data: '0x' + sel('transfer(address,uint256)') + 'ff'.repeat(12) + THIRD.slice(2).toLowerCase() + num(5),
  })],
  ['ERC-20 transfer that also carries coin → a call', tx({ to: USDC, value: '0x1', data: transfer })],
  ['delegatecall to a stranger', tx({ to: THIRD, operation: 1, data: '0x12345678' })],
  ['delegatecall to MultiSendCallOnly with coin', tx({ to: MSCO, operation: 1, value: '0x1', data: multiSend([{ to: THIRD }]) })],
  ['a batch with a delegatecall leg', tx({ to: MSCO, operation: 1, data: multiSend([{ to: THIRD }, { op: 1, to: THIRD, data: '0xdeadbeef' }]) })],
  ['a batch with a zero-address leg', tx({ to: MSCO, operation: 1, data: multiSend([{ to: ZERO }]) })],
  ['a batch that enables a module', tx({
    to: MSCO, operation: 1, data: multiSend([{ to: SAFE, data: '0x' + sel('enableModule(address)') + word(THIRD) }]),
  })],
  ['a batch with a member change and a transfer', tx({
    to: MSCO, operation: 1, data: multiSend([
      { to: SAFE, data: '0x' + sel('changeThreshold(uint256)') + num(1) },
      { to: USDC, data: transfer },
    ]),
  })],
  ['a batch with trailing bytes', tx({ to: MSCO, operation: 1, data: multiSend([{ to: THIRD }]) + 'ff' })],
  ['a batch whose padding is not zero', tx({
    to: MSCO, operation: 1,
    data: multiSend([{ to: THIRD }]).slice(0, -2) + '01',
  })],
  ['an empty batch', tx({ to: MSCO, operation: 1, data: '0x' + sel('multiSend(bytes)') + num(32) + num(0) })],
  ['setGuard on itself', tx({ to: SAFE, data: '0x' + sel('setGuard(address)') + word(THIRD) })],
  ['setFallbackHandler on itself', tx({ to: SAFE, data: '0x' + sel('setFallbackHandler(address)') + word(THIRD) })],
  ['changeMasterCopy on itself', tx({ to: SAFE, data: '0x' + sel('changeMasterCopy(address)') + word(THIRD) })],
  ['coin to itself', tx({ to: SAFE, value: '0x1' })],
  ['addOwner with a short encoding', tx({ to: SAFE, data: '0x' + sel('addOwnerWithThreshold(address,uint256)') + word(THIRD) })],
  ['pays gas from the multisig', tx({ gasPrice: '0x1' })],
  ['a refund receiver', tx({ refundReceiver: THIRD })],
  ['a gas token', tx({ gasToken: USDC })],
  ['safeTxGas', tx({ safeTxGas: '100000' })],
  ['operation 2', tx({ operation: 2 })],
  ['decimal quantities', tx({ value: '1000', nonce: '42' })],
  ['a huge nonce', tx({ nonce: '0x' + 'f'.repeat(64) })],
];

// --- 1. the same hashes, the same approval bytes, the same reading -------------

const hex = (bytes) => '0x' + Buffer.from(bytes).toString('hex');

for (const [name, safeTx] of CASES) {
  const parsed = ns.multisig.parseTx(safeTx);
  if (!check(`${name}: the page reads the fields`, !parsed.error, parsed.error)) continue;
  const ours = ns.multisig.hashes(CHAIN, SAFE, parsed.tx);
  const theirs = ms('safeTxHashes', { chainId: CHAIN, safe: SAFE, tx: safeTx });
  check(`${name}: safeTxHash, domain and message hashes = vela-core`,
    ours.safeTxHash === theirs.safeTxHash && ours.domainHash === theirs.domainHash &&
      ours.messageHash === theirs.messageHash,
    ours.safeTxHash + ' vs ' + theirs.safeTxHash);
  const challenge = hex(ns.multisig.approvalChallenge(CHAIN, MEMBER, ours.safeTxHash));
  const coreChallenge = ms('approvalChallenge', { chainId: CHAIN, owner: MEMBER, safeTxHash: theirs.safeTxHash });
  check(`${name}: the approval bytes = vela-core approvalChallenge`, challenge === coreChallenge);

  const review = ms('review', { chainId: CHAIN, multisig: SAFE, tx: safeTx });
  const reading = ns.multisig.read(SAFE, parsed.tx);
  if (review.action === null) {
    check(`${name}: refused, as vela-core refuses it`, !!reading.refusal,
      `core: ${review.refusal}; page: ${JSON.stringify(reading.action)}`);
  } else {
    check(`${name}: read as vela-core reads it`,
      !reading.refusal && canonical(reading.action) === canonical(review.action),
      reading.refusal ? `page refused (${reading.refusal}); core: ${JSON.stringify(review.action)}`
        : `page ${JSON.stringify(reading.action)} vs core ${JSON.stringify(review.action)}`);
  }
}

// The page is stricter than the core where the core would accept a shape the
// page cannot draw exactly: never the other way round.
for (const [name, bad] of [
  ['an address without 0x', { to: OTHER.slice(2) }],
  ['a number for a quantity', { value: 5 }],
  ['a negative quantity', { value: '-1' }],
  ['odd-length calldata', { data: '0x123' }],
  ['a fractional operation', { operation: 0.5 }],
  ['a missing field', { nonce: undefined }],
]) {
  const raw = tx(bad);
  if (bad.nonce === undefined && 'nonce' in bad) delete raw.nonce;
  check(`the page refuses ${name}`, ns.multisig.parseTx(raw).error === 'refuse.msShape');
}

// --- 2. through the page ------------------------------------------------------

const WALLET_CTX = { channel: 'post', originVerified: true, requester: 'https://wallet.getvela.app' };

function request(safeTx, extra = {}) {
  const parsed = ns.multisig.parseTx(safeTx);
  const claimed = parsed.error ? '0x' + '00'.repeat(32) : ns.multisig.hashes(CHAIN, SAFE, parsed.tx).safeTxHash;
  return {
    method: 'vela_multisigApprove',
    origin: '',
    params: [{ chainId: CHAIN, safe: SAFE, member: MEMBER, safeTx, safeTxHash: claimed, ...extra }],
  };
}

function view(intent, ctx = {}) {
  return ns.resolve(intent, { chainId: CHAIN, account: MEMBER, ...WALLET_CTX, method: intent.method, ...ctx });
}

const warned = (v, key) => v.warnings.some((w) => w.key === key);

{
  // On Base, where this page knows USDC and its six decimals (Arc Testnet's
  // catalogue names only its native coin).
  const BASE = 8453;
  const BASE_USDC = '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913';
  const safeTx = ms('transfersTx', { transfers: [{ token: BASE_USDC, to: THIRD, amount: '250000000' }], nonce: '0x7' });
  const intent = {
    method: 'vela_multisigApprove', origin: '',
    params: [{ chainId: BASE, safe: SAFE, member: MEMBER, safeTx,
      safeTxHash: ms('safeTxHashes', { chainId: BASE, safe: SAFE, tx: safeTx }).safeTxHash }],
  };
  const v = view(intent, { chainId: BASE, multisig: { name: 'Team fund' } });
  check('an ERC-20 transfer: read, not refused, the Approve button', !v.refuse && v.confirmKey === 'button.msApprove');
  check('…the amount in the token\'s units (USDC, 6 decimals)', v.hero && v.hero.amount && v.hero.amount.text === '250',
    v.hero && v.hero.amount && v.hero.amount.text);
  check('…the multisig row: its address, its name marked as the app\'s', (() => {
    const row = v.fields.find((f) => f.label === 'field.multisig');
    return row && row.identity.address.toLowerCase() === SAFE.toLowerCase() && row.identity.contractName === 'Team fund' && row.identity.tag === 'tag.nameByApp';
  })());
  check('…the nonce', v.fields.some((f) => f.label === 'field.nonce' && f.value === '7'));
  check('…its reading is certain (read by this page)', v.provenance && v.provenance.kind === 'read', JSON.stringify(v.provenance));
  check('…the three hashes are the page\'s and match the app\'s', v.multisig.hashMatches && /^0x[0-9a-f]{64}$/.test(v.multisig.hashes.domainHash));
  const digest = ns.digest.of(intent, { chainId: BASE });
  const expected = ms('approvalChallenge', {
    chainId: BASE, owner: MEMBER,
    safeTxHash: ms('safeTxHashes', { chainId: BASE, safe: SAFE, tx: intent.params[0].safeTx }).safeTxHash,
  });
  check('…the passkey signs exactly vela-core\'s approval bytes', digest.hash && hex(digest.hash) === expected);
  const sheet = ns.render(v, {});
  const text = sheet.textContent;
  check('…the card draws the three hashes in full, each with a copy button',
    [v.multisig.hashes.safeTxHash, v.multisig.hashes.domainHash, v.multisig.hashes.messageHash].every((h) => text.includes(h)) &&
      sheet.find('.copy-button-small').length === 3);
}

{
  const safeTx = ms('transfersTx', { transfers: [{ to: THIRD, amount: '1000000000000000000' }], nonce: '0x7' });
  const forged = ms('transfersTx', { transfers: [{ to: OTHER, amount: '1' }], nonce: '0x7' });
  // The app shows one transaction's hash and hands over another's fields.
  const claimed = ms('safeTxHashes', { chainId: CHAIN, safe: SAFE, tx: forged }).safeTxHash;
  const intent = request(safeTx, { safeTxHash: claimed });
  const v = view(intent);
  check('fields that do not make the claimed hash: refused, loudly, first', v.refuse && v.risk === 'hard-danger' &&
    v.warnings[0].key === 'refuse.msHashMismatch' && v.warnings[0].params.claimed === claimed);
  check('…and the digest refuses too (nothing to sign)', ns.digest.of(intent, {}).refuse === 'refuse.msHashMismatch');
  check('…the card still shows what the fields do', v.hero && v.hero.amount && v.hero.amount.text === '1');
}

{
  const intent = request(ms('cancelTx', { multisig: SAFE, nonce: '0x7' }));
  const v = view(intent, {
    multisig: { name: 'Team‮dnuf' },
    signer: { name: 'Alice​', letter: 'A' },
    chainName: 'Arc⁦',
  });
  const row = v.fields.find((f) => f.label === 'field.multisig');
  check('hidden characters in the multisig\'s name are drawn as code points', row.identity.contractName === 'Team[U+202E]dnuf');
  check('…and in the member\'s name', v.accountName === 'Alice[U+200B]');
  check('…with a caution', warned(v, 'warn.invisibleChars'));
  check('a rejection is read as one, with its own button', v.intentKey === 'intent.msReject' && v.confirmKey === 'button.msReject');
}

{
  const huge = tx({ to: THIRD, data: '0x' + 'ab'.repeat(ns.multisig.MAX_DATA_BYTES + 1) });
  const v = view({ method: 'vela_multisigApprove', origin: '', params: [{ chainId: CHAIN, safe: SAFE, member: MEMBER, safeTx: huge, safeTxHash: '0x' + '11'.repeat(32) }] });
  check('a huge SafeTx: refused before anything is decoded', v.refuse && warned(v, 'refuse.msTooLarge'));
  const many = tx({
    to: MSCO, operation: 1,
    data: multiSend(Array.from({ length: ns.multisig.MAX_LEGS + 1 }, () => ({ to: THIRD, value: 1 }))),
  });
  check('a batch of more legs than the page draws: refused', view(request(many)).refuse);
}

{
  const intent = request(tx({ to: THIRD, operation: 1, data: '0x12345678' }));
  const v = view(intent);
  check('a delegatecall to anything but MultiSendCallOnly: refused, said as such', v.refuse && warned(v, 'refuse.msDelegateCall'));
  const guard = view(request(tx({ to: SAFE, data: '0x' + sel('setGuard(address)') + word(THIRD) })));
  check('setGuard on the multisig: refused, the function named', guard.refuse &&
    guard.warnings.some((w) => w.key === 'refuse.msSelfCall' && w.params.fn === 'setGuard'));
  const handler = view(request(tx({ to: SAFE, data: '0x' + sel('setFallbackHandler(address)') + word(THIRD) })));
  check('setFallbackHandler on the multisig: refused', handler.refuse && warned(handler, 'refuse.msSelfCall'));
  const module = view(request(tx({
    to: MSCO, operation: 1, data: multiSend([{ to: SAFE, data: '0x' + sel('enableModule(address)') + word(THIRD) }]),
  })));
  check('enableModule hidden in a batch: refused', module.refuse && warned(module, 'refuse.msSelfCall'));
  const singleton = view(request(tx({ to: SAFE, data: '0x' + sel('changeMasterCopy(address)') + word(THIRD) })));
  check('a singleton change: refused', singleton.refuse);
}

{
  const v = view(request(memberTx({ kind: 'replace', oldOwner: OTHER, newOwner: THIRD })));
  check('a member swap: both addresses, a caution, not refused', !v.refuse && v.intentKey === 'intent.msSwapOwner' &&
    v.fields.some((f) => f.label === 'field.newMember' && f.identity.address.toLowerCase() === THIRD.toLowerCase()) &&
    v.fields.some((f) => f.label === 'field.oldMember' && f.identity.address.toLowerCase() === OTHER.toLowerCase()) &&
    warned(v, 'warn.msMembers'));
  const batch = view(request(tx({
    to: MSCO, operation: 1, data: multiSend([
      { to: SAFE, data: '0x' + sel('changeThreshold(uint256)') + num(1) },
      { to: THIRD, value: 2n * 10n ** 18n },
    ]),
  })));
  check('a batch: each leg read (a threshold change, a native transfer in units)', !batch.refuse && batch.legs.length === 2 &&
    batch.legs[0].view.intentKey === 'intent.msThreshold' && batch.legs[1].view.hero.amount.text === '2',
    JSON.stringify(batch.legs.map((l) => [l.view.intentKey, l.view.hero && l.view.hero.amount && l.view.hero.amount.text])));
}

{
  const intent = request(ms('transfersTx', { transfers: [{ to: THIRD, amount: '1' }], nonce: '0x7' }));
  const stranger = view(intent, { requester: 'https://evil.example' });
  check('an approval for a site that is not the web wallet: refused (R7)', stranger.refuse && warned(stranger, 'refuse.answerElsewhere'));
  const subdomain = view(intent, { requester: 'https://sign.getvela.app' });
  check('…nor another getvela.app subdomain', subdomain.refuse);
  const claimed = view(intent, { originVerified: false });
  check('…nor an origin the browser did not vouch for', claimed.refuse);
  const loopback = view(intent, { requester: 'http://localhost:5322' });
  check('…nor a loopback wallet, while this page is not on the loopback', loopback.refuse);
  globalThis.location = { hostname: 'localhost' };
  const dev = view(intent, { requester: 'http://localhost:5322' });
  check('a loopback page answers a loopback wallet (development)', !dev.refuse);
  delete globalThis.location;
  const url = view(intent, { channel: 'url', callback: 'velawallet://sign-result' });
  check('the apps\' url channel still answers to velawallet://', !url.refuse);
  const other = ns.resolve({ method: 'personal_sign', params: ['0x68656c6c6f', MEMBER], origin: '' },
    { chainId: CHAIN, account: MEMBER, ...WALLET_CTX });
  check('any other signature over postMessage from the wallet: still refused', other.refuse && warned(other, 'refuse.answerElsewhere'));
}

{
  const intent = request(ms('transfersTx', { transfers: [{ to: THIRD, amount: '1' }], nonce: '0x7' }));
  const v = view(intent, { chainId: 1 });
  check('a context on another chain than the request: refused', v.refuse && warned(v, 'refuse.msChainMismatch'));
  const w = view(intent, { account: OTHER });
  check('a context account that is not the member: refused', w.refuse && warned(w, 'refuse.msMemberMismatch'));
  const shape = view({ method: 'vela_multisigApprove', origin: '', params: [{ chainId: CHAIN, safe: SAFE }] });
  check('a request missing its SafeTx: refused, nothing derived', shape.refuse && warned(shape, 'refuse.msShape') &&
    ns.digest.of({ method: 'vela_multisigApprove', params: [{}] }, {}).refuse === 'refuse.msShape');
}

// Every word this approval can put on the card, in every language.
const KEYS = Object.keys(ns.i18n.catalogue ? ns.i18n.catalogue('en') : {});
const msKeys = (KEYS.length ? KEYS : []).filter((k) => /\.ms[A-Z]|^ui\.copy$|^tag\.(multisig|thisMultisig)$|^field\.(multisig|member|newMember|oldMember|threshold|newName)$/.test(k));
const english = {};
ns.i18n.setLocale('en');
const sample = ['intent.msApprove', 'refuse.msHashMismatch', 'refuse.msDelegateCall', 'ui.msHashes', 'button.msApprove', 'warn.msMembers', 'sentence.msReject'];
for (const key of sample) english[key] = ns.i18n.t(key, { claimed: 'A', computed: 'B', nonce: '7', fn: 'f' });
for (const locale of LOCALES) {
  ns.i18n.setLocale(locale);
  const missing = sample.filter((key) => {
    const said = ns.i18n.t(key, { claimed: 'A', computed: 'B', nonce: '7', fn: 'f' });
    return !said || said === key || (locale !== 'en' && said === english[key]);
  });
  check(`${locale}: the approval's words are its own`, missing.length === 0, missing.join(', '));
}
ns.i18n.setLocale('en');
void msKeys;
void doc;

process.exit(check.summary() ? 0 : 1);
