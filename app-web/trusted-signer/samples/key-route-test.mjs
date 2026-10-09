// The key route (spec 102 R5): the page asks the browser for the ONE key the
// wallet signs with, where it lives — so the browser goes straight to it and
// never draws its generic "this device / phone / security key" chooser.
//
//   node samples/key-route-test.mjs      (bun runs it unchanged)
//
// What reaches `navigator.credentials` is the whole story here, so this stubs
// it and reads the options the page built — for a signature (vela-core
// `trusted_signer::request`'s `context.keyRoute`) and for every ceremony
// (`ceremony::request`'s `place` / `hints` / `transports`). Also: the page's
// own catalogue reads the amounts the apps send (Base USDC was "0 ?"), and the
// card says the key's place and the action on its button.
import { webcrypto } from 'node:crypto';
import { installFakeDom, loadPageLibs, makeChecks } from './test-kit.mjs';

installFakeDom();
const ns = loadPageLibs([
  'src/lib/i18n.js', 'src/lib/locales/en.js', 'src/lib/locales/zh.js', 'src/lib/catalog.js',
  'src/lib/keccak.js', 'src/lib/identicon-features.js', 'src/lib/identicon.js', 'src/lib/abi.js',
  'src/lib/encode.js', 'src/lib/fee.js', 'src/lib/logos.js', 'src/lib/registry.js', 'src/lib/resolve.js',
  'src/lib/render.js', 'src/lib/safeop.js', 'src/lib/digest.js', 'src/lib/signer.js', 'src/lib/ceremony.js',
]);
ns.i18n.setLocale('en');
const t = ns.i18n.t;
const check = makeChecks();

// --- a stand-in browser: record what the page asks for -----------------------
globalThis.location = { hostname: 'sign.getvela.app', origin: 'https://sign.getvela.app' };
globalThis.localStorage = { getItem: () => null, setItem() {} };
Object.defineProperty(globalThis, 'crypto', { value: webcrypto, configurable: true });
const asked = [];
Object.defineProperty(globalThis, 'navigator', {
  configurable: true,
  value: {
    credentials: {
      get: (options) => { asked.push({ kind: 'get', publicKey: options.publicKey }); return Promise.reject(Object.assign(new Error('stub'), { name: 'NotAllowedError' })); },
      create: (options) => { asked.push({ kind: 'create', publicKey: options.publicKey }); return Promise.reject(Object.assign(new Error('stub'), { name: 'NotAllowedError' })); },
    },
  },
});
const last = () => asked[asked.length - 1].publicKey;
const idOf = (entry) => Buffer.from(entry.id).toString('base64url');
const settle = (promise) => promise.then(() => null, () => null);

const SAFE = '0x88cca0f8b4e1f0dc0e7c4f9a2b3d5e6f7a8b6894';
const KEY = Buffer.from('a1b2c3d4e5f60718293a4b5c6d7e8f90', 'hex').toString('base64url');
const OTHER = Buffer.from('0f0e0d0c0b0a09080706050403020100', 'hex').toString('base64url');
const AS_THE_APPS = { channel: 'url', callback: 'velawallet://sign-result' };
const message = { method: 'personal_sign', params: ['0x68656c6c6f', SAFE], origin: '' };
// vela-core trusted_signer::request with a key route: allowCredentials is the one key.
const route = (place, extra = {}) => ({
  credentialId: KEY,
  place,
  transports: { platform: ['internal', 'hybrid'], hybrid: ['hybrid'], security_key: ['usb', 'nfc'] }[place],
  hints: { platform: ['client-device'], hybrid: ['hybrid'], security_key: ['security-key'] }[place],
  ...extra,
});
const signWith = (view, context) => {
  const allowed = view.key && view.key.credentialId ? [view.key.credentialId] : (context.allowCredentials || []);
  return settle(ns.signer.sign(new Uint8Array(32), { allowCredentials: allowed, key: view.key }));
};

