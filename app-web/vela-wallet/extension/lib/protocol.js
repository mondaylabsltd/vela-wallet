/**
 * Shared protocol: constants and pure helpers (spec 027 T320).
 *
 * Ported from packages/safari-extension/src/lib/protocol.js @ 52ad8fa9.
 *
 * Imported by inpage.js (the page's MAIN world), content.js (isolated world)
 * and background.js (the service worker). It must stay dependency-free and use
 * ZERO extension or DOM APIs — pure functions and constants only, so the same
 * logic is unit-testable outside a browser.
 *
 * What did NOT come across from Safari: the App Group account file, the
 * `velawallet://sign` launch URLs and the Universal-Link attestation dance.
 * All three exist because the Safari extension has to hand a signature to a
 * NATIVE app. Here the wallet is the extension, so a request goes to a window
 * this extension owns and there is nothing to launch.
 *
 * The method classification MIRRORS the app's own split, so an extension
 * session behaves identically to any other:
 *   - isSigningMethod           → src/hooks/use-dapp-signing.ts:440
 *   - INSTANT_READONLY_METHODS  → src/hooks/use-dapp-signing.ts:453
 *   - BUNDLER_METHODS           → src/services/rpc-adapter.ts
 * Keep this file in sync if the app's classification changes.
 */

// ---- postMessage channel (MAIN world ↔ isolated world) ----------------------
// Every message is tagged so it never collides with the page, another wallet's
// provider, or a nested-iframe provider.
export const CHANNEL = 'vela-1193';

// ---- EIP-6963 provider identity ---------------------------------------------
export const RDNS = 'app.getvela';
export const WALLET_NAME = 'Vela Wallet';

// ---- storage keys (the extension's own storage.local) -----------------------
/** Per-origin connect grant. The CORE decides what a grant contains; this is
 *  only where the answer is kept. */
export const PERM_PREFIX = 'vela.perm.';
/**
 * A request in flight — `vela.req.<tabId>:<pageRequestId>` in
 * `storage.SESSION` (spec 082 RB1): written the moment it arrives, so an
 * evicted service worker comes back knowing what it owes; cleared with the
 * browser, and unreadable by content scripts. It leaves only when answered or
 * settled. (Before 082 these lived in storage.local and a start sweep deleted
 * them all — which is how a restarted worker lost a live request, G19.)
 */
export const REQUEST_PREFIX = 'vela.req.';

// ---- the request lifecycle (spec 082 RB1–RB11, contract §14) ----------------

/** content.js ↔ worker: open only while the page owes a sign/connect answer. */
export const DOC_PORT = 'vela.doc';
/** panel or request window ↔ worker: the surface a request is answered on. */
export const SURFACE_PORT = 'vela.surface';
/**
 * How long a request may wait for a decision — the core's
 * `EXTENSION_REQUEST_TTL_MS` (`instant.test.ts` pins it to `signRequestTtlMs()`).
 * The worker refuses an approve/sign claim at this age.
 */
export const REQUEST_TTL_MS = 300_000;
/** content.js gives up this long AFTER the worker's deadline, never before it. */
export const CONTENT_GRACE_MS = 5_000;
/** A claim with no answer in this long (after one reconnect) is not live. */
export const CLAIM_TIMEOUT_MS = 5_000;
/** The surface's heartbeat while it owes an answer — keeps the worker awake. */
export const SURFACE_PING_MS = 20_000;
/** A side panel that neither opened nor failed in this long is given up on. */
export const PANEL_OPEN_WAIT_MS = 2_000;
/** content.js's reconnect backoff after its port to the worker closed. */
export const RECONNECT_BACKOFF_MS = [0, 250, 1_000, 3_000];

/**
 * What the page is told when a request ends without a decision (RB6). ALWAYS
 * 4900 — "it may have happened" — pinned to the core's
 * `popupCloseSettlement().code`, never 4001 (a dApp reads 4001 as "nothing
 * happened" and sends again). These go to the dApp's code, not to a screen,
 * so they are plain English and not corpus strings; Chrome's own error text
 * is never passed on.
 */
