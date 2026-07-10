// Vela Safari extension — in-page provider (runs in the page's MAIN world).
//
// Phase A: the REAL EIP-1193 + EIP-6963 provider. Replaces the R1 stub
// (window.__velaR1). NO extension APIs here (MAIN world) — every request is
// relayed to content.js over a tagged window.postMessage channel and correlated
// by a page-local rpcId. content.js answers read/state/connect locally (via the
// background) and routes signing to the native app (the proven launch+return).
//
// Compatibility rules implemented per docs/safari-extension/ARCHITECTURE.md §12.4:
//   - EIP-6963 eager announce, frozen info, announced provider === window.ethereum
//   - dispatch ethereum#initialized; set window.ethereum only if absent, configurable
//   - legacy shims: send / sendAsync / enable; sync props selectedAddress /
//     chainId / networkVersion / isConnected() backed by an inpage session cache
//   - reconcile {chainId, accounts} on every response; dedupe delivery per rpcId
//   - spoof isMetaMask on the LEGACY window.ethereum singleton for MM-only dApps,
//     while EIP-6963 keeps announcing the true Vela identity (see the provider
//     object); reject nothing here that the router can answer — the router owns
//     policy (eth_sign refusal, etc.)
/* global browser, chrome */
import { CHANNEL, RDNS, WALLET_NAME, ERR, rpcError, toHexChainId } from './lib/protocol.js';