// --- 1. a signature with the key route ----------------------------------------
for (const place of ['platform', 'hybrid', 'security_key']) {
  const context = { account: SAFE, chainId: 1, ...AS_THE_APPS, allowCredentials: [KEY], keyRoute: route(place) };
  const view = ns.resolve(message, context);
  await signWith(view, context);
  const options = last();
  check(`${place}: the request names the one key`, options.allowCredentials.length === 1 && idOf(options.allowCredentials[0]) === KEY);
  check(`${place}: …with the transports it reported`,
    JSON.stringify(options.allowCredentials[0].transports) === JSON.stringify(route(place).transports),
    JSON.stringify(options.allowCredentials[0].transports));
  check(`${place}: …and the hints for its place`, JSON.stringify(options.hints) === JSON.stringify(route(place).hints),
    JSON.stringify(options.hints));
  check(`${place}: user verification is still required, on the page's own relying party`,
    options.userVerification === 'required' && options.rpId === 'getvela.app');
  const sheet = ns.render(view, {});
  const placeText = sheet.find('.key-place-name').map((n) => n.textContent).join();
  check(`${place}: the card says where the person confirms`, placeText === t('place.' + place), placeText);
}

// --- 2. without a route: what every older wallet sends, unchanged --------------
{
  const context = { account: SAFE, chainId: 1, ...AS_THE_APPS, allowCredentials: [KEY, OTHER] };
  const view = ns.resolve(message, context);
  await signWith(view, context);
  const options = last();
  check('no route: every key the account has, as before', options.allowCredentials.length === 2);
  check('no route: no transports, no hints — the browser chooses', options.allowCredentials.every((a) => !a.transports) && !options.hints);
  check('no route: no place line on the card', ns.render(view, {}).find('key-row').length === 0);
}

// --- 3. a route is checked, and anything wrong drops it --------------------------
{
  const narrow = ns.resolve.keyRoute;
  check('a route naming a key outside allowCredentials is no route', narrow(route('platform'), [OTHER]) === null);
  check('a malformed credential id is no route', narrow(route('platform', { credentialId: 'not base64url!' }), [KEY]) === null);
  check('an unknown place keeps the key but draws no place', (() => {
    const r = narrow({ credentialId: KEY, place: 'telepathy', transports: ['internal'] }, [KEY]);
    return r && r.place === null && r.hints.length === 0 && r.transports.join() === 'internal';
  })());
  check('unknown transports and hints are dropped, known ones kept once', (() => {
    const r = narrow({ credentialId: KEY, place: 'hybrid', transports: ['hybrid', 'carrier-pigeon', 'hybrid'], hints: ['hybrid', 'x'] }, [KEY]);
    return r && r.transports.join() === 'hybrid' && r.hints.join() === 'hybrid';
  })());
  check('hints the wallet did not send are its place\'s', narrow({ credentialId: KEY, place: 'security_key' }, [KEY]).hints.join() === 'security-key');
  check('transports the wallet did not send are its place\'s', narrow({ credentialId: KEY, place: 'platform' }, [KEY]).transports.join() === 'internal');
  check('a route that is not an object is no route', narrow('platform', [KEY]) === null && narrow([KEY], [KEY]) === null);
  // A route naming a key the account does not list: ignored, so the request asks for the account's own keys.
  const context = { account: SAFE, chainId: 1, ...AS_THE_APPS, allowCredentials: [OTHER], keyRoute: route('platform') };
  const view = ns.resolve(message, context);
  await signWith(view, context);
  check('…and the request then asks for the account\'s own keys only',
    last().allowCredentials.length === 1 && idOf(last().allowCredentials[0]) === OTHER && !last().hints);
}

