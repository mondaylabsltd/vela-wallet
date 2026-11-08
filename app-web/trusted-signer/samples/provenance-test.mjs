// Whose reading is it? (2026-10-28 clear-signing audit, the page's half.)
//
// Every intent says where its reading came from, in one line under it — the
// web wallet's design (owner, 2026-10-28): CERTAIN, read by this page from
// the bytes; INTERPRETED, by a descriptor, a 4-byte name or a resemblance,
// with the source named; CLAIMED, by the app or the site. A descriptor does
// not make a contract honest. The adversarial cases, each on the view
// resolve.js builds and on the sheet render.js draws:
//
//   1. a protocol's function shape on a contract its descriptor was not
//      written for (1inch's `swap` on an attacker's contract) — a contract
//      call, "Sign", "Looks like a 1inch swap — shape only" (cs-4);
//   2. the right contract on the wrong chain — the same (cs-4, cs-13);
//   3. the app's `context.vaultAsset` — never a token amount (cs-4);
//   4. the app's simulation — never lowers a blind call's risk, never "can't
//      be faked" (cs-6);
//   5. output that goes to an address that is not this account, and a
//      minimum of nothing (cs-2);
//   6. the app's `currentAllowance` — its claim, said as one (cs-16);
//   7. the five provenance lines, in all fifteen locales;
//   8. no demo fixture in the published page (cs-20).
//
//   node samples/provenance-test.mjs        (and under bun: JavaScriptCore)
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { installFakeDom, loadPageLibs, makeChecks, root } from './test-kit.mjs';

installFakeDom();
// In the order `src/sign.html` loads them, every locale included.
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
  'src/lib/digest.js',
]);
ns.i18n.setLocale('en');
const t = ns.i18n.t;
const check = makeChecks();
const call = ns.encode.call;

const SAFE = '0x88cca0f8b4e1f0dc0e7c4f9a2b3d5e6f7a8b6894';
const THIEF = '0x0000000000000000000000000000000000badbad';
const ATTACKER = '0x618a3cd671d8e4d1efa8ac9fee42dbcdada5713a';
const ONEINCH = '0x1111111254eeb25477b68fb85ed929f73a960582';
const UNI_V2 = '0x7a250d5630b4cf539739df2c5dacb4c659f2488d';
const UNI_V3 = '0x68b3465833fb72a70ecdf485e0e4c7bd8665fc45';
const SDAI = '0x83f20f44975d03b1b09e64809b757c47f942beea';
const PERMIT2 = '0x000000000022d473030f116ddee9f6b43ac78ba3';
const USDC = '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48';
const DAI = '0x6b175474e89094c44da98b954eedeac495271d0f';
const WETH = '0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2';
const USDC_GNOSIS = '0xddafbb505ad214d7b80b1f830fccc89b60fb7a83';
const WXDAI = '0xe91d153e0b41518a2ce8dd3d7944fa863463a97d';
const RELAYER = '0x4d2c7a3b1e9f0a8b7c6d5e4f3a2b1c0d9e8f7a6b';
const MULTI_SEND = '0x38869bf66a61cf6bdb996a6ae40d5853fd43b526';
const AS_THE_APPS = { channel: 'url', callback: 'velawallet://sign-result' };

/** The call, assembled as the wallet assembles it (with a fee leg), and resolved. */
function sign(target, chainId, extra = {}) {
  const fee = { to: chainId === 100 ? USDC_GNOSIS : USDC, value: 0, data: call('transfer(address,uint256)', [RELAYER, 5000n]) };
  const leg = { to: target.to, value: target.value || 0n, data: target.data };
  const userOp = {
    sender: SAFE, nonce: '0x7', initCode: '0x',
    callData: call('executeUserOp(address,uint256,bytes,uint8)', [
      MULTI_SEND, 0n, call('multiSend(bytes)', [ns.encode.packMultiSend([leg, fee])]), 1,
    ]),
    verificationGasLimit: '0', callGasLimit: '0', preVerificationGas: '0',
    maxFeePerGas: '0', maxPriorityFeePerGas: '0', paymasterAndData: '0x', validAfter: '0', validUntil: '1800001800',
  };
  const intent = {
    method: 'eth_sendTransaction', origin: 'https://explore.example',
    params: [{ to: target.to, value: '0x' + (target.value || 0n).toString(16), data: target.data }],
  };
  const view = ns.resolve(intent, { chainId, account: SAFE, ...AS_THE_APPS, operation: { userOp, feeLegIndex: 1 }, ...extra });
  return { view, sheet: ns.render(view, {}) };
}
const sheetText = (sheet) => sheet.textContent;
// What the card says about the call: its blocks, not the footer (the fee leg
// of these operations is a USDC transfer of its own).
const blocksText = (sheet) => sheet.find('blocks').map((n) => n.textContent).join(' ');
const line = (sheet) => sheet.find('provenance-text').map((n) => n.textContent);
const warned = (view, key) => view.warnings.find((w) => w.key === key);

