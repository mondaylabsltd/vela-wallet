/**
 * THE page-side provider — EIP-1193 + EIP-6963. One file, every Vela.
 *
 * Home: `rust/crates/vela-core/provider/inpage.js` (spec 070). The Chrome
 * extension bundles it as its MAIN-world content script; the core embeds it
 * (`app::dapp_rpc::provider_script`) with the one bridge that the desktop,
 * iOS and Android in-app browsers install at document start. It used to be an
 * ES module importing six constants from the extension's `lib/protocol.js`,
 * and three native shells each stripped the module keywords with their own
 * line filter; it is a classic script now, so nothing strips anything.
 *
 * The six constants below are PINNED to `extension/lib/protocol.js` (which the
 * extension's service worker still reads) by `src/lib/dapp/protocol.test.ts`
 * and by the core's `dapp_rpc` tests — change one side and a test fails.
 */
/**
 * The in-page provider — EIP-1193 + EIP-6963 (spec 027 T320).
 *
 * Ported from packages/safari-extension/src/inpage.js @ 52ad8fa9. Runs in the
 * page's MAIN world, where `window.ethereum` has to be visible to the dApp.
 * NO extension APIs here: every request is relayed to content.js over a tagged
 * `window.postMessage` channel and correlated by a page-local id.
 *
 * What changed for Chrome: Safari needed a second injection path (content.js
 * writing a `<script src>` tag) because older Safari ignored `world: "MAIN"`.
 * This manifest requires Chrome 116, where the MAIN world is reliable and — the
 * part that matters — is not subject to the PAGE's own CSP, so a strict site
 * can no longer refuse the provider. The world guard below stays anyway: it is
 * two lines, and the failure it prevents is a provider installed where the page
 * cannot see it.
 *
 * Compatibility rules, unchanged from the Safari port:
 *   - EIP-6963 eager announce, frozen info, announced provider === window.ethereum
 *   - dispatch ethereum#initialized; set window.ethereum only if absent, configurable
 *   - legacy shims: send / sendAsync / enable; sync props selectedAddress /
 *     chainId / networkVersion / isConnected() backed by an inpage session cache
 *   - reconcile {chainId, accounts} on every response; dedupe delivery per rpcId
 *   - spoof isMetaMask on the LEGACY window.ethereum singleton for MM-only dApps,
 *     while EIP-6963 keeps announcing the true Vela identity (see the provider
 *     object); refuse nothing here that the router can answer — the router owns
 *     policy (eth_sign refusal, and so on)
 */
/* global browser, chrome */

