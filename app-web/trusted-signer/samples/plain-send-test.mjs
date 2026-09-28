// Spec 082 G14 / RC8: a call with no calldata is a send.
//
// The wallet's own rule (vela-core `clear_signing::is_empty_calldata`): no
// calldata is absent, "", "0x" or "0X", after trimming whitespace, and such a
// call is a plain send of the native coin WHATEVER the recipient and whatever
// the value — 0 included. The page used to send a 0-value empty call down the
// blind ladder, where it read as a red 盲签 "nothing decodes" (level 6). It must
// reach the same verdict as the wallet: 发送 · 0 xDAI · 接收方, with no minus
// and no "nothing else changes" line. Only calldata goes down the ladder.
//
//   node samples/plain-send-test.mjs
import { readFileSync } from 'node:fs';
import { installFakeDom, loadPageLibs, makeChecks } from './test-kit.mjs';

installFakeDom();
// In the order `src/sign.html` loads them.
const ns = loadPageLibs([
  'src/lib/i18n.js',
  'src/lib/locales/en.js',
  'src/lib/locales/zh.js',
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
]);
ns.i18n.setLocale('zh');
const t = ns.i18n.t;
const check = makeChecks();

const SAFE = '0x88cca0f8b4e1f0dc0e7c4f9a2b3d5e6f7a8b6894';
// The device pass's recipient (quickstart DX-G14): another Safe, so it HAS code
// — which does not matter (RC2).
const TO = '0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141';
const USDC = '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48';
const RELAYER = '0x4d2c7a3b1e9f0a8b7c6d5e4f3a2b1c0d9e8f7a6b';
const MULTI_SEND = '0x38869bf66a61cf6bdb996a6ae40d5853fd43b526';
const GNOSIS = 100;

const tx = (call, ctx = {}) => ns.resolve(
  { method: 'eth_sendTransaction', origin: 'http://127.0.0.1:8137', params: [call] },
  { account: SAFE, chainId: GNOSIS, ...ctx },
);
const draw = (view) => ns.render(view, {});
const heroText = (sheet) => sheet.find('.amount').map((n) => n.textContent).join();
const slideText = (sheet) => sheet.find('.slide-label').map((n) => n.textContent).join();
const isSend = (view) => view.intentKey === 'intent.send' && view.hero && view.hero.kind === 'amount';
const noDanger = (view) => view.risk !== 'danger' && view.risk !== 'hard-danger' &&
  !view.warnings.some((w) => w.tone === 'danger');

// 1. What the device pass sends (DX-G14): 0.001 xDAI, no data.
{
  const view = tx({ to: TO, value: '0x38d7ea4c68000' });
  const sheet = draw(view);
  check('DX-G14: 0.001 xDAI with no data is a send', isSend(view) && noDanger(view));
  check('DX-G14: the hero reads 0.001 xDAI', heroText(sheet) === '0.001' && view.hero.amount.symbol === 'xDAI',
    heroText(sheet) + ' ' + view.hero.amount.symbol);
  check('DX-G14: the recipient row names 0x7687…D141',
    sheet.find('.identity-address').some((n) => n.textContent === '0x7687…D141'));
  check('DX-G14: the tech panel keeps "−0.001 xDAI · nothing else changes"',
    sheet.find('.tech-sim').map((n) => n.textContent).join() === t('ui.simNoOther', { delta: '-0.001 xDAI' }));
}