export const SETTLE = {
	page_left: { code: 4900, message: 'The page navigated away' },
	surface_closed: { code: 4900, message: 'The browser closed before the request finished' },
	expired: { code: 4900, message: 'Vela did not answer in time — check its activity' },
	restarted: {
		code: 4900,
		message: 'Vela restarted before the request finished — check its activity'
	},
	updated: { code: 4900, message: 'Vela was updated — reload this page and try again' }
};

/** A read whose channel dropped twice: plain words, never Chrome's (G19). */
export const READ_DROPPED_MESSAGE = 'Vela could not finish this read — try again';

/**
 * Reads that SEND something. A dropped channel on one of these may have
 * delivered it, so it is never retried — it is answered `restarted` (4900).
 */
export const SENDING_READS = new Set(['eth_sendRawTransaction', 'eth_sendUserOperation']);

/**
 * What content.js does when its channel to the worker dropped under a request
 * (data-model §4) — never echoing Chrome's `error.message`:
 *
 *   - a read: retried once, then `-32603 "Vela could not finish this read"`;
 *   - a read that sends (`eth_sendRawTransaction` / `eth_sendUserOperation`):
 *     never retried — `4900 restarted`, it may have gone out;
 *   - a sign/connect: retried once (the reconnect wakes a new worker, which
 *     still holds the record), then `4900 restarted`.
 *
 * `attempt` counts the drops already seen for this request (0 = the first).
 * Returns `{ retry: true }` or `{ error }`.
 */
export function droppedChannelAnswer(bucket, method, attempt) {
	if (bucket === 'sign' || bucket === 'connect') {
		return attempt < 1 ? { retry: true } : { error: settleError('restarted') };
	}
	if (SENDING_READS.has(method)) return { error: settleError('restarted') };
	return attempt < 1 ? { retry: true } : { error: rpcError(ERR.INTERNAL, READ_DROPPED_MESSAGE) };
}

/** The error a settlement cause answers the page with. */
export function settleError(cause) {
	const settle = SETTLE[cause] ?? SETTLE.surface_closed;
	return rpcError(settle.code, settle.message);
}

// ---- the worker's chain reads (spec 082 RF2) --------------------------------

/**
 * One endpoint's read budget — the core's `RPC_READ_TIMEOUT_MS` (8 s), pinned
 * by `protocol.test.ts` to `rpcReadTimeoutMs()`. It was 20 s, so a dead Gnosis
 * cost 3 × 20 s on EVERY read (G20).
 */
export const READ_TIMEOUT_MS = 8_000;
/** Where the worker remembers endpoints that just failed (storage.session). */
export const ENDPOINTS_KEY = 'vela.ext.endpoints';
const COOLDOWN_BASE_MS = 30_000;
const COOLDOWN_CAP_MS = 300_000;

/**
 * How long an endpoint is skipped after `failures` in a row — the core's
 * `rpc_pool::cooldown_ms`: 30 s · 2^(n−1), capped at 300 s; 0 for none.
 * Pinned to `rpcCooldownMs(n)` for n = 1..5.
 */
export function cooldownMs(failures) {
	if (!Number.isFinite(failures) || failures <= 0) return 0;
	return Math.min(COOLDOWN_CAP_MS, COOLDOWN_BASE_MS * 2 ** (failures - 1));
}

/**
 * The order to try `endpoints` in: those not cooling down first, in their own
 * order; the cooled ones last (still tried — a node that recovered must be
 * reachable), soonest-back first. `health` is the stored
 * `{<url>: {failures, until}}`.
 */
