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

// --- 1b. never accept what the wallet's one review refuses ---------------------
//
// Round 5 (signing parity, §3.6): the wallet signs a member's approval only
// from `multisigReview` and its sign rule, asked again right before the
// passkey (`signingGate`). This page cannot read the chain (its CSP connects
// nowhere), so its state-diff guard is the decoding rules alone; what it must
// never do is accept a transaction the core's review refuses. Two checks:
//  - every case above, through `multisigReview` with no chain reads (the
//    review the page's position is in): refused there ⇒ refused here, and a
//    review the core cannot build at all is refused here too;
//  - the core's own state-diff vectors (`tests/vectors/multisig-review/`):
//    the transactions the guard refuses on a real chain's answers (Bybit:
//    a DELEGATECALL that writes slot 0) are refused here from their bytes.

const reviewOf = (input) => {
  try {
    return ms('multisigReview', { input });
  } catch (error) {
    return { thrown: String(error && error.message || error) };
  }
};
let aligned = 0;
for (const [name, safeTx] of CASES) {
  const parsed = ns.multisig.parseTx(safeTx);
  if (parsed.error) continue;
  const out = reviewOf({ chainId: CHAIN, safe: SAFE, tx: safeTx, owners: members.owners, threshold: members.threshold, deployed: true });
  const coreRefuses = !!out.thrown || out.error !== undefined || out.sign?.allowed === false;
  const pageRefuses = !!ns.multisig.read(SAFE, parsed.tx).refusal;
  if (coreRefuses) {
    check(`${name}: the one review (multisigReview) refuses it — so does the page`, pageRefuses,
      JSON.stringify(out.thrown ?? out.concerns));
  } else {
    check(`${name}: what the page accepts, the one review signs too`, pageRefuses || out.sign?.allowed === true,
      JSON.stringify(out));
  }
  aligned += 1;
}
check(`every readable case went through multisigReview (${aligned})`, aligned >= CASES.length - 2);