const oneInchSwap = (recipient, min = 10n ** 18n, dst = DAI, src = USDC) =>
  call('swap(address,(address,address,address,address,uint256,uint256,uint256),bytes,bytes)',
    [THIEF, [src, dst, THIEF, recipient, 250_000_000n, min, 0n], '0x', '0x']);

// --- 1. the same selector on a contract the descriptor was not written for ------
{
  for (const chainId of [1, 100]) {
    const { view, sheet } = sign({ to: ATTACKER, data: oneInchSwap(THIEF, 1n, WXDAI, USDC_GNOSIS) }, chainId);
    check(`chain ${chainId}: 1inch's swap shape on another contract is a contract call, not a swap`,
      view.intentKey === 'intent.contractCall' && view.kind !== 'swap', view.intentKey);
    check(`chain ${chainId}: …its button is the certain action, "Sign"`,
      view.confirmKey === 'button.sign' && !sheetText(sheet).includes(t('button.swap')), view.confirmKey);
    check(`chain ${chainId}: …and the resemblance is said: "Looks like a 1inch swap — shape only, not checked"`,
      view.provenance && view.provenance.kind === 'shape' &&
      line(sheet)[0] === t('prov.shape', { what: t('shape.oneInchSwap') }), line(sheet).join());
    check(`chain ${chainId}: …no token amount is drawn as a fact (no hero amount, no "You pay")`,
      (!view.hero || view.hero.kind !== 'amount') && !view.fields.some((f) => f.amount) &&
      !sheetText(sheet).includes('You pay'), view.hero && view.hero.kind);
    check(`chain ${chainId}: …every parameter is still read, by type`,
      view.fields.some((f) => f.labelRaw === 'address') && view.tech.signature.startsWith('swap('));
    check(`chain ${chainId}: …and the recipient in its bytes is not this account: a danger line`,
      !!warned(view, 'warn.notThisAccount') && warned(view, 'warn.notThisAccount').tone === 'danger' && view.risk === 'danger',
      view.risk);
    check(`chain ${chainId}: …and a 1-wei minimum is said to be next to nothing`,
      !!warned(view, 'warn.dustMinimum'), JSON.stringify(view.warnings.map((w) => w.key)));
  }

  // ERC-4626's deposit on a contract that is no vault this page knows, with
  // the app naming USDC as its asset (t15).
  const { view, sheet } = sign({ to: ATTACKER, data: call('deposit(uint256,address)', [1_000_000_000n, SAFE]), value: 2n * 10n ** 18n },
    100, { vaultAsset: USDC_GNOSIS });
  check('a vault\'s deposit shape on another contract: a contract call, "Looks like a vault deposit"',
    view.intentKey === 'intent.contractCall' && view.provenance.whatKey === 'shape.vaultDeposit' && view.confirmKey === 'button.sign');
  check('…context.vaultAsset is never a token amount: no "USDC" anywhere on the card',
    !blocksText(sheet).includes('USDC') && !view.fields.some((f) => f.amount && f.role === 'send-amount'), blocksText(sheet));
  check('…the amount is the raw integer from the bytes', view.fields.some((f) => f.labelRaw === 'uint256' && f.value === '1,000,000,000'));
  check('…the coin it carries is still a row and a caution', !!warned(view, 'warn.nativeValue'));

  // A Safe batch's shape on a contract that is not MultiSend: its legs are
  // read, under "contract call" and the line.
  const inner = call('transfer(address,uint256)', [THIEF, 5n]);
  const batch = sign({ to: ATTACKER, data: call('multiSend(bytes)', [ns.encode.packMultiSend([{ to: USDC, value: 0, data: inner }])]) }, 1);
  check('multiSend on a contract that is not MultiSend: a contract call that looks like a Safe batch, legs still read',
    batch.view.intentKey === 'intent.contractCall' && batch.view.provenance.whatKey === 'shape.safeBatch' && batch.view.legs.length === 1,
    batch.view.intentKey);
  const real = sign({ to: MULTI_SEND, data: call('multiSend(bytes)', [ns.encode.packMultiSend([{ to: USDC, value: 0, data: inner }])]) }, 1);
  check('…on Safe\'s MultiSend it is a batch, described by the page', real.view.intentKey === 'intent.multiSend' &&
    real.view.provenance.kind === 'described');
}

