// Vela Safari extension — background (MV3 non-persistent, evictable).
//
// Phase A router. Answers everything that does NOT need a passkey, entirely in
// Safari (zero app hop):
//   - STATE  (eth_accounts / eth_chainId / net_version / wallet_getPermissions)
//            → local perms (storage.local) + the app-written account cache
//   - CONNECT (eth_requestAccounts / wallet_requestPermissions) → grant check;
//            content renders the consent sheet when no grant exists
//   - SWITCH / ADD-CHAIN → update the per-origin chain, ack
//   - READ   (eth_call / eth_getBalance / … / bundler methods) → proxy to the
//            same endpoints the app uses (chains map from the account cache)
// SIGN is NOT handled here — content.js drives the proven launch (velawallet://
// sign?rid) + focus-poll return; background only relays writeSignRequest /
// pollSignResult to native (unchanged from the R1 spike).
//
// Everything native is a SHORT atomic sendNativeMessage round-trip (FACT-3: the
// worker is evictable; never long-poll, never hold state we can't rebuild).
/* global browser, fetch, AbortController */
import {
  ERR,
  rpcError,
  classifyMethod,
  toHexChainId,
  parseChainId,
  rpcTargetFor,
  PERM_PREFIX,
} from './lib/protocol.js';

// The single native handler (App Group + sign mailbox). On iOS Safari there is
// exactly one, so the id is effectively ignored — but an argument is required.
const NATIVE_APP_ID = 'app.getvela.VelaWallet';

// ---- native round-trips -----------------------------------------------------

async function native(message) {
  return browser.runtime.sendNativeMessage(NATIVE_APP_ID, message);
}

// Short-TTL in-memory memo of the app-written account cache so a burst of reads
// doesn't hit native once per call. Lost on eviction (then re-fetched) — fine.
let _acct = null;
let _acctAt = 0;
const ACCT_TTL = 4000;

async function getAccountCache({ fresh = false } = {}) {
  const now = Date.now();
  if (!fresh && _acct && now - _acctAt < ACCT_TTL) return _acct;
  try {
    const resp = await native({ type: 'getAccount' });
    if (resp && resp.type === 'account' && resp.found && resp.account) {
      _acct = resp.account;
      _acctAt = now;
      return _acct;
    }
  } catch (e) {
    console.log('[Vela bg] getAccount failed', String(e));
  }
  return null;
}

// ---- per-origin grants (storage.local) --------------------------------------

const permKey = (origin) => PERM_PREFIX + origin;

async function getPerm(origin) {
  const all = await browser.storage.local.get(permKey(origin));
  return all[permKey(origin)] || null;
}
async function setPerm(origin, patch) {
  const cur = (await getPerm(origin)) || { origin };
  const next = { ...cur, ...patch };
  await browser.storage.local.set({ [permKey(origin)]: next });
  return next;
}
async function removePerm(origin) {
  await browser.storage.local.remove(permKey(origin));
}

// The chainId this origin should see: its own grant/switch pref, else the app's
// current default, else Ethereum.
function originChainId(perm, cache) {
  return (perm && perm.chainId) || (cache && cache.chainId) || 1;
}

// The still-valid granted address, or null. Re-validates the stored grant
// against the current account cache so a dApp never sees an address the user has
// since removed. §12.4: only drop the grant when the cache is PRESENT and no
// longer lists it — NEVER on a cold-cache miss (that would log the user out of
// every open dApp).
function validGrantedAddress(perm, cache) {
  if (!perm || !perm.address) return null;
  if (cache && Array.isArray(cache.accounts) && cache.accounts.length) {
    const owned = cache.accounts.map((a) => (a.address || '').toLowerCase());
    if (!owned.includes(perm.address.toLowerCase())) return null;
  }
  return perm.address;
}

// ---- read proxy -------------------------------------------------------------

async function proxyRpc(method, params, chainId, cache) {
  const chains = (cache && cache.chains) || {};
  const chain = chains[String(chainId)] || chains[chainId];
  const target = rpcTargetFor(method, chain);
  if (!target) {
    return { ok: false, error: rpcError(ERR.INTERNAL, `No RPC endpoint for chain ${chainId} / ${method}`) };
  }
  // Idempotent reads: one bounded retry on a network blip.
  let lastErr;
  for (let attempt = 0; attempt < 2; attempt++) {
    // Keep the abort timer armed across BOTH the fetch and the body read —
    // fetch() resolves on headers, so a stalled response body would otherwise
    // hang forever (the timer must cover res.json(), not just the headers).
    const ctrl = new AbortController();
    const timer = setTimeout(() => ctrl.abort(), 8000);
    try {
      const res = await fetch(target.url, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params: params ?? [] }),
        signal: ctrl.signal,
      });
      if (!res.ok) {
        lastErr = rpcError(ERR.INTERNAL, `RPC HTTP ${res.status}`);
        continue;
      }
      const body = await res.json();
      if (body && body.error) {
        // A JSON-RPC error is authoritative — forward it verbatim, do not retry.
        return { ok: false, error: rpcError(body.error.code ?? ERR.INTERNAL, body.error.message || 'RPC error', body.error.data) };
      }
      return { ok: true, result: body ? body.result ?? null : null };
    } catch (e) {
      lastErr = rpcError(ERR.INTERNAL, `RPC request failed: ${String(e && e.message ? e.message : e)}`);
    } finally {
      clearTimeout(timer);
    }
  }
  return { ok: false, error: lastErr || rpcError(ERR.INTERNAL, 'RPC request failed') };
}

