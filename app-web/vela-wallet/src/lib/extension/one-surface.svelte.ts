/**
 * The side panel's half of "one wallet per window" (`extension/lib/one-surface.js`).
 *
 * The owner's report: the wallet open in a tab and in the side panel of the
 * same window, the two disagreeing. A window holds one live wallet, and a tab
 * is the deliberate one — so the panel watches its own window's tabs, and
 * while a wallet tab is open there it stands aside: "Vela is open in a tab",
 * with Go to tab (the tab comes forward and the panel closes, where Chrome
 * lets it) and Use here instead (the tab closes, the panel is the wallet).
 *
 * Never while the panel is busy with a dApp's request — one it owes, or one
 * whose receipt or landing is on screen (`hold`): a request is not a wallet
 * surface, it is answered where spec 082 routes it, over the wallet.
 *
 * The panel learns of the tabs by itself, from `chrome.tabs` events: a tab
 * typed in, the install welcome, the toolbar — whoever opened it. The
 * worker's toolbar closes the panel too where `sidePanel.close` exists
 * (Chrome 141+); this is what makes it honest everywhere else.
 */
import { untrack } from 'svelte';
import { closeSidePanel, panelStanding, walletTabsIn } from '../../../extension/lib/one-surface.js';

interface TabLike {
	id?: number;
	windowId?: number;
	url?: string;
	pendingUrl?: string;
	active?: boolean;
}

type Event<F> = { addListener(fn: F): void; removeListener?(fn: F): void };

interface ChromeTabsLike {
	runtime?: { getURL?(path: string): string };
	windows?: {
		getCurrent(): Promise<{ id?: number }>;
		update(id: number, info: { focused: boolean }): Promise<unknown>;
	};
	tabs?: {
		query(info: { windowId: number }): Promise<TabLike[]>;
		update(id: number, info: { active: boolean }): Promise<unknown>;
		remove(ids: number | number[]): Promise<unknown>;
		onCreated?: Event<() => void>;
		onUpdated?: Event<() => void>;
		onRemoved?: Event<() => void>;
		onAttached?: Event<() => void>;
		onDetached?: Event<() => void>;
		onReplaced?: Event<() => void>;
	};
	sidePanel?: { close?(options: { windowId: number }): Promise<void> };
}

const TAB_EVENTS = [
	'onCreated',
	'onUpdated',
	'onRemoved',
	'onAttached',
	'onDetached',
	'onReplaced'
] as const;

export class OneSurface {
	#chrome: ChromeTabsLike | undefined;
	#windowId = $state<number | null>(null);
	#walletTabs = $state<TabLike[]>([]);
	/** Tabs the person said "Use here instead" for and that would not close. */
	#usedHere = $state<number[]>([]);
	#holds = $state(0);
	#stops: (() => void)[] = [];
	#checking: Promise<void> | null = null;
	#again = false;
	/** For tests: the window close a panel may fall back on. */
	#closeWindow: () => void;

	constructor(chrome?: ChromeTabsLike, closeWindow?: () => void) {
		this.#chrome = chrome;
		this.#closeWindow = closeWindow ?? (() => globalThis.close?.());
	}

	#api(): ChromeTabsLike | undefined {
		return this.#chrome ?? (globalThis as { chrome?: ChromeTabsLike }).chrome;
	}

	/** What the panel shows: the wallet, or the line about the tab. */
	get standing(): { kind: 'live' } | { kind: 'elsewhere'; tabId: number } {
		return panelStanding({
			walletTabs: this.#walletTabs,
			usedHere: this.#usedHere,
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

	/**
	 * A request is on this panel — owed, or its receipt or landing on screen:
	 * the wallet stays live until the returned release is called.
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

	/** Watch this panel's window. Only the side panel calls it. */
	async start(): Promise<void> {
		const api = this.#api();
		if (!api?.tabs || !api.windows || this.#windowId !== null) return;
		try {
			const own = await api.windows.getCurrent();
			if (typeof own.id !== 'number') return;
			this.#windowId = own.id;
		} catch {
			return;
		}
		const check = () => void this.check();
		for (const name of TAB_EVENTS) {
			const event = api.tabs[name];
			if (!event) continue;
			try {
				event.addListener(check);
				this.#stops.push(() => event.removeListener?.(check));
			} catch {
				/* that event is not offered here */
			}
		}
		await this.check();
	}

	stop(): void {
		for (const stop of this.#stops.splice(0)) stop();
		this.#windowId = null;
	}

	/** Read this window's tabs again (one read at a time; a burst coalesces). */
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
		const base = api?.runtime?.getURL?.('');
		if (windowId === null || !api?.tabs || !base) return;
		let tabs: TabLike[];
		try {
			tabs = await api.tabs.query({ windowId });
		} catch {
			return;
		}
		this.#walletTabs = walletTabsIn(tabs, windowId, base) as TabLike[];
		// A tab the person kept the panel over and that has since gone is
		// forgotten: a new tab is asked about afresh.
		const open = this.#walletTabs.map((t) => t.id);
		if (this.#usedHere.some((id) => !open.includes(id))) {
			this.#usedHere = this.#usedHere.filter((id) => open.includes(id));
		}
	}

	/** "Go to tab": the tab comes forward, and the panel goes where Chrome lets it. */
	async goToTab(): Promise<void> {
		const api = this.#api();
		const target = this.elsewhere;
		if (!api?.tabs || !api.windows || !target) return;
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

	/** "Use here instead": the window's wallet tabs close and the panel is the wallet. */
	async useHere(): Promise<void> {
		const api = this.#api();
		const ids = this.#walletTabs.map((t) => t.id).filter((id): id is number => id !== undefined);
		if (ids.length === 0) return;
		// Live at once; a tab that will not close is kept out of the count.
		this.#usedHere = [...this.#usedHere, ...ids.filter((id) => !this.#usedHere.includes(id))];
		try {
			await api?.tabs?.remove(ids);
		} catch {
			/* it stays open; the person chose this panel all the same */
		}
		await this.check();
	}
}

/** The one of this document (only the side panel starts it). */
export const oneSurface = new OneSurface();