// --- 2. the right contract on the wrong chain ---------------------------------
{
  const own = sign({ to: ONEINCH, data: oneInchSwap(SAFE) }, 1);
  check('1inch\'s router on Ethereum: a swap, "Confirm swap"', own.view.intentKey === 'intent.swap' && own.view.confirmKey === 'button.swap');
  check('…described, and said to be a description: "Described by this page’s built-in notes on 1inch Router — a description, not a guarantee"',
    own.view.provenance.kind === 'described' &&
    line(own.sheet)[0] === t('prov.described', { source: t('prov.sourcePage', { contract: '1inch Router' }) }), line(own.sheet).join());
  check('…output to this account: "You receive", no danger line', own.view.sentence.key === 'sentence.swap' &&
    !warned(own.view, 'warn.notThisAccount') && !warned(own.view, 'warn.dustMinimum'));

  const swapIn = call('swapExactETHForTokens(uint256,address[],address,uint256)', [10n ** 18n, [WETH, DAI], SAFE, 9999999999n]);
  const mainnet = sign({ to: UNI_V2, data: swapIn, value: 10n ** 18n }, 1);
  const gnosis = sign({ to: UNI_V2, data: swapIn, value: 10n ** 18n }, 100);
  check('Uniswap V2\'s router address on Ethereum: a swap', mainnet.view.intentKey === 'intent.swap');
  check('…the same address on Gnosis: a contract call that only looks like a Uniswap swap',
    gnosis.view.intentKey === 'intent.contractCall' && gnosis.view.provenance.whatKey === 'shape.uniswapV2Swap', gnosis.view.intentKey);

  const vault = call('deposit(uint256,address)', [10n ** 18n, SAFE]);
  check('sDAI\'s address on Gnosis is no vault this page knows', sign({ to: SDAI, data: vault }, 100).view.intentKey === 'intent.contractCall');

  // cs-13: the known-contracts table is per chain.
  const blindV3 = (chainId) => sign({ to: UNI_V3, data: call('exactInputSingle((address,address,uint24,address,uint256,uint256,uint160))',
    [[USDC, DAI, 500n, SAFE, 1n, 1n, 0n]]) }, chainId);
  const v3gnosis = blindV3(100);
  const v3main = blindV3(1);
  check('the Uniswap V3 router\'s address on Gnosis is not named "Uniswap V3 Router"',
    !sheetText(v3gnosis.sheet).includes('Uniswap') && v3gnosis.view.hero.subParams.contract === '0x68b346…65Fc45',
    v3gnosis.view.hero.subParams.contract);
  check('…on Ethereum it is', v3main.view.hero.subParams.contract === 'Uniswap V3 Router');
  check('registry.contract is per chain', ns.registry.contract(UNI_V3, 1) && !ns.registry.contract(UNI_V3, 100) &&
    ns.registry.contract(PERMIT2, 100) && ns.registry.contract(PERMIT2, 8453));
}

