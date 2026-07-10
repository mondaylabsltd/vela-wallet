// Vela Safari extension — toolbar popup (browser action).
//
// Read-only status surface: the active Vela account (name / address / chain, from
// the app-written cache via the background), whether the CURRENT site is connected,
// and every site this browser has granted — each with a one-tap disconnect. Plus an
// "open Vela" affordance. No signing, no key material; grants live in storage.local
// and the account snapshot is the same PUBLIC cache the provider reads.
/* global browser */
import { PERM_PREFIX, WALLET_NAME, hostLabel, universalLinkSelfTestUrl, UL_BROKEN_KEY, UL_PENDING_KEY } from './lib/protocol.js';
import { dataThemeFor } from './lib/theme.js';
import { t as tr, pickLocale } from './lib/i18n.js';

const $ = (id) => document.getElementById(id);

// The app's language, learned from the status response; empty → browser language → en.
let appLocale = '';
function L(key, vars) {
  return tr(key, pickLocale(appLocale), vars);
}

// Match the app's color scheme: the app writes its preference into the cache, which
// rides the status response. Forced light/dark set data-theme (winning over the OS);
// 'auto' clears it so the popup's prefers-color-scheme media query decides — exactly
// what the app shows for 'auto'.
function applyTheme(pref) {
  const dt = dataThemeFor(pref);
  if (dt) document.documentElement.setAttribute('data-theme', dt);
  else document.documentElement.removeAttribute('data-theme');
}

function short(addr) {
  return addr && addr.length > 12 ? addr.slice(0, 6) + '…' + addr.slice(-4) : addr || '';
}

function chainName(account, chainId) {
  const c = account && account.chains ? account.chains[String(chainId)] || account.chains[chainId] : null;
  return (c && c.name) || (chainId ? 'Chain ' + chainId : '—');
}

async function activeOrigin() {
  try {
    const tabs = await browser.tabs.query({ active: true, currentWindow: true });
    const url = tabs && tabs[0] && tabs[0].url;
    if (!url) return null;
    return new URL(url).origin;
  } catch {
    return null; // no tabs permission / internal page — degrade to account-only
  }
}

function send(msg) {
  return browser.runtime.sendMessage(msg);
}

// Every granted origin in storage.local (PERM_PREFIX). The popup can read storage
// directly — no background round-trip needed for the list.
async function allSites() {
  try {
    const all = await browser.storage.local.get(null);
    return Object.keys(all)
      .filter((k) => k.startsWith(PERM_PREFIX) && all[k] && all[k].address)
      .map((k) => ({ origin: all[k].origin || k.slice(PERM_PREFIX.length), perm: all[k] }))
      .sort((a, b) => (b.perm.grantedAt || 0) - (a.perm.grantedAt || 0));
  } catch {
    return [];
  }
}

function openVela() {
  // Launch the app. tabs.create with the scheme is the extension-popup-safe way to
  // hand off to the native app (a bare location nav from a popup is unreliable).
  try {
    browser.tabs.create({ url: 'velawallet://' });
  } catch {
    /* ignore — user can open the app from the home screen */
  }
  try { window.close(); } catch { /* popup already closing */ }
}

function siteRow(origin, isCurrent) {
  const row = document.createElement('div');
  row.className = 'site' + (isCurrent ? ' current' : '');
  const label = document.createElement('span');
  label.className = 'site-host';
  label.textContent = hostLabel(origin);
  const btn = document.createElement('button');
  btn.className = 'disconnect';
  btn.textContent = L('popup.disconnect');
  btn.onclick = async () => {
    btn.disabled = true;
    await send({ type: 'revoke', origin }).catch(() => {});
    await render(); // re-read state
  };
  row.append(label, btn);
  return row;
}

