// Vela Safari extension — shared protocol constants + pure helpers.
//
// Imported by inpage.js (MAIN world), content.js and background.js (ISOLATED /
// background). esbuild inlines this per-entry, so it must stay dependency-free
// and use ZERO extension/DOM APIs — pure functions + constants only, so the same
// logic can be unit-tested under `node --test` (test/protocol.test.mjs).
//
// The method classification MIRRORS the app's own split so an extension session
// behaves identically to a WalletPair one:
//   - isSigningMethod           → src/hooks/use-dapp-signing.ts:440
//   - INSTANT_READONLY_METHODS  → src/hooks/use-dapp-signing.ts:453
//   - BUNDLER_METHODS           → src/services/rpc-adapter.ts (BUNDLER_METHODS)
// Keep this file in sync if the app's classification changes.

// ---- postMessage channel (inpage MAIN world ↔ content ISOLATED world) --------
// Every message is tagged so we never collide with the page, another wallet's
// provider, or a nested-iframe provider.
export const CHANNEL = 'vela-1193';

// ---- EIP-6963 provider identity ---------------------------------------------
export const RDNS = 'app.getvela';
export const WALLET_NAME = 'Vela Wallet';

// ---- App Group / storage keys -----------------------------------------------
// Written by the app (<AccountFileWriter/>), read by the extension via the
// native `getAccount` handler. Shape: see AccountCache below.
export const ACCOUNT_FILE = 'vela.ext.account.json';
// Per-origin connect grant, stored in the extension's storage.local (Phase A;
// Phase B may also mirror to the App Group so the app's Connections surface can
// show + revoke them).
export const PERM_PREFIX = 'vela.perm.';
// Durable pending-sign mirror (rid → { rpcId, origin, method, state, ... }).
export const SIGN_PREFIX = 'vela.sign.';

// ---- sign hand-off launch (scheme vs. Universal Link) -----------------------
// The app is launched for a sign by navigating the CURRENT tab to a Vela URL.
//
//  - SCHEME  `velawallet://sign?rid=…`  — the R1-proven default. If the app is
//    absent the nav fails in place (a banner / "Cannot Open Page") and the dApp
//    tab + its pending promise SURVIVE, so the focus-poll can still deliver.
//  - UNIVERSAL LINK  `https://getvela.app/sign?rid=…` — one-tap, no "Open in
//    Vela?" banner, BUT if the AASA association fails (file not hosted, app not
//    installed, CDN stale) iOS does a REAL navigation to the web URL → the dApp
//    tab is REPLACED and the pending sign promise is LOST (a fund-safety
//    regression). So the UL cannot be chosen speculatively.
//
// The choice is ATTESTATION-DRIVEN, not a compile-time flag: the app sets
// `ulVerified` in the account cache the first time it is opened via a
// getvela.app UL (proving the association resolves on THIS device); the extension
// reads that flag and passes it to signLaunchUrl. Until the app has attested,
// `ulVerified` is false and we launch via the scheme — always safe. See the app's
// app-group-account-sync.ts + <AccountFileWriter/> and PHASE-3-RUNBOOK.md.
export const UNIVERSAL_LINK_HOST = 'getvela.app';
// The rid the popup's "test one-tap sign" probe uses to bootstrap attestation.
// The applinks AASA only matches /sign, so the probe rides that path; app/sign.tsx
// special-cases this rid (shows a confirmation, runs no sign).
export const UL_SELFTEST_RID = 'ul-selftest';

// Extension-side self-heal (storage.local). Even with the app's attestation, a UL
// can break AFTER attestation (e.g. iOS "Open in Safari" for getvela.app). When that
// happens the failed UL NAVIGATES the tab to getvela.app — where content.js also
// runs. UL_PENDING_KEY is stamped just before a UL launch; if content.js then loads
// on getvela.app with that stamp fresh, the UL failed → it sets UL_BROKEN_KEY, which
// vetoes the UL at every launch site (falls back to the safe scheme) until the popup
// probe clears it and re-verifies. This heals after a SINGLE failed sign, faster than
// the app-side TTL backstop.
export const UL_PENDING_KEY = 'vela.ul.pending';
export const UL_BROKEN_KEY = 'vela.ul.broken';

