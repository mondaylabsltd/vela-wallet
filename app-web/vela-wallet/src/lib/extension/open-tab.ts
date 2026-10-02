/**
 * Open an extension page in an ordinary browser tab, from any extension page —
 * the request window included (spec 094 S5).
 *
 * A request window is a `popup` window, which holds exactly one tab: a tab
 * opened "here" has nowhere to go. So the page goes to the window the person
 * last used (focused), or to a new one when there is none.
 */
interface ChromeWindowsTabs {
	runtime: { getURL(path: string): string };
	windows: {
		getLastFocused(filter: { windowTypes: string[] }): Promise<{ id?: number }>;
		update(id: number, info: { focused: boolean }): Promise<unknown>;
		create(info: { url: string; type: 'normal'; focused: boolean }): Promise<unknown>;
		getCurrent(): Promise<{ id?: number }>;
	};
	tabs: { create(info: { url: string; windowId: number }): Promise<unknown> };
}

function chromeApi(): ChromeWindowsTabs | null {
	const api = (globalThis as { chrome?: Partial<ChromeWindowsTabs> }).chrome;
	return api?.runtime?.getURL && api.windows && api.tabs ? (api as ChromeWindowsTabs) : null;
}

/** `path` is relative to the extension's root: `en/create.html`. */
export async function openInBrowserTab(path: string): Promise<void> {
	const api = chromeApi();
	if (!api) return;
	const url = api.runtime.getURL(path);
	try {
		const last = await api.windows.getLastFocused({ windowTypes: ['normal'] });
		if (last.id !== undefined) {
			await api.tabs.create({ windowId: last.id, url });
			await api.windows.update(last.id, { focused: true });
			return;
		}
	} catch {
		// No normal window at all: Chrome says so by throwing.
	}
	await api.windows.create({ url, type: 'normal', focused: true });
}

/** Bring the window this page is in to the front. */
export async function focusOwnWindow(): Promise<void> {
	const api = chromeApi();
	if (!api) return;
	try {
		const own = await api.windows.getCurrent();
		if (own.id !== undefined) await api.windows.update(own.id, { focused: true });
	} catch {
		/* nothing to bring forward */
	}
}