// 2. Value 0: the same card, "0", no minus, no "nothing else changes" line.
for (const [name, call] of [
  ["value '0x0'", { to: TO, value: '0x0' }],
  ["value '0x'", { to: TO, value: '0x' }],
  ['value omitted', { to: TO }],
  ['value 0 (a number)', { to: TO, value: 0 }],
  ['value null', { to: TO, value: null }],
]) {
  const view = tx(call);
  const sheet = draw(view);
  check(`${name}: a send, not the blind ladder`, isSend(view) && view.level === 1 && noDanger(view),
    `${view.intentKey} level ${view.level} ${view.risk}`);
  check(`${name}: the hero reads 0, with no minus`, heroText(sheet) === '0' && !/[-−]/.test(heroText(sheet)),
    heroText(sheet));
  check(`${name}: no "nothing else changes" line`, sheet.find('.tech-sim').length === 0 && !view.tech.sim);
  check(`${name}: the slide says ${t('intent.send')}, not ${t('intent.blind')}`,
    slideText(sheet) === t('ui.slide', { intent: t('intent.send') }), slideText(sheet));
}

// 3. Every spelling of "no calldata" the wallet accepts, at value 0.
for (const data of [undefined, null, '', '0x', '0X', ' 0x ', '\t0X\n', '   ']) {
  const view = tx({ to: TO, value: '0x0', data });
  check(`data ${JSON.stringify(data)}: a send`, isSend(view) && noDanger(view), `${view.intentKey} ${view.risk}`);
}

// 4. Anything that IS calldata still goes down the ladder.
for (const data of ['0x12', '0x00', '0xdeadbeef']) {
  const view = tx({ to: TO, value: '0x0', data });
  check(`data ${data}: not a send (the ladder decides)`, view.intentKey !== 'intent.send', view.intentKey);
}

// 5. A value no chain carries is not printed as a send.
{
  const view = tx({ to: TO, value: '-5' });
  check('a negative value is not drawn as a send', view.intentKey !== 'intent.send', view.intentKey);
}

// 6. The same verdict inside the operation the wallet actually signs: the
//    page draws the operation's call (value '0x0', data '0x'), and the site's
//    own spelling of "no calldata" / "zero" still matches it.
{
  const feeLeg = { to: USDC, value: 0, data: ns.encode.call('transfer(address,uint256)', [RELAYER, 420000n]) };
  const operationOf = (calls) => ({
    userOp: {
      sender: SAFE,
      nonce: '0x7',
      initCode: '0x',
      callData: ns.encode.call('executeUserOp(address,uint256,bytes,uint8)', [
        MULTI_SEND, 0n, ns.encode.call('multiSend(bytes)', [ns.encode.packMultiSend(calls)]), 1,
      ]),
      verificationGasLimit: '0', callGasLimit: '0', preVerificationGas: '0',
      maxFeePerGas: '0', maxPriorityFeePerGas: '0', paymasterAndData: '0x',
    },
    feeLegIndex: 1,
  });
  const ctx = { currency: '$', rates: { USDC: 1 }, operation: operationOf([{ to: TO, value: 0, data: '0x' }, feeLeg]) };
  for (const asked of [
    { to: TO, value: '0x0' },
    { to: TO, value: '0x', data: '0X' },
    { to: TO, data: ' 0x ' },
  ]) {
    const view = tx(asked, ctx);
    check(`in the operation, asked as ${JSON.stringify(asked)}: a send of 0, not refused`,
      isSend(view) && !view.refuse && heroText(draw(view)) === '0' && noDanger(view),
      `${view.intentKey} refuse=${!!view.refuse} ${JSON.stringify(view.warnings.map((w) => w.key))}`);
  }
  // The binding check is unchanged for a real difference.
  const other = tx({ to: TO, value: '0x1' }, ctx);
  check('in the operation, a site that asked for 1 wei is still refused', other.refuse === true);
}

