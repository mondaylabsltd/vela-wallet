// Spec 079: a plain send is a send, not 批量 of two. Every Vela operation
// carries its fee as a leg of its own, and the page used to draw that leg as
// the second call of a batch. The fee row already shows it in full — the
// amount and the recipient, both read from the calldata — so a fee leg the row
// can show leaves the list, and one it cannot stays a leg.
//
// §35 (section 6): the operation's expiry, read from its window, is in the
// fee note; an operation that never expires, or whose window does not read,
// is refused on the card.
//
//   node samples/fee-leg-test.mjs
import { installFakeDom, loadPageLibs, makeChecks } from './test-kit.mjs';

installFakeDom();
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
  'src/lib/render.js',
  'src/lib/safeop.js',
  'src/lib/digest.js',
]);
ns.i18n.setLocale('en');
const t = ns.i18n.t;
const check = makeChecks();

const SAFE = '0x88cca0f8b4e1f0dc0e7c4f9a2b3d5e6f7a8b6894';
const USDC = '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48';
const RECIPIENT = '0x9a8b7c6d5e4f3a2b1c0d9e8f7a6b5c4d3e2f1a09';
const RELAYER = '0x4d2c7a3b1e9f0a8b7c6d5e4f3a2b1c0d9e8f7a6b';
const SPENDER = '0x' + 'bb'.repeat(20);
const MULTI_SEND = '0x38869bf66a61cf6bdb996a6ae40d5853fd43b526';
const VALID_UNTIL = '1800001800';

const transfer = { to: USDC, value: 0, data: ns.encode.call('transfer(address,uint256)', [RECIPIENT, 7654321000n]) };
const feePayment = { to: USDC, value: 0, data: ns.encode.call('transfer(address,uint256)', [RELAYER, 420000n]) };
const approve = { to: USDC, value: 0, data: ns.encode.call('approve(address,uint256)', [SPENDER, 5n]) };

function userOpOf(calls) {
  return {
    sender: SAFE,
    nonce: '0x7',
    initCode: '0x',
    callData: ns.encode.call('executeUserOp(address,uint256,bytes,uint8)', [
      MULTI_SEND, 0n, ns.encode.call('multiSend(bytes)', [ns.encode.packMultiSend(calls)]), 1,
    ]),
    verificationGasLimit: '0', callGasLimit: '0', preVerificationGas: '0',
    maxFeePerGas: '0', maxPriorityFeePerGas: '0', paymasterAndData: '0x',
    // §35: stamped as the core stamps (no lower bound, a fixed second).
    validAfter: '0', validUntil: VALID_UNTIL,
  };
}
const asked = (call) => ({
  method: 'eth_sendTransaction',
  origin: 'https://app.example',
  params: [{ to: call.to, value: '0x0', data: call.data }],
});
// As the apps send it: over the URL, answered to the wallet (spec 102 R7).
const ctxOf = (calls, feeLegIndex) => ({
  account: SAFE, chainId: 1, currency: '$', rates: { USDC: 1 }, channel: 'url', callback: 'velawallet://sign-result',
  operation: { userOp: userOpOf(calls), feeLegIndex },
});

// 1. A send with its fee: the send, and the fee row.
let view = ns.resolve(asked(transfer), ctxOf([transfer, feePayment], 1));
check('a send with its fee leg reads as a send', view.method === 'eth_sendTransaction', view.method);
check('…not as a batch', view.intentKey !== 'intent.batch' && view.legs.length === 0, view.intentKey);
check('the fee row carries the fee leg, read from the calldata',
  view.fee && view.fee.leg && view.fee.leg.amount === '0.42' && view.fee.leg.to.toLowerCase() === RELAYER);
check('nothing is refused', !view.refuse);

// 2. Two calls the site asked for, plus the fee: a batch of the two.
const second = { to: USDC, value: 0, data: ns.encode.call('transfer(address,uint256)', [SPENDER, 1000000n]) };
view = ns.resolve({
  method: 'wallet_sendCalls', origin: 'https://app.example',
  params: [{ calls: [transfer, second].map((c) => ({ to: c.to, value: '0x0', data: c.data })) }],
}, ctxOf([transfer, second, feePayment], 2));
check('two asked-for calls and a fee are a batch of two', view.intentKey === 'intent.batch' && view.legs.length === 2, String(view.legs.length));

// 3. A "fee" the row cannot show in full stays a leg — nothing is hidden behind the label.
view = ns.resolve(asked(transfer), ctxOf([transfer, approve], 1));
check('a "fee" that is not a plain payment stays a leg', view.legs.length === 2 && !(view.fee && view.fee.leg));

// 4. The operation must still contain what the site asked for.
view = ns.resolve(asked(second), ctxOf([transfer, feePayment], 1));
check('an operation without the asked-for call is still refused', view.refuse === true);

