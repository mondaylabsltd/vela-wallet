// Whose word is it? (2026-10-28 exploratory pass on the signing page.)
//
// Five things the requester writes into `context` had been drawn as the
// page's own facts. Each is checked here on the view resolve.js builds and on
// the sheet render.js draws from it:
//
//   · "this account" — only for the address the signature is FOR
//     (`userOp.sender`; the account a message is wrapped under), never for
//     whatever `context.account` names: one naming the recipient drew a
//     stranger as "this account", and its send as safe;
//   · the fee's coin — from this page's catalogue for the chain, not the
//     requester's `nativeSymbol` ("0.0009 USDC" on Ethereum; "ETH" by default);
//     on a chain the page does not know, the app's symbol marked as the app's
//     ("coin named by the app") beside the chain id, and "?" when it gives none;
//   · fiat — the requester's rate times a fact, so it is drawn as the
//     requester's (`tag.rateByRequester`), and only in an ISO 4217 currency:
//     free text there was a line of the requester's own words on the card;
//   · invisible characters — direction overrides and zero-width marks in a
//     message or a name are drawn as their code points, with a caution;
//   · a key id the browser cannot be asked for — refused on the card, with
//     the reason, instead of an `atob` that threw inside the tap.
//
// (A site's name that is not its host is origin-line-test's, section 3.)
//
//   node samples/claims-test.mjs
import { installFakeDom, loadPageLibs, makeChecks } from './test-kit.mjs';

installFakeDom();
// In the order `src/sign.html` loads them.
const ns = loadPageLibs([
  'src/lib/i18n.js',
  'src/lib/locales/en.js',
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
  'src/lib/digest.js',
  'src/lib/signer.js',
  'src/lib/ceremony.js',
]);
ns.i18n.setLocale('en');
const t = ns.i18n.t;
const check = makeChecks();

const SAFE = '0x88cca0f8b4e1f0dc0e7c4f9a2b3d5e6f7a8b6894';
const RECIPIENT = '0xaf5e8917831ef08a64e18b2cde9f8f5d32c7b3e1';
const USDC = '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48';
const RELAYER = '0x4d2c7a3b1e9f0a8b7c6d5e4f3a2b1c0d9e8f7a6b';
const MULTI_SEND = '0x38869bf66a61cf6bdb996a6ae40d5853fd43b526';
const AS_THE_APPS = { channel: 'url', callback: 'velawallet://sign-result' };
const transfer = ns.encode.call('transfer(address,uint256)', [RECIPIENT, 1_000_000_000n]);

const userOpOf = (calls, gas = {}) => ({
  sender: SAFE, nonce: '0x7', initCode: '0x',
  callData: ns.encode.call('executeUserOp(address,uint256,bytes,uint8)', [
    MULTI_SEND, 0n, ns.encode.call('multiSend(bytes)', [ns.encode.packMultiSend(calls)]), 1,
  ]),
  verificationGasLimit: '0', callGasLimit: '0', preVerificationGas: '0',
  maxFeePerGas: '0', maxPriorityFeePerGas: '0', paymasterAndData: '0x', validAfter: '0', validUntil: '1800001800', ...gas,
});
const feeLeg = { to: USDC, value: 0, data: ns.encode.call('transfer(address,uint256)', [RELAYER, 5000n]) };
const asked = { method: 'eth_sendTransaction', origin: 'https://app.uniswap.org', params: [{ to: USDC, value: '0x0', data: transfer }] };
const operation = { userOp: userOpOf([{ to: USDC, value: 0, data: transfer }, feeLeg]), feeLegIndex: 1 };
const sign = (ctx, intent = asked) => {
  const view = ns.resolve(intent, { chainId: 1, account: SAFE, ...AS_THE_APPS, operation, ...ctx });
  return { view, sheet: ns.render(view, {}) };
};
const rowTags = (sheet) => sheet.find('row-tag').map((n) => n.textContent);
const texts = (sheet, name) => sheet.find(name).map((n) => n.textContent);