// 7. A batch leg and a Safe multiSend leg with no calldata are sends too, and
//    never lift a danger into the batch.
{
  const transfer = ns.encode.call('transfer(address,uint256)', [TO, 5n]);
  const batch = ns.resolve({
    method: 'wallet_sendCalls', origin: 'http://127.0.0.1:8137',
    params: [{ calls: [{ to: USDC, value: '0x0', data: transfer }, { to: TO, value: '0x0', data: '0x' }] }],
  }, { account: SAFE, chainId: GNOSIS });
  const leg = batch.legs[1].view;
  check('wallet_sendCalls: a 0-value empty leg is a send', isSend(leg) && noDanger(leg), `${leg.intentKey} ${leg.risk}`);
  check('wallet_sendCalls: no danger reaches the batch', noDanger(batch), JSON.stringify(batch.warnings.map((w) => w.key)));

  const nested = tx({
    to: MULTI_SEND, value: '0x0',
    data: ns.encode.call('multiSend(bytes)', [ns.encode.packMultiSend([
      { to: USDC, value: 0, data: transfer }, { to: TO, value: 0, data: '0x' },
    ])]),
  });
  const inner = nested.legs && nested.legs[1] && nested.legs[1].view;
  check('multiSend: a 0-value empty leg is a send', !!inner && isSend(inner) && noDanger(inner),
    inner ? `${inner.intentKey} ${inner.risk}` : 'no legs');
}

// 8. The figure is exact (the wallet's RC5: no rounding, trailing zeros
//    trimmed). Now that "0" is what a zero send reads, a send that DOES move a
//    few wei must not read "0" as well — the page used to cut every native
//    figure at six decimals, so anything under 0.000001 xDAI drew as "0".
{
  for (const [value, want] of [
    ['0x1', '0.000000000000000001'],
    ['0xe8d4a50fff', '0.000000999999999999'],
    ['0xde0b6b3a7640001', '1.000000000000000001'],
    ['0x38d7ea4c68000', '0.001'],
  ]) {
    const view = tx({ to: TO, value });
    const sheet = draw(view);
    check(`value ${value}: the hero reads ${want}`, isSend(view) && heroText(sheet) === want, heroText(sheet));
    check(`value ${value}: the tech panel's minus is the same figure`,
      sheet.find('.tech-sim').map((n) => n.textContent).join() === t('ui.simNoOther', { delta: '-' + want + ' xDAI' }),
      sheet.find('.tech-sim').map((n) => n.textContent).join());
  }
  // The same inside the operation the wallet signs: 1 wei is not "0".
  const feeLeg = { to: USDC, value: 0, data: ns.encode.call('transfer(address,uint256)', [RELAYER, 420000n]) };
  const operation = {
    userOp: {
      sender: SAFE, nonce: '0x7', initCode: '0x',
      callData: ns.encode.call('executeUserOp(address,uint256,bytes,uint8)', [
        MULTI_SEND, 0n,
        ns.encode.call('multiSend(bytes)', [ns.encode.packMultiSend([{ to: TO, value: 1, data: '0x' }, feeLeg])]), 1,
      ]),
      verificationGasLimit: '0', callGasLimit: '0', preVerificationGas: '0',
      maxFeePerGas: '0', maxPriorityFeePerGas: '0', paymasterAndData: '0x',
    },
    feeLegIndex: 1,
  };
  const view = tx({ to: TO, value: '0x1' }, { operation });
  check('in the operation, 1 wei reads 0.000000000000000001, not 0',
    isSend(view) && !view.refuse && heroText(draw(view)) === '0.000000000000000001', heroText(draw(view)));

  // The exact figure fits the card: at 32px "1,000.000000000000000001" ran
  // under the card's edge on a phone and read 1,000 — the last digit, the one
  // that differs, was the one cut. A long figure steps down a size, and every
  // line that can carry it wraps rather than clip. (The DOM stand-in cannot
  // measure; the device pass looks at O-T1 on a phone.)
  const size = (value) => draw(tx({ to: TO, value })).find('.amount')[0].classes()
    .filter((c) => c === 'amount-long' || c === 'amount-longer').join() || '32px';
  check('0.001 keeps the full size', size('0x38d7ea4c68000') === '32px', size('0x38d7ea4c68000'));
  check('1,000.000000000000000001 steps down to the smallest size',
    size('0x3635c9adc5dea00001') === 'amount-longer',
    size('0x3635c9adc5dea00001'));
  const css = readFileSync(new URL('../src/sheet.css', import.meta.url), 'utf8');
  const rule = (selector) => {
    const at = css.indexOf('\n' + selector + ' {');
    return at < 0 ? '' : css.slice(at, css.indexOf('}', at));
  };
  for (const selector of ['.amount', '.sentence', '.tech-sim']) {
    check(`${selector} wraps a figure wider than the card`, /overflow-wrap:\s*anywhere/.test(rule(selector)),
      rule(selector).replace(/\s+/g, ' ').trim() || 'no rule');
  }
}

