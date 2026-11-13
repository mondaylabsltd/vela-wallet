/**
 * "Has this person been introduced yet?" (spec 020).
 *
 * The intro is a FIRST-RUN screen, not a recurring one: it plays once and never
 * again (founder direction, 2026-09-01 — "不然就很烦"). That makes it a
 * different rule from the launch animation next door, which replays after a
 * week, so the two gates stay separate flags rather than one shared "first run"
 * that would have to compromise between them.
 *
 * Storage is best-effort in exactly the way `$lib/launch/constants` is: private
 * modes and sandboxed frames throw on `localStorage`, and a wallet's front door
 * must not break because a decoration could not read a flag. When we cannot
 * tell, we show it — a second viewing is a cost; a broken first run is not.
 */

/** Where the "already seen it" flag lives. */
export const STORAGE_KEY = 'vela.intro.seen';

/** Deterministic disable, for e2e runs that want the Welcome page directly. */
export const SKIP_PARAM = 'skipIntro';

/** Deterministic ENABLE, so the intro can be looked at without clearing storage. */
export const FORCE_PARAM = 'intro';

export function shouldShowIntro(win: Window = window): boolean {
	try {
		const params = new URLSearchParams(win.location.search);
		if (params.has(FORCE_PARAM)) return true;
		if (params.has(SKIP_PARAM)) return false;
		return win.localStorage.getItem(STORAGE_KEY) === null;
	} catch {
		return true;
	}
}

export function markIntroSeen(win: Window = window, now: number = Date.now()): void {
	try {
		win.localStorage.setItem(STORAGE_KEY, String(now));
	} catch {
		// Unwritable storage means it may show again — cosmetic, not a failure.
	}
}

/** Where the signed-in wallets live (`$lib/onboarding/core/storage` STORAGE_KEYS.accounts). */
export const ACCOUNTS_KEY = 'vela.accounts';

/** Welcome opened on purpose: to sign into ANOTHER wallet, or sent here by a session that holds none. */
export const STAY_PARAM = 'stay';

/**
 * Welcome opened to sign in (vis-22): the way-in sheet is up on arrival. What
 * the retired `/import` placeholder forwards to, so an old link or a typed
 * address lands on the real thing.
 */
export const SIGN_IN_PARAM = 'signin';

/** {@link STAY_PARAM} as the query a route is resolved with. */
export const STAY_QUERY = `?${STAY_PARAM}` as const;

/**
 * Should Welcome hand this visit to the wallet (ob-6)? A signed-in person,
 * unless they came to add a wallet or asked for the intro by name.
 */
export function welcomeBelongsInWallet(win: Window = window): boolean {
	try {
		const params = new URLSearchParams(win.location.search);
		if (params.has(STAY_PARAM) || params.has(FORCE_PARAM)) return false;
	} catch {
		return false;
	}
	return hasSignedInWallet(win);
}

/**
 * Is somebody signed in on this browser (ob-6)? Welcome is the landing page
 * for everybody else; a person with a wallet who opens `/` or `/en` — or
 * presses Back into it — belongs in the wallet. Read straight from storage,
 * because this page stays wasm-free: any stored account list that is not
 * empty counts (the session core validates it once the wallet boots).
 */
export function hasSignedInWallet(win: Window = window): boolean {
	try {
		const raw = win.localStorage.getItem(ACCOUNTS_KEY);
		if (!raw) return false;
		const parsed: unknown = JSON.parse(raw);
		return Array.isArray(parsed) && parsed.length > 0;
	} catch {
		return false;
	}
}