(() => {
	// ---- the page-side constants (pinned; see the header) ------------------
	const CHANNEL = 'vela-1193';
	const RDNS = 'app.getvela';
	const WALLET_NAME = 'Vela Wallet';
	const ERR = {
		USER_REJECTED: 4001,
		UNAUTHORIZED: 4100,
		UNSUPPORTED_METHOD: 4200,
		UNKNOWN_PENDING: 4900,
		CHAIN_NOT_ADDED: 4902,
		METHOD_NOT_FOUND: -32601,
		INVALID_PARAMS: -32602,
		INTERNAL: -32603
	};
	function rpcError(code, message, data) {
		const e = { code, message };
		if (data !== undefined) e.data = data;
		return e;
	}
	/** EIP-1193: minimal lowercase hex, e.g. 1 → "0x1". */
	function toHexChainId(n) {
		const num = typeof n === 'string' ? parseInt(n, n.startsWith('0x') ? 16 : 10) : n;
		if (!Number.isFinite(num) || num <= 0) return '0x1';
		return '0x' + Math.floor(num).toString(16);
	}

	// World guard. In the page's MAIN world `chrome`/`browser` are undefined; if
	// they are defined we are running as an ISOLATED content script instead, where
	// `window.ethereum` would be invisible to the page. Bail rather than install a
	// provider nobody can reach.
	try {
		const ext =
			(typeof browser !== 'undefined' && browser) || (typeof chrome !== 'undefined' && chrome);
		if (ext && ext.runtime && ext.runtime.id) return;
	} catch {
		/* touching `browser` threw → page world → proceed */
	}

	// Idempotency: a document_start content script can inject twice on some pages.
	if (window.__velaProviderInstalled) return;
	window.__velaProviderInstalled = true;
	// Shared-DOM marker: `window.__velaProviderInstalled` lives in THIS world only,
	// so the isolated content script cannot read it. A documentElement attribute
	// rides the shared DOM across worlds, which is how content.js (and the e2e)
	// can tell the provider is live.
	try {
		document.documentElement.setAttribute('data-vela-inpage', '1');
	} catch {
		/* no documentElement yet — content.js falls back to the tag inject */
	}

	// Per-page-load session uuid (EIP-6963 requires a fresh uuid each announce set).
	const SESSION_UUID =
		(window.crypto && window.crypto.randomUUID && window.crypto.randomUUID()) ||
		'vela-' + Date.now().toString(16) + Math.floor(Math.random() * 1e9).toString(16);

	// The wallet's mark, as a data URI (EIP-6963 requires one — a remote icon
	// URL would leak every dApp visit to our host). This is docs/design/icon/
	// app-icon.svg verbatim — THE canonical mark every platform's icon is
	// rendered from — URI-encoded rather than base64 so it stays readable here
	// and in a stranger's dev tools. SVG, so a wallet picker draws it crisp at
	// any size; `image/svg+xml` is what MetaMask and Rabby announce too.
	const ICON =
		'data:image/svg+xml,' +
		encodeURIComponent(
			'<svg xmlns="http://www.w3.org/2000/svg" width="68" height="68" viewBox="0 0 68 68">' +
				'<rect x="1" y="1" width="66" height="66" rx="18" ry="18" fill="#f46d50"/>' +
				'<path d="M33,12C25,21,20,32,17,42L33,42L33,12Z" fill="#fff3ec"/>' +
				'<path d="M36,19C45,25,50,34,52,42L36,42L36,19Z" fill="#ffc6b0"/>' +
				'<path d="M13,46L55,46C52,52,47,55,40,55L28,55C21,55,16,52,13,46Z" fill="#5a4037"/>' +
				'</svg>'
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
		connected: false
	};

	// Reconcile cache from a fresh (method,result) or an event; emit the EIP-1193
	// events that actually changed. Called on every response and every 'evt'.
	function applyAccounts(next) {
		const norm = Array.isArray(next)
			? next.filter((a) => typeof a === 'string').map((a) => a.toLowerCase())
			: [];
		const changed =
			norm.length !== session.accounts.length || norm.some((a, i) => a !== session.accounts[i]);
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
		window.postMessage(
			{ ch: CHANNEL, dir: 'req', id, method, params: params ?? [] },
			window.location.origin
		);
	}

	// Update the cache from a method's own successful result (cheap reconciliation
	// that needs no annotation from content).
	function reconcileFromResult(method, result, params) {
		if (method === 'eth_chainId') applyChain(parseIntChain(result));
		else if (method === 'eth_accounts' || method === 'eth_requestAccounts') applyAccounts(result);
		else if (method === 'net_version') applyChain(parseIntChain(result));
		else if (method === 'wallet_switchEthereumChain' || method === 'wallet_addEthereumChain') {
			// A switch answers `null`, and the wallet pushes `chainChanged` as an
			// event. Learn it from the request too, so the page's own next read
			// of `chainId` is right whichever of the two arrives first.
			const asked = Array.isArray(params) && params[0] ? params[0].chainId : undefined;
			applyChain(parseIntChain(asked));
		}
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
				reconcileFromResult(entry.method, d.result, entry.params);
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
					// The wallet cut this site off. Forget the chain too: the next
					// successful `eth_chainId` (a dApp asks on reconnect) is then a
					// FIRST learn again and re-emits `connect`, so a site that connects
					// a second time sees the provider come back the way it first came.
					session.connected = false;
					session.chainIdNum = null;
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
			if (
				!args ||
				typeof args !== 'object' ||
				typeof args.method !== 'string' ||
				args.method.length === 0
			) {
				reject(
					Object.assign(
						new Error('Invalid request arguments'),
						rpcError(ERR.INVALID_PARAMS, 'Expected { method, params }')
					)
				);
				return;
			}
			const params = args.params === undefined ? [] : args.params;
			if (params !== null && typeof params !== 'object') {
				reject(
					Object.assign(
						new Error('Invalid params'),
						rpcError(ERR.INVALID_PARAMS, 'params must be an array or object')
					)
				);
				return;
			}
			const id = nextId();
			pending.set(id, { resolve, reject, method: args.method, params });
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
			return request({
				method: methodOrPayload,
				params: Array.isArray(paramsOrCb) ? paramsOrCb : []
			});
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
				return {
					id: p.id,
					jsonrpc: '2.0',
					result: session.chainIdNum ? toHexChainId(session.chainIdNum) : null
				};
			case 'net_version':
				return {
					id: p.id,
					jsonrpc: '2.0',
					result: session.chainIdNum ? String(session.chainIdNum) : null
				};
			default: {
				const msg =
					'Vela: synchronous send() is only supported for eth_accounts/eth_chainId/net_version';
				throw Object.assign(new Error(msg), rpcError(ERR.UNSUPPORTED_METHOD, msg));
			}
		}
	}
	function sendAsync(payload, cb) {
		// Batch support (rare) — resolve each independently.
		if (Array.isArray(payload)) {
			Promise.all(payload.map((p) => request({ method: p.method, params: p.params })))
				.then((results) =>
					cb(
						null,
						results.map((r, i) => ({ id: payload[i].id, jsonrpc: '2.0', result: r }))
					)
				)
				.catch((err) => cb(err, null));
			return;
		}
		request({ method: payload.method, params: payload.params }).then(
			(result) => cb(null, { id: payload.id, jsonrpc: '2.0', result }),
			(err) => cb(err, null)
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
		// left legacy MM-only dApps unusable, which defeats the point of being in the
		// page at all.)
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
		isConnected
	};
	// Synchronous legacy props as live getters (kept in sync with the cache).
	Object.defineProperties(provider, {
		selectedAddress: { get: () => session.accounts[0] ?? null, enumerable: true },
		chainId: {
			get: () => (session.chainIdNum ? toHexChainId(session.chainIdNum) : null),
			enumerable: true
		},
		networkVersion: {
			get: () => (session.chainIdNum ? String(session.chainIdNum) : null),
			enumerable: true
		},
		_vela: { get: () => ({ ...session }) } // test/debug snapshot (non-enumerable)
	});

	// ---- publish on window.ethereum (defensively) -----------------------------
	// Set only if absent; keep configurable; contribute to providers[]. Never
	// clobber an existing wallet — 6963 is the primary discovery path.
	try {
		if (!window.ethereum) {
			Object.defineProperty(window, 'ethereum', {
				value: provider,
				configurable: true,
				writable: true
			});
		} else {
			// Respect a pre-existing provider but join its providers[] list if present.
			if (
				Array.isArray(window.ethereum.providers) &&
				!window.ethereum.providers.includes(provider)
			) {
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
				new CustomEvent('eip6963:announceProvider', { detail: Object.freeze({ info, provider }) })
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