// --- 4. ceremonies: where the new key goes, and where a key is ------------------
const ceremony = (method, params) => ns.resolve(
  { method, params: [params], origin: '' },
  { walletName: 'Savings', rpId: 'pages.example', ...AS_THE_APPS },
);
for (const [place, attachment, hint] of [
  ['platform', 'platform', 'client-device'],
  ['hybrid', 'cross-platform', 'hybrid'],
  ['security_key', 'cross-platform', 'security-key'],
]) {
  const view = ceremony('vela_createPasskey', { name: 'Savings', excludeCredentialIds: [], place, hints: [hint] });
  await settle(ns.ceremony.create({ name: view.ceremony.name, excludeCredentialIds: [], key: view.key }));
  const options = last();
  check(`create on ${place}: authenticatorAttachment ${attachment}`,
    options.authenticatorSelection.authenticatorAttachment === attachment &&
    options.authenticatorSelection.residentKey === 'required' && options.authenticatorSelection.userVerification === 'required');
  check(`create on ${place}: hints ${hint}`, JSON.stringify(options.hints) === JSON.stringify([hint]));
  const sheet = ns.render(view, {});
  check(`create on ${place}: the card says where the key goes`,
    sheet.find('key-place-name').map((n) => n.textContent).join() === t('place.' + place) &&
    sheet.find('key-row')[0].textContent.startsWith(t('field.keyOn')));
}
{
  const view = ceremony('vela_createPasskey', { name: 'Savings', excludeCredentialIds: [] });
  await settle(ns.ceremony.create({ name: 'Savings', excludeCredentialIds: [], key: view.key }));
  check('create with no place (an older wallet): no attachment, no hints — as before',
    last().authenticatorSelection.authenticatorAttachment === undefined && !last().hints && view.key === null);
}
{
  const view = ceremony('vela_signIn', { place: 'hybrid', hints: ['hybrid'] });
  await settle(ns.ceremony.assert(new Uint8Array(8), [], view.key));
  check('sign-in on a phone: any key of the site (discoverable), the phone hint',
    !last().allowCredentials && JSON.stringify(last().hints) === '["hybrid"]');
}
for (const method of ['vela_proof', 'vela_memberProof']) {
  const params = method === 'vela_proof'
    ? { credentialId: KEY, purpose: 'verify', place: 'security_key', transports: ['usb', 'nfc'], hints: ['security-key'] }
    : {
      credentialId: KEY, publicKey: '04' + 'ab'.repeat(64), attestation: '', groupPublicKey: '04' + 'cd'.repeat(64),
      registry: 'https://p256-index-v2.getvela.app', chainId: 8453, registryContract: '0x' + '11'.repeat(20),
      place: 'security_key', transports: ['usb', 'nfc'], hints: ['security-key'],
    };
  const view = ceremony(method, params);
  check(`${method}: the request is not refused`, !view.refuse, view.warnings.map((w) => w.key).join());
  await settle(ns.ceremony.assert(new Uint8Array(8), [view.ceremony.credentialId], view.key));
  check(`${method}: the named key, with its transports and the security-key hint`,
    last().allowCredentials.length === 1 && idOf(last().allowCredentials[0]) === KEY &&
    last().allowCredentials[0].transports.join() === 'usb,nfc' && last().hints.join() === 'security-key');
}

// --- 5. the button says the action -------------------------------------------------
{
  const USDC_BASE = '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913';
  const ALICE = '0x9a8b7c6d5e4f3a2b1c0d9e8f7a6b5c4d3e2f1a09';
  const tx = (data, chainId = 8453) => ns.resolve(
    { method: 'eth_sendTransaction', origin: '', params: [{ to: USDC_BASE, value: '0x0', data }] },
    { account: SAFE, chainId, ...AS_THE_APPS },
  );
  const transfer = ns.encode.call('transfer(address,uint256)', [ALICE, 250000000n]);
  const approve = ns.encode.call('approve(address,uint256)', [ALICE, 500000000n]);
  const button = (view) => ns.render(view, {}).find('confirm').map((n) => n.textContent).join();
  check('a transfer: "Confirm send"', button(tx(transfer)) === t('button.send'), button(tx(transfer)));
  check('an approval: "Approve"', button(tx(approve)) === t('button.approve'), button(tx(approve)));
  check('a message: "Sign"', button(ns.resolve(message, { account: SAFE, chainId: 1, ...AS_THE_APPS })) === t('button.sign'));
  const refused = ns.resolve(message, { account: SAFE, chainId: 1, channel: 'url', callback: 'https://evil.example/cb' });
  const off = ns.render(refused, {}).find('confirm')[0];
  check('a refusal: the button is off and says so', off.disabled === true && off.textContent === t('button.cannotSign'));
  check('a create: "Create key"', button(ceremony('vela_createPasskey', { name: 'Savings' })) === t('button.create'));

  // --- 6. the amounts the apps actually send ------------------------------------
  const sent = tx(transfer);
  check('Base USDC: 250 USDC, not "0 ?" (the page knows the apps\' tokens)',
    sent.hero.amount.text === '250' && sent.hero.amount.symbol === 'USDC' &&
    !sent.warnings.some((w) => w.key === 'warn.unverifiedDecimals'), `${sent.hero.amount.text} ${sent.hero.amount.symbol}`);
  const elsewhere = tx(transfer, 137);
  check('the same address on another chain is not that token',
    elsewhere.hero.amount.symbol === '?' && elsewhere.warnings.some((w) => w.key === 'warn.unverifiedDecimals'));
  check('an unknown token shows every digit in its smallest units, never a guessed decimal',
    elsewhere.hero.amount.text === '250,000,000', elsewhere.hero.amount.text);
  check('the network is named as the apps name it', sent.chain === 'Base' && !sent.chainClaimed);
}

process.exit(check.summary() ? 0 : 1);