// --- 3. the vault's asset is the page's, never the app's --------------------------
{
  const deposit = call('deposit(uint256,address)', [1000n * 10n ** 18n, SAFE]);
  const { view, sheet } = sign({ to: SDAI, data: deposit }, 1, { vaultAsset: USDC });
  check('sDAI on Ethereum with the app claiming USDC: 1,000 DAI — the vault\'s asset, not the claim',
    view.intentKey === 'intent.vaultDeposit' && view.hero.amount.symbol === 'DAI' && view.hero.amount.text === '1,000' &&
    !blocksText(sheet).includes('USDC'), `${view.hero.amount.text} ${view.hero.amount.symbol}`);
  check('…and it is a description: the line names the page\'s notes on the vault',
    view.provenance.kind === 'described' && view.provenance.contract === 'Savings DAI Vault');
  const away = sign({ to: SDAI, data: call('deposit(uint256,address)', [10n ** 18n, THIEF]) }, 1);
  check('…shares to an address that is not this account: "Goes to an address that is not this account"',
    away.view.risk === 'danger' && warned(away.view, 'warn.notThisAccount') &&
    away.sheet.find('warning-text').some((n) => n.textContent === t('warn.notThisAccount', { to: '0x000000…baDbad' })));
}

// --- 4. a simulation is the requester's ------------------------------------------
{
  const blindData = '0x034331ab' + '0'.repeat(63) + '7';
  const sim = { rows: [{ symbol: 'USDC', delta: '+100' }], note: 'You receive 100 USDC', summary: '+100 USDC · nothing else changes' };
  const bare = sign({ to: ATTACKER, data: blindData }, 100);
  const claimed = sign({ to: ATTACKER, data: blindData }, 100, { simulation: sim });
  check('a blind call with the app\'s simulation is no less dangerous than one without',
    bare.view.risk === 'danger' && claimed.view.risk === 'danger', `${bare.view.risk} / ${claimed.view.risk}`);
  check('…its warning is a danger, not a caution', warned(claimed.view, 'warn.blindSim').tone === 'danger');
  check('…the line says whose word it is: "Said by the requester"',
    claimed.view.provenance.kind === 'claimed' && line(claimed.sheet)[0] === t('prov.claimed'));
  check('…the balance card is drawn as the requester\'s', claimed.view.balance.claimed &&
    claimed.sheet.find('balance-claimed').length === 1);
  check('…the details mark the summary as the requester\'s', claimed.view.tech.simClaimed === true &&
    claimed.sheet.find('tech-note').some((n) => n.textContent === t('ui.simClaimed')));
  for (const locale of LOCALES) {
    ns.i18n.setLocale(locale);
    const words = t('sentence.blindSim') + ' ' + t('warn.blindSim');
    check(`${locale}: no "cannot be faked by the site" beside "supplied by the requester"`,
      !/faked|falsif|偽造|伪造|偽る|위조|làm giả|dipalsukan|taklit|gefälscht|подделать/i.test(words), words);
  }
  ns.i18n.setLocale('en');
  const noSim = sign({ to: ATTACKER, data: blindData }, 100);
  check('a blind call without one draws no provenance line (nothing was read)', !noSim.view.provenance && line(noSim.sheet).length === 0);
}

// --- 5. a minimum of nothing, said on the real router too --------------------------
{
  const { view, sheet } = sign({ to: ONEINCH, data: oneInchSwap(THIEF, 1n) }, 1);
  check('1inch\'s router, output to a stranger: not "you receive" but "goes to 0x000000…baDbad, not to this account"',
    view.sentence.key === 'sentence.swapElsewhere' && view.sentence.params.to === '0x000000…baDbad');
  check('…a danger line, and the card is red', !!warned(view, 'warn.notThisAccount') && view.risk === 'danger');
  check('…the 1-wei minimum is spelled out exactly', warned(view, 'warn.dustMinimum') &&
    warned(view, 'warn.dustMinimum').params.amount === '0.000000000000000001 DAI' &&
    sheet.find('warning-text').some((n) => n.textContent.includes('0.000000000000000001 DAI')));
  const fair = sign({ to: ONEINCH, data: oneInchSwap(SAFE, 240n * 10n ** 18n) }, 1);
  check('…a real minimum to this account: neither line', !warned(fair.view, 'warn.dustMinimum') && !warned(fair.view, 'warn.notThisAccount'));
  // No account in any digest: nothing can be said either way.
  const bare = ns.resolve({ method: 'eth_sendTransaction', origin: 'https://x.y', params: [{ to: ONEINCH, value: '0x0', data: oneInchSwap(THIEF) }] },
    { chainId: 1, ...AS_THE_APPS });
  check('no operation: "not this account" is not claimed', !warned(bare, 'warn.notThisAccount'));
}

