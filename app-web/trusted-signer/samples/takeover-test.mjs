// The page refuses what would hand the account over — the same rule as the
// core's `self_call_guard.rs` (spec 081), checked again here because the page
// does not trust the app that assembled the operation.
//
//   node samples/takeover-test.mjs
//
// Node only, zero dependencies: the page's own libraries, loaded as the page
// loads them, and `resolve()` run over each shape.
import { loadPageLibs, makeChecks } from './test-kit.mjs';

// In the order `src/sign.html` loads them.
const ns = loadPageLibs([
  'src/lib/i18n.js',
  'src/lib/locales/en.js',
  'src/lib/keccak.js',
  'src/lib/identicon-features.js',
  'src/lib/identicon.js',
  'src/lib/abi.js',
  'src/lib/encode.js',
  'src/lib/fee.js',
  'src/lib/logos.js',
  'src/lib/catalog.js',
  'src/lib/registry.js',
  'src/lib/resolve.js',
  'src/lib/safeop.js',
]);
const check = makeChecks();

const ACCOUNT = '0x' + 'aa'.repeat(20);
const OTHER = '0x' + 'bb'.repeat(20);
const MULTI_SEND = '0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526';

const word = (hex) => hex.replace(/^0x/, '').toLowerCase().padStart(64, '0');
const num = (n) => BigInt(n).toString(16).padStart(64, '0');
const bytes = (hex) => {
  const body = hex.replace(/^0x/, '');
  const padded = body.padEnd(Math.ceil(body.length / 64) * 64, '0');
  return num(body.length / 2) + padded;
};

const addOwner = '0x0d582f13' + word(OTHER) + num(1);
const transfer = '0xa9059cbb' + word(OTHER) + num(1000);

/** Safe multiSend's packed legs: operation(1) ‖ to(20) ‖ value(32) ‖ len(32) ‖ data. */
function packed(legs) {
  return legs
    .map(({ operation = 0, to, data = '0x' }) => {
      const body = data.replace(/^0x/, '');
      return operation.toString(16).padStart(2, '0') + to.replace(/^0x/, '').toLowerCase() +
        num(0) + num(body.length / 2) + body;
    })
    .join('');
}
const multiSend = (legs) => '0x8d80ff0a' + num(32) + bytes(packed(legs));

/** execTransaction(to, value, data, operation, 0, 0, 0, 0x0, 0x0, 0x) */
function execTransaction(to, data, operation) {
  const head = word(to) + num(0) + num(32 * 10) + num(operation) + num(0) + num(0) + num(0) +
    word('0x0') + word('0x0');
  const dataPart = bytes(data);
  const sigOffset = 32 * 10 + dataPart.length / 2;
  return '0x6a761202' + head.slice(0, 64 * 9) + num(sigOffset) + dataPart + num(0);
}

/** Safe4337Module.executeUserOp(to, value, data, operation) — the operation's callData. */
function executeUserOp(to, data, operation) {
  const selector = ns.keccak.selector('executeUserOp(address,uint256,bytes,uint8)');
  return selector + word(to) + num(0) + num(128) + num(operation) + bytes(data);
}

const tx = (to, data) => ({ method: 'eth_sendTransaction', origin: 'https://site.test', params: [{ to, data, value: '0x0' }] });
// Over the URL, answered to the wallet's own address, as the apps send it —
// otherwise every signature is refused for that alone (spec 102 R7).
const ctx = { account: ACCOUNT, chainId: 100, channel: 'url', callback: 'velawallet://sign-result' };
const refusal = (view, key) => view.refuse && view.warnings.some((w) => w.key === key);
const noTakeover = (view) =>
  !view.warnings.some((w) => ['refuse.selfCall', 'refuse.delegateCall', 'refuse.safeTx'].includes(w.key));

// --- the request itself ------------------------------------------------------

let view = ns.resolve(tx(ACCOUNT, addOwner), ctx);
check('addOwnerWithThreshold on the account is refused', refusal(view, 'refuse.selfCall'));
check('…and the warning names the function',
  view.warnings.some((w) => w.key === 'refuse.selfCall' && w.params && w.params.fn === 'addOwnerWithThreshold'));