// --- 1. "this account" is the signature's own account ---------------------------
{
  const { view, sheet } = sign({ account: RECIPIENT });
  const to = view.fields.find((f) => f.role === 'recipient');
  check('context.account naming the recipient: no "this account" on the recipient',
    !!to && to.identity.tag !== 'tag.self' && !rowTags(sheet).includes(t('tag.self')), JSON.stringify(rowTags(sheet)));
  check('…and the send is not drawn as safe', view.risk !== 'safe', view.risk);
  check('…the signing account is still the operation\'s sender', view.account === SAFE, view.account);

  // The real self-send keeps its tag: the recipient IS userOp.sender.
  const self = ns.encode.call('transfer(address,uint256)', [SAFE, 1n]);
  const own = sign({ account: RECIPIENT, operation: { userOp: userOpOf([{ to: USDC, value: 0, data: self }, feeLeg]), feeLegIndex: 1 } },
    { ...asked, params: [{ to: USDC, value: '0x0', data: self }] });
  check('a transfer to userOp.sender is "this account", whatever context.account says',
    rowTags(own.sheet).includes(t('tag.self')), JSON.stringify(rowTags(own.sheet)));

  // No operation: no account in any digest, so nothing is tagged.
  const bare = ns.resolve({ method: 'eth_sendTransaction', origin: 'https://x.y', params: [{ to: RECIPIENT, value: '0x1' }] },
    { chainId: 1, account: RECIPIENT, ...AS_THE_APPS });
  check('a transaction with no operation tags nobody "this account"',
    bare.fields.every((f) => !f.identity || f.identity.tag !== 'tag.self') && bare.risk !== 'safe', bare.risk);

  // A message is wrapped under context.account (SafeMessage), so that IS the account.
  const typed = ns.resolve({
    method: 'eth_signTypedData_v4', origin: 'https://x.y',
    params: [SAFE, JSON.stringify({
      types: { EIP712Domain: [], Permit: [{ name: 'owner', type: 'address' }, { name: 'spender', type: 'address' }, { name: 'value', type: 'uint256' }, { name: 'nonce', type: 'uint256' }, { name: 'deadline', type: 'uint256' }] },
      primaryType: 'Permit', domain: { verifyingContract: USDC },
      message: { owner: SAFE, spender: SAFE, value: '1', nonce: '0', deadline: '9999999999' },
    })],
  }, { chainId: 1, account: SAFE, ...AS_THE_APPS });
  check('a message: the account it is wrapped under is "this account"',
    typed.fields.some((f) => f.identity && f.identity.tag === 'tag.self'));
  // …and a requester cannot hand the page its own answer to the question.
  const planted = sign({ account: RECIPIENT, selfAccount: RECIPIENT });
  check('a context.selfAccount of the requester\'s is ignored',
    planted.view.fields.every((f) => !f.identity || f.identity.tag !== 'tag.self'));
}

// --- 2. the fee's coin is the chain's, from this page's catalogue ---------------
{
  const gas = { verificationGasLimit: '100000', callGasLimit: '100000', preVerificationGas: '105000', maxFeePerGas: '3000000000' };
  const gasOp = { userOp: userOpOf([{ to: USDC, value: 0, data: transfer }], gas) };
  for (const [chainId, nativeSymbol, want] of [[1, 'USDC', 'ETH'], [100, 'ETH', 'xDAI'], [5042002, 'ETH', 'USDC'], [137, undefined, 'POL']]) {
    const { view, sheet } = sign({ chainId, nativeSymbol, operation: gasOp });
    const fee = texts(sheet, 'fee-value').join(' ');
    check(`gas on chain ${chainId} with nativeSymbol ${nativeSymbol}: the fee is in ${want}`,
      view.fee && view.fee.gas && view.fee.gas.symbol === want && fee.includes(' ' + want), fee);
  }
  // A native fee leg, the same.
  const nativeFee = { to: RELAYER, value: 10n ** 15n, data: '0x' };
  const legOp = { userOp: userOpOf([{ to: USDC, value: 0, data: transfer }, nativeFee]), feeLegIndex: 1 };
  const leg = sign({ chainId: 100, nativeSymbol: 'ETH', operation: legOp });
  check('a native fee leg on Gnosis is xDAI, not the requester\'s ETH',
    leg.view.fee.leg && leg.view.fee.leg.symbol === 'xDAI', leg.view.fee.leg && leg.view.fee.leg.symbol);
  // The catalogue is the core's, testnets included (gen-catalog).
  check('Arc Testnet is a page fact now: named by the page, not "named by the app"',
    ns.registry.chainName(5042002) === 'Arc Testnet' && ns.registry.nativeSymbol(5042002) === 'USDC');
}