export function orderEndpoints(endpoints, health, now) {
	const cooling = (url) => {
		const entry = health && typeof health === 'object' ? health[url] : null;
		return entry && typeof entry.until === 'number' && entry.until > now ? entry.until : 0;
	};
	const live = endpoints.filter((url) => cooling(url) === 0);
	const cooled = endpoints
		.filter((url) => cooling(url) > 0)
		.sort((a, b) => cooling(a) - cooling(b));
	return [...live, ...cooled];
}

/** `health` after one endpoint failed: one more failure, the next cool-down. */
export function endpointFailed(health, url, now) {
	const prior = health && typeof health === 'object' ? health[url] : null;
	const failures = (prior && Number.isFinite(prior.failures) ? prior.failures : 0) + 1;
	return { ...(health ?? {}), [url]: { failures, until: now + cooldownMs(failures) } };
}

/** `health` after one endpoint answered: its entry is gone. */
export function endpointAnswered(health, url) {
	if (!health || typeof health !== 'object' || !(url in health)) return health ?? {};
	const next = { ...health };
	delete next[url];
	return next;
}

/** A read that took longer than this is logged `read.slow` (it still answered). */
export const READ_SLOW_MS = 3_000;

/**
 * The final words when no node answered a read for `chainId` (G33): the
 * chain by name, never the engine's text ("Failed to fetch", "signal timed
 * out"), which says nothing to a dApp's user.
 */
export function unreachableChainMessage(chainName, chainId) {
	const name = typeof chainName === 'string' && chainName.trim() ? chainName.trim() : 'unknown';
	return `Vela could not reach a node for chain ${name} (${chainId})`;
}

/** The catalog's name for `chainId`, or `null`. */
export function chainNameOf(catalog, chainId) {
	const chains = catalog && typeof catalog === 'object' ? catalog.chains : null;
	const entry = chains && typeof chains === 'object' ? chains[String(chainId)] : null;
	return entry && typeof entry.name === 'string' && entry.name ? entry.name : null;
}

/**
 * How a failed endpoint read is named in the log: `timeout` (the 8 s budget
 * ran out), `http_<status>`, or `network` (anything the engine threw).
 */
export function readFailureKind(error, status) {
	if (typeof status === 'number') return `http_${status}`;
	const name = error && typeof error === 'object' ? error.name : '';
	return name === 'TimeoutError' || name === 'AbortError' ? 'timeout' : 'network';
}

/**
 * The chain an origin is on — `vela.chain.<origin>` → chain id (number).
 *
 * The wallet has no global "current network" (ext_cache invariant ⑤): each
 * site picks and switches its own chain with `wallet_switchEthereumChain`, and
 * this is where that pick is kept. Absent → the snapshot's default. A grant's
 * own `chainId` is the chain the site was CONNECTED on — an audit fact the
 * core wrote — and is never rewritten by a switch.
 */
export const CHAIN_PREFIX = 'vela.chain.';
/**
 * The network catalog the wallet publishes — `vela.ext.chains`. The service
 * worker cannot run the core, and the catalog (built-in chains plus the custom
 * networks a person added in Settings) is what says which chain ids exist and
 * where their nodes and bundlers are. Absent → no read can be answered, and a
 * switch to any chain is 4902.
 */
export const CHAINS_KEY = 'vela.ext.chains';
/**
 * Where a request is answered — `vela.ext.surface`: `'panel'` (the default:
 * the asking tab's side panel, falling back to a window when there is no user
 * gesture to open one on) or `'window'` (always the dedicated window). A
 * preference the wallet may write; the e2e writes it because Playwright can
 * drive a window and cannot drive a side panel.
 */
export const SURFACE_KEY = 'vela.ext.surface';