// 9. A value the page cannot read never takes the page down. Before, BigInt()
//    threw inside resolve(): no card, no refusal, only a status line — and a
//    page being closed cannot hand the wallet its answer over the custom
//    scheme. No submit path builds an operation from such a value; the page
//    prints no figure for it and, handed an operation anyway, draws what the
//    operation does and refuses, since it cannot find the site's call in it.
{
  const feeLeg = { to: USDC, value: 0, data: ns.encode.call('transfer(address,uint256)', [RELAYER, 420000n]) };
  const operation = {
    userOp: {
      sender: SAFE, nonce: '0x7', initCode: '0x',
      callData: ns.encode.call('executeUserOp(address,uint256,bytes,uint8)', [
        MULTI_SEND, 0n,
        ns.encode.call('multiSend(bytes)', [ns.encode.packMultiSend([{ to: TO, value: 0, data: '0x' }, feeLeg])]), 1,
      ]),
      verificationGasLimit: '0', callGasLimit: '0', preVerificationGas: '0',
      maxFeePerGas: '0', maxPriorityFeePerGas: '0', paymasterAndData: '0x',
    },
    feeLegIndex: 1,
  };
  // "0b1" and "0o7" are BigInt()'s in every engine, and "0x0x", "0x 1" or
  // "1 000" are nobody's. Run this file under node AND bun.
  for (const value of ['0x0x', '0x 1', '1 000', 'abc', '-1', '-0x1', '0b1', '0o7', '1e3', '0x1g', 1.5, -1, NaN]) {
    let bare = null;
    let bound = null;
    let error = '';
    try {
      bare = tx({ to: TO, value });
      bound = tx({ to: TO, value }, { operation });
      draw(bare);
      draw(bound);
    } catch (e) {
      error = String(e && e.message);
    }
    check(`value ${JSON.stringify(value)}: the page draws a card`, !error && !!bare && !!bound, error);
    check(`value ${JSON.stringify(value)}: no figure is printed for it`, !!bare && bare.intentKey !== 'intent.send',
      bare ? bare.intentKey : 'threw');
    check(`value ${JSON.stringify(value)}: inside an operation it is refused`,
      !!bound && bound.refuse === true && bound.warnings.some((w) => w.key === 'refuse.opMismatch'),
      bound ? JSON.stringify(bound.warnings.map((w) => w.key)) : 'threw');
  }
}