// --- 2b. a chain the page does not know: its coin is the app's word -------------
//
// Only for a chain missing from the page's table does the symbol come from the
// requester's `nativeSymbol`, and then it is not a fact: "ETH" on a chain
// whose coin is worth nothing reads as a price it does not have. The id (the
// one thing about the chain that is signed) is on the network row, and the
// symbol is marked wherever it names an amount; nobody naming it is "?".
{
  const UNKNOWN = 424242;
  const coinTag = t('tag.coinByApp');
  const claimedCoin = (sheet) => sheet.find('coin-claimed').map((n) => n.textContent);
  const value = 10n ** 18n;
  const send = { method: 'eth_sendTransaction', origin: 'https://x.y', params: [{ to: RECIPIENT, value: '0x' + value.toString(16) }] };
  const sendOp = { userOp: userOpOf([{ to: RECIPIENT, value, data: '0x' }]) };

  const app = sign({ chainId: UNKNOWN, chainName: 'Foochain', nativeSymbol: 'ETH', operation: sendOp }, send);
  check('unknown chain, app says ETH: the hero says ETH, as the app\'s word',
    app.view.hero.amount.symbol === 'ETH' && app.view.hero.amount.symbolClaimed === true &&
    app.sheet.find('hero-amount')[0].textContent.includes(coinTag), app.sheet.find('hero-amount')[0].textContent);
  const network = app.sheet.find('network-row')[0].textContent;
  check('…the network row shows the chain id and that the coin is the app\'s',
    network.includes(t('value.chainId', { id: String(UNKNOWN) })) && network.includes('ETH ' + coinTag) &&
    network.includes(t('tag.nameByApp')), network);

  const gas = { verificationGasLimit: '100000', callGasLimit: '100000', preVerificationGas: '105000', maxFeePerGas: '3000000000' };
  const gasFee = sign({ chainId: UNKNOWN, nativeSymbol: 'ETH', operation: { userOp: userOpOf([{ to: RECIPIENT, value, data: '0x' }], gas) } }, send);
  check('…the gas ceiling is in the app\'s ETH, and its note says so',
    gasFee.view.fee.gas.symbol === 'ETH' && gasFee.view.fee.gas.symbolClaimed === true &&
    texts(gasFee.sheet, 'fee-coin').join() === 'ETH ' + coinTag, texts(gasFee.sheet, 'fee-coin').join());
  const nativeFee = { to: RELAYER, value: 10n ** 15n, data: '0x' };
  const legFee = sign({ chainId: UNKNOWN, nativeSymbol: 'ETH', operation: { userOp: userOpOf([{ to: RECIPIENT, value, data: '0x' }, nativeFee]), feeLegIndex: 1 } }, send);
  check('…a native fee leg, the same',
    legFee.view.fee.leg.symbol === 'ETH' && legFee.view.fee.leg.symbolClaimed === true &&
    texts(legFee.sheet, 'fee-coin').join() === 'ETH ' + coinTag, texts(legFee.sheet, 'fee-coin').join());

  // Coin sent along with calldata: the native row carries the mark too.
  const deposit = ns.encode.call('deposit()', []);
  const withData = sign({ chainId: UNKNOWN, nativeSymbol: 'ETH', operation: { userOp: userOpOf([{ to: RECIPIENT, value, data: deposit }]) } },
    { method: 'eth_sendTransaction', origin: 'https://x.y', params: [{ to: RECIPIENT, value: '0x' + value.toString(16), data: deposit }] });
  const nativeRow = withData.view.fields.find((f) => f.role === 'native-out');
  check('…coin attached to a call: its row is marked as the app\'s coin',
    !!nativeRow && nativeRow.amount.symbolClaimed === true &&
    // the network row's, and the native row's own
    claimedCoin(withData.sheet).filter((s) => s === coinTag).length === 2,
    JSON.stringify(claimedCoin(withData.sheet)));

  // A chain the page knows: the page's own coin, no mark anywhere.
  const known = sign({ chainId: 1, nativeSymbol: 'USDC', operation: sendOp }, send);
  check('a known chain: the page\'s coin, nothing marked, no chain id line',
    known.view.hero.amount.symbol === 'ETH' && !known.view.hero.amount.symbolClaimed &&
    claimedCoin(known.sheet).length === 0 && known.view.chainId === null && known.sheet.find('network-id').length === 0);

  // Nobody names the coin: "?", never a guessed ETH.
  const nobody = sign({ chainId: UNKNOWN, operation: { userOp: userOpOf([{ to: RECIPIENT, value, data: '0x' }], gas) } }, send);
  check('unknown chain, no symbol from the app: "?", not "ETH", and not marked as the app\'s',
    nobody.view.hero.amount.symbol === '?' && nobody.view.fee.gas.symbol === '?' &&
    !nobody.view.hero.amount.symbolClaimed && claimedCoin(nobody.sheet).length === 0,
    nobody.view.hero.amount.symbol + ' / ' + nobody.view.fee.gas.symbol);
  check('…and the network row is the chain id, in the page\'s words',
    texts(nobody.sheet, 'network-name').join() === t('value.chainId', { id: String(UNKNOWN) }),
    texts(nobody.sheet, 'network-name').join());

  // The app's words are drawn as what they are: hidden characters shown, and cut short.
  const hidden = sign({ chainId: UNKNOWN, chainName: 'Foo‮chain', nativeSymbol: 'E​TH', operation: sendOp }, send);
  check('a symbol or chain name with hidden characters: shown as code points, with a caution',
    hidden.view.hero.amount.symbol === 'E[U+200B]TH' && hidden.view.chain === 'Foo[U+202E]chain' &&
    hidden.view.warnings.some((w) => w.key === 'warn.invisibleChars'), hidden.view.hero.amount.symbol);
  const long = sign({ chainId: UNKNOWN, nativeSymbol: 'X'.repeat(80), operation: sendOp }, send);
  check('a symbol of 80 characters is cut to 12', long.view.hero.amount.symbol === 'X'.repeat(12));
  const planted = sign({ chainId: 1, coin: { symbol: 'FAKE', claimed: false }, operation: sendOp }, send);
  check('a context.coin of the requester\'s is ignored', planted.view.hero.amount.symbol === 'ETH');
}

