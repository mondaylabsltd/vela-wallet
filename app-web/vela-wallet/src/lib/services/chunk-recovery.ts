/**
 * A lazy chunk failed to load: what the app does about it.
 *
 * Vite turns a dynamic import that could not be fetched into a
 * `vite:preloadError` event on the window. The layout used to answer every
 * one with `location.reload()`, on the reasoning that "a deploy moved the
 * hashes" is the only way that happens. It is not, and the other way is far
 * more common:
 *
 * **A page that is being LEFT has its requests cancelled.** A reload, a link
 * to another document, the address bar — the browser tears down what is in
 * flight, and this app usually has lazy chunks in flight (it warms the other
 * tabs' routes while the first one is read). In Firefox each cancelled import
 * rejects while the old document is still running, Vite reports it, and the
 * handler issued a NEW navigation from a document that was already
 * navigating — which cancels the one under way. Measured on the preview
 * build: `beforeunload`, then four `vite:preloadError`s 19 ms later
 * ("error loading dynamically imported module …", "Unable to preload CSS …"),
 * each answered with a reload; the navigation that had been asked for ended
 * `NS_BINDING_ABORTED`. A person clicking a link out of the wallet while it
 * was still loading got the wallet again. (It is also why three e2e suites
 * failed on Firefox, a different test each run.)
 *
 * So a failed import means one of three things, and only one of them is
 * helped by a reload:
 *
 * - **the document is being left** — nothing to recover: do nothing;
 * - **the network dropped it** (offline, a flaky link) — the build is still
 *   there. A reload would throw away whatever the person was doing — a
 *   half-typed send, a request on the signing sheet — to show a browser error
 *   page. Do nothing: the import's own rejection goes to whoever asked;
 * - **the build moved** (a deploy deleted the chunk) — the one case a fresh
 *   document fixes. It is ASKED, not assumed: SvelteKit's version check reads
 *   the deployed `version.json`.
 *
 * And a recovery reload must not become a loop. If the fresh document still
 * cannot load its chunks (a cached shell in front of a moved build), reloading
 * again fixes nothing, for ever: so a tab reloads away from a given build at
 * most ONCE, remembered in its session storage. With no storage there is no
 * memory, and so no automatic reload at all — a loop is worse than an error.
 *
 * It also waits for a sheet to close: a reload under a signing request would
 * answer the page that asked with nothing. The version check has by then told
 * SvelteKit the build moved, so the next navigation is a full document load
 * (`beforeNavigate` in the layout) — that is the recovery, a moment later.
 */

/** Everything the decision reads. */
export interface PreloadFailureFacts {
	/** The document is being left: a navigation has been announced. */
	leaving: boolean;
	/** The server says a different build is deployed than this document's. */
	moved: boolean;
	/** This document's build. */
	version: string;
	/**
	 * The build this tab last reloaded away from: `null` — it never has;
	 * `undefined` — this tab has no memory to ask (storage refused).
	 */
	recoveredFrom: string | null | undefined;
	/** A sheet is open over the page: something a reload would drop. */
	busy: boolean;
}

/**
 * Whether a fresh document is the answer to this failed import: only when
 * nobody is leaving, the build really moved, nothing is open that a reload
 * would drop, and this tab has not already reloaded away from this very build.
 */
export function reloadRecovers(facts: PreloadFailureFacts): boolean {
	if (facts.leaving || !facts.moved || facts.busy) return false;
	if (facts.recoveredFrom === undefined) return false;
	return facts.recoveredFrom !== facts.version;
}

/** Where a tab remembers the build it reloaded away from. */
export const RECOVERY_KEY = 'vela.chunkRecovery';

export interface ChunkRecoveryPorts {
	/** This document's build (`version` of `$app/environment`). */
	version: string;
	/** Ask the server whether another build is deployed (`updated.check()`). */
	moved: () => Promise<boolean>;
	/** A sheet is open over the page. */
	busy: () => boolean;
	reload: () => void;
	/** The window, as a seam. */
	target?: Pick<Window, 'addEventListener' | 'removeEventListener'>;
	/** The tab's memory (session storage), `null` when it is refused. */
	memory?: Pick<Storage, 'getItem' | 'setItem'> | null;
}

/** The tab's session storage, or `null` where a browser refuses it. */
function sessionMemory(): Pick<Storage, 'getItem' | 'setItem'> | null {
	try {
		return globalThis.sessionStorage ?? null;
	} catch {
		return null;
	}
}

/**
 * Listen for failed lazy imports and recover from the one kind a reload
 * fixes ({@link reloadRecovers}). Returns the way to stop listening.
 *
 * "Leaving" is what the browser announces: `beforeunload` and `pagehide`. An
 * announced unload that did not happen — a download, a `mailto:` link, a
 * leave-prompt answered "stay" — is over as soon as the person does anything
 * on the page, or the page is shown again.
 */
export function installChunkRecovery(ports: ChunkRecoveryPorts): () => void {
	const target = ports.target ?? window;
	const memory = ports.memory === undefined ? sessionMemory() : ports.memory;
	let leaving = false;
	let asking = false;

	const left = () => (leaving = true);
	const here = () => (leaving = false);
	const recoveredFrom = (): string | null | undefined => {
		if (memory === null) return undefined;
		try {
			return memory.getItem(RECOVERY_KEY);
		} catch {
			return undefined;
		}
	};

	const failed = async () => {
		// Leaving, or already asking about an earlier one of the same burst (a
		// route's chunks fail together): one question, one answer.
		if (leaving || asking) return;
		asking = true;
		try {
			const moved = await ports.moved().catch(() => false);
			const facts = {
				leaving,
				moved,
				version: ports.version,
				recoveredFrom: recoveredFrom(),
				busy: ports.busy()
			};
			if (!reloadRecovers(facts)) return;
			try {
				// Remembered BEFORE the reload: what comes back must know.
				memory?.setItem(RECOVERY_KEY, ports.version);
			} catch {
				// It could not be remembered: reloading now could loop.
				return;
			}
			ports.reload();
		} finally {
			asking = false;
		}
	};
	const onFailed = () => void failed();

	target.addEventListener('vite:preloadError', onFailed);
	target.addEventListener('beforeunload', left);
	target.addEventListener('pagehide', left);
	target.addEventListener('pageshow', here);
	target.addEventListener('pointerdown', here, true);
	target.addEventListener('keydown', here, true);
	return () => {
		target.removeEventListener('vite:preloadError', onFailed);
		target.removeEventListener('beforeunload', left);
		target.removeEventListener('pagehide', left);
		target.removeEventListener('pageshow', here);
		target.removeEventListener('pointerdown', here, true);
		target.removeEventListener('keydown', here, true);
	};
}
