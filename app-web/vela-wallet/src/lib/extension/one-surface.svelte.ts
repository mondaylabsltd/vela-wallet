/**
 * One wallet per window (`extension/lib/one-surface.js`), as the side panel
 * and a wallet tab each keep it.
 *
 * The owner's report: the wallet open in a tab and in the side panel of the
 * same window, the two disagreeing. A window holds one live wallet. By
 * default it is the tab — the deliberate one — so the panel watches its own
 * window's tabs and, while a wallet tab is open there, stands aside: "Vela is
 * open in a tab", with Go to tab (the tab comes forward and the panel closes,
 * where Chrome lets it) and Use here instead.
 *
 * Use here instead closes nothing: a half-filled send is never cut off. It
 * hands the window over (`ownerKey` in `storage.session`), and the surface
 * that had it stands aside the same way, its wallet loaded beneath — the tab
 * then says "Vela is open in the side panel", with Go to side panel and its
 * own Use here instead, which hands the window back.
 *
 * Never while a surface is busy (`hold`): a dApp's request the panel owes,
 * shows or is landing — answered where spec 082 routes it, over the wallet —
 * or a send or signature in flight in either.
 *
 * The panel learns of the tabs by itself, from `chrome.tabs` events: a tab
 * typed in, the install welcome, the toolbar — whoever opened it. The
 * worker's toolbar closes the panel too where `sidePanel.close` exists
 * (Chrome 141+), and takes the window back for the tab.
 */
import { untrack } from 'svelte';
import {
	closeSidePanel,
	OWNER_PREFIX,
	ownerKey,
	panelStanding,
	tabStanding,
	walletTabsIn
} from '../../../extension/lib/one-surface.js';

interface TabLike {
	id?: number;
	windowId?: number;
	url?: string;
	pendingUrl?: string;
	active?: boolean;
}

type Event<F> = { addListener(fn: F): void; removeListener?(fn: F): void };
type StorageChanges = Record<string, { newValue?: unknown } | undefined>;

interface ChromeLike {
	runtime?: {
		getURL?(path: string): string;
		getContexts?(filter: { contextTypes: string[] }): Promise<unknown[]>;
	};
	windows?: {
		getCurrent(): Promise<{ id?: number }>;
		update(id: number, info: { focused: boolean }): Promise<unknown>;
	};
	tabs?: {
		query(info: { windowId: number }): Promise<TabLike[]>;
		update(id: number, info: { active: boolean }): Promise<unknown>;
		onCreated?: Event<() => void>;
		onUpdated?: Event<() => void>;
		onRemoved?: Event<() => void>;
		onAttached?: Event<() => void>;
		onDetached?: Event<() => void>;
		onReplaced?: Event<() => void>;
	};
	storage?: {
		session?: {
			get(key: string): Promise<Record<string, unknown>>;
			set(items: Record<string, unknown>): Promise<void>;
			remove(key: string): Promise<void>;
		};
		onChanged?: Event<(changes: StorageChanges, area: string) => void>;
	};
	sidePanel?: {
		close?(options: { windowId: number }): Promise<void>;
		open?(options: { windowId: number }): Promise<void>;
	};
}

const TAB_EVENTS = [
	'onCreated',
	'onUpdated',
	'onRemoved',
	'onAttached',
	'onDetached',
	'onReplaced'
] as const;

/** sessionStorage of the panel: this panel took its window ("Use here instead"). */
export const PANEL_CLAIM_KEY = 'vela.surface.claimed';

export type SurfaceRole = 'panel' | 'tab';

export class OneSurface {
	#chrome: ChromeLike | undefined;
	#role = $state<SurfaceRole | null>(null);
	#windowId = $state<number | null>(null);
	#walletTabs = $state<TabLike[]>([]);
	/** `'panel'` while the person chose the panel for this window. */
	#owner = $state<string | null>(null);
	/** Is a side panel up (the tab's guard against a choice left behind)? */
	#panelUp = $state<boolean | null>(null);
	#holds = $state(0);
	/** The panel's document is going (`pagehide`). */
	#leaving = false;
	#stops: (() => void)[] = [];
	#checking: Promise<void> | null = null;
	#again = false;
	/** For tests: the window close a panel may fall back on, and its sessionStorage. */
	#closeWindow: () => void;
	#store: Pick<Storage, 'getItem' | 'setItem' | 'removeItem'> | undefined;

