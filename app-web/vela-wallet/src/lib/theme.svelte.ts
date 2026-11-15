/**
 * Which palette the page is wearing, resolved the way `tokens.css` resolves it:
 * a pinned `data-theme` first, the OS preference second (spec 012 FR-009).
 *
 * Exists because artwork that is CHOSEN rather than tinted — a passkey
 * provider's own logo, which ships a light and a dark cut — has to ask, and two
 * places asking it two different ways is how they end up disagreeing.
 */
import { MediaQuery } from 'svelte/reactivity';
import { browser } from '$app/env';

const light = new MediaQuery('(prefers-color-scheme: light)', false);

/** Reactive: read it in a component and the component follows the theme. */
export function isDarkTheme(): boolean {
	const pinned = browser ? document.documentElement.dataset.theme : undefined;
	if (pinned === 'dark') return true;
	if (pinned === 'light') return false;
	return !light.current;
}
