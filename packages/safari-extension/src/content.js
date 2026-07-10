// Vela Safari extension — content script (ISOLATED world).
//
// Phase A. Three jobs:
//   1. inject inpage.js (the real EIP-1193/6963 provider) into the MAIN world.
//   2. bridge the provider's tagged postMessage requests to the background
//      router and post results/events back — resolving the dApp's promise.
//   3. own the in-Safari UI (connect sheet, sign hand-off sheet) and the proven
//      sign return path (velawallet://sign?rid launch + focus-poll → deliver the
//      result to the provider). This REPLACES the R1 stub button + status pill.
//
// The return-path machinery (durable storage.local mirror, bounded focus-poll,
// single-resolve, non-4001-on-timeout) is carried over verbatim from the R1
// spike (Increment 4, device-proven) and rewired so a settle also resolves the
// waiting inpage promise instead of only painting a pill.
/* global browser */
import {
  CHANNEL,
  ERR,
  rpcError,
  classifyMethod,
  toHexChainId,
  parseChainId,
  hostLabel,
  SIGN_PREFIX,
  signLaunchUrl,
  UNIVERSAL_LINK_HOST,
  UL_PENDING_KEY,
  UL_BROKEN_KEY,
} from './lib/protocol.js';
import { themeCss, dataThemeFor } from './lib/theme.js';
import { t as tr, pickLocale } from './lib/i18n.js';

