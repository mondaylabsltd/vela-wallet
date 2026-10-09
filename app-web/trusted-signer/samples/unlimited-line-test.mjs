// Where "unlimited" starts on this page — the wallet's own two lines
// (vela-core `approval_guard`): 2^200 for a uint256 amount, 2^152 for
// Permit2's uint160 one. The app's sheet draws the same line.
//
//   node samples/unlimited-line-test.mjs
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
const SPENDER = '0x' + 'bb'.repeat(20);
const USDC = '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48';
const PERMIT2 = '0x000000000022d473030f116ddee9f6b43ac78ba3';
// As the apps send it: over the URL, answered to the wallet (spec 102 R7).
const ctx = { account: ACCOUNT, chainId: 1, now: 1_700_000_000_000, channel: 'url', callback: 'velawallet://sign-result' };

const word = (n) => BigInt(n).toString(16).padStart(64, '0');
const approve = (amount) => ns.resolve({
  method: 'eth_sendTransaction', origin: 'https://site.test',
  params: [{ to: USDC, value: '0x0', data: '0x095ea7b3' + SPENDER.slice(2).padStart(64, '0') + word(amount) }],
}, ctx);
const typed = (primaryType, message, verifyingContract) => ns.resolve({
  method: 'eth_signTypedData_v4', origin: 'https://site.test',
  params: [ACCOUNT, JSON.stringify({ primaryType, domain: { chainId: 1, verifyingContract }, types: {}, message })],
}, ctx);
const heroUnlimited = (view) => !!(view.hero && view.hero.amount && view.hero.amount.unlimited);

const MAX256 = (1n << 256n) - 1n;
const MAX160 = (1n << 160n) - 1n;

check('approve(MaxUint256) — what Uniswap sends to Permit2 — is unlimited', heroUnlimited(approve(MAX256)));
check('approve(2^255 - 1) is unlimited', heroUnlimited(approve((1n << 255n) - 1n)));
check('approve(2^200) is unlimited', heroUnlimited(approve(1n << 200n)));
check('approve(2^200 - 1) is a figure', !heroUnlimited(approve((1n << 200n) - 1n)));
check('approve(2^128) — past every supply, below the line — is a figure', !heroUnlimited(approve(1n << 128n)));

const permit = (value) => typed('Permit', { owner: ACCOUNT, spender: SPENDER, value: String(value), nonce: '0', deadline: '1900000000' }, USDC);
check('ERC-2612 permit of 2^200 is unlimited', heroUnlimited(permit(1n << 200n)));
check('ERC-2612 permit of 2^200 - 1 is a figure', !heroUnlimited(permit((1n << 200n) - 1n)));

const single = (amount) => typed('PermitSingle', {
  details: { token: USDC, amount: String(amount), expiration: '1900000000', nonce: '0' },
  spender: SPENDER, sigDeadline: '1900000000',
}, PERMIT2);
check('PermitSingle for uint160 max — what Uniswap signs — is unlimited', heroUnlimited(single(MAX160)));
check('PermitSingle for 2^152 is unlimited', heroUnlimited(single(1n << 152n)));
check('PermitSingle for 2^152 - 1 is a figure', !heroUnlimited(single((1n << 152n) - 1n)));

const transfer = (amount) => typed('PermitTransferFrom', {
  permitted: { token: USDC, amount: String(amount) }, spender: SPENDER, nonce: '0', deadline: '1900000000',
}, PERMIT2);
check('PermitTransferFrom is read as uint256: 2^152 is a figure', !heroUnlimited(transfer(1n << 152n)));

process.exit(check.summary() ? 0 : 1);
