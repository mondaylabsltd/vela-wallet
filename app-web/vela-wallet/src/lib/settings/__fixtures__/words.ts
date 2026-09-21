/**
 * Settings words for component tests in a browser.
 *
 * The corpus is resolved at build time by the wasm engine, which a browser
 * test cannot run — so here every string is its own path (`networks.remove`,
 * `storage.itemContacts`). A test of what a control DOES finds it by that
 * path. Each node is a `String` that also answers deeper paths, and the same
 * path always answers the same object, so the fixture builders' own
 * comparisons hold.
 */
import type { SettingsMessages } from '../messages';

function node(path: string): unknown {
	const children = new Map<string, unknown>();
	const text = new String(path);
	return new Proxy(text, {
		get(target, key) {
			if (typeof key === 'symbol' || key in target) {
				const value = Reflect.get(target, key, target);
				return typeof value === 'function' ? value.bind(target) : value;
			}
			const next = path === '' ? key : `${path}.${key}`;
			if (!children.has(key)) children.set(key, node(next));
			return children.get(key);
		}
	});
}

export function words(): SettingsMessages {
	return node('') as SettingsMessages;
}