/**
 * The URL that launches the app to sign `rid`.
 * @param {string} rid
 * @param {boolean} [ulVerified] true only once the app has attested the getvela.app
 *   applinks association resolves on this device — then (and only then) use the UL.
 */
export function signLaunchUrl(rid, ulVerified) {
  const q = 'sign?rid=' + encodeURIComponent(rid);
  return ulVerified ? `https://${UNIVERSAL_LINK_HOST}/${q}` : `velawallet://${q}`;
}

/** The UL the toolbar popup opens (new tab) to bootstrap attestation. */
export function universalLinkSelfTestUrl() {
  return `https://${UNIVERSAL_LINK_HOST}/sign?rid=${UL_SELFTEST_RID}`;
}

// AccountCache (what the app writes to ACCOUNT_FILE):
//   {
//     address:  string,                      // active Safe address
//     name:     string,                      // active account display name
//     accounts: { name, address }[],         // all accounts
//     chainId:  number,                       // default/current chain (>=1)
//     chains:   { [chainId]: { name, rpcUrl, bundlerUrl } },
//     updatedAt: number,
//   }

// ---- EIP-1193 / EIP-1474 error codes ----------------------------------------
export const ERR = {
  USER_REJECTED: 4001, // explicit reject in the app / a sheet Cancel
  UNAUTHORIZED: 4100, // method needs a prior eth_requestAccounts grant
  UNSUPPORTED_METHOD: 4200, // e.g. eth_sign — refused by policy
  // §12.1.3: reserve a DISTINCT non-4001 code for timeout/unknown so a
  // stuck-but-submitted tx never looks like a clean decline (double-spend risk).
  UNKNOWN_PENDING: 4900, // "pending / unknown — check Vela Activity"
  CHAIN_NOT_ADDED: 4902, // wallet_switchEthereumChain to an unknown chain
  METHOD_NOT_FOUND: -32601,
  INVALID_PARAMS: -32602,
  INTERNAL: -32603,
};

export function rpcError(code, message, data) {
  const e = { code, message };
  if (data !== undefined) e.data = data;
  return e;
}

// ---- Method classification (mirrors the app) --------------------------------

// src/hooks/use-dapp-signing.ts:440 — the single predicate for "needs a passkey".
export function isSigningMethod(method) {
  return (
    method === 'eth_sendTransaction' ||
    method === 'wallet_sendCalls' ||
    method === 'personal_sign' ||
    method === 'eth_sign' ||
    method.includes('signTypedData')
  );
}

// src/hooks/use-dapp-signing.ts:453 — answered instantly from local state.
export const INSTANT_READONLY_METHODS = new Set([
  'eth_accounts',
  'eth_requestAccounts',
  'eth_chainId',
  'net_version',
  'wallet_getPermissions',
  'wallet_requestPermissions',
  'wallet_addEthereumChain',
]);

// src/services/rpc-adapter.ts — routed to the ERC-4337 bundler, not the node RPC.
export const BUNDLER_METHODS = new Set([
  'eth_sendUserOperation',
  'eth_estimateUserOperationGas',
  'eth_getUserOperationReceipt',
  'eth_getUserOperationByHash',
  'pimlico_getUserOperationGasPrice',
]);

// The node read methods the app advertises (src/services/walletpair-transport.ts
// READ_ONLY_RPC_METHODS) — the authoritative allowlist we forward to the RPC.
export const READ_ONLY_RPC_METHODS = [
  'eth_call', 'eth_estimateGas', 'eth_getBalance', 'eth_getCode',
  'eth_getStorageAt', 'eth_getTransactionCount', 'eth_getTransactionByHash',
  'eth_getTransactionReceipt', 'eth_getLogs', 'eth_blockNumber',
  'eth_getBlockByNumber', 'eth_getBlockByHash', 'eth_feeHistory',
  'eth_gasPrice', 'eth_maxPriorityFeePerGas', 'eth_newFilter',
  'eth_newBlockFilter', 'eth_getFilterChanges', 'eth_uninstallFilter',
  'eth_sendRawTransaction', 'eth_syncing',
];