	constructor(
		chrome?: ChromeLike,
		closeWindow?: () => void,
		store?: Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>
	) {
		this.#chrome = chrome;
		this.#closeWindow = closeWindow ?? (() => globalThis.close?.());
		this.#store = store;
	}

	#api(): ChromeLike | undefined {
		return this.#chrome ?? (globalThis as { chrome?: ChromeLike }).chrome;
	}

	#session(): Pick<Storage, 'getItem' | 'setItem' | 'removeItem'> | undefined {
		try {
			return this.#store ?? globalThis.sessionStorage;
		} catch {
			return undefined;
		}
	}

	/** What the panel shows: the wallet, or the line about the tab. */
	get standing(): { kind: 'live' } | { kind: 'elsewhere'; tabId: number } {
		if (this.#role !== 'panel') return { kind: 'live' };
		return panelStanding({
			walletTabs: this.#walletTabs,
			owner: this.#owner,
			busy: this.#holds > 0
		});
	}

	/** The wallet tab this panel stands aside for, or `null` while it is the wallet. */
	get elsewhere(): { tabId: number; windowId: number } | null {
		const standing = this.standing;
		const windowId = this.#windowId;
		return standing.kind === 'elsewhere' && windowId !== null
			? { tabId: standing.tabId, windowId }
			: null;
	}

	/** A wallet tab standing aside for the side panel its window's person chose. */
	get tabAside(): boolean {
		if (this.#role !== 'tab' || this.#windowId === null) return false;
		return (
			tabStanding({ owner: this.#owner, panelUp: this.#panelUp, busy: this.#holds > 0 }).kind ===
			'aside'
		);
	}

	/**
	 * Something is under way on this surface — a request owed, shown or
	 * landing, a send or signature in flight: it stays live until the
	 * returned release is called.
	 */
	hold(): () => void {
		// Untracked: a hold is taken from an effect, and an effect that read
		// the count it writes would run itself again, without end.
		untrack(() => (this.#holds += 1));
		let released = false;
		return () => {
			if (released) return;
			released = true;
			untrack(() => (this.#holds -= 1));
		};
	}

	/** Watch this surface's window: the side panel's, or a wallet tab's. */
	async start(role: SurfaceRole = 'panel'): Promise<void> {
		const api = this.#api();
		if (!api?.windows || this.#role !== null) return;
		try {
			const own = await api.windows.getCurrent();
			if (typeof own.id !== 'number') return;
			this.#windowId = own.id;
		} catch {
			return;
		}
		this.#role = role;
		const check = () => void this.check();
		if (role === 'panel') {
			for (const name of TAB_EVENTS) this.#listen(api.tabs?.[name], check);
			// A panel that took its window and was reloaded (its language
			// changed) still has it; one that goes gives it back.
			if (this.#session()?.getItem(PANEL_CLAIM_KEY) === '1') await this.#writeOwner('panel');
			const gone = () => {
				// Leaving: the claim mark stays with this panel's frame, for a reload.
				this.#leaving = true;
				if (this.#session()?.getItem(PANEL_CLAIM_KEY) === '1') void this.#writeOwner(null);
			};
			globalThis.addEventListener?.('pagehide', gone);
			this.#stops.push(() => globalThis.removeEventListener?.('pagehide', gone));
		} else {
			// Whether a panel is still up is asked again when the person looks.
			globalThis.addEventListener?.('focus', check);
			this.#stops.push(() => globalThis.removeEventListener?.('focus', check));
		}
		const changed = (changes: StorageChanges, area: string) => {
			if (area !== 'session' || this.#windowId === null) return;
			if (!Object.keys(changes).some((key) => key.startsWith(OWNER_PREFIX))) return;
			const change = changes[ownerKey(this.#windowId)];
			if (change) {
				this.#owner = change.newValue === 'panel' ? 'panel' : null;
				// The tab or the toolbar took the window back: this panel's claim is over.
				if (role === 'panel' && this.#owner === null && !this.#leaving) this.#claim(false);
			}
			if (role === 'tab') void this.check();
		};
		this.#listen(api.storage?.onChanged, changed);
		await this.check();
	}

	#listen<F>(event: Event<F> | undefined, fn: F): void {
		if (!event) return;
		try {
			event.addListener(fn);
			this.#stops.push(() => event.removeListener?.(fn));
		} catch {
			/* that event is not offered here */
		}
	}

	stop(): void {
		for (const stop of this.#stops.splice(0)) stop();
		this.#windowId = null;
		this.#role = null;
	}

	/** Read this window's facts again (one read at a time; a burst coalesces). */
	check(): Promise<void> {
		if (this.#checking) {
			this.#again = true;
			return this.#checking;
		}
		this.#checking = (async () => {
			do {
				this.#again = false;
				await this.#read();
			} while (this.#again);
			this.#checking = null;
		})();
		return this.#checking;
	}

	async #read(): Promise<void> {
		const api = this.#api();
		const windowId = this.#windowId;
		if (windowId === null) return;
		try {
			const all = (await api?.storage?.session?.get(ownerKey(windowId))) ?? {};
			this.#owner = all[ownerKey(windowId)] === 'panel' ? 'panel' : null;
		} catch {
			this.#owner = null;
		}
		if (this.#role === 'tab') {
			// Chrome names a global side panel's window -1, so this can only
			// say whether ANY panel is up — enough to drop a choice left behind.
			try {
				const contexts = await api?.runtime?.getContexts?.({ contextTypes: ['SIDE_PANEL'] });
				this.#panelUp = Array.isArray(contexts) ? contexts.length > 0 : null;
			} catch {
				this.#panelUp = null;
			}
			return;
		}
		const base = api?.runtime?.getURL?.('');
		if (!api?.tabs || !base) return;
		try {
			const tabs = await api.tabs.query({ windowId });
			this.#walletTabs = walletTabsIn(tabs, windowId, base) as TabLike[];
		} catch {
			/* the last read stands */
		}
	}

	async #writeOwner(owner: 'panel' | null): Promise<void> {
		const api = this.#api();
		const windowId = this.#windowId;
		if (windowId === null) return;
		this.#owner = owner;
		try {
			if (owner) await api?.storage?.session?.set({ [ownerKey(windowId)]: owner });
			else await api?.storage?.session?.remove(ownerKey(windowId));
		} catch {
			/* this surface still follows the choice; the other one asks again */
		}
	}

	#claim(claimed: boolean): void {
		try {
			if (claimed) this.#session()?.setItem(PANEL_CLAIM_KEY, '1');
			else this.#session()?.removeItem(PANEL_CLAIM_KEY);
		} catch {
			/* storage denied: the choice lasts as long as this document */
		}
	}

	/** The panel's "Go to tab": the window back to the tab, which comes forward; the panel goes. */
	async goToTab(): Promise<void> {
		const api = this.#api();
		const target = this.elsewhere;
		if (!api?.tabs || !api.windows || !target) return;
		this.#claim(false);
		await this.#writeOwner(null);
		try {
			await api.tabs.update(target.tabId, { active: true });
			await api.windows.update(target.windowId, { focused: true });
		} catch {
			// The tab went in the meantime: the next read says so.
			await this.check();
			return;
		}
		if (this.#holds > 0) return;
		if (await closeSidePanel(api.sidePanel, target.windowId)) return;
		// Before Chrome 141 a panel can still close itself.
		try {
			this.#closeWindow();
		} catch {
			/* it stays, saying where the wallet is */
		}
	}

	/**
	 * "Use here instead". In the panel: the window is the panel's, and its
	 * tab stands aside — closed, never: whatever it holds stays as it was. In
	 * a tab: the window is the tab's again, and the panel stands aside.
	 */
	async useHere(): Promise<void> {
		if (this.#role === 'panel') {
			this.#claim(true);
			await this.#writeOwner('panel');
		} else if (this.#role === 'tab') {
			await this.#writeOwner(null);
		}
	}

	/**
	 * The tab's "Go to side panel": the panel brought up in this window. Called
	 * straight from the click, before any `await` — Chrome opens a side panel
	 * only inside a user gesture.
	 */
	goToPanel(): void {
		const api = this.#api();
		const windowId = this.#windowId;
		if (windowId === null || typeof api?.sidePanel?.open !== 'function') return;
		try {
			void Promise.resolve(api.sidePanel.open({ windowId })).catch(() => {});
		} catch {
			/* no gesture or no panel: the tab still says where the wallet is */
		}
	}
}

/** The one of this document (the side panel and a wallet tab start it). */
export const oneSurface = new OneSurface();
