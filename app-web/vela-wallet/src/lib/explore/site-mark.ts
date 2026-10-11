/**
 * The mark beside a site's address, and the words for it (device walk of
 * 2026-10-11, item 11).
 *
 * A padlock was drawn for `http://127.0.0.1:8137` in the phones' address bars:
 * every shell took its lock from `secure`, which means "safe to use the wallet
 * from" and is true of http on loopback. A padlock says one thing to a person
 * — the connection is encrypted — so it is drawn for https and for nothing
 * else. The rule is the core's (`dapp_origin_lock`):
 *
 *   - `secure`   — https: the padlock, "Secure site";
 *   - `local`    — http on this device or the person's own network: a neutral
 *                  mark, "Local page" — never a padlock, never the warning;
 *   - `insecure` — public http: the warning, "Insecure site — not encrypted".
 *
 * This module asks the core and picks the corpus's words; it decides nothing.
 * **`loadCore()` must have resolved** (or, while prerendering, the build-time
 * engine must be up — it always is where a page's strings are resolved).
 */
import { dappOriginLock } from '$lib/core/client';
import type { OriginLock } from '$lib/core/generated/OriginLock';
import type { ExploreMessages } from './messages';
import type { SiteMarkModel } from './model';

type MarkMessages = Pick<
	ExploreMessages,
	'secureSite' | 'localSite' | 'a11yLocal' | 'a11yInsecure'
>;

/** The core's word for an origin. Anything it does not say is not a padlock. */
export function originLock(origin: string): OriginLock {
	const lock = dappOriginLock(origin);
	return lock === 'secure' || lock === 'local' ? lock : 'insecure';
}

export function siteMark(origin: string, m: MarkMessages): SiteMarkModel {
	const lock = originLock(origin);
	switch (lock) {
		case 'secure':
			return { lock, words: m.secureSite, label: m.secureSite };
		case 'local':
			return { lock, words: m.localSite, label: m.a11yLocal };
		case 'insecure':
			return { lock, words: m.a11yInsecure, label: m.a11yInsecure };
	}
}
