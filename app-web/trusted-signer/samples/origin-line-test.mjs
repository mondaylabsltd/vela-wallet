// Spec 082 L-HOST (FR-013): the signing page names the site once.
//
// The wallet names a site it opened by its host (vela-core
// `trusted_signer.rs`: `context.dapp.name = host`), and the page drew that
// name and then the host under it — "127.0.0.1:8137" over "127.0.0.1:8137".
// The apps fixed the same doubling in 079 (F14); their rule is vela-core
// `browser_load::site_label`: both trimmed, equal ignoring ASCII case → the
// host once. resolve.js now makes that judgement (`dapp.originShown`) and
// render.js only draws what it was told.
//
// What must NOT change: the host stays in the view (`dapp.origin`), and the
// "self-reported site" warning reads it, so a name that hides the host line
// never hides the warning.
//
//   node samples/origin-line-test.mjs
import { installFakeDom, loadPageLibs, makeChecks } from './test-kit.mjs';

installFakeDom();
// In the order `src/sign.html` loads them (render.js draws, ceremony.js is
// what tells resolve.js a ceremony from a signature).
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
  'src/lib/ceremony.js',
]);
ns.i18n.setLocale('zh');
const check = makeChecks();

const ACCOUNT = '0x88cca0f8b4e1f0dc0e7c4f9a2b3d5e6f7a8b6894';
const USDC = '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48';
const RECIPIENT = '0x9a8b7c6d5e4f3a2b1c0d9e8f7a6b5c4d3e2f1a09';
const transferCall = ns.encode.call('transfer(address,uint256)', [RECIPIENT, 1_000_000n]);

// What the apps send over the URL channel: the host as the site's name, the
// origin on the intent, and a channel the browser does not vouch for.
const signing = (origin, dappName, extra = {}) => {
  const intent = {
    method: 'eth_sendTransaction',
    origin,
    params: [{ to: USDC, value: '0x0', data: transferCall }],
  };
  const ctx = {
    account: ACCOUNT, chainId: 100, channel: 'url', originVerified: false,
    ...(dappName === undefined ? {} : { dapp: { name: dappName, origin, source: 'vela_browser' } }),
    ...extra,
  };
  const view = ns.resolve(intent, ctx);
  return { view, sheet: ns.render(view, {}) };
};
const head = (sheet) => sheet.find('.sheet-identity')[0];
const lines = (sheet) => {
  const identity = head(sheet);
  return [...identity.find('.dapp-name'), ...identity.find('.dapp-origin')].map((n) => n.textContent);
};
const warned = (view, sheet) =>
  view.warnings.some((w) => w.key === 'warn.claimedOrigin') &&
  sheet.find('.warning-text').some((n) => n.textContent === ns.i18n.t('warn.claimedOrigin'));

// 1. The name IS the host: one line.
{
  const { view, sheet } = signing('http://127.0.0.1:8137', '127.0.0.1:8137');
  check('name == host: the head has one line, the host', lines(sheet).join(' | ') === '127.0.0.1:8137',
    JSON.stringify(lines(sheet)));
  check('name == host: no host line is drawn', head(sheet).find('.dapp-origin').length === 0);
  check('name == host: the view still carries the host (for the warning)', view.dapp.origin === '127.0.0.1:8137');
  check('name == host: the self-reported-site warning is still drawn', warned(view, sheet));
}

// 2. The same words by the apps' rule: ASCII case and surrounding spaces.
{
  const { sheet } = signing('https://App.Example.COM', ' app.example.com ');
  check('name == host ignoring ASCII case and spaces: one line', head(sheet).find('.dapp-origin').length === 0,
    JSON.stringify(lines(sheet)));
  const { sheet: lan } = signing('http://192.168.50.9:8137', '192.168.50.9:8137');
  check('the LAN host of the device pass: one line', lines(lan).join(' | ') === '192.168.50.9:8137');
}

// 3. A different name: two lines, the name over the host.
{
  const { view, sheet } = signing('https://app.uniswap.org', 'Uniswap');
  check('a different name: two lines, name then host', lines(sheet).join(' | ') === 'Uniswap | app.uniswap.org',
    JSON.stringify(lines(sheet)));
  check('a different name: the self-reported-site warning is drawn', warned(view, sheet));
}

// 4. Only ASCII case folds, as in the core (`eq_ignore_ascii_case`): a name
//    that differs from the host in a non-ASCII letter is a different name.
{
  const { sheet: umlaut } = signing('https://äpp.test', 'ÄPP.TEST');
  check('ÄPP.TEST over äpp.test: two lines (the core folds ASCII only)', head(umlaut).find('.dapp-origin').length === 1,
    JSON.stringify(lines(umlaut)));
}

// 5. No name from the wallet: the "unknown site" tag, then the host.
{
  const { view, sheet } = signing('https://site.test', undefined);
  check('no name: the unknown-site tag over the host',
    lines(sheet).join(' | ') === ns.i18n.t('tag.unknownSite') + ' | site.test', JSON.stringify(lines(sheet)));
  check('no name: the self-reported-site warning is drawn', warned(view, sheet));
}

// 6. A channel the browser vouches for: one line when the name is the host,
//    and no self-reported warning (unchanged).
{
  const { view, sheet } = signing('https://getvela.app', 'getvela.app', { originVerified: true, channel: 'postMessage' });
  check('verified, name == host: one line', head(sheet).find('.dapp-origin').length === 0);
  check('verified: no self-reported-site warning', !view.warnings.some((w) => w.key === 'warn.claimedOrigin'));
}

// 7. The wallet asking itself (no origin): no host line, as before.
{
  const view = ns.resolve({ method: 'personal_sign', params: ['0x68656c6c6f', ACCOUNT] }, { account: ACCOUNT, chainId: 1 });
  const sheet = ns.render(view, {});
  check('no origin: no host line', head(sheet).find('.dapp-origin').length === 0 && view.dapp.originShown === false);
}

// 8. The ceremony head keeps its line: the verified requester's host, the
//    destination scheme, or the channel's words — none of them is its name.
{
  const ceremony = (ctx) => {
    const view = ns.resolve({ method: 'vela_signIn', params: [{}] }, { rpId: 'getvela.app', walletName: 'Vela', ...ctx });
    return { view, sheet: ns.render(view, {}) };
  };
  const verified = ceremony({ originVerified: true, channel: 'postMessage', requester: 'https://getvela.app' });
  check('ceremony, verified requester: the host line is drawn',
    head(verified.sheet).find('.dapp-origin').map((n) => n.textContent).join() === 'getvela.app',
    JSON.stringify(lines(verified.sheet)));
  const url = ceremony({ channel: 'url', callback: 'velawallet://sign-result' });
  check('ceremony over the URL channel: the channel line is drawn (a key, not a host)',
    head(url.sheet).find('.dapp-origin').length === 1 && url.view.dapp.originKey !== null,
    JSON.stringify(lines(url.sheet)));
  const elsewhere = ceremony({ channel: 'url', callback: 'evil://steal' });
  check('ceremony answering elsewhere: the destination line is drawn',
    head(elsewhere.sheet).find('.dapp-origin').map((n) => n.textContent).join() === 'evil:',
    JSON.stringify(lines(elsewhere.sheet)));
}

process.exit(check.summary() ? 0 : 1);