// ---- EIP-1193 / EIP-1474 error codes ----------------------------------------
export const ERR = {
	USER_REJECTED: 4001, // an explicit reject — a window closed without deciding is 4900 (CLOSED_WITHOUT_ANSWER)
	UNAUTHORIZED: 4100, // method needs a prior eth_requestAccounts grant
	UNSUPPORTED_METHOD: 4200, // e.g. eth_sign — refused by policy
	/** A distinct non-4001 code for timeout/unknown, so a stuck-but-submitted
	 *  transaction never looks like a clean decline (a double-spend risk). */
	UNKNOWN_PENDING: 4900,
	CHAIN_NOT_ADDED: 4902,
	METHOD_NOT_FOUND: -32601,
	INVALID_PARAMS: -32602,
	INTERNAL: -32603
};

export function rpcError(code, message, data) {
	const e = { code, message };
	if (data !== undefined) e.data = data;
	return e;
}

// ---- method classification (mirrors the app) --------------------------------

/** The single predicate for "this needs a passkey". */
export function isSigningMethod(method) {
	return (
		method === 'eth_sendTransaction' ||
		method === 'wallet_sendCalls' ||
		method === 'personal_sign' ||
		method === 'eth_sign' ||
		method.includes('signTypedData')
	);
}

/** Answered from local state, with no network. */
export const INSTANT_READONLY_METHODS = new Set([
	'eth_accounts',
	'eth_requestAccounts',
	'eth_chainId',
	'net_version',
	'wallet_getPermissions',
	'wallet_requestPermissions',
	'wallet_addEthereumChain'
]);

/** Routed to the ERC-4337 bundler, not the node RPC. */
export const BUNDLER_METHODS = new Set([
	'eth_sendUserOperation',
	'eth_estimateUserOperationGas',
	'eth_getUserOperationReceipt',
	'eth_getUserOperationByHash',
	'pimlico_getUserOperationGasPrice'
]);

/** The node reads the app itself advertises. */
export const READ_ONLY_RPC_METHODS = [
	'eth_call',
	'eth_estimateGas',
	'eth_getBalance',
	'eth_getCode',
	'eth_getStorageAt',
	'eth_getTransactionCount',
	'eth_getTransactionByHash',
	'eth_getTransactionReceipt',
	'eth_getLogs',
	'eth_blockNumber',
	'eth_getBlockByNumber',
	'eth_getBlockByHash',
	'eth_feeHistory',
	'eth_gasPrice',
	'eth_maxPriorityFeePerGas',
	'eth_newFilter',
	'eth_newBlockFilter',
	'eth_getFilterChanges',
	'eth_uninstallFilter',
	'eth_sendRawTransaction',
	'eth_syncing'
];

/**
 * The COMPLETE set of methods that may ever be proxied to a node.
 *
 * An ALLOWLIST, not a denylist: a method outside it is refused, never
 * forwarded. Denylist routing fails OPEN — `eth_signTransaction`, for one, is
 * not caught by `isSigningMethod`, so a catch-all "read" bucket would proxy it
 * to a public node and turn the extension into an open RPC relay for any site.
 */
export const READ_PROXY_METHODS = new Set([
	...READ_ONLY_RPC_METHODS,
	...BUNDLER_METHODS,
	'eth_getBlockReceipts',
	'eth_getProof',
	'eth_createAccessList',
	'eth_getFilterLogs',
	'eth_getTransactionByBlockHashAndIndex',
	'eth_getTransactionByBlockNumberAndIndex',
	'eth_getBlockTransactionCountByHash',
	'eth_getBlockTransactionCountByNumber',
	'web3_clientVersion'
]);

/**
 * The routing bucket: who answers.
 *   'unsupported' → refuse (policy, e.g. eth_sign)
 *   'sign'        → the signing sheet, in a window this extension owns
 *   'connect'     → eth_requestAccounts / wallet_requestPermissions
 *   'state'       → the wallet's own answer about accounts/chain/permissions
 *   'revoke'      → wallet_revokePermissions (the site disconnects itself)
 *   'switch'      → wallet_switchEthereumChain
 *   'addChain'    → wallet_addEthereumChain (a switch, for a chain the wallet has)
 *   'watchAsset'  → wallet_watchAsset (`false`: not added)
 *   'read'        → a node or bundler read
 *
 * A MIRROR of the core's `dapp_rpc::classify` (spec 070), which the in-app
 * browsers on desktop, iOS and Android route through. The worker cannot run
 * the core on every page load, so `protocol.test.ts` replays every method name
 * either side knows through the real core and demands the same bucket.
 */