// The COMPLETE set of methods the background may proxy to a public RPC. An
// ALLOWLIST (not a denylist) — a method outside it is rejected, never forwarded.
// Denylist routing would fail-open: e.g. eth_signTransaction is NOT caught by
// isSigningMethod, so a catch-all 'read' would proxy it to a public node and let
// any site use the extension as an open RPC relay. Includes the app's node reads,
// the bundler reads, and a few extra reads viem/ethers commonly need.
export const READ_PROXY_METHODS = new Set([
  ...READ_ONLY_RPC_METHODS,
  ...BUNDLER_METHODS,
  'eth_getBlockReceipts', 'eth_getProof', 'eth_createAccessList',
  'eth_getFilterLogs', 'eth_getTransactionByBlockHashAndIndex',
  'eth_getTransactionByBlockNumberAndIndex', 'eth_getBlockTransactionCountByHash',
  'eth_getBlockTransactionCountByNumber', 'web3_clientVersion',
]);

// Coarse routing bucket used by content.js (who answers) and background.js.
//   'unsupported' → refuse (policy, e.g. eth_sign)
//   'sign'        → app hop (personal_sign / typed-data / send / sendCalls)
//   'connect'     → eth_requestAccounts / wallet_requestPermissions (connect sheet)
//   'state'       → local state answer (accounts/chainId/permissions), no network
//   'switch'      → wallet_switchEthereumChain (local, emits chainChanged)
//   'addChain'    → wallet_addEthereumChain (local no-op ack)
//   'read'        → proxy to node RPC or bundler
export function classifyMethod(method) {
  if (method === 'eth_sign') return 'unsupported'; // §12.4 security: refuse outright
  if (isSigningMethod(method)) return 'sign';
  if (method === 'eth_requestAccounts' || method === 'wallet_requestPermissions') return 'connect';
  if (method === 'eth_accounts' || method === 'eth_chainId' || method === 'net_version' || method === 'wallet_getPermissions') return 'state';
  if (method === 'wallet_switchEthereumChain') return 'switch';
  if (method === 'wallet_addEthereumChain') return 'addChain';
  if (method === 'wallet_watchAsset') return 'addChain'; // ack true, no state change
  // ALLOWLIST the read proxy — anything not explicitly a known read is refused,
  // so we never forward eth_signTransaction / arbitrary methods to a public RPC.
  if (READ_PROXY_METHODS.has(method)) return 'read';
  return 'unsupported';
}

// ---- Param / value helpers --------------------------------------------------

const ADDR_RE = /^0x[0-9a-fA-F]{40}$/;
export function isAddressLike(v) {
  return typeof v === 'string' && ADDR_RE.test(v);
}

// EIP-1193 §12.4: minimal lowercase hex, e.g. 1 → "0x1".
export function toHexChainId(n) {
  const num = typeof n === 'string' ? parseInt(n, n.startsWith('0x') ? 16 : 10) : n;
  if (!Number.isFinite(num) || num <= 0) return '0x1';
  return '0x' + Math.floor(num).toString(16);
}

// Accepts number | "0x.." | decimal-string → number (NaN-safe → 0).
export function parseChainId(v) {
  if (typeof v === 'number') return Math.floor(v);
  if (typeof v === 'string') {
    const n = v.startsWith('0x') || v.startsWith('0X') ? parseInt(v, 16) : parseInt(v, 10);
    return Number.isFinite(n) ? n : 0;
  }
  return 0;
}

// personal_sign = [message, address]; typed-data = [address, typedData].
// Detect the address by SHAPE (0x+40hex), not position — the #1 hand-rolled
// provider bug (§12.4). Returns the address or null (display / validation only;
// the sign-req still forwards params verbatim to the app).
export function pickSignAddress(method, params) {
  if (!Array.isArray(params)) return null;
  for (const p of params) if (isAddressLike(p)) return p.toLowerCase();
  return null;
}

// Which endpoint answers a read for a given chain entry ({rpcUrl,bundlerUrl}).
export function rpcTargetFor(method, chain) {
  if (!chain) return null;
  if (BUNDLER_METHODS.has(method)) return chain.bundlerUrl ? { url: chain.bundlerUrl, kind: 'bundler' } : null;
  return chain.rpcUrl ? { url: chain.rpcUrl, kind: 'node' } : null;
}

// Short, human origin label for the sheets ("biubiu.tools" from a full origin).
export function hostLabel(origin) {
  try {
    return new URL(origin).host || origin;
  } catch {
    return String(origin || '');
  }
}