// ---- the rpc router (state / switch / add-chain / read) ---------------------

async function handleRpc(method, params, origin) {
  const cls = classifyMethod(method);

  if (cls === 'unsupported') {
    return { ok: false, error: rpcError(ERR.UNSUPPORTED_METHOD, `${method} is not supported by Vela`) };
  }

  if (cls === 'state') {
    const perm = await getPerm(origin);
    const cache = await getAccountCache();
    const addr = validGrantedAddress(perm, cache); // re-validated vs the cache
    const cid = originChainId(perm, cache);
    switch (method) {
      case 'eth_accounts':
        // §12.4: NEVER prompt / error — [] when ungranted, locked, or the granted
        // account no longer exists.
        return { ok: true, result: addr ? [addr] : [] };
      case 'eth_chainId':
        return { ok: true, result: toHexChainId(cid) };
      case 'net_version':
        return { ok: true, result: String(cid) };
      case 'wallet_getPermissions':
        return {
          ok: true,
          result: addr
            ? [{ parentCapability: 'eth_accounts', caveats: [{ type: 'restrictReturnedAccounts', value: [addr] }] }]
            : [],
        };
      default:
        return { ok: true, result: null };
    }
  }

  if (cls === 'switch') {
    const target = parseChainId(Array.isArray(params) && params[0] ? params[0].chainId : undefined);
    if (!target) return { ok: false, error: rpcError(ERR.INVALID_PARAMS, 'wallet_switchEthereumChain requires { chainId }') };
    const cache = await getAccountCache();
    const chains = (cache && cache.chains) || {};
    if (!chains[String(target)] && !chains[target]) {
      return { ok: false, error: rpcError(ERR.CHAIN_NOT_ADDED, `Unrecognized chain ${target}. Add it in Vela first.`) };
    }
    await setPerm(origin, { chainId: target });
    // content emits chainChanged synchronously once this resolves.
    return { ok: true, result: null, chainId: target };
  }

  if (cls === 'addChain') {
    if (method === 'wallet_watchAsset') return { ok: true, result: true };
    // wallet_addEthereumChain: Vela's chain set is app-managed. Ack if known,
    // else tell the dApp it's unavailable (can't proxy reads for it).
    const target = parseChainId(Array.isArray(params) && params[0] ? params[0].chainId : undefined);
    const cache = await getAccountCache();
    const chains = (cache && cache.chains) || {};
    if (target && (chains[String(target)] || chains[target])) return { ok: true, result: null };
    return { ok: false, error: rpcError(ERR.CHAIN_NOT_ADDED, `Chain ${target || '?'} is not available in Vela`) };
  }

  // cls === 'read' (or a connect/sign that leaked here — reads are the default)
  if (cls === 'read') {
    const perm = await getPerm(origin);
    const cache = await getAccountCache();
    const cid = originChainId(perm, cache);
    return proxyRpc(method, params, cid, cache);
  }

  return { ok: false, error: rpcError(ERR.METHOD_NOT_FOUND, `${method} cannot be handled here`) };
}

// ---- connect (grant lifecycle) ----------------------------------------------

async function handleConnect(origin) {
  const perm = await getPerm(origin);
  const cache = await getAccountCache({ fresh: true }); // §12.1.6: fresh at connect
  const addr = validGrantedAddress(perm, cache);
  if (addr) {
    // ulVerified / ulVerifiedAt ride the connect response so content.js knows, at
    // sign time, whether the UL launch is safe on this device (app-attested) and can
    // compare the attestation age against its self-heal veto. No extra hop.
    // theme / locale ride it too so the sign sheet renders in the app's exact
    // color scheme + language (the connect flow is content.js's theme source).
    return { ok: true, granted: true, result: [addr], chainId: originChainId(perm, cache), ulVerified: !!(cache && cache.ulVerified), ulVerifiedAt: (cache && cache.ulVerifiedAt) || 0, theme: (cache && cache.theme) || 'auto', locale: (cache && cache.locale) || '' };
  }
  if (!cache || !cache.address) {
    return { ok: false, noAccount: true }; // content: "打开 Vela 登录"
  }
  return {
    ok: false,
    needsConsent: true,
    // theme/locale at top level so the connect sheet matches the app's scheme + language.
    theme: cache.theme || 'auto',
    locale: cache.locale || '',
    account: {
      address: cache.address,
      name: cache.name,
      accounts: cache.accounts || [{ name: cache.name, address: cache.address }],
      chainId: originChainId(perm, cache),
      chains: cache.chains || {},
    },
  };
}