// --- 6. the current allowance is the app's word ------------------------------------
{
  const inc = call('increaseAllowance(address,uint256)', [THIEF, 50_000_000n]);
  for (const current of ['5000000', 5_000_000, 5_000_000n]) {
    const { view, sheet } = sign({ to: USDC, data: inc }, 1, { currentAllowance: current });
    const after = view.fields.find((f) => f.label === 'field.allowanceAfter');
    check(`currentAllowance ${typeof current}: "allowance after" is 55 USDC, marked "per the app"`,
      !!after && after.value === '55 USDC' && after.tagKey === 'tag.byApp' &&
      sheet.find('tag').some((n) => n.textContent === t('tag.byApp')), after && after.value);
    check(`currentAllowance ${typeof current}: the sentence says the app says so`,
      view.sentence.key === 'sentence.increase' && t(view.sentence).includes('the app says 5 USDC'), t(view.sentence));
  }
  const unknown = sign({ to: USDC, data: inc }, 1);
  check('no currentAllowance: no made-up "0 now", no "allowance after"',
    unknown.view.sentence.key === 'sentence.increaseUnknown' && !unknown.view.fields.some((f) => f.label === 'field.allowanceAfter'),
    t(unknown.view.sentence));
  const junk = sign({ to: USDC, data: inc }, 1, { currentAllowance: 'lots' });
  check('an unreadable currentAllowance is no claim at all', junk.view.sentence.key === 'sentence.increaseUnknown');
}