const VECTORS = join(repo, 'rust/crates/vela-core/tests/vectors/multisig-review');
for (const file of readdirSync(VECTORS).filter((f) => f.endsWith('.json')).sort()) {
  const vector = JSON.parse(readFileSync(join(VECTORS, file), 'utf8'));
  const out = reviewOf(vector.input);
  check(`${file}: the core reviews it as the vector says (${vector.expect.sign.allowed ? 'signed' : 'refused'})`,
    out.sign && out.sign.allowed === vector.expect.sign.allowed, JSON.stringify(out.sign ?? out));
  const parsed = ns.multisig.parseTx(vector.input.tx);
  const reading = parsed.error ? { refusal: parsed.error } : ns.multisig.read(vector.input.safe, parsed.tx);
  if (vector.expect.sign.allowed === false) {
    check(`${file}: refused by the state-diff guard — and by the page, from its bytes`, !!reading.refusal,
      JSON.stringify(reading.action));
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

  // r8-5: a batch whose leg CALLs multiSend of two transfers (round 8's #9):
  // every leg of every level is listed, and the confirm is never the plain one.
  const transferTo = (to, amount) => '0x' + sel('transfer(address,uint256)') + word(to) + num(amount);
  const nested = view(request(tx({
    to: MSCO, operation: 1, data: multiSend([
      { to: MSCO, data: multiSend([{ to: USDC, data: transferTo(THIRD, 1000) }, { to: USDC, data: transferTo(OTHER, 1000) }]) },
    ]),
  })));
  check('r8-5: a nested batch lists every leg of every level ("1", "1.1", "1.2")',
    !nested.refuse && nested.legs.map((l) => l.index).join(',') === '1,1.1,1.2' &&
    nested.legs[1].view.hero && nested.legs[1].view.hero.amount.text === '0.001',
    JSON.stringify(nested.legs.map((l) => [l.index, l.view.intentKey])));
  check('r8-5: a nested batch is a danger, said (never the plain accent Approve)',
    nested.risk === 'danger' && warned(nested, 'warn.nested'), nested.risk);
  check('r8-5: one call packed reads as one', nested.legs[0].view.sentence &&
    !/\b1 calls\b/.test(String(nested.legs[0].view.sentence.text || nested.legs[0].view.sentence)));
  let deep = transferTo(THIRD, 1);
  for (let i = 0; i < 4; i++) deep = multiSend([{ to: MSCO, data: deep }]);
  const tooDeep = view(request(tx({ to: MSCO, operation: 1, data: multiSend([{ to: MSCO, data: deep }]) })));
  check('r8-5: nested deeper than three: refused, nothing signed', tooDeep.refuse &&
    warned(tooDeep, 'refuse.msBatchUnreadable'));
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
  // 2026-10-30, the page everywhere: a message from the web wallet is answered
  // too, and the published extension is one of Vela's wallets.
  const other = ns.resolve({ method: 'personal_sign', params: ['0x68656c6c6f', MEMBER], origin: '' },
    { chainId: CHAIN, account: MEMBER, ...WALLET_CTX });
  check('a message over postMessage from the web wallet: answered (the page, everywhere)',
    !other.refuse && !warned(other, 'refuse.answerElsewhere'));
  const fromExtension = view(intent, { requester: 'chrome-extension://bjbdmnmpgcfkocfcfdocopkioacojkhl' });
  check('an approval from the published extension: answered', !fromExtension.refuse);
  const otherExtension = view(intent, { requester: 'chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' });
  check('…but not from another extension id', otherExtension.refuse && warned(otherExtension, 'refuse.answerElsewhere'));
  globalThis.location = { hostname: 'localhost' };
  const devExtension = view(intent, { requester: 'chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' });
  check('a loopback page answers an unpacked extension (development)', !devExtension.refuse);
  delete globalThis.location;
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

// --- 3. an executor's operation (round 7, r7-1) --------------------------------
//
// Executing a multisig transaction is the executor's own user operation
// (vela-core `execution_calls`): the deploys it needs, then `execTransaction`,
// then the in-band fee. Signed on this page it used to read "Batch —
// Unknowable", every leg a blind signature. Every bundle here is made of the
// core's own pieces (`genesis`, `velaAccountGenesis`, `deployCall`,
// `signatures`, `execTransactionData`, `executionCalls`) and handed to the
// page as the wallet hands it (`trustedSignerSignatureRequest`'s shape: the
// wallet's own `wallet_sendCalls`, the assembled operation, its fee leg, and
// the execution's `context.multisig`).

const MULTI_SEND = '0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526';
const EXECUTOR = '0x7a3b5C8d9E0f1A2b3C4d5E6f7A8b9C0d1E2f3A4b';
const FEE_TO = '0x1111111111111111111111111111111111111111';
const WEBAUTHN_SIGNER = '0x94a4F6affBd8975951142c3999aEAB7ecee555c2';
const { createECDH } = await import('node:crypto');

/** A passkey's public key, as an account stores it (uncompressed P-256, hex). */
const passkey = (seed) => {
  const ecdh = createECDH('prime256v1');
  ecdh.setPrivateKey(Buffer.alloc(32, seed));
  return { credentialId: 'c0ffee' + seed.toString(16).padStart(2, '0'), publicKeyHex: ecdh.getPublicKey('hex') };
};
const ALICE = ms('velaAccountGenesis', { keys: [passkey(11)] });
const CAROL = ms('velaAccountGenesis', { keys: [passkey(21), passkey(22), passkey(23)] });
const team = ms('genesis', { owners: [ALICE.address, EXECUTOR], threshold: 2, name: 'R7 Team', saltNonce: '0x2a' });
const MS = team.address;

/** A member's passkey approval, as the mailbox holds it (its bytes are not checked here). */
const passkeyApproval = (owner, ownerGenesis) => ({
  owner, ownerGenesis, signer: WEBAUTHN_SIGNER, authenticatorData: '0x' + '05'.repeat(37),
  clientDataFields: '"origin":"https://getvela.app"', r: '0x1', s: '0x2',
});

/** `executeUserOp(MultiSend, 0, multiSend(legs), DELEGATECALL)`: the executor's callData. */
function callDataOf(calls) {
  const inner = '0x' + sel('multiSend(bytes)') + num(32) + bytesArg(packed(calls.map((c) => ({
    to: c.to, value: BigInt(c.value), data: c.data,
  }))));
  const body = inner.slice(2);
  return '0x' + sel('executeUserOp(address,uint256,bytes,uint8)') + word(MULTI_SEND) + num(0) + num(128) + num(1) + bytesArg(body);
}

/**
 * The page's request for an executor's operation: `calls` before the fee leg,
 * then a native fee of 0.02 USDC. `about` is `context.multisig` (null: a
 * wallet that names none).
 */
function executionRequest(calls, about, extra = {}) {
  const fee = { to: FEE_TO, value: '0x' + (2n * 10n ** 16n).toString(16), data: '0x' };
  const userOp = {
    sender: extra.sender || EXECUTOR, nonce: '0x0', initCode: '0x', callData: callDataOf([...calls, fee]),
    verificationGasLimit: '0', callGasLimit: '0', preVerificationGas: '0', maxFeePerGas: '0',
    maxPriorityFeePerGas: '0', paymasterAndData: '0x',
  };
  const intent = {
    method: 'wallet_sendCalls', origin: extra.origin || '',
    params: [{ version: '1.0', chainId: '0x' + CHAIN.toString(16), from: EXECUTOR, calls }],
  };
  const context = {
    chainId: CHAIN, account: EXECUTOR, ...WALLET_CTX, method: 'wallet_sendCalls',
    operation: { userOp, feeLegIndex: calls.length, entryPoint: '0x0000000071727De22E5E9d8BAf0edAc6f37da032' },
  };
  if (about) context.multisig = about;
  return { intent, context };
}

/** `context.multisig` as the core builds it (`approval::execution_context`). */
const aboutOf = (safeTx, extra = {}) => ({
  address: MS, name: 'R7 Team', threshold: 2, nonce: safeTx.nonce,
  safeTxHash: ms('safeTxHash', { chainId: CHAIN, safe: MS, tx: safeTx }), ...extra,
});

/** The `execTransaction` leg for `safeTx`, carrying `approvals` (and the executor's `msg.sender`). */
const execLeg = (safeTx, approvals, executor = EXECUTOR) => ({
  to: MS, value: '0x0',
  data: ms('execTransactionData', { tx: safeTx, signatures: ms('signatures', { approvals, executor }) }),
});
const deployLeg = (genesis) => ms('deployCall', { genesis });

const pay = ms('transfersTx', { transfers: [{ token: USDC, to: THIRD, amount: '50000' }], nonce: '0x7' });
const aliceSigned = [passkeyApproval(ALICE.address, ALICE.genesis)];

{
  // The plain case: the multisig and Alice's account exist; Alice approved,
  // the executor approves as the sender. The calls are the core's own.
  const calls = ms('executionCalls', {
    plan: { multisig: MS, deployMultisig: null, deployOwners: [], tx: pay, approvals: aliceSigned, executor: EXECUTOR },
  });
  const { intent, context } = executionRequest(calls, aboutOf(pay));
  const v = ns.resolve(intent, context);
  check('execution: read as one, never a batch of blind calls', v.intentKey === 'intent.msExecute' && !v.refuse &&
    v.confirmKey === 'button.msExecute', JSON.stringify([v.intentKey, v.refuse, v.warnings.map((w) => w.key)]));
  check('…what the multisig sends, in the token\'s units: 0.05 USDC (Arc\'s own coin behind 0x3600…)',
    v.hero && v.hero.amount && v.hero.amount.text === '0.05' && v.hero.amount.symbol === 'USDC',
    v.hero && v.hero.amount && v.hero.amount.text + ' ' + v.hero.amount.symbol);
  check('…said as the multisig\'s: "The multisig sends 0.05 USDC to 0x6813Eb…1cBA69"',
    v.sentence && v.sentence.key === 'sentence.msSends' && v.sentence.params.to.toLowerCase() === '0x6813eb…1cba69');
  check('…"Executes multisig transaction #7"', v.lead && v.lead.key === 'sentence.msExecute' && v.lead.params.nonce === '7');
  check('…its safeTxHash, computed by the page = vela-core\'s, matching the app\'s',
    v.multisig && v.multisig.hashMatches && v.multisig.hashes.safeTxHash === ms('safeTxHash', { chainId: CHAIN, safe: MS, tx: pay }));
  const approvalsRow = v.fields.find((f) => f.label === 'field.approvals');
  check('…approved 2 of 2 (the threshold the app\'s word, said so)', approvalsRow && approvalsRow.value.key === 'value.approvalsOf' &&
    approvalsRow.value.params.count === 2 && approvalsRow.value.params.threshold === '2' && approvalsRow.tagKey === 'tag.byApp',
    JSON.stringify(approvalsRow));
  const by = v.fields.filter((f) => f.label === 'field.approvedBy').map((f) => f.identity && f.identity.address.toLowerCase());
  check('…by Alice (her passkey) and the executor (as the sender)', by.length === 2 &&
    by.includes(ALICE.address.toLowerCase()) && by.includes(EXECUTOR.toLowerCase()), JSON.stringify(by));
  check('…the fee leg is the fee row, not a leg', v.fee && v.fee.leg && v.fee.leg.amount === '0.02' && !v.legs.length);
  check('…the reading is the page\'s own (✓)', v.provenance && v.provenance.kind === 'read');
  const digest = ns.digest.of(intent, context);
  check('…and the passkey signs the operation\'s SafeOp hash (nothing else)', !digest.refuse && !!digest.hash);
  ns.i18n.setLocale('en');
  const text = ns.render(v, {}).textContent;
  check('…the card says it in words', text.includes('Executes multisig transaction #7:') &&
    text.toLowerCase().includes('the multisig sends 0.05 usdc to 0x6813eb…1cba69.') && text.includes('2 of 2') &&
    text.includes(v.multisig.hashes.safeTxHash) && text.includes('Execute'), text.slice(0, 400));
}

{
  // D1 (real-device pass 2): the app's one review asks "… anyway" for the
  // execution (neither its state-change check nor its simulation read, or a
  // danger) — the executor's card says "Execute anyway", in the danger fill,
  // with why; with no word from the app, a plain "Execute".
  const calls = ms('executionCalls', {
    plan: { multisig: MS, deployMultisig: null, deployOwners: [], tx: pay, approvals: aliceSigned, executor: EXECUTOR },
  });
  const asked = executionRequest(calls, aboutOf(pay, { anyway: true }));
  const v = ns.resolve(asked.intent, asked.context);
  check('D1: the app asks "anyway" — "Execute anyway", said why', !v.refuse && v.anyway &&
    v.confirmKey === 'button.msExecuteAnyway' && warned(v, 'warn.msAppAnywayExec') && v.risk === 'danger',
    JSON.stringify([v.confirmKey, v.risk, v.warnings.map((w) => w.key)]));
  check('D1: …in the danger fill, never the accent', hasClass(ns.render(v, {}), 'confirm-anyway'));
  const plainReq = executionRequest(calls, aboutOf(pay));
  const plain = ns.resolve(plainReq.intent, plainReq.context);
  check('D1: no word from the app — a plain "Execute"', plain.confirmKey === 'button.msExecute' && !plain.anyway);
  // A hash the app got wrong is still refused, "anyway" or not.
  const wrong = executionRequest(calls, aboutOf(pay, { anyway: true, safeTxHash: '0x' + '11'.repeat(32) }));
  const refused = ns.resolve(wrong.intent, wrong.context);
  check('D1: "anyway" never opens a refusal', refused.refuse && refused.confirmKey === 'button.cannotSign');
}

{
  // The first execution: it deploys the multisig and Alice's account first.
  const calls = [deployLeg(team.genesis), deployLeg(ALICE.genesis), execLeg(pay, aliceSigned)];
  const { intent, context } = executionRequest(calls, aboutOf(pay));
  const v = ns.resolve(intent, context);
  check('execution with deploys: read, not refused', !v.refuse && v.intentKey === 'intent.msExecute',
    JSON.stringify(v.warnings.map((w) => w.key)));
  check('…"It also sets up the multisig on Arc Testnet: 2 members, 2 approvals needed"', v.notes.some((n) =>
    n.key === 'sentence.msExecSetsUp' && n.params.chain === 'Arc Testnet' && n.params.count === 2 && n.params.threshold === '2'));
  check('…its members, by address', v.fields.filter((f) => f.label === 'field.member').length === 2);
  check('…"It also activates 0x…\'s Vela account"', v.notes.some((n) => n.key === 'sentence.msExecActivates') &&
    v.fields.some((f) => f.label === 'field.activates' && f.identity.address.toLowerCase() === ALICE.address.toLowerCase()));
  const row = v.fields.find((f) => f.label === 'field.approvals');
  check('…the threshold is a fact here (the setup says it): no "per the app"', row.tagKey === undefined);

  // A member with three passkeys: its account's setup deploys two more signers.
  const three = ms('genesis', { owners: [CAROL.address, EXECUTOR], threshold: 2, name: 'Three keys', saltNonce: '0x2b' });
  const tx3 = ms('transfersTx', { transfers: [{ to: THIRD, amount: '1' }], nonce: '0x0' });
  const calls3 = [deployLeg(three.genesis), deployLeg(CAROL.genesis), {
    to: three.address, value: '0x0',
    data: ms('execTransactionData', { tx: tx3, signatures: ms('signatures', { approvals: [passkeyApproval(CAROL.address, CAROL.genesis)], executor: EXECUTOR }) }),
  }];
  const r3 = executionRequest(calls3, { address: three.address, nonce: '0x0', threshold: 2,
    safeTxHash: ms('safeTxHash', { chainId: CHAIN, safe: three.address, tx: tx3 }) });
  const v3 = ns.resolve(r3.intent, r3.context);
  check('…a member account with three passkeys is verified from its setup too', !v3.refuse &&
    v3.fields.some((f) => f.label === 'field.activates' && f.identity.address.toLowerCase() === CAROL.address.toLowerCase()),
    JSON.stringify(v3.warnings.map((w) => w.key)));
}

{
  const refusedWith = (calls, about, key, extra) => {
    const { intent, context } = executionRequest(calls, about, extra);
    const v = ns.resolve(intent, context);
    return { v, ok: v.refuse && (!key || warned(v, key)) && v.confirmKey === 'button.cannotSign' };
  };
  // A deploy of "the multisig" whose setup is not the one Vela writes.
  const hooked = { ...team.genesis, initializer: team.genesis.initializer.slice(0, 2 + 8 + 64 * 2) +
    word(THIRD) + team.genesis.initializer.slice(2 + 8 + 64 * 3) };
  const hookedAddress = ms('genesisAddress', { genesis: hooked });
  const hookedPay = { ...pay };
  const hookedExec = { to: hookedAddress, value: '0x0', data: ms('execTransactionData', { tx: hookedPay, signatures: ms('signatures', { approvals: aliceSigned, executor: EXECUTOR }) }) };
  check('a deploy whose setup has a hook (delegatecall at setup): refused, never "sets up the multisig"',
    refusedWith([deployLeg(hooked), hookedExec], null, 'refuse.msExecDeploy').ok);
  const handler = { ...team.genesis, initializer: team.genesis.initializer.replace(
    'fd0732dc9e303f09fcef3a7388ad10a83459ec99', THIRD.slice(2).toLowerCase()) };
  const handlerExec = { ...hookedExec, to: ms('genesisAddress', { genesis: handler }) };
  check('a deploy with another fallback handler: refused', refusedWith([deployLeg(handler), handlerExec], null, 'refuse.msExecDeploy').ok);
  check('a deploy of a Vela account whose owner did not approve: refused',
    refusedWith([deployLeg(CAROL.genesis), execLeg(pay, aliceSigned)], aboutOf(pay), 'refuse.msExecDeploy').ok);
  check('a call besides the execution (an ERC-20 transfer out of the executor): refused',
    refusedWith([{ to: USDC, value: '0x0', data: transfer }, execLeg(pay, aliceSigned)], aboutOf(pay), 'refuse.msExecExtra').ok);
  check('two executions in one operation: refused',
    refusedWith([execLeg(pay, aliceSigned), execLeg(pay, aliceSigned)], aboutOf(pay), 'refuse.msExecExtra').ok);
  const exec = execLeg(pay, aliceSigned);
  check('an execTransaction with bytes after it: refused (not the core\'s encoding)',
    refusedWith([{ ...exec, data: exec.data + '00' }], aboutOf(pay), 'refuse.msExecShape').ok);
  check('an execution that carries coin: refused', refusedWith([{ ...exec, value: '0x1' }], aboutOf(pay), 'refuse.msExecShape').ok);
  check('the executor executing on itself: refused', refusedWith([{ ...exec, to: EXECUTOR }], null, null).ok);
  const other = ms('transfersTx', { transfers: [{ token: USDC, to: OTHER, amount: '1' }], nonce: '0x7' });
  const forged = refusedWith([exec], aboutOf(pay, { safeTxHash: ms('safeTxHash', { chainId: CHAIN, safe: MS, tx: other }) }), 'refuse.msHashMismatch');
  check('the app\'s safeTxHash is another transaction\'s: refused, loudly, first', forged.ok && forged.v.risk === 'hard-danger' &&
    forged.v.warnings[0].key === 'refuse.msHashMismatch');
  check('the app names another multisig: refused', refusedWith([exec], aboutOf(pay, { address: THIRD }), 'refuse.msExecMismatch').ok);
  check('the app\'s threshold is not the setup\'s: refused',
    refusedWith([deployLeg(team.genesis), deployLeg(ALICE.genesis), exec], aboutOf(pay, { threshold: 1 }), 'refuse.msExecThreshold').ok);
  const bare = executionRequest([exec], null);
  const vb = ns.resolve(bare.intent, bare.context);
  check('a wallet that names no number: still read in full, no hashes, said so', !vb.refuse && !vb.multisig &&
    warned(vb, 'warn.msExecNoNonce') && vb.lead.key === 'sentence.msExecuteNoNonce' && vb.hero.amount.text === '0.05');
  const dapp = executionRequest([exec], null, { origin: 'https://app.example' });
  const vd = ns.resolve(dapp.intent, dapp.context);
  check('a site\'s own operation is not read as the wallet\'s execution', vd.intentKey !== 'intent.msExecute');
}

{
  // The SafeTx inside an execution is read by the approval's rules: a batch
  // through MultiSendCallOnly is drawn leg by leg (and is no "delegatecall"
  // takeover of the executor), what the core refuses is refused.
  const batchTx = tx({ to: MSCO, operation: 1, data: multiSend([
    { to: SAFE, data: '0x' + sel('changeThreshold(uint256)') + num(1) }, { to: USDC, data: transfer },
  ]) });
  const legsOf = (safeTx) => {
    const r = executionRequest([execLeg(safeTx, aliceSigned)], aboutOf(safeTx));
    return ns.resolve(r.intent, r.context);
  };
  const vb = legsOf(batchTx);
  check('an execution of a MultiSendCallOnly batch: each leg read, not refused', !vb.refuse && vb.legs.length === 2 &&
    vb.legs[1].view.hero.amount.text === '250', JSON.stringify(vb.warnings.map((w) => w.key)));
  check('an execution of a delegatecall to a stranger: refused, as Bybit', legsOf(tx({ to: THIRD, operation: 1, data: '0x12345678' })).refuse);
  check('an execution that pays a refund: refused', legsOf(tx({ gasPrice: '0x1', refundReceiver: THIRD })).refuse);
  check('an execution that enables a module on the multisig: refused',
    legsOf(tx({ to: MS, data: '0x' + sel('enableModule(address)') + word(THIRD) })).refuse);

  // Parity with the shipped core: every SafeTx above, executed. Whatever the
  // core's one review refuses, the page refuses as an execution too; what it
  // reads, the page reads the same.
  let paired = 0;
  for (const [name, safeTx] of CASES) {
    const parsed = ns.multisig.parseTx(safeTx);
    if (parsed.error) continue;
    const onMs = { ...safeTx, to: safeTx.to.toLowerCase() === SAFE.toLowerCase() ? MS : safeTx.to };
    let data;
    try {
      data = ms('execTransactionData', { tx: onMs, signatures: ms('signatures', { approvals: aliceSigned, executor: EXECUTOR }) });
    } catch {
      continue;
    }
    const r = executionRequest([{ to: MS, value: '0x0', data }], aboutOf(onMs));
    const v = ns.resolve(r.intent, r.context);
    const out = reviewOf({ chainId: CHAIN, safe: MS, tx: onMs, owners: members.owners, threshold: members.threshold, deployed: true });
    const coreRefuses = !!out.thrown || out.error !== undefined || out.sign?.allowed === false;
    if (coreRefuses) check(`executing "${name}": the core refuses it — so does the page`, v.refuse, JSON.stringify(out.concerns ?? out.thrown));
    else check(`executing "${name}": what the page signs, the core signs too`, v.refuse || out.sign?.allowed === true);
    paired += 1;
  }
  check(`every case was executed through the page (${paired})`, paired >= CASES.length - 4);
}

{
  // r7-4: Arc's own USDC behind its ERC-20 interface, named — never "1,000 ? ?".
  const v = ns.resolve({ method: 'eth_sendTransaction', origin: '', params: [{
    to: USDC, value: '0x0', data: '0x' + sel('transfer(address,uint256)') + word(THIRD) + num(1000),
  }] }, { chainId: CHAIN, ...WALLET_CTX, method: 'eth_sendTransaction' });
  check('Arc\'s USDC (0x3600…) is read in its units: 0.001 USDC', v.hero && v.hero.amount.text === '0.001' &&
    v.hero.amount.symbol === 'USDC' && !v.hero.amount.unverifiedDecimals, v.hero && JSON.stringify(v.hero.amount.text));
}

{
  // r9-10: "… anyway" exactly where the app's one review asks for it, and in
  // the danger fill; a plain transfer the review signs as it is stays "Approve".
  const BASE = 8453;
  const BASE_USDC = '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913';
  const safeTx = ms('transfersTx', { transfers: [{ token: BASE_USDC, to: THIRD, amount: '250000000' }], nonce: '0x7' });
  const intent = {
    method: 'vela_multisigApprove', origin: '',
    params: [{ chainId: BASE, safe: SAFE, member: MEMBER, safeTx,
      safeTxHash: ms('safeTxHashes', { chainId: BASE, safe: SAFE, tx: safeTx }).safeTxHash }],
  };
  const plain = view(intent, { chainId: BASE, multisig: { name: 'Team fund' } });
  check('r9-10: no word from the app — a plain "Approve"', plain.confirmKey === 'button.msApprove' && !plain.anyway);
  const asked = view(intent, { chainId: BASE, multisig: { name: 'Team fund', anyway: true } });
  check('r9-10: the app\'s review asks "anyway" (a run that fails) — "Approve anyway", said why',
    !asked.refuse && asked.anyway && asked.confirmKey === 'button.msApproveAnyway' && warned(asked, 'warn.msAppAnyway') &&
    (asked.risk === 'danger' || asked.risk === 'hard-danger'), JSON.stringify([asked.confirmKey, asked.risk]));
  ns.i18n.setLocale('en');
  check('r9-10: …in the danger fill, never the accent', hasClass(ns.render(asked, {}), 'confirm-anyway'));

  // A token this page cannot name: its base units, said as such — never "1,000,000 ? ?".
  const UNKNOWN = '0x7777777777777777777777777777777777777777';
  const nodec = ms('transfersTx', { transfers: [{ token: UNKNOWN, to: THIRD, amount: '1000000' }], nonce: '0x8' });
  const v = view({
    method: 'vela_multisigApprove', origin: '',
    params: [{ chainId: BASE, safe: SAFE, member: MEMBER, safeTx: nodec,
      safeTxHash: ms('safeTxHashes', { chainId: BASE, safe: SAFE, tx: nodec }).safeTxHash }],
  }, { chainId: BASE });
  check('r9-10: unknown decimals read "1000000 base units · decimals unknown"',
    v.hero && v.hero.amount.text === '1000000' && v.hero.amount.symbol === 'base units · decimals unknown',
    v.hero && JSON.stringify([v.hero.amount.text, v.hero.amount.symbol]));

  // One approval is "1 approval".
  const one = view(request(memberTx({ kind: 'threshold', threshold: 1 })));
  check('r9-10: "From now on 1 approval will be needed."',
    one.sentence && one.sentence.key === 'sentence.msThresholdOne' &&
    ns.i18n.t(one.sentence) === 'From now on 1 approval will be needed.', JSON.stringify(one.sentence));
}

// --- 4. round 10: whose coins, and a member change before → after ------------
//
// r10 (S4): an unlimited approve on a multisig's transaction said "all of
// YOUR USDC" — it is the multisig's. And a member change said only "1
// approval will be needed", where the app says "2 of 4 → 1 of 5": the page
// now draws it against the members the app says the multisig has now
// (`context.multisig.owners` / `.threshold`, vela-core `approval::request`),
// the "before" marked as the app's word; the members a setup in this very
// operation names are facts.
{
  ns.i18n.setLocale('en');
  const BASE = 8453;
  const BASE_USDC = '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913';
  const SPENDER = '0xcA0b000000000000000000000000000000c070a5';
  const MAX = (1n << 256n) - 1n;
  const approveTx = tx({ to: BASE_USDC, data: '0x' + sel('approve(address,uint256)') + word(SPENDER) + num(MAX) });
  const intent = {
    method: 'vela_multisigApprove', origin: '',
    params: [{ chainId: BASE, safe: SAFE, member: MEMBER, safeTx: approveTx,
      safeTxHash: ms('safeTxHashes', { chainId: BASE, safe: SAFE, tx: approveTx }).safeTxHash }],
  };
  const v = view(intent, { chainId: BASE });
  const said = ns.render(v, {}).textContent;
  check('r10: an unlimited approve of the multisig\'s USDC says the multisig\'s, not yours',
    v.sentence && v.sentence.key === 'sentence.approveUnlimitedMs' && warned(v, 'warn.unlimitedMs') &&
      !warned(v, 'warn.unlimited') && said.includes('all of the multisig’s USDC') &&
      said.includes('every USDC the multisig has') && !/\byour USDC\b|\bUSDC you have\b/.test(said),
    JSON.stringify([v.sentence, v.warnings.map((w) => w.key)]));
  ns.i18n.setLocale('zh');
  const zh = ns.render(v, {}).textContent;
  check('r10: …in Chinese too ("多签全部 USDC", never "你全部")', zh.includes('多签全部 USDC') && !zh.includes('你全部'), zh.slice(0, 300));
  ns.i18n.setLocale('en');
  const capped = tx({ to: BASE_USDC, data: '0x' + sel('approve(address,uint256)') + word(SPENDER) + num(5_000_000n) });
  const vc = view({ ...intent, params: [{ ...intent.params[0], safeTx: capped,
    safeTxHash: ms('safeTxHashes', { chainId: BASE, safe: SAFE, tx: capped }).safeTxHash }] }, { chainId: BASE });
  check('r10: …a capped approve too ("up to 5 USDC of the multisig’s")', vc.sentence && vc.sentence.key === 'sentence.approveMs',
    JSON.stringify(vc.sentence));
  const sent = view(request(ms('transfersTx', { transfers: [{ to: THIRD, amount: '1' }], nonce: '0x7' })));
  check('r10: …and a transfer is the multisig\'s ("The multisig sends …")', sent.sentence && sent.sentence.key === 'sentence.msSends',
    JSON.stringify(sent.sentence));
  // A site's own transaction is the account's: "your", whatever its context says.
  const own = ns.resolve({ method: 'eth_sendTransaction', origin: 'https://app.example',
    params: [{ from: MEMBER, to: BASE_USDC, value: '0x0', data: '0x' + sel('approve(address,uint256)') + word(SPENDER) + num(MAX) }] },
  { chainId: BASE, account: MEMBER, channel: 'url', callback: 'velawallet://sign-result', selfTag: 'tag.thisMultisig' });
  check('r10: a requester cannot make an account\'s approve read as a multisig\'s', own.sentence &&
    own.sentence.key === 'sentence.approveUnlimited' && warned(own, 'warn.unlimited'), JSON.stringify(own.sentence));
}

{
  ns.i18n.setLocale('en');
  const FOURTH = '0x9A8b000000000000000000000000000000001a09';
  const FIFTH = '0x88cCa00000000000000000000000000000266894';
  const now = { owners: [MEMBER, OTHER, THIRD, FOURTH], threshold: 2 };
  const changeOn = (state, change) => {
    const verdict = ms('memberChangeTx', { multisig: SAFE, state, change, nonce: '0x7' });
    if (!verdict.tx) throw new Error('the core refused ' + JSON.stringify(change) + ': ' + JSON.stringify(verdict));
    return verdict.tx;
  };
  // "Approvals needed" carries the before → after; without a claim it is the
  // bare threshold the bytes say, as before.
  const changeRow = (f) => f.label === 'field.threshold' && f.value && f.value.key === 'value.msApprovalsChange';
  const rowsOf = (v) => ({
    members: v.fields.find((f) => f.label === 'field.msMembers'),
    approvals: v.fields.find(changeRow),
    threshold: v.fields.find((f) => f.label === 'field.threshold' && !changeRow(f)),
  });
  const words = (row) => row && ns.i18n.t(row.value);

  // The round-10 case: 2 of 4, add a fifth needing 1.
  const add = view(request(changeOn(now, { kind: 'add', owner: FIFTH, threshold: 1 })), { multisig: { name: 'R10 Team', ...now } });
  const r = rowsOf(add);
  check('r10: add a member — "Members 4 → 5", "Approvals 2 of 4 → 1 of 5", the app\'s word',
    !add.refuse && words(r.members) === '4 → 5' && words(r.approvals) === '2 of 4 → 1 of 5' &&
      r.members.tagKey === 'tag.byApp' && r.approvals.tagKey === 'tag.byApp' && !r.threshold,
    JSON.stringify([words(r.members), words(r.approvals), r.members && r.members.tagKey]));
  const card = ns.render(add, {}).textContent;
  check('r10: …drawn on the card, with "per the app"', card.includes('4 → 5') && card.includes('2 of 4 → 1 of 5') &&
    card.includes('per the app'), card.slice(0, 500));
  ns.i18n.setLocale('zh');
  check('r10: …in Chinese ("2 / 4 → 1 / 5")', ns.render(add, {}).textContent.includes('2 / 4 → 1 / 5'));
  ns.i18n.setLocale('en');

  const remove = view(request(changeOn(now, { kind: 'remove', owner: OTHER, threshold: 2 })), { multisig: now });
  check('r10: remove a member — "4 → 3", "2 of 4 → 2 of 3"',
    words(rowsOf(remove).members) === '4 → 3' && words(rowsOf(remove).approvals) === '2 of 4 → 2 of 3',
    JSON.stringify([words(rowsOf(remove).members), words(rowsOf(remove).approvals)]));
  const swap = view(request(changeOn(now, { kind: 'replace', oldOwner: OTHER, newOwner: FIFTH })), { multisig: now });
  check('r10: replace a member — "4 → 4", "2 of 4 → 2 of 4"',
    words(rowsOf(swap).members) === '4 → 4' && words(rowsOf(swap).approvals) === '2 of 4 → 2 of 4');
  const rule = view(request(changeOn(now, { kind: 'threshold', threshold: 3 })), { multisig: now });
  check('r10: change the threshold — "4 → 4", "2 of 4 → 3 of 4"',
    words(rowsOf(rule).members) === '4 → 4' && words(rowsOf(rule).approvals) === '2 of 4 → 3 of 4' && !rowsOf(rule).threshold);

  // A wallet that names no members (an older one): the change as before.
  const bare = view(request(changeOn(now, { kind: 'add', owner: FIFTH, threshold: 1 })));
  check('r10: no members from the app — no before, the threshold row as before, no caution',
    !rowsOf(bare).members && rowsOf(bare).threshold && !warned(bare, 'warn.msMembersClaimOff'));
  // Members that do not fit the change: no figures this page cannot stand behind, and said.
  const off = view(request(changeOn(now, { kind: 'add', owner: FIFTH, threshold: 1 })),
    { multisig: { owners: [MEMBER, FIFTH], threshold: 1 } });
  check('r10: members that do not fit the change (adds one it has) — no before → after, a caution, not refused',
    !off.refuse && !rowsOf(off).members && rowsOf(off).threshold && warned(off, 'warn.msMembersClaimOff'));
  for (const [name, bad] of [
    ['a threshold above the members', { owners: [MEMBER, OTHER], threshold: 3 }],
    ['the same member twice', { owners: [MEMBER, MEMBER.toLowerCase()], threshold: 1 }],
    ['an address that is not one', { owners: [MEMBER, 'Bob'], threshold: 1 }],
    ['a threshold in words', { owners: [MEMBER, OTHER], threshold: '2' }],
  ]) {
    const v = view(request(changeOn(now, { kind: 'add', owner: FIFTH, threshold: 1 })), { multisig: bad });
    check(`r10: a malformed claim (${name}) is no claim`, !rowsOf(v).members && rowsOf(v).threshold);
  }

  // A batch: each change against the one before it.
  const batch = view(request(tx({
    to: MSCO, operation: 1, data: multiSend([
      { to: SAFE, data: '0x' + sel('addOwnerWithThreshold(address,uint256)') + word(FIFTH) + num(2) },
      { to: SAFE, data: '0x' + sel('changeThreshold(uint256)') + num(3) },
    ]),
  })), { multisig: now });
  const legRows = batch.legs.map((l) => [words(rowsOf(l.view).members), words(rowsOf(l.view).approvals)]);
  check('r10: a batch — each change against the one before ("4 → 5, 2 of 4 → 2 of 5", then "5 → 5, 2 of 5 → 3 of 5")',
    JSON.stringify(legRows) === JSON.stringify([['4 → 5', '2 of 4 → 2 of 5'], ['5 → 5', '2 of 5 → 3 of 5']]),
    JSON.stringify(legRows));
  check('r10: a batch\'s legs are numbered "1", "2" (never an empty number)', batch.legs.map((l) => l.index).join(',') === '1,2');
}

{
  // An executor runs a member change: the members now are the app's
  // (`execution_context`'s `owners`), or the setup's when this operation
  // deploys the multisig — facts, no "per the app".
  ns.i18n.setLocale('en');
  const NEWCOMER = '0x88cCa00000000000000000000000000000266894';
  const change = ms('memberChangeTx', { multisig: MS, state: { owners: [ALICE.address, EXECUTOR], threshold: 2 },
    change: { kind: 'add', owner: NEWCOMER, threshold: 2 }, nonce: '0x7' }).tx;
  const calls = ms('executionCalls', {
    plan: { multisig: MS, deployMultisig: null, deployOwners: [], tx: change, approvals: aliceSigned, executor: EXECUTOR },
  });
  const run = executionRequest(calls, aboutOf(change, { owners: [ALICE.address, EXECUTOR] }));
  const v = ns.resolve(run.intent, run.context);
  const members = v.fields.find((f) => f.label === 'field.msMembers');
  check('r10: an execution of "add a member" — "2 → 3", the app\'s word', !v.refuse && members &&
    ns.i18n.t(members.value) === '2 → 3' && members.tagKey === 'tag.byApp', JSON.stringify([v.warnings.map((w) => w.key), members]));
  const change0 = { ...change, nonce: '0x0' };
  const first = [deployLeg(team.genesis), deployLeg(ALICE.genesis), execLeg(change0, aliceSigned)];
  const deployed = executionRequest(first, aboutOf(change0));
  const vd = ns.resolve(deployed.intent, deployed.context);
  const factual = vd.fields.find((f) => f.label === 'field.msMembers');
  check('r10: …deployed in the same operation: "2 → 3" from the setup, a fact (no "per the app")', !vd.refuse && factual &&
    ns.i18n.t(factual.value) === '2 → 3' && factual.tagKey === undefined, JSON.stringify([vd.warnings.map((w) => w.key), factual]));

  // r10 (r8-5 on the executor's card): a batch inside the executed batch,
  // and a lone call that is a batch, list every leg with its number — an
  // execution drew a nested leg with nothing under it, and dropped a lone
  // call's legs altogether.
  const transferTo = (to, amount) => '0x' + sel('transfer(address,uint256)') + word(to) + num(amount);
  const runOf = (safeTx) => {
    const c = ms('executionCalls', {
      plan: { multisig: MS, deployMultisig: null, deployOwners: [], tx: safeTx, approvals: aliceSigned, executor: EXECUTOR },
    });
    const r = executionRequest(c, aboutOf(safeTx));
    return ns.resolve(r.intent, r.context);
  };
  const nestedRun = runOf(tx({ to: MSCO, operation: 1, data: multiSend([
    { to: MSCO, data: multiSend([{ to: USDC, data: transferTo(THIRD, 1000) }, { to: USDC, data: transferTo(OTHER, 1000) }]) },
  ]) }));
  check('r10: an executed batch inside a batch lists every leg ("1", "1.1", "1.2"), the multisig\'s',
    !nestedRun.refuse && nestedRun.legs.map((l) => l.index).join(',') === '1,1.1,1.2' &&
      nestedRun.legs[1].view.hero && nestedRun.legs[1].view.hero.amount.text === '0.001' &&
      nestedRun.legs[1].view.sentence.key === 'sentence.msSends',
    JSON.stringify(nestedRun.legs.map((l) => [l.index, l.view.intentKey, l.view.sentence && l.view.sentence.key])));
  const lone = runOf(tx({ to: MSCO, data: multiSend([{ to: USDC, data: transferTo(THIRD, 1000) }, { to: THIRD, value: 5 }]) }));
  check('r10: an executed lone call that is a batch lists its legs ("1", "2"), never an empty number',
    lone.legs.map((l) => l.index).join(',') === '1,2' && lone.legs.every((l) => String(l.index).length > 0),
    JSON.stringify([lone.refuse, lone.legs.map((l) => l.index), lone.warnings.map((w) => w.key)]));
  ns.i18n.setLocale('zh');
  const loneText = ns.render(lone, {}).textContent;
  check('r10: …in Chinese the leg heads carry their numbers ("1 · …", "2 · …")', /(^|[^.\d])1 · /.test(loneText) &&
    /(^|[^.\d])2 · /.test(loneText) && !/第\s*笔/.test(loneText), loneText.slice(0, 300));
  ns.i18n.setLocale('en');
  let deep = transferTo(THIRD, 1);
  for (let i = 0; i < 4; i++) deep = multiSend([{ to: MSCO, data: deep }]);
  const tooDeep = runOf(tx({ to: MSCO, operation: 1, data: multiSend([{ to: MSCO, data: deep }]) }));
  check('r10: an executed batch nested deeper than the core reads: refused', tooDeep.refuse &&
    warned(tooDeep, 'refuse.msBatchUnreadable'));
}

{
  // The levels the page lists agree with the core's one review, level by
  // level (vela-core `reading::MAX_NESTING`): what the core reads, the page
  // lists; what the core refuses, the page refuses.
  const transferTo = (to, amount) => '0x' + sel('transfer(address,uint256)') + word(to) + num(amount);
  const agree = [];
  for (let k = 0; k <= 5; k++) {
    let deep = transferTo(THIRD, 1);
    for (let i = 0; i < k; i++) deep = multiSend([{ to: MSCO, data: deep }]);
    const safeTx = tx({ to: MSCO, operation: 1, data: multiSend([{ to: k ? MSCO : USDC, data: deep }]) });
    const out = reviewOf({ chainId: CHAIN, safe: SAFE, tx: safeTx, owners: members.owners, threshold: members.threshold, deployed: true });
    const coreRefuses = !!out.thrown || out.sign?.allowed === false;
    agree.push([k, coreRefuses, view(request(safeTx)).refuse]);
  }
  check('r10: nesting — the page refuses exactly where the core does (0–5 batches deep)',
    agree.every(([, core, page]) => core === page) && agree.some(([, core]) => core) && agree.some(([, core]) => !core),
    JSON.stringify(agree));
}

// --- 3. a member's owner proof (vela_multisigCreate, §16) ----------------------
//
// Creating a multisig asks one member's passkey for the genesis service's
// sponsor challenge. The app hands this page the GENESIS: the address, the
// members, the rule and the challenge are read from it (lib/multisig.js
// `ownerProof`, transcribed from vela-core `trusted_signer::owner_proof`).
// Checked against the shared vectors AND the shipped wasm, case by case, then
// through the page's pipeline (resolve → digest → render) on the hostile cases.

{
  const vectors = JSON.parse(readFileSync(join(repo, 'rust/crates/vela-core/tests/vectors/multisig/owner-proof.json'), 'utf8'));
  check(`owner proof: the shared vectors are there (${vectors.cases.length})`, vectors.cases.length >= 30);
  for (const c of vectors.cases) {
    const ours = ns.multisig.ownerProof(c.params);
    const core = ms('ownerProofRead', { params: c.params });
    check(`owner proof — ${c.name}: the page reads it as vela-core (vector and wasm)`,
      canonical(ours) === canonical(c.expect) && canonical(core) === canonical(c.expect),
      `page ${JSON.stringify(ours)} · vector ${JSON.stringify(c.expect)} · wasm ${JSON.stringify(core)}`);
  }

  const BASE = 8453;
  const made = ms('genesis', { owners: [OTHER, THIRD, MEMBER], threshold: 2, name: 'Team fund', saltNonce: '0x75bcd15' });
  const me = ms('genesisMembers', { genesis: made.genesis }).owners[2];
  const proofParams = (extra = {}) => ms('ownerProofParams', {
    multisig: made.address, member: me, genesis: made.genesis, chainIds: [CHAIN, BASE], ...extra,
  });
  const proofIntent = (params = proofParams()) => ({ method: 'vela_multisigCreate', origin: '', params });
  const proofView = (intent = proofIntent(), ctx = {}) => ns.resolve(intent, {
    chainId: CHAIN, account: me, ...WALLET_CTX, method: intent.method, ...ctx,
  });

  const v = proofView();
  check('owner proof: read, not refused, its own title and button', !v.refuse && v.intentKey === 'intent.msOwnerProof' &&
    v.confirmKey === 'button.msOwnerProof' && v.badge === 'tag.multisig', JSON.stringify([v.refuse, v.warnings]));
  const row = v.fields.find((f) => f.label === 'field.multisig');
  check('owner proof: …the multisig derived from the genesis, its name the app\'s', row &&
    row.identity.address === made.address && row.identity.short === ns.format.short(made.address) &&
    row.identity.contractName === 'Team fund' && row.identity.tag === 'tag.nameByApp');
  check('owner proof: …6+6 EIP-55, as vela-core names::short_address writes it',
    /^0x[0-9a-fA-F]{6}…[0-9a-fA-F]{6}$/.test(row.identity.short) && row.identity.short.slice(2, 8) === made.address.slice(2, 8));
  const networks = v.fields.find((f) => f.label === 'field.msNetworks');
  check('owner proof: …its networks, by this page\'s names, as the app\'s word', networks &&
    networks.value === 'Arc Testnet, Base' && networks.tagKey === 'tag.byApp', JSON.stringify(networks));
  check('owner proof: …no single network row (the networks row says them all)', v.chain === null && v.chainId === null);
  const memberRows = v.fields.filter((f) => f.label === 'field.msMemberAt');
  check('owner proof: …every member, in the setup\'s order, numbered', memberRows.length === 3 &&
    memberRows.map((f) => f.labelParams.n).join(',') === '1,2,3' &&
    memberRows.map((f) => f.identity.address.toLowerCase()).join() === [OTHER, THIRD, MEMBER].map((a) => a.toLowerCase()).join());
  check('owner proof: …this signer\'s place marked "this account", and only its', memberRows[2].identity.tag === 'tag.self' &&
    !memberRows[0].identity.tag && !memberRows[1].identity.tag);
  const rule = v.fields.find((f) => f.label === 'field.threshold');
  check('owner proof: …the rule, "Needs 2 of 3"', rule && rule.value.key === 'value.msNeeds' &&
    rule.value.params.m === 2 && rule.value.params.n === 3 && ns.i18n.t(rule.value) === 'Needs 2 of 3');
  check('owner proof: …the sentence names its place', v.sentence.key === 'sentence.msOwnerProof' &&
    v.sentence.params.place === 3 && v.sentence.params.count === 3);
  check('owner proof: …the account signing is the member', v.account === me);
  check('owner proof: …read by this page from what is signed', v.provenance && v.provenance.kind === 'readMessage');

  const digest = ns.digest.of(proofIntent(), { account: me, chainId: CHAIN });
  check('owner proof: the passkey signs exactly vela-core\'s sponsor challenge for the DERIVED address',
    digest.hash && hex(digest.hash) === ms('sponsorChallenge', { multisig: made.address }));

  // What the person reads, drawn.
  const sheet = ns.render(v, {});
  const text = sheet.textContent;
  check('owner proof: the card says the multisig, the networks, the members, the rule, the button',
    ['Prove multisig membership', 'Arc Testnet, Base', 'Member 1', 'Member 3', 'Needs 2 of 3', 'Prove membership',
      row.identity.short].every((s) => text.includes(s)), text.slice(0, 600));
  ns.i18n.setLocale('zh');
  const zh = ns.render(proofView(), {}).textContent;
  check('owner proof: …in Chinese, from the page\'s own catalogue', ['证明多签成员身份', '需 3 位中的 2 位', '成员 1', '网络'].every((s) => zh.includes(s)),
    zh.slice(0, 400));
  ns.i18n.setLocale('en');

  // The hostile cases.
  const lying = proofView(proofIntent(proofParams({ multisig: OTHER })));
  check('owner proof: an address the genesis does not make — refused, loudly', lying.refuse && lying.risk === 'hard-danger' &&
    warned(lying, 'refuse.msOwnerAddress'));
  check('owner proof: …and nothing to sign', ns.digest.of(proofIntent(proofParams({ multisig: OTHER })), { account: me }).refuse === 'refuse.msOwnerAddress');
  const stranger = proofView(proofIntent(), { account: OTHER });
  check('owner proof: another account to sign with than the member — refused', stranger.refuse &&
    warned(stranger, 'refuse.msMemberMismatch'));
  check('owner proof: …and the digest refuses too', ns.digest.of(proofIntent(), { account: OTHER }).refuse === 'refuse.msMemberMismatch');
  const notMember = proofView(proofIntent(proofParams({ member: SAFE })), { account: SAFE });
  check('owner proof: a member the multisig does not have — refused', notMember.refuse && warned(notMember, 'refuse.msOwnerNotMember'));
  const hostile = JSON.parse(readFileSync(join(repo, 'rust/crates/vela-core/tests/vectors/multisig/hostile-geneses.json'), 'utf8'));
  const hooked = hostile.geneses.find((g) => g.name === 'HOOK_MODULE');
  const hookedGenesis = { factory: hostile.factory, singleton: hostile.singleton, saltNonce: hooked.saltNonce, initializer: hooked.initializer };
  const hookedOwner = ms('genesisMembers', { genesis: hookedGenesis }).owners[0];
  const hook = proofView(proofIntent([{ multisig: ms('genesisAddress', { genesis: hookedGenesis }), member: hookedOwner,
    genesis: hookedGenesis, chainIds: [CHAIN] }]), { account: hookedOwner });
  check('owner proof: a setup with a hook that enables an attacker\'s module — refused', hook.refuse &&
    warned(hook, 'refuse.msOwnerGenesis'));
  const supplied = proofView(proofIntent(), { challenge: '0x' + '11'.repeat(32) });
  check('owner proof: a requester that supplies a challenge — refused', supplied.refuse && warned(supplied, 'refuse.suppliedChallenge'));
  const named = proofView(proofIntent(proofParams({ genesis: { ...made.genesis, name: 'Team‮dnuf' } })));
  check('owner proof: a name a multisig cannot have is not drawn', !named.refuse &&
    named.fields.find((f) => f.label === 'field.multisig').identity.contractName === null);

  // The answer rule (102 R7): only a Vela wallet hears it.
  const site = proofView(proofIntent(), { requester: 'https://evil.example' });
  check('owner proof: opened by a site, over postMessage — refused, where the answer would go said', site.refuse &&
    site.warnings.some((w) => w.key === 'refuse.answerElsewhere'));
  const safari = 'safari-web-extension://0A1B2C3D-4E5F-4A6B-8C7D-9E0F1A2B3C4D';
  const fromSafari = proofView(proofIntent(), { requester: safari, attestedExtensions: [safari] });
  check('owner proof: the Safari extension, attested in this window — answered', !fromSafari.refuse, JSON.stringify(fromSafari.warnings));
  const unattested = proofView(proofIntent(), { requester: safari, attestedExtensions: [] });
  check('owner proof: …a Safari extension origin nobody attested — refused', unattested.refuse);
  const chrome = proofView(proofIntent(), { requester: ns.resolve.EXTENSIONS[0] });
  check('owner proof: the Chrome extension — answered', !chrome.refuse);
  const app = proofView(proofIntent(), { channel: 'url', callback: 'velawallet://sign-result', originVerified: false, requester: null });
  check('owner proof: an app, over the URL channel to velawallet:// — answered', !app.refuse, JSON.stringify(app.warnings));
}

function hasClass(node, name) {
  if (!node) return false;
  if (String(node.className || '').split(/\s+/).includes(name)) return true;
  return [...(node.children || [])].some((child) => hasClass(child, name));
}

// Every word this approval can put on the card, in every language.
const KEYS = Object.keys(ns.i18n.catalogue ? ns.i18n.catalogue('en') : {});
const msKeys = (KEYS.length ? KEYS : []).filter((k) => /\.ms[A-Z]|^ui\.copy$|^tag\.(multisig|thisMultisig)$|^field\.(multisig|member|newMember|oldMember|threshold|newName)$/.test(k));
const english = {};
ns.i18n.setLocale('en');
const sample = ['intent.msApprove', 'refuse.msHashMismatch', 'refuse.msDelegateCall', 'ui.msHashes', 'button.msApprove', 'warn.msMembers', 'sentence.msReject',
  // round 7: an execution
  'intent.msExecute', 'sentence.msExecute', 'sentence.msSends', 'sentence.msExecSetsUp', 'sentence.msExecActivates',
  'refuse.msExecShape', 'refuse.msExecExtra', 'refuse.msExecDeploy', 'refuse.msExecMismatch', 'refuse.msExecThreshold',
  'warn.msExecNoNonce', 'ui.msHashesExecNote', 'field.approvedBy', 'ui.walletWithdrew', 'ui.answerNotTaken',
  // round 10: the multisig's coins; a member change before → after
  // (`value.msCountChange` is "{before} → {after}" in every language)
  'sentence.approveMs', 'sentence.approveUnlimitedMs', 'warn.unlimitedMs', 'warn.approveAllMs', 'warn.nativeValueMs',
  'field.msMembers', 'value.msApprovalsChange', 'warn.msMembersClaimOff',
  // D1: the executor's "… anyway"
  'button.msExecuteAnyway', 'warn.msAppAnywayExec',
  // §16: a member's owner proof
  'intent.msOwnerProof', 'sentence.msOwnerProof', 'field.msNetworks', 'button.msOwnerProof',
  'refuse.msOwnerShape', 'refuse.msOwnerGenesis', 'refuse.msOwnerAddress', 'refuse.msOwnerNotMember'];
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
