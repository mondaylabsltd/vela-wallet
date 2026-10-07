/**
 * Where analytics may run — the real deployments only, decided in one pure
 * function so the rule is a test, not a hope.
 *
 * - the hosted wallet, and only at its production origin: a local, preview or
 *   e2e origin would write test runs into the production account (and an open
 *   request would hold Playwright's `networkidle`);
 * - the packaged extension, and only from its own `chrome-extension:` pages;
 * - never under automation (`navigator.webdriver`): the extension e2e loads the
 *   real package at its real `chrome-extension:` origin, so the origin alone
 *   cannot tell a test run from a person;
 * - never in the parallel space, whose fixture wallet is a developer's;
 * - never once the person has switched it off in Settings.
 */
import { SITE_ORIGIN } from '$lib/site';

export interface AnalyticsEnvironment {
	/** `location.origin`. */
	origin: string;
	/** `location.protocol`. */
	protocol: string;
	/** `__VELA_EXTENSION__` — this is the extension's build. */
	extensionBuild: boolean;
	/** `navigator.webdriver`. */
	automated: boolean;
	/** The parallel space (fixture wallet) is on. */
	parallelSpace: boolean;
	/** The person switched "Share anonymous usage statistics" off. */
	optedOut: boolean;
}

export function analyticsAllowed(env: AnalyticsEnvironment): boolean {
	if (env.optedOut || env.automated || env.parallelSpace) return false;
	if (env.extensionBuild) return env.protocol === 'chrome-extension:';
	return env.origin === SITE_ORIGIN;
}