(() => {
  // World guard. This file must run in the page's MAIN world (window.ethereum has
  // to be visible to the dApp). It gets there two ways: a `world:"MAIN"` content
  // script (Safari 18+) or content.js's <script src> tag (older Safari). If an
  // OLDER Safari honored the content_scripts entry but IGNORED `world:"MAIN"`, this
  // would run as an ISOLATED content script instead — where extension APIs exist
  // and window.ethereum is invisible to the page. Detect that and bail WITHOUT
  // marking, so content.js still fires its MAIN-world <script> fallback. In the
  // real MAIN world `browser`/`chrome` are undefined → we proceed.
  try {
    const ext = (typeof browser !== 'undefined' && browser) || (typeof chrome !== 'undefined' && chrome);
    if (ext && ext.runtime && ext.runtime.id) return;
  } catch (_) {
    /* touching `browser` threw → page world → proceed */
  }

  // Idempotency: a document_start content script can inject twice on some pages,
  // and Phase-3 injects the provider TWO ways (a MAIN-world content script on
  // Safari 18+, and content.js's runtime.getURL <script> fallback on older Safari
  // / strict-CSP misses). Whichever wins, the other no-ops here.
  if (window.__velaProviderInstalled) return;
  window.__velaProviderInstalled = true;
  // Shared-DOM marker: window.__velaProviderInstalled lives in THIS world only, so
  // the isolated content script can't read it. A documentElement attribute rides
  // the shared DOM across worlds — content.js checks it to skip its fallback
  // <script> inject (which a strict CSP would block + log) once MAIN-world won.
  try {
    document.documentElement.setAttribute('data-vela-inpage', '1');
  } catch (_) {
    /* no documentElement yet — content.js falls back to the tag inject */
  }

  // Per-page-load session uuid (EIP-6963 requires a fresh uuid each announce set).
  const SESSION_UUID =
    (window.crypto && window.crypto.randomUUID && window.crypto.randomUUID()) ||
    'vela-' + Date.now().toString(16) + Math.floor(Math.random() * 1e9).toString(16);

  // Data-URI icon: the REAL Vela app icon (the sailboat brand mark) so dApp wallet
  // pickers (EIP-6963) show the correct logo, not a placeholder. 128px PNG, inlined.
  const ICON = 'data:image/png;base64,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';

  // ---- tiny event emitter (EIP-1193 events) ---------------------------------
  const listeners = new Map(); // event -> Set<fn>
  function on(event, fn) {
    if (typeof fn !== 'function') return provider;
    let s = listeners.get(event);
    if (!s) listeners.set(event, (s = new Set()));
    s.add(fn);
    return provider;
  }
  function removeListener(event, fn) {
    const s = listeners.get(event);
    if (s) s.delete(fn);
    return provider;
  }
  function once(event, fn) {
    const wrap = (...a) => {
      removeListener(event, wrap);
      fn(...a);
    };
    return on(event, wrap);
  }
  function emit(event, ...args) {
    const s = listeners.get(event);
    if (!s) return;
    for (const fn of [...s]) {
      try {
        fn(...args);
      } catch (e) {
        // A throwing dApp listener must never break the provider.
        console.error('[Vela] listener error for', event, e);
      }
    }
  }

  // ---- session cache (backs the synchronous legacy props) -------------------
  const session = {
    accounts: [], // lowercased addresses the dApp is authorized to see
    chainIdNum: null, // number, or null until first learned
    connected: false,
  };

  // Reconcile cache from a fresh (method,result) or an event; emit the EIP-1193
  // events that actually changed. Called on every response and every 'evt'.
  function applyAccounts(next) {
    const norm = Array.isArray(next) ? next.filter((a) => typeof a === 'string').map((a) => a.toLowerCase()) : [];
    const changed = norm.length !== session.accounts.length || norm.some((a, i) => a !== session.accounts[i]);
    session.accounts = norm;
    if (changed) emit('accountsChanged', norm);
    return changed;
  }
  function applyChain(nextNum) {
    if (!Number.isFinite(nextNum) || nextNum <= 0) return false;
    if (session.chainIdNum === nextNum) return false; // dedupe
    const first = session.chainIdNum === null;
    session.chainIdNum = nextNum;
    const hex = toHexChainId(nextNum);
    if (first) {
      // Learning the chain for the FIRST time (init warm) is NOT a change — emit
      // `connect` (provider is now usable) but never `chainChanged`, which many
      // dApps react to with location.reload(). Only subsequent switches change.
      if (!session.connected) {
        session.connected = true;
        emit('connect', { chainId: hex });
      }
    } else {
      emit('chainChanged', hex);
    }
    return true;
  }

  // ---- request/response correlation over postMessage ------------------------
  let seq = 0;
  const pending = new Map(); // rpcId -> { resolve, reject }

  function nextId() {
    seq += 1;
    return SESSION_UUID + ':' + seq;
  }

  function post(method, params, id) {
    window.postMessage({ ch: CHANNEL, dir: 'req', id, method, params: params ?? [] }, window.location.origin);
  }

  // Update the cache from a method's own successful result (cheap reconciliation
  // that needs no annotation from content).
  function reconcileFromResult(method, result) {
    if (method === 'eth_chainId') applyChain(parseIntChain(result));
    else if (method === 'eth_accounts' || method === 'eth_requestAccounts') applyAccounts(result);
    else if (method === 'net_version') applyChain(parseIntChain(result));
    // NOTE: do NOT set session.connected here. `connected` (and the EIP-1193
    // 'connect' event) is owned by applyChain's first-learn branch. Setting it on
    // any non-null result would let eth_accounts (which resolves [] before the
    // warm eth_chainId) flip connected early and permanently SUPPRESS 'connect'.
  }
  function parseIntChain(v) {
    if (typeof v === 'number') return v;
    if (typeof v === 'string') return v.startsWith('0x') ? parseInt(v, 16) : parseInt(v, 10);
    return NaN;
  }

  window.addEventListener('message', (ev) => {
    // Only trust same-window messages on our channel (the page shares this world,
    // but tagging avoids collisions with other providers / nested iframes).
    if (ev.source !== window) return;
    const d = ev.data;
    if (!d || d.ch !== CHANNEL) return;

    if (d.dir === 'res') {
      const entry = pending.get(d.id);
      if (!entry) return; // unknown / already-settled → dedupe double-delivery
      pending.delete(d.id);
      if (d.error) {
        entry.reject(Object.assign(new Error(d.error.message || 'Request failed'), d.error));
      } else {
        reconcileFromResult(entry.method, d.result);
        entry.resolve(d.result);
      }
      return;
    }

    if (d.dir === 'evt') {
      switch (d.event) {
        case 'accountsChanged':
          applyAccounts(d.data);
          break;
        case 'chainChanged':
          applyChain(parseIntChain(d.data));
          break;
        case 'connect':
          if (!session.connected) {
            session.connected = true;
            emit('connect', { chainId: toHexChainId(session.chainIdNum || 1) });
          }
          break;
        case 'disconnect':
          session.connected = false;
          emit('disconnect', rpcError(ERR.UNKNOWN_PENDING, 'Provider disconnected'));
          break;
        case 'message':
          emit('message', d.data);
          break;
        default:
          break;
      }
    }
  });

  // ---- the EIP-1193 request() -----------------------------------------------
  function request(args) {
    return new Promise((resolve, reject) => {
      if (!args || typeof args !== 'object' || typeof args.method !== 'string' || args.method.length === 0) {
        reject(Object.assign(new Error('Invalid request arguments'), rpcError(ERR.INVALID_PARAMS, 'Expected { method, params }')));
        return;
      }
      const params = args.params === undefined ? [] : args.params;
      if (params !== null && typeof params !== 'object') {
        reject(Object.assign(new Error('Invalid params'), rpcError(ERR.INVALID_PARAMS, 'params must be an array or object')));
        return;
      }
      const id = nextId();
      pending.set(id, { resolve, reject, method: args.method });
      post(args.method, params, id);
    });
  }

  // ---- legacy shims (web3.js / ethers ≤v4 / detect-provider) ----------------
  function enable() {
    return request({ method: 'eth_requestAccounts' });
  }
  // Legacy dual-form send(): send(method, params) OR send({method,params}[, cb]).
  function send(methodOrPayload, paramsOrCb) {
    if (typeof methodOrPayload === 'string') {
      return request({ method: methodOrPayload, params: Array.isArray(paramsOrCb) ? paramsOrCb : [] });
    }
    // Some very old callers pass (payload, callback) synchronously.
    if (typeof paramsOrCb === 'function') {
      return sendAsync(methodOrPayload, paramsOrCb);
    }
    // ethers v4 style: synchronous result for a few pure methods, else throw.
    const p = methodOrPayload || {};
    switch (p.method) {
      case 'eth_accounts':
        return { id: p.id, jsonrpc: '2.0', result: session.accounts };
      case 'eth_chainId':
        return { id: p.id, jsonrpc: '2.0', result: session.chainIdNum ? toHexChainId(session.chainIdNum) : null };
      case 'net_version':
        return { id: p.id, jsonrpc: '2.0', result: session.chainIdNum ? String(session.chainIdNum) : null };
      default: {
        const msg = 'Vela: synchronous send() is only supported for eth_accounts/eth_chainId/net_version';
        throw Object.assign(new Error(msg), rpcError(ERR.UNSUPPORTED_METHOD, msg));
      }
    }
  }
  function sendAsync(payload, cb) {
    // Batch support (rare) — resolve each independently.
    if (Array.isArray(payload)) {
      Promise.all(payload.map((p) => request({ method: p.method, params: p.params })))
        .then((results) => cb(null, results.map((r, i) => ({ id: payload[i].id, jsonrpc: '2.0', result: r }))))
        .catch((err) => cb(err, null));
      return;
    }
    request({ method: payload.method, params: payload.params }).then(
      (result) => cb(null, { id: payload.id, jsonrpc: '2.0', result }),
      (err) => cb(err, null),
    );
  }
  function isConnected() {
    return session.connected;
  }

  // ---- the provider object --------------------------------------------------
  const provider = {
    isVela: true,
    // MetaMask-compat flag on the LEGACY window.ethereum singleton. Many older (and
    // some lazy newer) dApps hard-gate on `window.ethereum.isMetaMask` and never
    // implement EIP-6963, so without it they show no wallet / refuse to connect.
    // Industry-standard for non-MetaMask wallets (Rabby, Coinbase, OKX, Trust, Brave
    // all set it). We spoof ONLY this legacy boolean: EIP-6963 still announces our
    // TRUE identity (name "Vela Wallet", rdns app.getvela — see `info` below), so
    // modern multi-wallet dApps discover Vela, not MetaMask, and we never collide
    // with real MetaMask's 6963 broadcast. `isVela` stays for dApps that want the
    // real wallet. (Supersedes the earlier "never spoof isMetaMask" stance — that
    // left legacy MM-only dApps unusable, which defeats the Safari-extension goal
    // of making nearly every iPhone-Safari dApp work.)
    isMetaMask: true,
    // Some dApps call `ethereum._metamask.isUnlocked()` once they see isMetaMask;
    // stub it so the call resolves instead of throwing on `undefined`. Vela unlocks
    // per-signature via passkey, so "unlocked" is the honest steady state.
    _metamask: Object.freeze({ isUnlocked: () => Promise.resolve(true) }),
    request,
    on,
    removeListener,
    addListener: on,
    once,
    // legacy:
    enable,
    send,
    sendAsync,
    isConnected,
  };
  // Synchronous legacy props as live getters (kept in sync with the cache).
  Object.defineProperties(provider, {
    selectedAddress: { get: () => session.accounts[0] ?? null, enumerable: true },
    chainId: { get: () => (session.chainIdNum ? toHexChainId(session.chainIdNum) : null), enumerable: true },
    networkVersion: { get: () => (session.chainIdNum ? String(session.chainIdNum) : null), enumerable: true },
    _vela: { get: () => ({ ...session }) }, // test/debug snapshot (non-enumerable)
  });

  // ---- publish on window.ethereum (defensively) -----------------------------
  // Set only if absent; keep configurable; contribute to providers[]. Never
  // clobber an existing wallet — 6963 is the primary discovery path.
  try {
    if (!window.ethereum) {
      Object.defineProperty(window, 'ethereum', { value: provider, configurable: true, writable: true });
    } else {
      // Respect a pre-existing provider but join its providers[] list if present.
      if (Array.isArray(window.ethereum.providers) && !window.ethereum.providers.includes(provider)) {
        window.ethereum.providers.push(provider);
      }
    }
  } catch {
    // Some pages define window.ethereum as a non-configurable getter — leave it.
  }
  // Resolves @metamask/detect-provider instantly instead of its 3s timeout.
  try {
    window.dispatchEvent(new Event('ethereum#initialized'));
  } catch {
    /* noop */
  }

  // ---- EIP-6963 announce (eager + on request) -------------------------------
  const info = Object.freeze({ uuid: SESSION_UUID, name: WALLET_NAME, icon: ICON, rdns: RDNS });
  function announce() {
    try {
      window.dispatchEvent(
        new CustomEvent('eip6963:announceProvider', { detail: Object.freeze({ info, provider }) }),
      );
    } catch {
      /* noop */
    }
  }
  // Register the request listener FIRST so a dApp that already asked gets us.
  window.addEventListener('eip6963:requestProvider', announce);
  announce();

  // ---- warm the cache (silent — no prompts) ---------------------------------
  // Populates chainId + (if already granted) accounts so synchronous props and
  // early dApp reads have real values. eth_accounts returns [] when ungranted.
  request({ method: 'eth_chainId' }).catch(() => {});
  request({ method: 'eth_accounts' }).catch(() => {});

  console.log('[Vela] EIP-1193/6963 provider installed', RDNS, SESSION_UUID);
})();