view = ns.resolve(tx(OTHER, addOwner), ctx);
check('the same call to another Safe is not a takeover of this one', noTakeover(view));

view = ns.resolve(tx(ACCOUNT, '0x'), { ...ctx });
check('an empty self-call (the fee leg\'s shape) is allowed', noTakeover(view));

view = ns.resolve({
  method: 'wallet_sendCalls', origin: 'https://site.test',
  params: [{ calls: [{ to: OTHER, data: transfer, value: '0x0' }, { to: OTHER, data: multiSend([{ operation: 1, to: OTHER, data: transfer }]), value: '0x0' }] }],
}, ctx);
check('a delegatecall inside a batched multiSend is refused', refusal(view, 'refuse.delegateCall'));

view = ns.resolve(tx(OTHER, multiSend([{ to: ACCOUNT, data: '0x610b5925' + word(OTHER) }])), ctx);
check('enableModule on the account, one multiSend down, is refused', refusal(view, 'refuse.selfCall'));

view = ns.resolve(tx(OTHER, execTransaction(OTHER, transfer, 1)), ctx);
check('execTransaction carrying a delegatecall is refused', refusal(view, 'refuse.delegateCall'));

view = ns.resolve(tx(OTHER, execTransaction(ACCOUNT, '0xf08a0323' + word(OTHER), 0)), ctx);
check('setFallbackHandler on the account inside execTransaction is refused', refusal(view, 'refuse.selfCall'));

view = ns.resolve(tx(OTHER, transfer), ctx);
check('an ordinary token transfer is untouched', noTakeover(view));

// --- the operation the app assembled -----------------------------------------

const userOp = (callData) => ({ operation: { userOp: { sender: ACCOUNT, callData } } });

view = ns.resolve(tx(OTHER, transfer), {
  ...ctx,
  ...userOp(executeUserOp(MULTI_SEND, multiSend([{ to: OTHER, data: transfer }, { to: ACCOUNT, data: '0x' }]), 1)),
});
check('Vela\'s own shape (MultiSend, the call, an empty fee leg) signs', noTakeover(view) && !refusal(view, 'refuse.opMismatch'));

view = ns.resolve(tx(OTHER, transfer), {
  ...ctx,
  ...userOp(executeUserOp(MULTI_SEND, multiSend([{ to: OTHER, data: transfer }, { to: ACCOUNT, data: addOwner }]), 1)),
});
check('a self-call leg the app slipped into the operation is refused', refusal(view, 'refuse.selfCall'));

view = ns.resolve(tx(OTHER, transfer), {
  ...ctx,
  ...userOp(executeUserOp(MULTI_SEND, multiSend([{ to: OTHER, data: transfer }, { operation: 1, to: OTHER, data: transfer }]), 1)),
});
check('a delegatecall leg inside the operation\'s MultiSend is refused', refusal(view, 'refuse.delegateCall'));

view = ns.resolve(tx(OTHER, transfer), { ...ctx, ...userOp(executeUserOp(OTHER, transfer, 1)) });
check('an operation that delegatecalls anything but MultiSend is refused', refusal(view, 'refuse.delegateCall'));

// --- typed data --------------------------------------------------------------

const typed = (primaryType) => ({
  method: 'eth_signTypedData_v4',
  origin: 'https://site.test',
  params: [ACCOUNT, JSON.stringify({
    primaryType,
    domain: { chainId: 100, verifyingContract: ACCOUNT },
    types: { EIP712Domain: [], [primaryType]: [] },
    message: {},
  })],
});
view = ns.resolve(typed('SafeTx'), ctx);
check('a SafeTx signature is refused', refusal(view, 'refuse.safeTx'));
view = ns.resolve(typed('Mail'), ctx);
check('other typed data is not treated as SafeTx', !view.warnings.some((w) => w.key === 'refuse.safeTx'));

// --- words -------------------------------------------------------------------

for (const key of ['refuse.selfCall', 'refuse.delegateCall', 'refuse.safeTx']) {
  const phrase = ns.i18n ? ns.i18n.t(key, { fn: 'enableModule' }) : null;
  check(`"${key}" has English words`, typeof phrase === 'string' && phrase !== key && !phrase.includes('{'), phrase);
}

process.exit(check.summary() ? 0 : 1);
