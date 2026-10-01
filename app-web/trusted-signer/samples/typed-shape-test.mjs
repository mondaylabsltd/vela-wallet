// What you see is what you sign — typed data on this page (audit 2026-10-01).
// The preview (`resolve`) and the digest (`digest.of`) take the request's ONE
// document by the same reader; a request that is not exactly one document in
// its method's order is refused by both.
//
//   node samples/typed-shape-test.mjs
import { loadPageLibs, makeChecks } from './test-kit.mjs';

const ns = loadPageLibs([
  'src/lib/i18n.js', 'src/lib/locales/en.js', 'src/lib/keccak.js', 'src/lib/identicon-features.js',
  'src/lib/identicon.js', 'src/lib/abi.js', 'src/lib/encode.js', 'src/lib/fee.js', 'src/lib/logos.js',
  'src/lib/registry.js', 'src/lib/resolve.js', 'src/lib/safeop.js', 'src/lib/digest.js',
]);
const check = makeChecks();
const ACCOUNT = '0x88cca0eedbf2c4426110bbfc998f048689266894';
const ctx = { account: ACCOUNT, chainId: 100 };
const mail = { types: { EIP712Domain: [{ name: 'name', type: 'string' }], Mail: [{ name: 'contents', type: 'string' }] }, primaryType: 'Mail', domain: { name: 'Ether Mail' }, message: { contents: 'Hello' } };
const permit = { types: { EIP712Domain: [{ name: 'name', type: 'string' }, { name: 'chainId', type: 'uint256' }, { name: 'verifyingContract', type: 'address' }], Permit: [{ name: 'owner', type: 'address' }, { name: 'spender', type: 'address' }, { name: 'value', type: 'uint256' }, { name: 'nonce', type: 'uint256' }, { name: 'deadline', type: 'uint256' }] }, primaryType: 'Permit', domain: { name: 'USD Coin', chainId: 100, verifyingContract: '0x2a22f9c3b484c3629090FeED35F17Ff8F88f76F0' }, message: { owner: ACCOUNT, spender: '0x000000000000000000000000000000000000dEaD', value: '1000000', nonce: '0', deadline: '99999999999' } };
const intent = (method, params) => ({ method, origin: 'https://site.test', params });
const hex = (b) => (typeof b === 'string' ? b : Array.from(b, (x) => x.toString(16).padStart(2, '0')).join(''));
const refusedBoth = (req) => {
  const view = ns.resolve(req, ctx);
  const digest = ns.digest.of(req, ctx);
  return view.refuse === true && view.warnings.some((w) => w.key === 'refuse.typedShape') && digest.refuse === 'refuse.typedShape';
};

// The audit's two shapes.
check('v4 [mail, permit] is refused by the preview and the digest', refusedBoth(intent('eth_signTypedData_v4', [JSON.stringify(mail), JSON.stringify(permit)])));
for (const m of ['eth_signTypedData', 'eth_signTypedData_v1']) {
  check(`${m} [permit, mail] is refused by the preview and the digest`, refusedBoth(intent(m, [JSON.stringify(permit), JSON.stringify(mail)])));
}
// One param, the wrong order, a name that only resembles one.
check('v4 [permit] (no account) is refused', refusedBoth(intent('eth_signTypedData_v4', [JSON.stringify(permit)])));
check('v4 [permit, account] (legacy order) is refused', refusedBoth(intent('eth_signTypedData_v4', [JSON.stringify(permit), ACCOUNT])));
check('eth_signTypedData_v2 is refused', refusedBoth(intent('eth_signTypedData_v2', [ACCOUNT, JSON.stringify(permit)])));

// Well-formed, every method: the preview names the Permit, the digest is the Permit's.
for (const [m, params] of [
  ['eth_signTypedData_v4', [ACCOUNT, JSON.stringify(permit)]],
  ['eth_signTypedData_v3', [ACCOUNT, permit]],
  ['eth_signTypedData', [JSON.stringify(permit), ACCOUNT]],
  ['eth_signTypedData_v1', [permit, ACCOUNT]],
]) {
  const req = intent(m, params);
  const view = ns.resolve(req, ctx);
  const digest = ns.digest.of(req, {});
  check(`${m}: the preview is the Permit`, view.intentKey === 'intent.permit' && !view.refuse);
  check(`${m}: the digest is the Permit's`, hex(digest.hash) === hex(ns.digest.typedDataHash(permit)));
}