// --- 3. fiat is the requester's, in an ISO currency only -------------------------
{
  const claimed = (sheet) => sheet.find('amount-fiat').concat(sheet.find('row-sub'), sheet.find('fee-value'));
  const ok = sign({ currency: 'USD', rates: { USDC: 1 } });
  const hero = texts(ok.sheet, 'amount-fiat').join();
  check('an ISO currency: the hero\'s fiat is drawn, as the requester\'s rate',
    hero === '≈ USD 1,000 ' + t('tag.rateByRequester'), hero);
  const fee = texts(ok.sheet, 'fee-value').join();
  check('…and the fee\'s fiat says whose rate too', fee.includes('≈ USD 0.01') && fee.includes(t('tag.rateByRequester')), fee);
  check('…and the fee note names the rate', texts(ok.sheet, 'fee-note').join().includes('1 USDC = USD 1'),
    texts(ok.sheet, 'fee-note').join());

  for (const currency of ['$', '<img src=x onerror=alert(1)>', 'usd', 'US Dollar', 'USDT', '', 42, null, undefined]) {
    const { view, sheet } = sign({ currency, rates: { USDC: 1e12 } });
    check(`currency ${JSON.stringify(currency)}: no fiat anywhere`,
      !view.hero.amount.fiat && !(view.fee.leg && view.fee.leg.fiat) &&
      claimed(sheet).every((n) => !n.textContent.includes('≈')), claimed(sheet).map((n) => n.textContent).join(' | '));
  }
  for (const rate of [-5, 0, Infinity, NaN, '1']) {
    const { view } = sign({ currency: 'USD', rates: { USDC: rate } });
    check(`rate ${String(rate)}: no fiat`, !view.hero.amount.fiat);
  }
  const inherited = sign({ currency: 'USD', rates: Object.create({ USDC: 1 }) });
  check('a rate the rates object does not own: no fiat', !inherited.view.hero.amount.fiat);
}