(() => {
  const ORIGIN = location.origin;
  const hasStorage = !!(typeof browser !== 'undefined' && browser.storage && browser.storage.local);

  // ---- self-heal: a UL sign hand-off that FAILED landed the tab HERE ---------
  // If a UL launch is not routed to the app by iOS (e.g. the user chose "Open in
  // Safari" for getvela.app — an unobservable persistent per-domain preference),
  // the launch NAVIGATES the dApp tab to getvela.app, where this content script
  // also runs. A fresh UL_PENDING stamp when we load on getvela.app means exactly
  // that failure: veto the UL (UL_BROKEN) so every future sign falls back to the
  // safe velawallet:// scheme until the popup probe clears it + re-verifies. This
  // heals after ONE bad sign, well before the app-side TTL would lapse.
  if (
    hasStorage &&
    (location.hostname === UNIVERSAL_LINK_HOST || location.hostname.endsWith('.' + UNIVERSAL_LINK_HOST))
  ) {
    browser.storage.local
      .get(UL_PENDING_KEY)
      .then((r) => {
        const p = r && r[UL_PENDING_KEY];
        if (p && typeof p.ts === 'number' && Date.now() - p.ts < 20000) {
          browser.storage.local.set({ [UL_BROKEN_KEY]: { ts: Date.now(), origin: p.origin || '' } }).catch(() => {});
        }
        if (p) browser.storage.local.remove(UL_PENDING_KEY).catch(() => {});
      })
      .catch(() => {});
  }

  // ---- 1. inject the MAIN-world provider (fallback path) --------------------
  // Primary path is the manifest's `world:"MAIN"` inpage content script (Safari
  // 18+), which is immune to the page CSP. This <script src=…> injection is the
  // FALLBACK for older Safari (where `world` is ignored) — but a strict-CSP dApp
  // (script-src without the extension origin) BLOCKS it, which is exactly why the
  // MAIN-world entry exists. Skip the fallback when MAIN-world already installed
  // the provider (marker on the shared DOM) so we don't fire a doomed, CSP-logging
  // inject on every page. If the marker is absent (older Safari, or the isolated
  // script won the document_start race), inject synchronously — identical timing
  // to before; inpage's __velaProviderInstalled guard makes any double a no-op.
  if (!document.documentElement.hasAttribute('data-vela-inpage')) {
    try {
      const url = browser.runtime.getURL('inpage.js');
      const s = document.createElement('script');
      s.src = url;
      s.async = false; // preserve document_start execution order
      s.dataset.vela = '1';
      (document.head || document.documentElement).appendChild(s);
      s.onload = () => s.remove();
    } catch (e) {
      console.log('[Vela] inpage inject failed', e);
    }
  }

  // ---- provider transport (post back to the MAIN world) ---------------------
  function respond(rpcId, result) {
    window.postMessage({ ch: CHANNEL, dir: 'res', id: rpcId, result }, ORIGIN);
  }
  function respondErr(rpcId, error) {
    window.postMessage({ ch: CHANNEL, dir: 'res', id: rpcId, error }, ORIGIN);
  }
  function emitEvt(event, data) {
    window.postMessage({ ch: CHANNEL, dir: 'evt', event, data }, ORIGIN);
  }

  // ---- background round-trip (dead-worker safe) -----------------------------
  const TIMEOUT = Symbol('timeout');
  function withTimeout(promise, ms) {
    return Promise.race([promise, new Promise((res) => setTimeout(() => res(TIMEOUT), ms))]);
  }
  async function bg(msg, ms = 9000) {
    try {
      const r = await withTimeout(browser.runtime.sendMessage(msg), ms);
      if (r === TIMEOUT || r === undefined) {
        return { ok: false, error: rpcError(ERR.INTERNAL, 'Extension background unavailable') };
      }
      return r;
    } catch (e) {
      return { ok: false, error: rpcError(ERR.INTERNAL, String(e && e.message ? e.message : e)) };
    }
  }

  // ---- 3a. sheet host (open shadow root, CSS-isolated) ----------------------
  let sheetHost = null;
  let busy = false; // one connect/sign sheet at a time
  // Tap-outside-to-dismiss: each sheet state assigns what a backdrop tap does — the
  // grab handle + scrim imply dismissability, so this makes it real. Pre-launch states
  // (connect / sign intent) route to Cancel/reject (safe 4001); post-launch (waiting /
  // checking) route to dismissWaiting (NEVER 4001 — the app may have submitted).
  let sheetOnBackdrop = null;
  // The app's color-scheme preference ('auto'|'light'|'dark'), learned from the
  // connect/sign background round-trip (which reads the app-written cache). Applied
  // to the sheet so it matches the app EXACTLY — a forced-dark app shows a dark sheet
  // even on a light-mode device; 'auto'/unknown falls back to the OS via CSS. Cached
  // across sheets so a re-open reflects the last known preference without a flash.
  let appTheme = null;
  // The app's resolved display language, learned from the same connect/sign round-trip
  // as the theme, so the sheet reads in the user's app language. Empty → L() falls back
  // to the browser language, then English.
  let appLocale = '';
  /** Translate a UI string in the app's language. */
  function L(key, vars) {
    return tr(key, pickLocale(appLocale), vars);
  }

  /** Reflect the app's scheme onto the live host (idempotent; safe if no host). */
  function applyHostTheme() {
    if (!sheetHost) return;
    const dt = dataThemeFor(appTheme);
    if (dt) sheetHost.setAttribute('data-theme', dt);
    else sheetHost.removeAttribute('data-theme');
  }

  function ensureHost() {
    if (sheetHost && document.documentElement.contains(sheetHost)) {
      applyHostTheme();
      return sheetHost.__root;
    }
    sheetHost = document.createElement('div');
    sheetHost.id = 'vela-sheet-host';
    sheetHost.setAttribute('data-vela', '1');
    // Open root: CSS-isolated from the dApp but the E2E harness can still read
    // the CTA rect (host.shadowRoot.getElementById('cta').getBoundingClientRect()).
    const root = sheetHost.attachShadow({ mode: 'open' });
    root.innerHTML = `
      <style>
        ${themeCss(':host')}
        :host { all: initial; }
        * { box-sizing: border-box; }
        .backdrop { position: fixed; inset: 0; z-index: 2147483647;
          background: var(--vela-scrim); display: flex; align-items: flex-end;
          justify-content: center;
          font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", system-ui, sans-serif;
          -webkit-font-smoothing: antialiased;
          opacity: 0; animation: vela-fade .22s ease forwards; }
        .sheet { width: 100%; max-width: 460px; background: var(--vela-bg-base); color: var(--vela-fg-base);
          border-radius: 24px 24px 0 0; padding: 8px 22px calc(22px + env(safe-area-inset-bottom));
          box-shadow: 0 -1px 0 var(--vela-border), 0 -12px 48px rgba(0,0,0,.28); box-sizing: border-box;
          transform: translateY(14px); animation: vela-rise .26s cubic-bezier(.2,.9,.25,1) forwards; }
        .grab { width: 36px; height: 5px; border-radius: 3px; background: var(--vela-border-strong);
          margin: 6px auto 12px; opacity: .8; }
        .brow { display: flex; align-items: center; gap: 10px; margin-bottom: 2px; }
        .fav { width: 30px; height: 30px; border-radius: 9px; background: var(--vela-accent-soft); flex: none;
          display: flex; align-items: center; justify-content: center; font-weight: 700; font-size: 14px;
          color: var(--vela-accent); overflow: hidden; }
        .fav img { width: 100%; height: 100%; object-fit: cover; }
        .host { font-weight: 600; font-size: 14px; color: var(--vela-fg-muted); }
        .title { font-size: 21px; font-weight: 700; letter-spacing: -.01em; margin: 14px 0 2px; }
        .sub { font-size: 14px; line-height: 1.45; color: var(--vela-fg-muted); margin: 0 0 12px; }
        .divider { height: 1px; background: var(--vela-border); margin: 16px 0; }
        .row { display: flex; align-items: center; justify-content: space-between; gap: 12px;
          padding: 9px 0; font-size: 15px; }
        .muted { color: var(--vela-fg-muted); }
        .mono { font-family: ui-monospace, "SF Mono", Menlo, monospace; font-size: 13px;
          font-variant-numeric: tabular-nums; }
        .perm { display: flex; gap: 9px; align-items: flex-start; font-size: 14px; line-height: 1.4;
          padding: 5px 0; color: var(--vela-fg-base); }
        .perm .i { flex: none; width: 18px; text-align: center; font-weight: 700; }
        .ok { color: var(--vela-success); } .no { color: var(--vela-fg-subtle); }
        select { font: inherit; color: var(--vela-fg-base); padding: 7px 10px; border-radius: 10px;
          border: 1px solid var(--vela-border-strong); background: var(--vela-bg-raised);
          -webkit-appearance: none; appearance: none; }
        .netpick { display: inline-flex; align-items: center; gap: 8px; }
        .netbadge { position: relative; width: 22px; height: 22px; flex: none; }
        .netbadge .netmono { position: absolute; inset: 0; border-radius: 50%;
          display: flex; align-items: center; justify-content: center; overflow: hidden;
          font-size: 8.5px; font-weight: 700; letter-spacing: -.03em; line-height: 1;
          background: var(--vela-bg-sunken); color: var(--vela-fg-muted); }
        .netbadge img { position: absolute; inset: 0; width: 100%; height: 100%;
          border-radius: 50%; object-fit: cover; }
        .actions { display: flex; gap: 10px; margin-top: 20px; }
        .btn { flex: 1; text-align: center; padding: 15px 16px; border-radius: 15px; font-size: 16px;
          font-weight: 600; border: 0; cursor: pointer; text-decoration: none; box-sizing: border-box;
          transition: transform .06s ease, filter .12s ease; -webkit-tap-highlight-color: transparent; }
        .btn:active { transform: scale(.98); }
        .primary { background: var(--vela-accent); color: #fff; }
        .primary:active { filter: brightness(.94); }
        .ghost { background: var(--vela-bg-sunken); color: var(--vela-fg-base); }
        /* §12.3: a BREATHING ring, never a spinner — nothing happens on THIS page
           (the sign is in the app), so a rotating spinner would lie about progress.
           A solid accent ring that pulses opacity+scale reads as "waiting", honest. */
        .ring { width: 42px; height: 42px; margin: 14px auto 16px; border-radius: 50%;
          border: 3px solid var(--vela-accent);
          animation: vela-breathe 1.6s ease-in-out infinite; }
        @keyframes vela-breathe { 0%,100% { opacity: .3; transform: scale(.9); } 50% { opacity: 1; transform: scale(1.04); } }
        @keyframes vela-fade { to { opacity: 1; } }
        @keyframes vela-rise { to { transform: translateY(0); } }
        @media (prefers-reduced-motion: reduce) {
          .backdrop, .sheet, .ring { animation-duration: .01ms; animation-iteration-count: 1; }
        }
        .center { text-align: center; }
        .big { font-size: 30px; text-align: center; margin: 10px 0 4px;
          width: 52px; height: 52px; line-height: 52px; border-radius: 50%; }
        .big.wrap { margin: 10px auto 6px; }
      </style>
      <div class="backdrop" id="backdrop"><div class="sheet" role="dialog" aria-modal="true"><div class="grab"></div><div id="sheet"></div></div></div>`;
    sheetHost.__root = root;
    applyHostTheme();
    // Backdrop tap (only the dimmed area outside the sheet) → the current state's
    // dismiss action. e.target===bd excludes taps that bubbled up from the sheet.
    const bd = root.getElementById('backdrop');
    if (bd) bd.addEventListener('click', (e) => {
      if (e.target === bd && typeof sheetOnBackdrop === 'function') sheetOnBackdrop();
    });
    (document.body || document.documentElement).appendChild(sheetHost);
    return root;
  }

  function closeSheet() {
    if (sheetHost) sheetHost.remove();
    sheetHost = null;
    busy = false;
    sheetOnBackdrop = null;
  }

  function esc(s) {
    return String(s == null ? '' : s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
  }
  function truncAddr(a) {
    return a && a.length > 12 ? a.slice(0, 6) + '…' + a.slice(-4) : a || '';
  }

  // A network badge for the connect sheet: the chain's real logo (logoURL) with a
  // colored-monogram fallback (iconLabel on iconBg/iconColor) — mirrors the app's
  // ChainLogo. Data rides the account cache (app-group-account-sync buildChainsMap).
  function chainBadge(chains, cid) {
    const c = (chains && (chains[String(cid)] || chains[cid])) || {};
    const mono = esc(String(c.iconLabel || c.name || '?').slice(0, 3));
    const bg = esc(c.iconBg || '');
    const fg = esc(c.iconColor || '');
    const img = c.logoURL
      ? `<img src="${esc(c.logoURL)}" alt="" onerror="this.remove()">`
      : '';
    return `<span class="netmono" style="${bg ? 'background:' + bg + ';' : ''}${fg ? 'color:' + fg + ';' : ''}">${mono}</span>${img}`;
  }

  // The dApp's own favicon makes the sheet feel native to the site; fall back to a
  // themed letter avatar if the page declares none. The <img> onerror swaps back to
  // the letter so a 404 icon never leaves an empty box.
  function faviconUrl() {
    try {
      const link = document.querySelector('link[rel~="icon"], link[rel="shortcut icon"], link[rel="apple-touch-icon"]');
      if (link && link.href) return link.href;
      return ORIGIN + '/favicon.ico';
    } catch (_) {
      return '';
    }
  }
  function favMarkup(label) {
    const letter = esc((label || '?').slice(0, 1).toUpperCase());
    const url = faviconUrl();
    if (!url) return letter;
    return `<img src="${esc(url)}" alt="" onerror="this.remove();this.parentNode.textContent='${letter}'">`;
  }

  // ---- 3b. connect sheet ----------------------------------------------------
  function showConnectSheet({ account, onConfirm, onCancel, emptyState }) {
    const root = ensureHost();
    const sheet = root.getElementById('sheet');
    const label = hostLabel(ORIGIN);
    const fav = favMarkup(label);
    // Backdrop tap = Cancel (nothing granted yet — safe to reject the request).
    sheetOnBackdrop = () => { closeSheet(); onCancel && onCancel(); };

    if (emptyState) {
      sheet.innerHTML = `
        <div class="brow"><div class="fav">${fav}</div><div class="host">${esc(label)}</div></div>
        <div class="title">${esc(L('connect.emptyTitle'))}</div>
        <p class="sub">${esc(L('connect.emptySub'))}</p>
        <div class="actions">
          <button class="btn ghost" id="cancel">${esc(L('common.cancel'))}</button>
          <a class="btn primary" id="cta" href="velawallet://">${esc(L('common.openVelaLogin'))}</a>
        </div>`;
      root.getElementById('cancel').onclick = () => { closeSheet(); onCancel && onCancel(); };
      root.getElementById('cta').addEventListener('click', () => { setTimeout(() => { closeSheet(); onCancel && onCancel(); }, 50); });
      return;
    }

    const chains = account.chains || {};
    const chainIds = Object.keys(chains);
    const curCid = String(account.chainId || 1);
    const options = chainIds.length
      ? chainIds
          .map((id) => `<option value="${esc(id)}" ${id === curCid ? 'selected' : ''}>${esc(chains[id].name || 'Chain ' + id)}</option>`)
          .join('')
      : `<option value="${esc(curCid)}" selected>Chain ${esc(curCid)}</option>`;

    sheet.innerHTML = `
      <div class="brow"><div class="fav">${fav}</div><div class="host">${esc(label)}</div></div>
      <div class="title">${esc(L('connect.wantsToConnect', { host: label }))}</div>
      <div class="row"><span class="muted">${esc(L('connect.account'))}</span>
        <span><b>${esc(account.name || 'Account')}</b> <span class="mono">${esc(truncAddr(account.address))}</span></span></div>
      <div class="row"><span class="muted">${esc(L('connect.network'))}</span>
        <span class="netpick"><span class="netbadge" id="netbadge">${chainBadge(chains, curCid)}</span><select id="chain">${options}</select></span></div>
      <div class="divider"></div>
      <div class="perm"><span class="i ok">✓</span><span>${esc(L('connect.permView'))}</span></div>
      <div class="perm"><span class="i ok">✓</span><span>${esc(L('connect.permSign'))}</span></div>
      <div class="perm"><span class="i no">✕</span><span>${esc(L('connect.permNoFunds'))}</span></div>
      <div class="actions">
        <button class="btn ghost" id="cancel">${esc(L('common.cancel'))}</button>
        <button class="btn primary" id="cta">${esc(L('connect.connect'))}</button>
      </div>`;

    root.getElementById('cancel').onclick = () => { closeSheet(); onCancel && onCancel(); };
    // Keep the network badge in sync with the selected chain.
    const chainSel = root.getElementById('chain');
    if (chainSel) chainSel.onchange = () => {
      const badge = root.getElementById('netbadge');
      if (badge) badge.innerHTML = chainBadge(chains, chainSel.value);
    };
    root.getElementById('cta').onclick = () => {
      const cid = parseChainId(chainSel ? chainSel.value : curCid) || account.chainId || 1;
      closeSheet();
      onConfirm && onConfirm(account.address, cid);
    };
  }

  // ---- 3c. sign hand-off sheet (State A intent → C waiting → D resolved) -----
  function showSignIntent({ rid, method, params }) {
    const root = ensureHost();
    const sheet = root.getElementById('sheet');
    const label = hostLabel(ORIGIN);
    const fav = favMarkup(label);
    const summary = signSummary(method, params);
    // Custom-scheme launch shows an "Open in Vela?" banner (R3: one extra tap);
    // a REAL tap on this anchor is the user gesture iOS needs (FACT-1).
    // Human summary only — no raw RPC method row (jargon like 'eth_sendTransaction').
    // The authoritative, fully-decoded detail is the native SigningRequestModal one tap
    // away; this hand-off is a tight "confirm in Vela" preview.
    sheet.innerHTML = `
      <div class="brow"><div class="fav">${fav}</div><div class="host">${esc(label)}</div></div>
      <div class="title">${esc(L('sign.title'))}</div>
      <p class="sub">${esc(summary)}</p>
      <div class="actions">
        <button class="btn ghost" id="cancel">${esc(L('common.cancel'))}</button>
        <button class="btn primary" id="cta">${esc(L('sign.confirmInVela'))}</button>
      </div>`;
    root.getElementById('cancel').onclick = () => onSignReject(rid); // pre-launch, safe 4001
    sheetOnBackdrop = () => onSignReject(rid); // tap outside = cancel (pre-launch, safe)
    // A BUTTON + imperative location.href (the R1-proven launch), NOT an <a href>:
    // onSignLaunch swaps the sheet to the waiting state, which would detach an
    // anchor mid-click and cancel its default navigation. location.href fires
    // the scheme nav synchronously (keeps the tap's user activation) before the
    // UI swap is deferred.
    root.getElementById('cta').addEventListener('click', () => onSignLaunch(rid));
  }

  function showSignWaiting() {
    if (!sheetHost) return;
    const root = sheetHost.__root;
    const sheet = root.getElementById('sheet');
    sheet.innerHTML = `
      <div class="ring"></div>
      <div class="center" style="font-size:17px;font-weight:600;">${esc(L('sign.completeInVela'))}</div>
      <p class="sub center">${esc(L('sign.returnWhenDone'))}</p>
      <div class="actions">
        <button class="btn ghost" id="closereq">${esc(L('sign.closeContinue'))}</button>
        <a class="btn primary" id="reopen" href="#">${esc(L('sign.backToVela'))}</a>
      </div>`;
    // Post-launch: dismiss only — NEVER resolve 4001 (the app may have submitted).
    root.getElementById('closereq').onclick = () => dismissWaiting();
    root.getElementById('reopen').onclick = (e) => { e.preventDefault(); reopenActiveSign(); };
    sheetOnBackdrop = () => dismissWaiting(); // tap outside = hide (post-launch, never 4001)
  }

  // §12.3 dead-worker floor: after ~one poll cycle (~6s) returns nothing, the
  // waiting sheet must STOP implying progress — swap the breathing ring for a
  // recoverable "check Vela Activity" affordance. The rid stays pending (re-polls
  // on the next focus and can still settle submitted/rejected), so this is NOT a
  // terminal state and NEVER resolves 4001 — the ring just never hangs (gate c).
  function showSignChecking() {
    if (!sheetHost) return; // user dismissed → don't reopen the sheet
    const root = sheetHost.__root;
    const sheet = root.getElementById('sheet');
    if (!sheet || sheet.dataset.state === 'checking') return; // already shown — no re-render churn
    sheet.dataset.state = 'checking';
    // In-progress framing (NOT the terminal "didn't hear back" copy): this state is
    // still polling and fully recoverable, so it must read as "taking a moment / here's
    // how to check", distinct from the 3-min timeout's terminal notConfirmed.
    sheet.innerHTML = `
      <div class="big wrap" style="color:var(--vela-warning);background:var(--vela-accent-soft)">!</div>
      <div class="center" style="font-weight:600;font-size:17px">${esc(L('sign.completeInVela'))}</div>
      <p class="sub center">${esc(L('sign.checkingSub'))}</p>
      <div class="actions">
        <button class="btn ghost" id="closereq">${esc(L('sign.closeContinue'))}</button>
        <a class="btn primary" id="reopen" href="#">${esc(L('sign.backToVela'))}</a>
      </div>`;
    root.getElementById('closereq').onclick = () => dismissWaiting();
    root.getElementById('reopen').onclick = (e) => { e.preventDefault(); reopenActiveSign(); };
    sheetOnBackdrop = () => dismissWaiting(); // tap outside = hide (post-launch, never 4001)
  }

  // Re-fire the app launch for the active rid (State C "返回 Vela" / the check floor).
  // Consumed only on RESULT read, so a re-open is safe (sign.tsx de-dupes + the
  // transport replays an existing result rather than re-signing).
  function reopenActiveSign() {
    const rid = activeSignRid;
    if (!rid) return;
    const entry = signMap.get(rid) || {};
    // Mirror onSignLaunch: stamp UL_PENDING before a UL nav so this path self-heals
    // if the association broke since the first launch.
    if (entry.ulVerified && hasStorage) {
      try { browser.storage.local.set({ [UL_PENDING_KEY]: { ts: Date.now(), origin: ORIGIN } }); } catch (_) { /* best-effort */ }
    }
    window.location.href = signLaunchUrl(rid, entry.ulVerified);
  }

  function showSignResolved(kind, info) {
    if (!sheetHost) { closeSheet(); return; }
    const root = sheetHost.__root;
    const sheet = root.getElementById('sheet');
    sheetOnBackdrop = () => closeSheet(); // tap outside = dismiss the resolved sheet
    if (kind === 'signed') {
      sheet.innerHTML = `<div class="big wrap ok" style="background:var(--vela-success-soft)">✓</div>
        <div class="center" style="font-weight:600;font-size:17px">${esc(L('sign.sent'))}</div>
        <p class="sub center mono">${esc(String(info && info.hash).slice(0, 22))}…</p>`;
      setTimeout(closeSheet, 1600);
    } else if (kind === 'rejected') {
      sheet.innerHTML = `<div class="big wrap muted" style="background:var(--vela-bg-sunken)">✕</div>
        <div class="center" style="font-weight:600;font-size:17px">${esc(L('sign.cancelled'))}</div>`;
      setTimeout(closeSheet, 1300);
    } else {
      sheet.innerHTML = `<div class="big wrap" style="color:var(--vela-warning);background:var(--vela-accent-soft)">!</div>
        <div class="center" style="font-weight:600;font-size:17px">${esc(L('sign.notConfirmed'))}</div>
        <p class="sub center">${esc(L('sign.notConfirmedSub'))}</p>
        <div class="actions"><button class="btn ghost" id="ok">${esc(L('sign.ok'))}</button></div>`;
      const b = root.getElementById('ok');
      if (b) b.onclick = closeSheet;
    }
  }

  function signSummary(method, params) {
    if (method === 'personal_sign') return L('summary.personalSign');
    if (method.includes('signTypedData')) return L('summary.typedData');
    if (method === 'eth_sendTransaction') return L('summary.sendTx');
    if (method === 'wallet_sendCalls') return L('summary.sendCalls');
    return L('summary.default');
  }

  // ---- 2. provider request router -------------------------------------------
  window.addEventListener('message', (ev) => {
    if (ev.source !== window) return;
    const d = ev.data;
    if (!d || d.ch !== CHANNEL || d.dir !== 'req') return;
    handleReq(d.id, d.method, d.params);
  });

  async function handleReq(rpcId, method, params) {
    const cls = classifyMethod(method);
    try {
      if (cls === 'sign') return void handleSign(rpcId, method, params);
      if (cls === 'connect') return void handleConnect(rpcId);
      // state / read / switch / addChain / unsupported → background
      const resp = await bg({ type: 'rpc', method, params, origin: ORIGIN });
      if (resp && resp.ok) {
        respond(rpcId, resp.result);
        if (cls === 'switch' && typeof resp.chainId === 'number') emitEvt('chainChanged', toHexChainId(resp.chainId));
      } else {
        respondErr(rpcId, (resp && resp.error) || rpcError(ERR.INTERNAL, 'Request failed'));
      }
    } catch (e) {
      respondErr(rpcId, rpcError(ERR.INTERNAL, String(e && e.message ? e.message : e)));
    }
  }

  async function handleConnect(rpcId) {
    if (busy) return respondErr(rpcId, rpcError(-32002, 'A request is already pending. Open the Vela sheet.'));
    busy = true; // claim the single-sheet slot SYNCHRONOUSLY (before any await)
    const resp = await bg({ type: 'connect', origin: ORIGIN });
    if (resp && resp.theme) appTheme = resp.theme; // match the app's color scheme
    if (resp && resp.locale) appLocale = resp.locale; // and the app's language
    if (resp && resp.granted) {
      busy = false; // no sheet shown → release the slot
      respond(rpcId, resp.result);
      emitEvt('accountsChanged', resp.result);
      emitEvt('connect', { chainId: toHexChainId(resp.chainId || 1) });
      return;
    }
    if (resp && resp.noAccount) {
      showConnectSheet({ emptyState: true, onCancel: () => respondErr(rpcId, rpcError(ERR.USER_REJECTED, 'Open Vela and log in, then retry')) });
      return;
    }
    if (resp && resp.needsConsent) {
      showConnectSheet({
        account: resp.account,
        onConfirm: async (address, chainId) => {
          const g = await bg({ type: 'grantConnect', origin: ORIGIN, address, chainId });
          if (g && g.ok) {
            respond(rpcId, g.result);
            emitEvt('accountsChanged', g.result);
            emitEvt('connect', { chainId: toHexChainId(g.chainId || chainId || 1) });
          } else {
            respondErr(rpcId, (g && g.error) || rpcError(ERR.INTERNAL, 'Connect failed'));
          }
        },
        onCancel: () => respondErr(rpcId, rpcError(ERR.USER_REJECTED, 'User rejected the request')),
      });
      return;
    }
    busy = false; // error path, no sheet → release the slot
    respondErr(rpcId, (resp && resp.error) || rpcError(ERR.USER_REJECTED, 'Connect failed'));
  }

  // ===========================================================================
  //  SIGN — launch the app, focus-poll the result, resolve the dApp promise
  // ===========================================================================
  const SIGN_KEY = SIGN_PREFIX; // storage.local["vela.sign.<rid>"]
  const POLL_MS = 1200;
  const RETRY_GAP_MS = 300;
  const MAX_ATTEMPTS = 4;
  const CEILING_MS = 3 * 60 * 1000;

  const signMap = new Map(); // rid -> { rpcId, method, params }  (this realm only)
  const resolvedRids = new Set();
  let activeSignRid = null;

  const signKey = (rid) => SIGN_KEY + rid;
  async function getMirror(rid) {
    if (!hasStorage) return null;
    const all = await browser.storage.local.get(signKey(rid));
    return all[signKey(rid)] || null;
  }
  async function setMirror(rid, patch) {
    if (!hasStorage) return;
    const cur = (await getMirror(rid)) || { rid, origin: ORIGIN };
    await browser.storage.local.set({ [signKey(rid)]: { ...cur, ...patch } });
  }
  async function clearPending(rid) {
    if (hasStorage) await browser.storage.local.remove(signKey(rid));
  }
  async function listPending() {
    if (!hasStorage) return [];
    const all = await browser.storage.local.get(null);
    return Object.keys(all)
      .filter((k) => k.startsWith(SIGN_KEY))
      .map((k) => all[k])
      .filter((m) => m && m.origin === ORIGIN && m.state !== 'RESOLVED' && !resolvedRids.has(m.rid));
  }

  async function handleSign(rpcId, method, params) {
    if (busy) return respondErr(rpcId, rpcError(-32002, 'A request is already pending.'));
    busy = true; // claim the single-sheet slot SYNCHRONOUSLY (before any await)
    // Require a prior connect (defense-in-depth; the app is the real gate). The
    // connect response also carries the origin's granted chainId — the ONLY
    // authoritative source of the chain to sign against at cold-launch (F4). The
    // app has no meaningful global chain; passing it in the sign-req lets the
    // native sign build the Safe EIP-712 domain for the RIGHT chain.
    const c = await bg({ type: 'connect', origin: ORIGIN });
    if (c && c.theme) appTheme = c.theme; // match the app's color scheme on the sign sheet
    if (c && c.locale) appLocale = c.locale; // and the app's language
    if (!c || !c.granted) {
      busy = false; // no sheet shown → release the slot
      return respondErr(rpcId, rpcError(ERR.UNAUTHORIZED, 'Unauthorized. Call eth_requestAccounts first.'));
    }

    const rid = (crypto && crypto.randomUUID && crypto.randomUUID()) || String(Date.now()) + Math.random().toString(16).slice(2);
    // ulVerified: launch the one-tap UL only if the app has ATTESTED it (connect
    // response, app-side + TTL) AND the extension has not vetoed it after a prior
    // failure (UL_BROKEN, self-heal). Captured per-rid so onSignLaunch can read it
    // synchronously inside the tap turn (no async there — user activation is
    // load-bearing). Either falsy → the proven velawallet:// scheme.
    // Self-heal veto: a break AFTER attestation sets UL_BROKEN with a timestamp.
    // The veto holds only while it is NEWER than the attestation — a re-attestation
    // (successful popup probe / real UL sign) has a fresher ulVerifiedAt and thus
    // re-enables the UL with no optimistic clearing or race.
    let brokenAt = 0;
    if (hasStorage) {
      try {
        const b = await browser.storage.local.get(UL_BROKEN_KEY);
        const rec = b && b[UL_BROKEN_KEY];
        brokenAt = rec && typeof rec.ts === 'number' ? rec.ts : 0;
      } catch (_) { /* treat as not-broken; the app attestation still gates */ }
    }
    const attestedAt = (c && c.ulVerifiedAt) || 0;
    const useUL = !!(c && c.ulVerified) && (brokenAt === 0 || attestedAt > brokenAt);
    // Carry the origin's GRANTED address so the app can sign from the account the
    // dApp is actually connected to — not whatever account happens to be active
    // (§12.1.6: never silently sign from the wrong account).
    const grantedAddress = (c.result && c.result[0]) || null;
    signMap.set(rid, { rpcId, method, params, chainId: c.chainId, address: grantedAddress, ulVerified: useUL });
    activeSignRid = rid;

    // Durable mirror BEFORE any launch (survives reload/tab-discard). set() does
    // not consume user activation, and this runs before the sheet's tap.
    await setMirror(rid, {
      rid, origin: ORIGIN, method, state: 'PENDING_STORED',
      launchedAt: Date.now(), pendingStoredAt: Date.now(), pollAttempts: 0,
    });
    mirrorStatus('pending', rid);
    showSignIntent({ rid, method, params });
  }

  // Fired on the CTA tap. Order is load-bearing:
  //   1. fire-and-forget writeSignRequest (no DOM change, no await) — RACES the nav
  //   2. SYNCHRONOUS location.href scheme nav — inside the tap turn, keeps user
  //      activation (FACT-1); shows the "Open in Vela?" banner
  //   3. DEFER the sheet swap to a macrotask so replacing the DOM can't cancel the
  //      in-flight navigation started in step 2
  function onSignLaunch(rid) {
    const entry = signMap.get(rid);
    if (!entry) return;
    // chainId is ADDITIVE + optional on the frozen sign-req contract (Swift writes
    // the dict verbatim; old readers ignore it). It carries the origin's granted chain.
    const request = { rid, method: entry.method, params: entry.params, origin: ORIGIN, ts: Date.now(), chainId: entry.chainId, address: entry.address };
    try {
      browser.runtime.sendMessage({ type: 'writeSignRequest', rid, request }).catch(() => {});
    } catch (_) { /* worker evicted mid-flight; launch still proceeds */ }
    // Stamp BEFORE a UL nav so that, if iOS routes it to the web page instead of
    // the app, the getvela.app landing (above) can detect the failure and veto UL.
    // Cleared on success in armLaunchFallback's onHide (tab backgrounded = app open).
    if (entry.ulVerified && hasStorage) {
      try { browser.storage.local.set({ [UL_PENDING_KEY]: { ts: Date.now(), origin: ORIGIN } }); } catch (_) { /* best-effort */ }
    }
    window.location.href = signLaunchUrl(rid, entry.ulVerified);
    setTimeout(() => { showSignWaiting(); armLaunchFallback(rid); }, 0);
  }

  // PRE-LAUNCH reject only (State A "取消"): the app was never launched, so
  // nothing was submitted — 4001 is truthful. Reachable ONLY before the CTA tap
  // (showSignWaiting replaces this button). NEVER call after launch: once the app
  // is foregrounded it may already have submitted, and a local 4001 would be a
  // false decline (invariant b) → dApp retries → double-spend.
  function onSignReject(rid) {
    const entry = signMap.get(rid);
    if (entry && !resolvedRids.has(rid)) {
      resolvedRids.add(rid);
      respondErr(entry.rpcId, rpcError(ERR.USER_REJECTED, 'User rejected the request'));
      signMap.delete(rid);
    }
    void clearPending(rid);
    mirrorStatus('rejected', rid);
    closeSheet();
    if (activeSignRid === rid) activeSignRid = null;
  }

  // POST-LAUNCH dismiss (State C "关闭"): just hide the sheet. The rid stays
  // pending — the focus-poll still delivers a real submitted/rejected result, or
  // settles 4900 (never 4001) at the ceiling. We must NOT resolve here because
  // the app may already have submitted while the user was away.
  function dismissWaiting() {
    closeSheet(); // keeps signMap + the durable mirror; serviceRid continues
  }

  // If the tab never hides within ~1.5s, the app didn't foreground — offer a
  // fresh gesture (NEVER auto-retry; iOS throttles repeated scheme navigations).
  function armLaunchFallback() {
    let hidden = false;
    const onHide = () => {
      if (document.visibilityState === 'hidden') {
        hidden = true;
        // Tab backgrounded → the app opened → the launch SUCCEEDED. Clear the UL
        // pending stamp so a later unrelated visit to getvela.app can't false-veto.
        if (hasStorage) browser.storage.local.remove(UL_PENDING_KEY).catch(() => {});
      }
    };
    document.addEventListener('visibilitychange', onHide);
    window.addEventListener('pagehide', onHide, { once: true });
    setTimeout(() => {
      document.removeEventListener('visibilitychange', onHide);
      if (hidden || !sheetHost) return;
      const root = sheetHost.__root;
      const cta = root && root.getElementById('reopen');
      if (cta) cta.textContent = L('sign.openVela');
    }, 1500);
  }

  // ---- return poll (bounded, focus-triggered) -------------------------------
  function pollTimeout(promise, ms) {
    return Promise.race([promise, new Promise((res) => setTimeout(() => res(TIMEOUT), ms))]);
  }
  async function pollNativeOnce(rid) {
    let resp;
    try {
      resp = await pollTimeout(browser.runtime.sendMessage({ type: 'pollSignResult', rid }), POLL_MS);
    } catch (_) {
      return { status: 'unknown' };
    }
    if (resp === undefined || resp === TIMEOUT) return { status: 'unknown' };
    if (resp && resp.type === 'sign-result' && resp.found) return { found: true, result: resp.result };
    return { status: 'unknown' };
  }

  // Resolve the waiting dApp promise + settle durable state. At most once per rid.
  async function settleSign(rid, outcome) {
    if (resolvedRids.has(rid)) return;
    resolvedRids.add(rid);
    const entry = signMap.get(rid);
    if (outcome.kind === 'signed') {
      if (entry) respond(entry.rpcId, outcome.hash); // resolves dApp promise
      await setMirror(rid, { state: 'RESOLVED', outcome: 'submitted', userOpHash: outcome.hash, resolvedAt: Date.now() });
      mirrorStatus('signed', rid, outcome.hash);
      showSignResolved('signed', { hash: outcome.hash });
    } else if (outcome.kind === 'rejected') {
      if (entry) respondErr(entry.rpcId, rpcError(ERR.USER_REJECTED, 'User rejected the request'));
      await setMirror(rid, { state: 'RESOLVED', outcome: 'rejected', resolveCode: ERR.USER_REJECTED, resolvedAt: Date.now() });
      mirrorStatus('rejected', rid);
      showSignResolved('rejected', {});
    }
    signMap.delete(rid);
    if (activeSignRid === rid) activeSignRid = null;
    await clearPending(rid);
  }

  async function serviceRid(rid) {
    if (resolvedRids.has(rid)) return;
    const mirror = (await getMirror(rid)) || {};
    // OWNERSHIP: only the realm that created this rid (it's in this tab's signMap)
    // may settle/respond/clear it. storage.local is shared across every same-origin
    // tab, so without this a SECOND tab would service tab A's rid, mark the shared
    // mirror RESOLVED + clear it, yet be unable to resolve tab A's promise (no
    // signMap entry) → tab A's dApp promise hangs forever (invariants a + c). A
    // non-owner only GCs a clearly-abandoned mirror (post-reload orphan / dead
    // owner) so storage doesn't leak; it never touches a still-live one.
    if (!signMap.has(rid)) {
      const arm = mirror.launchedAt || mirror.pendingStoredAt || Date.now();
      if (mirror.state === 'RESOLVED' || Date.now() - arm >= CEILING_MS) await clearPending(rid);
      return;
    }
    if (mirror.state === 'RESOLVED') { resolvedRids.add(rid); return; }
    const firstArm = mirror.launchedAt || mirror.pendingStoredAt || Date.now();
    await setMirror(rid, { state: 'POLLING', returnedAt: Date.now() });

    for (let n = 1; n <= MAX_ATTEMPTS; n++) {
      if (resolvedRids.has(rid)) return;
      await setMirror(rid, { pollAttempts: n });
      const r = await pollNativeOnce(rid);
      if (r.found) {
        const st = r.result && r.result.status;
        const hash = r.result && r.result.userOpHash;
        if (st === 'submitted') return settleSign(rid, { kind: 'signed', hash });
        if (st === 'rejected') return settleSign(rid, { kind: 'rejected' });
      }
      if (n < MAX_ATTEMPTS) await new Promise((res) => setTimeout(res, RETRY_GAP_MS));
    }

    // Unresolved this focus. §12.1.3: NEVER 4001 here. Below the ~3min ceiling
    // stay pending (re-poll on next focus); at the ceiling settle the promise
    // with the distinct 4900 pending code + a recoverable "check Vela" state.
    if (Date.now() - firstArm >= CEILING_MS) {
      if (!resolvedRids.has(rid)) {
        resolvedRids.add(rid);
        const entry = signMap.get(rid);
        if (entry) respondErr(entry.rpcId, rpcError(ERR.UNKNOWN_PENDING, 'Pending or unknown — check Vela Activity'));
        signMap.delete(rid);
      }
      await setMirror(rid, { state: 'TIMEOUT', resolveCode: ERR.UNKNOWN_PENDING });
      mirrorStatus('timeout', rid);
      showSignResolved('timeout', {});
      if (activeSignRid === rid) activeSignRid = null;
    } else {
      await setMirror(rid, { state: 'CHECK_VELA' });
      mirrorStatus('check-vela', rid);
      // ~6s floor (one poll cycle elapsed with no result): the VISIBLE sheet swaps
      // the breathing ring for a recoverable "check Vela" affordance so the ring
      // never hangs. Stays pending — re-polls on the next focus (gate c).
      if (activeSignRid === rid) showSignChecking();
    }
  }

  async function serviceAllPending() {
    for (const m of await listPending()) serviceRid(m.rid);
  }

  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'visible') serviceAllPending();
  });
  // Re-arm on every content-script load (reload / tab-discard wiped the realm).
  serviceAllPending();

  // ---- machine-readable verdict mirror (E2E harness reads this) -------------
  // Light-DOM element; the human sheet lives in the shadow root. Kept so the
  // existing e2e/safari harness can read outcomes without shadow traversal.
  function mirrorStatus(kind, rid, hash) {
    let el = document.getElementById('vela-r1-sign-status');
    if (!el) {
      el = document.createElement('div');
      el.id = 'vela-r1-sign-status';
      el.style.cssText = 'position:fixed;left:-9999px;top:-9999px;';
      (document.body || document.documentElement).appendChild(el);
    }
    el.dataset.rid = rid || '';
    if (kind === 'signed') el.textContent = '✓ submitted ' + String(hash || '').slice(0, 14) + '…';
    else if (kind === 'rejected') el.textContent = '✕ rejected (4001)';
    else if (kind === 'check-vela') el.textContent = '在 Vela 的活动中查看这笔交易 (check Vela)';
    else if (kind === 'timeout') el.textContent = '结果暂不可用 · 请在 Vela 中查看 (4900)';
    else if (kind === 'pending') el.textContent = 'pending';
  }

  console.log('[Vela] content bridge ready on', ORIGIN);
})();
