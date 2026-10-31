/**
 * The Safari background's adapter: what `background.js` asks of Chrome, made
 * true on Safari — so the worker's code is ONE file on both browsers and the
 * Chrome package is not touched by this one (native AUDIT §6.5).
 *
 * Imported FIRST by `safari/background.js`, so it runs before the worker's
 * top level reads `chrome.*`. What iOS Safari gives an extension, measured
 * (`dev-probe.html`, iOS 26.2 simulator): `chrome` is `browser`, promise-
 * based; `storage.session` works; there is NO `windows` API at all, no
 * `sidePanel`, no `runtime.getContexts`. Three seams follow:
 *
 * 1. **`chrome` is the extension API**, whichever name Safari gave it.
 *
 * 2. **A request rises in a TAB.** iOS has no popup windows — no `windows`
 *    API at all — and the worker opens a request "window" when it cannot
 *    open a side panel. So a `type: 'popup'` window becomes a tab, opened
 *    from the dApp's own tab (`openerTabId`, read from the request id the
 *    doorway carries: `rid = <tabId>:<pageRequestId>`), and the worker keeps
 *    calling it a window: its id is a VIRTUAL window id, `-(1000 + tabId)` —
 *    stateless, so a non-persistent background page that wakes again still
 *    knows it. `windows.create/remove/update/getAll/onRemoved` and
 *    `tabs.query({windowId})` understand that id; a real window (macOS
 *    Safari) passes through. Chrome reserves only -1 and -2, and no real
 *    window has a negative id.
 *
 * 3. **No side panel.** `chrome.sidePanel` stays undefined: the worker
 *    already falls back to the request window (now a tab).
 *
 * Safari's API objects are host objects: this never writes onto them. It
 * hands the worker a `chrome` of its own whose `windows` and `tabs` are this
 * file's, and every other member is Safari's own, bound to its owner.
 */

/** Virtual window ids: a tab standing in for a popup window. */
const VIRTUAL_BASE = 1000;
export const virtualWindowId = (tabId) => -(VIRTUAL_BASE + tabId);
export const isVirtualWindow = (windowId) =>
	typeof windowId === 'number' && windowId <= -VIRTUAL_BASE;
export const tabOfVirtual = (windowId) => -windowId - VIRTUAL_BASE;

/** The dApp tab a request belongs to, from the doorway URL's `rid`. */
export function openerOf(url) {
	try {
		const rid = new URL(url).searchParams.get('rid') ?? '';
		const tabId = Number.parseInt(rid.split(':')[0], 10);
		return Number.isInteger(tabId) && tabId >= 0 ? tabId : undefined;
	} catch {
		return undefined;
	}
}

/** `target` with `overrides` laid over it; Safari's own methods stay bound to Safari's object. */
function over(target, overrides) {
	const object = target ?? {};
	// An EMPTY proxy target: Safari's own object may be frozen or hold
	// non-configurable members, and a proxy over it must then return exactly
	// those — which would forbid the very overrides this exists for.
	return new Proxy(Object.create(null), {
		get(_empty, key) {
			if (Object.prototype.hasOwnProperty.call(overrides, key)) return overrides[key];
			const value = object[key];
			return typeof value === 'function' ? value.bind(object) : value;
		},
		has(_empty, key) {
			return Object.prototype.hasOwnProperty.call(overrides, key) || key in object;
		}
	});
}

/**
 * The `chrome` the worker sees: `api` with tab-backed `windows` and a
 * `tabs.query` that understands virtual ids. Pure in `api` (tests pass a fake).
 */