// --- 4. invisible characters are made visible, and said --------------------------
{
  const RLO = '\u202e';
  const PDF = '\u202c';
  const ZWSP = '\u200b';
  const message = (text, origin = 'https://some.dapp') => ns.resolve({ method: 'personal_sign', origin, params: [text, SAFE] },
    { chainId: 1, account: SAFE, ...AS_THE_APPS });
  const original = 'Pay 1 USDC to bob' + RLO + 'evil' + PDF + ZWSP + ' — ok?';
  const m = message(original);
  const sheet = ns.render(m, {});
  const block = texts(sheet, 'block-message').join();
  check('a message with a direction override: drawn with its code points',
    block === 'Pay 1 USDC to bob[U+202E]evil[U+202C][U+200B] — ok?', block);
  check('…and the card says why', m.warnings.some((w) => w.key === 'warn.invisibleChars') &&
    texts(sheet, 'warning-text').includes(t('warn.invisibleChars')) && m.risk === 'caution', m.risk);
  // The bytes signed are the bytes sent: only the drawing changes.
  const signed = ns.digest.of({ method: 'personal_sign', params: [original, SAFE] }, { chainId: 1, account: SAFE });
  const hex = (bytes) => Buffer.from(bytes).toString('hex');
  check('…while the digest is over the original text, characters and all',
    hex(signed.hash) === hex(ns.safeop.messageHash(ns.digest.personalSignHash(original), 1, SAFE)) &&
    hex(ns.digest.personalSignHash(original)) !== hex(ns.digest.personalSignHash(m.hero.text)));
  const fromHex = message('0x' + Buffer.from('hello' + RLO + 'olleh', 'utf8').toString('hex'));
  check('a hex message that decodes to text with an override: the same',
    texts(ns.render(fromHex, {}), 'block-message').join() === 'hello[U+202E]olleh');
  const clean = message('Hello from the signer tester');
  check('a plain message: untouched, no warning', clean.hero.text === 'Hello from the signer tester' &&
    !clean.warnings.some((w) => w.key === 'warn.invisibleChars'));
  const siwe = message('app.uniswap.org wants you to sign in with your Ethereum account:\n' + SAFE + '\n\nSign in to Uni' + RLO + 'paws\n\nURI: https://app.uniswap.org\nNonce: 1', 'https://app.uniswap.org');
  check('a sign-in statement with an override: drawn visibly, said, not "safe"',
    siwe.hero.statement === 'Sign in to Uni[U+202E]paws' && siwe.risk === 'caution' &&
    siwe.warnings.some((w) => w.key === 'warn.invisibleChars'), siwe.risk);

  // Names: the account's, and a site's claim.
  const named = sign({ signer: { name: 'Vela' + RLO + 'niw' + PDF + ' Official' } });
  check('an account name with an override: drawn visibly, with the warning',
    texts(named.sheet, 'identity-name').join() === 'Vela[U+202E]niw[U+202C] Official' &&
    named.view.warnings.some((w) => w.key === 'warn.invisibleChars'), texts(named.sheet, 'identity-name').join());
  const site = sign({ dapp: { name: 'Uni' + ZWSP + 'swap', source: 'vela_browser' } });
  check('a site\'s claimed name with a zero-width space: drawn visibly, with the warning',
    texts(site.sheet, 'dapp-claimed').join().includes('Uni[U+200B]swap') &&
    site.view.warnings.some((w) => w.key === 'warn.invisibleChars'), texts(site.sheet, 'dapp-claimed').join());
  const typed = ns.resolve({
    method: 'eth_signTypedData_v4', origin: 'https://x.y',
    params: [SAFE, JSON.stringify({
      types: { EIP712Domain: [], Note: [{ name: 'text', type: 'string' }] },
      primaryType: 'Note', domain: { name: 'App' }, message: { text: 'to bob' + RLO + 'evil' },
    })],
  }, { chainId: 1, account: SAFE, ...AS_THE_APPS });
  check('a typed-data field with an override: drawn visibly, with the warning',
    typed.fields.some((f) => f.value === 'to bob[U+202E]evil') && typed.warnings.some((w) => w.key === 'warn.invisibleChars'));
}