async function handleGrant(origin, address, chainId) {
  // Defense-in-depth: only grant an address the app actually owns + a known chain.
  const cache = await getAccountCache({ fresh: true });
  const owned = (cache && cache.accounts ? cache.accounts.map((a) => (a.address || '').toLowerCase()) : []);
  if (!cache || !cache.address) return { ok: false, error: rpcError(ERR.INTERNAL, 'No Vela account available') };
  const addr = (address || '').toLowerCase();
  if (owned.length && !owned.includes(addr)) {
    return { ok: false, error: rpcError(ERR.INVALID_PARAMS, 'Address is not a Vela account') };
  }
  const chains = cache.chains || {};
  const cid = parseChainId(chainId) || cache.chainId || 1;
  if (Object.keys(chains).length && !chains[String(cid)] && !chains[cid]) {
    return { ok: false, error: rpcError(ERR.INVALID_PARAMS, 'Unknown chain') };
  }
  const canonical = owned.length ? cache.accounts.find((a) => (a.address || '').toLowerCase() === addr).address : cache.address;
  await setPerm(origin, { address: canonical, chainId: cid, grantedAt: Date.now() });
  return { ok: true, result: [canonical], chainId: cid };
}

// ---- popup status (read-only; no grant side effects) ------------------------

// Read-only snapshot for the toolbar popup: the active Vela account (from the
// app-written cache) + whether THIS origin is connected + the chain it sees.
// Never writes a grant / cache — purely reflects current state.
async function handleStatus(origin) {
  const cache = await getAccountCache();
  const perm = origin ? await getPerm(origin) : null;
  const addr = validGrantedAddress(perm, cache);
  return {
    ok: true,
    origin: origin || null,
    account: cache
      ? { address: cache.address, name: cache.name, chainId: cache.chainId, chains: cache.chains || {} }
      : null,
    granted: !!addr,
    grantedAddress: addr,
    chainId: originChainId(perm, cache),
    ulVerified: !!(cache && cache.ulVerified),
    ulVerifiedAt: (cache && cache.ulVerifiedAt) || 0,
    theme: (cache && cache.theme) || 'auto',
    locale: (cache && cache.locale) || '',
  };
}

// ---- message dispatch (from content.js) -------------------------------------

browser.runtime.onMessage.addListener((message, _sender, sendResponse) => {
  const type = message && message.type;

  // --- frozen R1 sign relays (unchanged) ---
  if (type === 'writeSignRequest') {
    // content has already navigated to velawallet://sign?rid and is NOT awaiting
    // this. Return true so Safari holds the worker until the native write lands.
    native(message).then(
      (r) => console.log('[Vela bg] sign write <- native', r),
      (err) => console.log('[Vela bg] sign write error', String(err)),
    );
    return true;
  }
  if (type === 'pollSignResult') {
    native(message).then(
      (nativeResponse) => sendResponse(nativeResponse),
      (err) => sendResponse({ type: 'error', error: String(err) }),
    );
    return true;
  }

  // --- Phase A provider routes ---
  if (type === 'rpc') {
    handleRpc(message.method, message.params, message.origin).then(sendResponse, (err) =>
      sendResponse({ ok: false, error: rpcError(ERR.INTERNAL, String(err && err.message ? err.message : err)) }),
    );
    return true;
  }
  if (type === 'connect') {
    handleConnect(message.origin).then(sendResponse, (err) =>
      sendResponse({ ok: false, error: rpcError(ERR.INTERNAL, String(err)) }),
    );
    return true;
  }
  if (type === 'grantConnect') {
    handleGrant(message.origin, message.address, message.chainId).then(sendResponse, (err) =>
      sendResponse({ ok: false, error: rpcError(ERR.INTERNAL, String(err)) }),
    );
    return true;
  }
  if (type === 'revoke') {
    removePerm(message.origin).then(
      () => sendResponse({ ok: true }),
      (err) => sendResponse({ ok: false, error: rpcError(ERR.INTERNAL, String(err)) }),
    );
    return true;
  }
  if (type === 'status') {
    handleStatus(message.origin).then(sendResponse, (err) =>
      sendResponse({ ok: false, error: rpcError(ERR.INTERNAL, String(err)) }),
    );
    return true;
  }

  // Unknown → soft error (keeps the channel well-defined).
  sendResponse({ ok: false, error: rpcError(ERR.METHOD_NOT_FOUND, `Unknown message ${type}`) });
  return false;
});