// 5. An operation that is only the fee payment is not emptied.
view = ns.resolve(asked(feePayment), ctxOf([feePayment], 0));
check('a lone leg marked as the fee is still drawn', view.method === 'eth_sendTransaction' && !view.refuse);

// 6. §35: the operation's validity window. The card says until when the
// operation is valid — read from the bytes being signed — and refuses one
// that never expires, or whose window does not read.
const localDate = (seconds) => {
  const d = new Date(Number(seconds) * 1000);
  const pad = (n) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
};
const withWindow = (calls, feeLegIndex, window) => {
  const ctx = ctxOf(calls, feeLegIndex);
  const userOp = { ...ctx.operation.userOp };
  delete userOp.validAfter;
  delete userOp.validUntil;
  return { ...ctx, operation: { ...ctx.operation, userOp: { ...userOp, ...window } } };
};
const drawn = (intent, ctx) => {
  const v = ns.resolve(intent, ctx);
  return { view: v, sheet: ns.render(v, {}), digest: ns.digest.of(intent, ctx) };
};
const warned = (v, key) => v.warnings.some((w) => w.key === key);
const expiryLine = (sheet) => sheet.find('fee-expires').map((n) => n.textContent);

let card = drawn(asked(transfer), ctxOf([transfer, feePayment], 1));
check('expiry · the card says until when the operation is valid (validUntil, the page’s date form)',
  card.view.expires === localDate(VALID_UNTIL) && !card.view.refuse, card.view.expires);
check('expiry · drawn in the fee note: "Valid until <date>"',
  expiryLine(card.sheet).length === 1 && expiryLine(card.sheet)[0] === `${t('field.validUntil')} ${localDate(VALID_UNTIL)}`,
  JSON.stringify(expiryLine(card.sheet)));
check('expiry · and the digest is the SafeOp hash over that window', !!card.digest.hash && !card.digest.refuse);

// A fee the page cannot state still leaves the expiry in sight.
card = drawn(asked(transfer), ctxOf([transfer], undefined));
check('expiry · shown even when the fee is unstated',
  card.view.fee && card.view.fee.unknown && expiryLine(card.sheet).length === 1 &&
  expiryLine(card.sheet)[0].endsWith(localDate(VALID_UNTIL)), JSON.stringify(expiryLine(card.sheet)));

// The widest uint48 second is past what a date holds: the second, never "NaN".
const widest = String((1n << 48n) - 1n);
card = drawn(asked(transfer), withWindow([transfer, feePayment], 1, { validAfter: '0', validUntil: widest }));
check('expiry · the widest uint48 second is shown as itself, not "NaN"',
  card.view.expires === widest && !card.view.refuse && !expiryLine(card.sheet)[0].includes('NaN'), card.view.expires);

// No expiry: refused on the card, in its words, with the button disabled.
for (const [window, name] of [
  [{}, 'no validAfter / validUntil at all'],
  [{ validAfter: '0' }, 'no validUntil'],
  [{ validAfter: '0', validUntil: '0' }, 'validUntil "0"'],
]) {
  card = drawn(asked(transfer), withWindow([transfer, feePayment], 1, window));
  const button = card.sheet.find('confirm')[0];
  check(`no expiry · ${name}: refused refuse.noExpiry, no expiry drawn, button disabled, no digest`,
    card.view.refuse && warned(card.view, 'refuse.noExpiry') && !card.view.expires &&
    expiryLine(card.sheet).length === 0 && button && button.disabled === true &&
    card.digest.refuse === 'refuse.noExpiry' && !card.digest.hash,
    card.view.warnings.map((w) => w.key).join(','));
}
check('no expiry · the refusal is words, not a key', t('refuse.noExpiry') !== 'refuse.noExpiry' && /expire/.test(t('refuse.noExpiry')));

// A window that does not read: refused in its own words, never signed.
for (const [window, name] of [
  [{ validAfter: '0', validUntil: String(1n << 48n) }, 'validUntil past uint48 max'],
  [{ validAfter: '0', validUntil: '-1800001800' }, 'a negative validUntil'],
  [{ validAfter: '0', validUntil: 'tomorrow' }, 'a non-numeric validUntil'],
  [{ validAfter: '-1', validUntil: VALID_UNTIL }, 'a negative validAfter'],
  [{ validAfter: '1800001801', validUntil: VALID_UNTIL }, 'a window that opens after it closes'],
]) {
  card = drawn(asked(transfer), withWindow([transfer, feePayment], 1, window));
  check(`bad window · ${name}: refused refuse.badWindow, no digest`,
    card.view.refuse && warned(card.view, 'refuse.badWindow') && !card.view.expires &&
    card.sheet.find('confirm')[0].disabled === true &&
    card.digest.refuse === 'refuse.badWindow' && !card.digest.hash,
    card.view.warnings.map((w) => w.key).join(','));
}
check('bad window · the refusal is words, not a key', t('refuse.badWindow') !== 'refuse.badWindow');

process.exitCode = check.summary() ? 0 : 1;