export function classifyMethod(method) {
	if (method === 'eth_sign') return 'unsupported'; // refused outright
	if (isSigningMethod(method)) return 'sign';
	if (method === 'eth_requestAccounts' || method === 'wallet_requestPermissions') return 'connect';
	if (
		method === 'eth_accounts' ||
		method === 'eth_coinbase' ||
		method === 'eth_chainId' ||
		method === 'net_version' ||
		method === 'wallet_getPermissions'
	)
		return 'state';
	if (method === 'wallet_revokePermissions') return 'revoke';
	if (method === 'wallet_switchEthereumChain') return 'switch';
	if (method === 'wallet_addEthereumChain') return 'addChain';
	if (method === 'wallet_watchAsset') return 'watchAsset';
	if (READ_PROXY_METHODS.has(method)) return 'read';
	return 'unsupported';
}

// ---- chain switching and reads (routed in the worker) ----------------------

/**
 * The chain a `wallet_switchEthereumChain` / `wallet_addEthereumChain` names.
 * Both take `[{ chainId: "0x…" }]` (EIP-3326 / EIP-3085). `0` = none named.
 */
export function switchChainParam(params) {
	const first = Array.isArray(params) ? params[0] : null;
	if (!first || typeof first !== 'object') return 0;
	return parseChainId(first.chainId);
}

/** The origin of a tab's URL, or `null` for anything that is not a web page. */
export function originOfUrl(url) {
	if (typeof url !== 'string') return null;
	try {
		const parsed = new URL(url);
		if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') return null;
		return parsed.origin;
	} catch {
		return null;
	}
}

/**
 * Where a read for `chainId` goes, from the catalog the wallet published.
 *
 * Node reads try the chain's RPC list in order; bundler methods go to the
 * chain's bundler alone. An empty list means the wallet knows no such chain,
 * which is 4902 to the page — never a guess at a public node.
 */
export function chainEndpoints(catalog, chainId, bundler) {
	const chains = catalog && typeof catalog === 'object' ? catalog.chains : null;
	const entry = chains && typeof chains === 'object' ? chains[String(chainId)] : null;
	if (!entry || typeof entry !== 'object') return [];
	if (bundler) return typeof entry.bundler === 'string' && entry.bundler ? [entry.bundler] : [];
	return Array.isArray(entry.rpc) ? entry.rpc.filter((u) => typeof u === 'string' && u) : [];
}

/** Does the wallet's catalog know this chain at all? */
export function chainKnown(catalog, chainId) {
	const chains = catalog && typeof catalog === 'object' ? catalog.chains : null;
	return !!(chains && typeof chains === 'object' && chains[String(chainId)]);
}

// ---- param / value helpers --------------------------------------------------

const ADDR_RE = /^0x[0-9a-fA-F]{40}$/;
export function isAddressLike(v) {
	return typeof v === 'string' && ADDR_RE.test(v);
}

/** EIP-1193: minimal lowercase hex, e.g. 1 → "0x1". */
export function toHexChainId(n) {
	const num = typeof n === 'string' ? parseInt(n, n.startsWith('0x') ? 16 : 10) : n;
	if (!Number.isFinite(num) || num <= 0) return '0x1';
	return '0x' + Math.floor(num).toString(16);
}

/** number | "0x…" | decimal-string → number (NaN-safe → 0). */
export function parseChainId(v) {
	if (typeof v === 'number') return Math.floor(v);
	if (typeof v === 'string') {
		const n = v.startsWith('0x') || v.startsWith('0X') ? parseInt(v, 16) : parseInt(v, 10);
		return Number.isFinite(n) ? n : 0;
	}
	return 0;
}