// 10. The reading itself (`abi.quantity`), the same in every engine: the
//     page's grammar, never BigInt()'s, and the reading the wallet's submit
//     path gives (the desktop's `wei_of`, vela-core `to_quantity`): trimmed,
//     "0x" or "0X" hex, plain decimal digits, a non-negative integral number.
//     " 0x " is 0 under V8 and JavaScriptCore alike, because BigInt() never
//     sees it.
{
  const read = (v) => { const w = ns.abi.quantity(v); return w === null ? null : w.toString(); };
  for (const [value, want] of [
    [undefined, '0'], [null, '0'], ['', '0'], ['0x', '0'], ['0x0', '0'], ['0x1F', '31'],
    ['1000', '1000'], [1000, '1000'], [0, '0'], [10n, '10'],
    [' 0x ', '0'], ['0x ', '0'], [' 0x', '0'], ['\t0x\n', '0'], [' ', '0'], ['0X', '0'],
    ['0X10', '16'], [' 0x10 ', '16'], ['\t1000\n', '1000'], [2 ** 53, '9007199254740992'],
    [1e18, '1000000000000000000'],
    ['-1', null], ['-0x1', null], ['+1', null], ['0b1', null], ['0o7', null], ['0x0x', null],
    ['0x 1', null], ['1 000', null], ['0x1g', null],
    ['1e3', null], [1.5, null], [-1, null], [-1n, null], [NaN, null], [Infinity, null],
    [true, null], [{}, null],
  ]) {
    const label = typeof value === 'bigint' ? value + 'n' : typeof value === 'string' ? JSON.stringify(value) : String(value);
    check(`quantity(${label}) is ${want === null ? 'unreadable' : want}`, read(value) === want, String(read(value)));
  }
}

// 11. The operation the desktop built from the site's own spelling is found in
//     it. The desktop reads a dApp's value leniently (executor/sign_request.rs
//     `wei_of`: trimmed, "0x" or "0X", else decimal; a JSON number as itself)
//     and assembles exactly that figure. Before 082 the page read these with
//     BigInt() and signed; a reading stricter than the one that built the
//     operation refuses a transaction nobody altered, and the refusal it
//     draws (refuse.opMismatch) tells the person the wallet tampered with it.
//     What the card states comes from the operation either way (section 6).
{
  const feeLeg = { to: USDC, value: 0, data: ns.encode.call('transfer(address,uint256)', [RELAYER, 420000n]) };
  const operationFor = (wei) => ({
    userOp: {
      sender: SAFE, nonce: '0x7', initCode: '0x',
      callData: ns.encode.call('executeUserOp(address,uint256,bytes,uint8)', [
        MULTI_SEND, 0n,
        ns.encode.call('multiSend(bytes)', [ns.encode.packMultiSend([{ to: TO, value: wei, data: '0x' }, feeLeg])]), 1,
      ]),
      verificationGasLimit: '0', callGasLimit: '0', preVerificationGas: '0',
      maxFeePerGas: '0', maxPriorityFeePerGas: '0', paymasterAndData: '0x',
    },
    feeLegIndex: 1,
  });
  for (const [asked, wei, want] of [
    ['0X38D7EA4C68000', 1000000000000000n, '0.001'],
    [' 0x38d7ea4c68000 ', 1000000000000000n, '0.001'],
    ['0x38d7ea4c68000\n', 1000000000000000n, '0.001'],
    [1e18, 10n ** 18n, '1'],
    ['0X', 0n, '0'],
    ['0x ', 0n, '0'],
    [' 0x ', 0n, '0'],
    [' ', 0n, '0'],
    [' 1000 ', 1000n, '0.000000000000001'],
  ]) {
    let view = null;
    let error = '';
    try {
      view = tx({ to: TO, value: asked }, { operation: operationFor(wei) });
    } catch (e) {
      error = String(e && e.message);
    }
    check(`the desktop's operation for value ${JSON.stringify(asked)} is signable, not "altered during assembly"`,
      !!view && !view.refuse && !view.warnings.some((w) => w.key === 'refuse.opMismatch'),
      error || JSON.stringify(view.warnings.map((w) => w.key)));
    check(`value ${JSON.stringify(asked)}: the card states the operation's ${want} xDAI`,
      !!view && isSend(view) && heroText(draw(view)) === want, view ? heroText(draw(view)) : error);
  }
  // A real difference is still a refusal: the site asked 0X10, the operation carries 17.
  const altered = tx({ to: TO, value: '0X10' }, { operation: operationFor(17n) });
  check('value "0X10" against an operation carrying 17 wei is still refused',
    altered.refuse === true && altered.warnings.some((w) => w.key === 'refuse.opMismatch'));
}

process.exit(check.summary() ? 0 : 1);