async function render() {
  // Immediate localized labels (browser language) so nothing flashes untranslated
  // before the status round-trip; refined to the app's exact locale once it returns.
  $('site-label').textContent = L('popup.currentSite');
  $('open').textContent = L('common.openVela');

  const origin = await activeOrigin();
  const status = await send({ type: 'status', origin }).catch(() => null);
  if (status) { applyTheme(status.theme); appLocale = status.locale || ''; }
  const account = status && status.account;

  const accountEl = $('account');
  const siteEl = $('site-status');
  const sitesEl = $('sites');
  accountEl.innerHTML = '';
  siteEl.innerHTML = '';
  sitesEl.innerHTML = '';
  $('ul-status').innerHTML = '';

  $('site-label').textContent = L('popup.currentSite');

  // --- account card ---
  if (!account || !account.address) {
    accountEl.innerHTML =
      '<div class="empty">' + L('popup.notLoggedIn') + '<br/>' + L('popup.notLoggedInSub') + '</div>';
    $('open').textContent = L('common.openVelaLogin');
    // No account → hide site sections entirely.
    $('sites-title').style.display = 'none';
    return;
  }
  $('open').textContent = L('common.openVela');
  const name = document.createElement('div');
  name.className = 'acct-name';
  name.textContent = account.name || 'Vela';
  const addr = document.createElement('div');
  addr.className = 'acct-addr';
  addr.textContent = short(account.address);
  const chain = document.createElement('div');
  chain.className = 'acct-chain';
  chain.textContent = chainName(account, status.chainId);
  accountEl.append(name, addr, chain);

  // --- current-site status ---
  if (origin) {
    const host = hostLabel(origin);
    if (status.granted) {
      siteEl.innerHTML =
        '<span class="dot on"></span><span class="site-host">' + host + '</span>' +
        '<span class="conn">' + L('popup.connected', { chain: chainName(account, status.chainId) }) + '</span>';
      const dc = document.createElement('button');
      dc.className = 'disconnect';
      dc.textContent = L('popup.disconnect');
      dc.onclick = async () => { dc.disabled = true; await send({ type: 'revoke', origin }).catch(() => {}); await render(); };
      siteEl.append(dc);
    } else {
      siteEl.innerHTML =
        '<span class="dot off"></span><span class="site-host">' + host + '</span>' +
        '<span class="conn muted">' + L('popup.notConnectedHint') + '</span>';
    }
  } else {
    siteEl.innerHTML = '<span class="conn muted">' + L('popup.cantConnect') + '</span>';
  }

  // --- all connected sites ---
  const sites = await allSites();
  const others = sites.filter((s) => s.origin !== origin);
  if (others.length) {
    $('sites-title').style.display = '';
    $('sites-title').textContent = L('popup.otherSites', { n: others.length });
    for (const s of others) sitesEl.append(siteRow(s.origin, false));
  } else {
    $('sites-title').style.display = 'none';
  }

  // --- one-tap sign (Universal Link) status / bootstrap ---
  // The extension self-heal veto (UL_BROKEN, storage.local) OVERRIDES the app's
  // attestation only while it is NEWER than the attestation — mirror that comparison
  // here so the popup shows the same state the launch decision uses.
  let brokenAt = 0;
  try {
    const rec = (await browser.storage.local.get(UL_BROKEN_KEY))[UL_BROKEN_KEY];
    brokenAt = rec && typeof rec.ts === 'number' ? rec.ts : 0;
  } catch {
    /* ignore */
  }
  const broken = brokenAt > 0 && brokenAt > (status.ulVerifiedAt || 0);
  renderUniversalLink(!!status.ulVerified, broken);
}

// Open the UL self-test in a NEW tab (non-destructive: worst case a getvela.app page
// in a throwaway tab; it never touches a dApp tab). Stamp UL_PENDING so a FAILED probe
// re-vetoes via the getvela.app landing (content.js self-heal); a SUCCESSFUL probe
// opens the app, which re-attests with a fresher timestamp that out-dates any veto —
// so nothing needs to be optimistically cleared here (no race).
function openProbe() {
  const go = () => {
    try { browser.tabs.create({ url: universalLinkSelfTestUrl() }); } catch { /* ignore */ }
    try { window.close(); } catch { /* closing */ }
  };
  try {
    browser.storage.local.set({ [UL_PENDING_KEY]: { ts: Date.now(), origin: 'popup-probe' } }).then(go, go);
  } catch {
    go();
  }
}

// Show whether the one-tap (UL) sign hand-off is active, with an ALWAYS-reachable
// affordance: a re-test even when active (so a device that later breaks — e.g. iOS
// "Open in Safari" for getvela.app — is never stuck) and a re-test when vetoed.
function renderUniversalLink(verified, broken) {
  const el = $('ul-status');
  el.innerHTML = '';
  if (verified && !broken) {
    const ok = document.createElement('span');
    ok.className = 'ul-ok';
    ok.textContent = L('popup.oneTapEnabled');
    const re = document.createElement('button');
    re.className = 'ul-reverify';
    re.textContent = L('popup.retest');
    re.onclick = openProbe;
    el.append(ok, re);
    return;
  }
  const hint = document.createElement('div');
  hint.className = 'ul-hint';
  hint.textContent = broken ? L('popup.oneTapBrokenHint') : L('popup.oneTapUnverifiedHint');
  const btn = document.createElement('button');
  btn.className = 'ul-test';
  btn.textContent = broken ? L('popup.retestOneTap') : L('popup.testOneTap');
  btn.onclick = openProbe;
  el.append(hint, btn);
}

$('open').onclick = openVela;
document.title = WALLET_NAME;
render();