export function withTabSurfaces(api) {
	const tabs = api?.tabs;
	if (!tabs) return api;
	/**
	 * Which dApp tab each request tab was opened from, so closing it puts the
	 * person back on the dApp — Safari otherwise shows whichever tab is next
	 * to it (measured: the wallet's own tab). In `storage.session`, because a
	 * non-persistent background page forgets everything in memory.
	 */
	const session = api.storage?.session;
	const openerKey = (tabId) => `vela.safari.opener.${tabId}`;
	async function rememberOpener(tabId, openerTabId) {
		if (openerTabId === undefined) return;
		try {
			await session?.set({ [openerKey(tabId)]: openerTabId });
		} catch {
			/* the person lands on a neighbouring tab instead */
		}
	}
	async function returnToOpener(tabId) {
		try {
			const key = openerKey(tabId);
			const opener = (await session?.get(key))?.[key];
			await session?.remove(key);
			if (typeof opener === 'number') await tabs.update(opener, { active: true });
		} catch {
			/* the dApp's tab is gone: nothing to return to */
		}
	}
	const windows = api.windows;
	const real = {
		create: windows?.create?.bind(windows),
		remove: windows?.remove?.bind(windows),
		update: windows?.update?.bind(windows),
		getAll: windows?.getAll?.bind(windows),
		query: tabs.query.bind(tabs)
	};

	const removedListeners = new Set();
	tabs.onRemoved?.addListener((tabId) => {
		// A request tab the wallet closed itself (or the person did): back to the dApp.
		void returnToOpener(tabId);
		for (const listener of removedListeners) {
			try {
				listener(virtualWindowId(tabId));
			} catch {
				/* one listener's failure is not the others' */
			}
		}
	});

	const shimWindows = {
		WINDOW_ID_NONE: -1,
		WINDOW_ID_CURRENT: -2,
		async create(options = {}) {
			const asTab = options.type === 'popup' || !real.create;
			if (!asTab || typeof options.url !== 'string') return real.create?.(options);
			const openerTabId = openerOf(options.url);
			const active = options.focused !== false;
			let tab;
			try {
				tab = await tabs.create({
					url: options.url,
					active,
					...(openerTabId !== undefined ? { openerTabId } : {})
				});
			} catch {
				// The dApp's tab went (or this Safari refuses `openerTabId`).
				tab = await tabs.create({ url: options.url, active });
			}
			await rememberOpener(tab.id, openerTabId);
			return { id: virtualWindowId(tab.id), type: 'popup', tabs: [tab], focused: active };
		},
		async remove(windowId) {
			if (!isVirtualWindow(windowId)) return real.remove?.(windowId);
			const tabId = tabOfVirtual(windowId);
			await returnToOpener(tabId);
			return tabs.remove(tabId);
		},
		async update(windowId, info = {}) {
			if (!isVirtualWindow(windowId)) {
				return real.update ? real.update(windowId, info) : { id: windowId };
			}
			const tab = await tabs.update(tabOfVirtual(windowId), { active: true });
			return { id: windowId, type: 'popup', tabs: [tab], focused: true };
		},
		async getAll(...args) {
			const all = real.getAll ? await real.getAll(...args) : [];
			let open = [];
			try {
				open = await real.query({});
			} catch {
				/* no tabs to tell: the real windows only */
			}
			const ids = new Set(all.map((w) => w.id));
			for (const tab of open) {
				if (typeof tab.windowId === 'number' && !ids.has(tab.windowId)) {
					ids.add(tab.windowId);
					all.push({ id: tab.windowId, type: 'normal' });
				}
			}
			return [
				...all,
				...open
					.filter((tab) => typeof tab.id === 'number')
					.map((tab) => ({ id: virtualWindowId(tab.id), type: 'popup' }))
			];
		},
		onRemoved: {
			addListener(listener) {
				removedListeners.add(listener);
				windows?.onRemoved?.addListener(listener);
			},
			removeListener(listener) {
				removedListeners.delete(listener);
				windows?.onRemoved?.removeListener?.(listener);
			},
			hasListener: (listener) => removedListeners.has(listener)
		}
	};

	const shimTabs = over(tabs, {
		async query(query = {}) {
			if (isVirtualWindow(query.windowId)) {
				try {
					return [await tabs.get(tabOfVirtual(query.windowId))];
				} catch {
					return [];
				}
			}
			if ('windowType' in query) {
				// Safari has normal windows only; some versions refuse the key.
				const rest = { ...query };
				delete rest.windowType;
				return real.query(rest);
			}
			return real.query(query);
		}
	});

	return over(api, { windows: shimWindows, tabs: shimTabs });
}

const native = globalThis.browser?.runtime ? globalThis.browser : globalThis.chrome;
if (native?.runtime) {
	const adapted = withTabSurfaces(native);
	try {
		globalThis.chrome = adapted;
	} catch {
		/* a read-only global: define it instead */
	}
	if (globalThis.chrome !== adapted) {
		Object.defineProperty(globalThis, 'chrome', {
			value: adapted,
			configurable: true,
			writable: true
		});
	}
}
