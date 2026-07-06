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
//   - never spoof isMetaMask (rely on 6963); reject nothing here that the router
//     can answer — the router owns policy (eth_sign refusal, etc.)
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

  // Data-URI icon (scalable SVG, satisfies the "≥96²" 6963 guidance). Vela accent.
  const ICON =
    'data:image/svg+xml;base64,' +
    btoa(
      '<svg xmlns="http://www.w3.org/2000/svg" width="96" height="96" viewBox="0 0 96 96">' +
        '<rect width="96" height="96" rx="22" fill="#111"/>' +
        '<path d="M30 30 L48 66 L66 30" fill="none" stroke="#E8572A" stroke-width="9" ' +
        'stroke-linecap="round" stroke-linejoin="round"/></svg>',
    );

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