// --- 7. the five lines, everywhere ----------------------------------------------------
{
  const transfer = call('transfer(address,uint256)', [THIEF, 1_000_000n]);
  const known = sign({ to: USDC, data: transfer }, 1);
  check('a transfer of a token this page knows here: "Read by this page from the transaction"',
    known.view.provenance.kind === 'read' && line(known.sheet)[0] === t('prov.read'));
  const unknownToken = sign({ to: ATTACKER, data: transfer }, 1);
  check('…of a token it does not: still a transfer (the standard is the meaning), but "Looks like a token call (ERC-20)"',
    unknownToken.view.intentKey === 'intent.transfer' && unknownToken.view.provenance.whatKey === 'shape.erc20');
  const plain = sign({ to: THIEF, data: '0x', value: 10n ** 18n }, 1);
  check('a plain send is read', plain.view.provenance.kind === 'read');
  const claim = sign({ to: ATTACKER, data: call('claim()', []) }, 1);
  check('a 4-byte name: "Function name from a public database (anyone can register one)"',
    claim.view.provenance.kind === 'fourbyte' && line(claim.sheet)[0] === t('prov.fourbyte'));
  const message = ns.resolve({ method: 'personal_sign', origin: 'https://x.y', params: ['0x68656c6c6f', SAFE] },
    { chainId: 1, account: SAFE, ...AS_THE_APPS });
  check('a message is read from what is signed', message.provenance.kind === 'readMessage');

  const permit2 = (verifyingContract) => ns.resolve({
    method: 'eth_signTypedData_v4', origin: 'https://x.y',
    params: [SAFE, JSON.stringify({
      types: {
        EIP712Domain: [], PermitSingle: [{ name: 'details', type: 'PermitDetails' }, { name: 'spender', type: 'address' }, { name: 'sigDeadline', type: 'uint256' }],
        PermitDetails: [{ name: 'token', type: 'address' }, { name: 'amount', type: 'uint160' }, { name: 'expiration', type: 'uint48' }, { name: 'nonce', type: 'uint48' }],
      },
      primaryType: 'PermitSingle', domain: { name: 'Permit2', chainId: 1, verifyingContract },
      message: { details: { token: USDC, amount: '1000000', expiration: '1893456000', nonce: '0' }, spender: THIEF, sigDeadline: '1893456000' },
    })],
  }, { chainId: 1, account: SAFE, ...AS_THE_APPS });
  const realP2 = permit2(PERMIT2);
  const fakeP2 = permit2(ATTACKER);
  check('a PermitSingle verified by Permit2 is read; by another contract it only looks like one',
    realP2.provenance.kind === 'readMessage' && fakeP2.provenance.kind === 'shape' && fakeP2.provenance.whatKey === 'shape.permit2');
  check('…and the details no longer call that other contract "Permit2"',
    fakeP2.tech.addresses[0].name !== 'Permit2' && realP2.tech.addresses[0].name === 'Permit2', fakeP2.tech.addresses[0].name);

  // A batch: every leg draws its own line.
  const batch = ns.resolve({
    method: 'wallet_sendCalls', origin: 'https://x.y',
    params: [{ calls: [
      { to: USDC, value: '0x0', data: transfer },
      { to: ATTACKER, value: '0x0', data: oneInchSwap(SAFE) },
      { to: ATTACKER, value: '0x0', data: call('claim()', []) },
    ] }],
  }, { chainId: 1, account: SAFE, ...AS_THE_APPS });
  const legLines = ns.render(batch, {}).find('leg').map((leg) => leg.find('provenance-text').map((n) => n.textContent).join());
  check('a batch: each leg says where its reading came from',
    legLines[0] === t('prov.read') && legLines[1] === t('prov.shape', { what: t('shape.oneInchSwap') }) &&
    legLines[2] === t('prov.fourbyte'), JSON.stringify(legLines));

  // Every locale draws real words for every line, with nothing left unfilled.
  const keys = ['prov.read', 'prov.readMessage', 'prov.described', 'prov.sourcePage', 'prov.fourbyte', 'prov.shape', 'prov.claimed',
    'shape.erc20', 'shape.erc721', 'shape.permit', 'shape.permit2', 'shape.vaultDeposit', 'shape.vaultWithdraw',
    'shape.oneInchSwap', 'shape.uniswapV2Swap', 'shape.safeBatch', 'shape.safeExec',
    'sentence.shapeOnly', 'sentence.swapElsewhere', 'sentence.increaseUnknown', 'warn.notThisAccount', 'warn.dustMinimum', 'tag.byApp'];
  for (const locale of LOCALES) {
    ns.i18n.setLocale(locale);
    const { sheet } = sign({ to: ATTACKER, data: oneInchSwap(THIEF, 1n) }, 1);
    const drawn = line(sheet)[0] || '';
    const missing = keys.filter((k) => t(k) === k);
    check(`${locale}: every provenance key is translated, and the shape line is filled in`,
      !missing.length && drawn && !/[{}]/.test(drawn) && drawn.includes('1inch'), missing.join() || drawn);
  }
  ns.i18n.setLocale('en');
}

// --- 8. the published page ships no demo fixture ---------------------------------------
{
  const page = readFileSync(join(root, 'src/sign.html'), 'utf8');
  const sources = [...page.matchAll(/<script src="([^"]+)"/g)].map((m) => readFileSync(join(root, 'src', m[1]), 'utf8')).join('\n');
  for (const fixture of ['StakePool', 'Alice Chen', '0x5f2b8c3a9d1e4f6a7b8c9d0e1f2a3b4c5d6e7f80', 'stake(uint256,uint256,address)']) {
    check(`the published page's scripts never name "${fixture}"`, !sources.toLowerCase().includes(fixture.toLowerCase()));
  }
  check('src/sign.html does not load the gallery\'s fixtures', !page.includes('fixtures.js'));
  check('no contacts table', ns.registry.contacts === undefined && ns.registry.contact === undefined);
  const gallery = readFileSync(join(root, 'demo/gallery.html'), 'utf8');
  check('the gallery still loads its fixtures', gallery.includes('<script src="fixtures.js"></script>'));
}

process.exit(check.summary() ? 0 : 1);