// --- 5. a key id the browser cannot be asked for --------------------------------
{
  for (const allow of [['not base64 !!'], ['A'.repeat(17)], [42], [null], 'abc', {}]) {
    const { view, sheet } = sign({ allowCredentials: allow });
    check(`allowCredentials ${JSON.stringify(allow)}: refused, and the card says why`,
      view.refuse === true && view.warnings.some((w) => w.key === 'refuse.badKeyId') &&
      texts(sheet, 'warning-text').includes(t('refuse.badKeyId')) && view.confirmKey === 'button.cannotSign');
  }
  const good = sign({ allowCredentials: ['A'.repeat(16), 'B'.repeat(22)] });
  check('readable key ids: signable as before', !good.view.refuse, JSON.stringify(good.view.warnings.map((w) => w.key)));
  check('an unreadable key route is dropped, not followed',
    sign({ keyRoute: { credentialId: 'A'.repeat(17), place: 'platform' } }).view.key === null);
  // And should one ever reach the tap, the passkey call fails as a promise
  // with a reason the page words — never a throw that leaves it busy.
  globalThis.location = { hostname: 'getvela.app' };
  let threw = false;
  let rejected = null;
  try {
    ns.signer.sign(new Uint8Array(32), { allowCredentials: ['not base64 !!'] }).catch((e) => { rejected = e; });
  } catch (e) {
    threw = true;
  }
  await new Promise((r) => setTimeout(r, 0));
  check('signer.sign with an unreadable id rejects (badKeyId), it does not throw',
    !threw && !!rejected && rejected.badKeyId === true);
  rejected = null;
  ns.ceremony.assert(new Uint8Array(32), ['not base64 !!'], null).catch((e) => { rejected = e; });
  await new Promise((r) => setTimeout(r, 0));
  check('ceremony.assert, the same', !!rejected && rejected.badKeyId === true);
  const ceremony = ns.resolve({ method: 'vela_proof', params: [{ purpose: 'verify', credentialId: 'A'.repeat(17) }] },
    { rpId: 'getvela.app', ...AS_THE_APPS });
  check('a proof naming an unreadable key id is refused as malformed', ceremony.refuse === true &&
    ceremony.warnings.some((w) => w.key === 'refuse.badCeremony'));
}

process.exit(check.summary() ? 0 : 1);