/**
 * `personal_sign` is [message, address]; typed data is [address, typedData].
 * Detect the address by SHAPE, not position — the classic hand-rolled-provider
 * bug. Display and validation only: the request still forwards its params
 * verbatim.
 */
export function pickSignAddress(method, params) {
	if (!Array.isArray(params)) return null;
	for (const p of params) if (isAddressLike(p)) return p.toLowerCase();
	return null;
}

/** A short, human origin label ("app.uniswap.org" from a full origin). */
export function hostLabel(origin) {
	try {
		return new URL(origin).host || origin;
	} catch {
		return String(origin || '');
	}
}

// ---- request discipline (the channel's promises) ----------------------------

/**
 * The largest request payload that may reach a screen. Typed data and batched
 * calls are comfortably inside it; anything larger is a page trying its luck
 * with the window's memory rather than asking for a signature.
 */
export const MAX_REQUEST_BYTES = 512 * 1024;

/**
 * Is this something a page may ask of the wallet at all? Shape only — policy
 * belongs to the core. Refusing here keeps a malformed or enormous payload
 * away from every screen that would otherwise have to render it.
 */
export function isWellFormedRequest(value) {
	const v = value;
	if (!v || typeof v !== 'object') return false;
	if (typeof v.id !== 'string' || v.id.length < 1 || v.id.length > 128) return false;
	if (typeof v.method !== 'string' || v.method.length < 1 || v.method.length > 100) return false;
	// An array — or, for EIP-747's `wallet_watchAsset` alone, the object its
	// params are specified as (the core's `parse_page_message`, spec 070).
	const objectParams =
		v.method === 'wallet_watchAsset' && v.params !== null && typeof v.params === 'object';
	if (!Array.isArray(v.params) && !objectParams) return false;
	try {
		return JSON.stringify(v.params).length <= MAX_REQUEST_BYTES;
	} catch {
		// Cyclic, or something that cannot be serialised — not a request.
		return false;
	}
}

// ---- what an already-granted origin may be told, without a window ----------

/**
 * The accounts an origin may see, given its grant and the wallet's snapshot.
 *
 * **This is a TWIN of a rule `dapp_permissions` owns** (`resolve_granted`), and
 * it exists only because the service worker cannot run the core: loading a
 * 3.6 MB binary to answer `eth_accounts` on every page load is not a trade
 * anyone would make. `dapp-instant.test.ts` drives the REAL core over the same
 * matrix of inputs and asserts identical answers, so the two cannot drift in
 * silence — the same treatment 026 gave the relay's error strings.
 *
 * The load-bearing case is the last one. A cold read, before the wallet has
 * published anything, must NOT be read as "the account is gone" — that would
 * log the person out of every open dApp on every browser start.
 */
export function resolveGrantedAccounts(grant, snapshotAddresses) {
	if (!grant || typeof grant.address !== 'string') return [];
	if (!Array.isArray(snapshotAddresses) || snapshotAddresses.length === 0) return [grant.address];
	const present = snapshotAddresses.some(
		(a) => typeof a === 'string' && a.toLowerCase() === grant.address.toLowerCase()
	);
	return present ? [grant.address] : [];
}

/**
 * How a window torn down with an answer still owed settles.
 *
 * A TWIN of `dapp_permissions`' `browser_closed` → `SettleForwarded`, for the
 * same reason as above, and pinned the same way. The code is the whole point:
 * **4900, never 4001**. A dApp reads 4001 as "the user said no, nothing
 * happened" and re-sends — double-spending an operation that may already be at
 * the bundler. An explicit Cancel is a different thing and really is 4001.
 *
 * The request WINDOW settles itself with the core's own answer; this is the
 * backstop for a window that died before it could.
 */
export const CLOSED_WITHOUT_ANSWER = SETTLE.surface_closed;
