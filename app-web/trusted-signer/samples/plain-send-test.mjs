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

process.exit(check.summary() ? 0 : 1);
