/**
 * A signature handed to the account's trusted signing page — the web wallet's
 * and the Chrome extension's one way there (owner, 2026-10-30: the page is
 * usable everywhere, superseding "the web opens no signing page").
 *
 * Every hand-off is the same four steps, whatever is signed (a transaction, a
 * batch, the wallet's own send, a message — `sign-challenge.ts`; a multisig
 * member's approval — `multisig/core/page-approval.svelte.ts`):
 *
 * 1. the page is checked as every Vela app checks it (spec 076/102 R6): the
 *    core picks the version (`trustedSignerPageTarget`), this fetches exactly
 *    that URL as a browser navigates to it, hashes the bytes, and the core
 *    rules (`trustedSignerPageAdmit`). A page that does not match is not
 *    opened, and the person reads why (the core's integrity line);
 * 2. the hand-off card: Open opens the checked version in a window of its own
 *    (a tap — a browser opens a window only for one), and once the page says
 *    it is ready, hands it the core's request over `postMessage`. §38
 *    (review 0-5): where the core says this wallet's origin may
 *    (`fromBytes`, `CheckedPage::opens_from_bytes` — the official page, from
 *    `wallet.getvela.app`), the window opens THE BYTES THE CHECK HASHED, as a
 *    `blob:` URL, never the address: a host that answered the check with one
 *    page cannot answer the window with another, because the window never
 *    asks it. Elsewhere (an extension, a self-hosted page's own domain) it
 *    opens the checked address, and only the page's Service Worker pin stands
 *    between the check and the open;
 * 3. the answer is accepted only as the core accepts it (the caller's
 *    `accept`: `trustedSignerSignatureAccept` / `trustedSignerApprovalAccept`)
 *    — over exactly the bytes the core computed, by the account's key, checked
 *    as the chain will. Nothing the page says about what it signed is read;
 * 4. while the page has the request, the asker is kept (`keepAlive`, the
 *    extension's request claim): the request lives as long as the page is
 *    open and the person may still answer. A request that is gone closes the
 *    page.
 *
 * Messages come only from the window this opened, from the page's own origin
 * (`event.source`, `event.origin` — the browser's facts), and go only there.
 * The page, for its part, answers only Vela's own wallet origins — the web
 * wallet and the published extension, as the browser reports the opener
 * (`resolve.answersToWallet`, PROTOCOL.md §12.3).
 */
import type { IntegrityLine } from '$lib/core/generated/IntegrityLine';
import type { KeyLabel } from '$lib/core/generated/KeyLabel';
import { trustedSignerPageAdmit, trustedSignerPageTarget } from '$lib/core/kernels';
import { safariSignerDeps } from '$lib/extension/safari-signer';
import { handoffPage } from '$lib/dev/harness-page';

/** What a browser sends when it navigates to a page (`launch::CHECK_HEADERS`). */
const CHECK_ACCEPT = 'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8';

/** How long the page has to say it is ready once opened. */
export const READY_MS = 30_000;

/**
 * How long the page's check may take (round 7, r7-2): the index and the
 * page's bytes, fetched as a browser navigates to them. A host that answers
 * slowly — or never — held "Checking the page…" for as long as it liked (75 s
 * in the round-7 test, with no end in sight) while the asking site waited.
 * Past this the hand-off ends as a page that is not answering, said as that
 * ("The signing page isn't answering — try again"), and the site is answered.
 */
export const CHECK_MS = 20_000;

/**
 * How often the asker is told the request is still being answered while the
 * page has it — well inside the extension's five-minute request window
 * (spec 082 `REQUEST_TTL_MS`), so a person reading a long batch on the page
 * never has the request expire under them.
 */
export const KEEP_ALIVE_MS = 60_000;

/** Why a signature on the page did not come back. */
export type PageHandoffFailure =
	/** The page's check did not pass: it was not opened (`line` says why). */
	| 'not_opened'
	/** The person closed the page, or the hand-off, without signing. */
	| 'closed'
	/** The page answered no — the person declined there (`user_rejected`), or its rules refused (`refused`, `code`). */
	| 'declined'
	/** The page answered, and the answer is not the account's signature of the core's bytes. */
	| 'unverified'
	/** The page never said it was ready, or its check never finished in time (`CHECK_MS`). */
	| 'silent'
	/** The request is gone (the asking page left, its window ran out): nothing more is asked. */
	| 'gone';

export class PageHandoffError extends Error {
	constructor(
		readonly failure: PageHandoffFailure,
		readonly line: IntegrityLine | null = null,
		/** The page's own code for a `declined` (`user_rejected` | `refused`), or the core's refusal for `unverified`. */
		readonly code = ''
	) {
		super(`the signing page did not sign: ${failure}${code ? ` (${code})` : ''}`);
		this.name = 'PageHandoffError';
	}

	/** The person's own "no": declined on the page, or the page closed unsigned. */
	get byPerson(): boolean {
		return this.failure === 'closed' || (this.failure === 'declined' && this.code !== 'refused');
	}
}

/**
 * What is handed to the page: a member's approval, any other signature, or a
 * key ceremony — creating a key, signing in, a proof (spec 102 R3, R8: a
 * custom-domain wallet's, and every one in the Safari package, whose pages
 * cannot use a `getvela.app` passkey).
 */
export type HandoffKind = 'approval' | 'signature' | 'ceremony';

/** The hand-off card, as the hosts draw it. */
export interface PageHandoff {
	/**
	 * Which host draws it: the multisig's (`approval`), the signing sheet's
	 * (`signature`), or onboarding's (`ceremony`).
	 */
	kind: HandoffKind;
	/**
	 * A ceremony's own title, by its corpus key (`trustedSignerCeremonyCard`):
	 * "Create a key on the signing page" — there is nothing to review.
	 * `null`: the signature card's "Review and sign on a trusted signing page".
	 */
	titleKey: string | null;
	/** `checking` while the page is checked; `ready` to open; `waiting` once open; `refused` when it may not open. */
	phase: 'checking' | 'ready' | 'waiting' | 'refused';
	/** The page's host, and whether it is Vela's official page. */
	host: string;
	official: boolean;
	/** The account signing (its name on this device). */
	member: string;
	/** The key row: which key confirms it (`SigningPlan.key_label`). */
	keyLabel: KeyLabel | null;
	/** The core's integrity line, once the check ran. */
	line: IntegrityLine | null;
	/** Open the checked page (a tap); again, to bring it back. */
	open: () => void;
	/** Give up: nothing is signed. */
	cancel: () => void;
}

class Handoffs {
	current = $state<PageHandoff | null>(null);
	/**
	 * How many hosts that draw each kind are mounted: a route with no signing
	 * host (the contacts page) still draws a multisig executor's signature —
	 * its multisig host stands in.
	 */
	drawers = $state<Record<HandoffKind, number>>({ approval: 0, signature: 0, ceremony: 0 });
	/**
	 * R22 (spec 102 D4): the card's Open IS the confirm that raised it — the
	 * dApp sheet's, a multisig approve or execute — and follows its posture:
	 * the danger style for an "… anyway" or a danger, held for the core's
	 * `hold_ms`. Set by that confirm as it is pressed; `null`: a plain Open.
	 */
	posture = $state<HandoffPosture | null>(null);
}

/** R22: how hard the confirm behind a hand-off is (`ConfirmState`, `SignDoor`). */
export interface HandoffPosture {
	danger: boolean;
	holdMs: number;
	holdLabel: string;
	againLabel: string;
}

/** The hand-off on screen, if any. */
export const pageHandoff = new Handoffs();

/** What the browser and the clock give, so a test can stand in for both. */
export interface PageHandoffDeps {
	fetch: typeof fetch;
	open: (url: string) => Window | null;
	now: () => number;
	lang: () => string;
	listen: (handler: (event: MessageEvent) => void) => () => void;
	sha256: (bytes: ArrayBuffer) => Promise<string>;
	/**
	 * This wallet's own origin, as the browser reports it — the opener the
	 * core asks about (`fromBytes`). Absent: the page always opens at its
	 * address.
	 */
	origin?: () => string;
	/** An object URL for the checked bytes, as an HTML document (§38). */
	objectUrl?: (bytes: ArrayBuffer) => string;
	/**
	 * Let an object URL go when the hand-off ends. Kept until then, so a
	 * reload of the window runs the same checked bytes — never the address.
	 */
	revokeUrl?: (url: string) => void;
	/** Timers, so a test can run the keep-alive and the ready wait on its own clock. */
	setInterval?: (fn: () => void, ms: number) => unknown;
	clearInterval?: (id: unknown) => void;
	/** The check's deadline (`CHECK_MS`), on the test's clock. */
	setTimeout?: (fn: () => void, ms: number) => unknown;
	clearTimeout?: (id: unknown) => void;
}

async function sha256Hex(bytes: ArrayBuffer): Promise<string> {
	const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', bytes));
	return Array.from(digest, (b) => b.toString(16).padStart(2, '0')).join('');
}

const windowDeps: PageHandoffDeps = {
	fetch: (input, init) => fetch(input, init),
	// The page must know which window opened it (`window.opener`): no `noopener`.
	open: (url) => window.open(url, 'vela-signing-page'),
	now: () => Date.now(),
	lang: () => (typeof document === 'undefined' ? '' : document.documentElement.lang),
	listen: (handler) => {
		window.addEventListener('message', handler);
		return () => window.removeEventListener('message', handler);
	},
	sha256: sha256Hex,
	origin: () => window.location.origin,
	objectUrl: (bytes) => URL.createObjectURL(new Blob([bytes], { type: 'text/html' })),
	revokeUrl: (url) => URL.revokeObjectURL(url)
};

/**
 * The Safari package carries the hand-off through a tab and Vela's content
 * script on the page (`$lib/extension/safari-signer.ts`): iOS Safari gives a
 * page an extension opens no `window.opener`. Every other build: windows.
 */
export const browserDeps: PageHandoffDeps =
	__VELA_EXTENSION_PROTOCOL__ === 'safari-web-extension:'
		? safariSignerDeps(windowDeps)
		: windowDeps;

function originOf(url: string): string {
	try {
		return new URL(url).origin;
	} catch {
		return '';
	}
}

function hostOf(url: string): string {
	try {
		return new URL(url).host;
	} catch {
		return url;
	}
}

/** A checked page, as the hand-off opens it. */
export interface CheckedPage {
	line: IntegrityLine;
	/** The checked address, handed the request by `postMessage` — `null`: not admitted. */
	launch: string | null;
	/**
	 * §38: the bytes the check hashed and the fragment to open them with, when
	 * the core says this wallet opens them rather than the address.
	 */
	bytes: { body: ArrayBuffer; fragment: string } | null;
}

/**
 * R6, the web's way: the deployment's index, the version the core picks, the
 * bytes that version serves — fetched as a browser navigates — and the core's
 * ruling on them. The page must answer the wallet's origin's request
 * (`Access-Control-Allow-Origin`); one that does not is a check that could
 * not complete, and nothing opens.
 *
 * The bytes are kept (§38): when the core rules that this wallet's origin
 * opens this version from its bytes (`fromBytes`), they are what the window
 * opens.
 */
export async function checkPage(
	base: string,
	deps: PageHandoffDeps = browserDeps
): Promise<CheckedPage> {
	const root = base.endsWith('/') ? base : `${base}/`;
	const index = await deps
		.fetch(`${root}index.json`, { cache: 'no-store', credentials: 'omit' })
		.then((r) => (r.ok ? r.text() : null))
		.catch(() => null);
	let observed: string | null = null;
	let body: ArrayBuffer | null = null;
	try {
		const target = trustedSignerPageTarget(root, index);
		const response = await deps.fetch(target.url, {
			headers: { Accept: CHECK_ACCEPT },
			cache: 'no-store',
			credentials: 'omit'
		});
		if (response.ok) {
			body = await response.arrayBuffer();
			observed = await deps.sha256(body);
		}
	} catch {
		observed = null;
		body = null;
	}
	const opener = deps.origin && deps.objectUrl ? deps.origin() : null;
	const ruling = trustedSignerPageAdmit(root, index, observed, deps.now(), deps.lang(), [], opener);
	return {
		line: ruling.line,
		launch: ruling.launch,
		bytes:
			ruling.launch !== null && ruling.fromBytes !== null && body !== null
				? { body, fragment: ruling.fromBytes }
				: null
	};
}

export interface HandoffInput<T> {
	kind: HandoffKind;
	/** The page the account reviews and signs on (its base address — `SigningRoute.url`). */
	page: string;
	/** The account signing, by its name on this device. */
	signer: string;
	/** Which key confirms it, for the card's key row (`SigningPlan.key_label`). */
	keyLabel?: KeyLabel | null;
	/** A ceremony's own card title, by its corpus key. */
	titleKey?: string | null;
	/** The core's `{intent, context}`. */
	request: { intent: unknown; context: unknown };
	/** The answer as the core accepts it — throws (its code as the message) when it is not. */
	accept: (result: unknown) => T;
	/** Withdrawn: nothing more is asked. */
	signal?: AbortSignal;
	/**
	 * Tell the asker the request is still being answered (the extension's
	 * `claim('sign')`); `false` — the request is gone, the page is closed.
	 * Absent: nobody to keep (the wallet's own send, a multisig).
	 */
	keepAlive?: () => Promise<boolean>;
}

/**
 * Hand `input.request` to the page and bring back what `input.accept` made of
 * its answer — or a {@link PageHandoffError} saying why not.
 */
export async function handOff<T>(
	input: HandoffInput<T>,
	deps: PageHandoffDeps = browserDeps
): Promise<T> {
	const every = deps.setInterval ?? ((fn: () => void, ms: number) => setInterval(fn, ms));
	const stop =
		deps.clearInterval ?? ((id: unknown) => clearInterval(id as ReturnType<typeof setInterval>));
	// The development package's loopback page stands in for the production
	// one (`$lib/dev/harness-page.ts`); the release package never does.
	const page = handoffPage(input.page);
	const official = /(^|\.)getvela\.app$/.test(hostOf(page).split(':')[0] ?? '');
	const base: Omit<PageHandoff, 'phase' | 'line' | 'open' | 'cancel'> = {
		kind: input.kind,
		host: hostOf(page),
		official,
		member: input.signer,
		keyLabel: input.keyLabel ?? null,
		titleKey: input.titleKey ?? null
	};
	let settle: { resolve: (a: T) => void; reject: (e: unknown) => void } | null = null;
	const done = new Promise<T>((resolve, reject) => {
		settle = { resolve, reject };
	});
	let win: Window | null = null;
	let stopListening: (() => void) | null = null;
	let watch: unknown = null;
	let alive: unknown = null;
	let readyAt: number | null = null;
	let finished = false;
	// The object URL of a page opened from its bytes, let go when the hand-off ends.
	let revokeOnFinish = () => {};

	const closeWindow = () => {
		try {
			win?.close();
		} catch {
			/* gone already */
		}
	};
	const finish = (error: unknown, value?: { ok: T }) => {
		if (finished) return;
		finished = true;
		revokeOnFinish();
		stopListening?.();
		if (watch !== null) stop(watch);
		if (alive !== null) stop(alive);
		pageHandoff.current = null;
		if (value) settle?.resolve(value.ok);
		else settle?.reject(error);
	};
	const cancel = () => {
		closeWindow();
		finish(new PageHandoffError('closed'));
	};
	// r7-5: the wallet gives up on the request (no answer in time, the asker
	// gone, an answer it would not take): an open page is told, so it stops
	// offering to sign what nobody waits for — never a "Confirm" that signs
	// into nothing.
	const withdraw = (error: PageHandoffError) => {
		if (finished) return;
		try {
			if (win && !win.closed)
				win.postMessage({ vela: 'bye', v: 1, reason: 'withdrawn' }, pageOrigin);
		} catch {
			/* gone already */
		}
		finish(error);
	};
	if (input.signal?.aborted) {
		cancel();
		return done;
	}
	input.signal?.addEventListener('abort', cancel, { once: true });

	pageHandoff.current = { ...base, phase: 'checking', line: null, open: () => {}, cancel };
	// r7-2: the check is bounded. A page that has not been checked by then is
	// a page that is not answering, and the asker hears so.
	const later = deps.setTimeout ?? ((fn: () => void, ms: number) => setTimeout(fn, ms));
	const unlater =
		deps.clearTimeout ?? ((id: unknown) => clearTimeout(id as ReturnType<typeof setTimeout>));
	let deadline: unknown = null;
	const checked = await Promise.race([
		checkPage(page, deps),
		new Promise<null>((resolve) => {
			deadline = later(() => resolve(null), CHECK_MS);
		})
	]);
	unlater(deadline);
	if (finished) return done;
	if (checked === null) {
		finish(new PageHandoffError('silent'));
		return done;
	}
	if (checked.launch === null) {
		const line = checked.line;
		pageHandoff.current = {
			...base,
			phase: 'refused',
			line,
			open: () => {},
			cancel: () => finish(new PageHandoffError('not_opened', line))
		};
		return done;
	}
	const launch = checked.launch;
	// §38: opened from the bytes the check hashed, the page is a blob document
	// of THIS origin, and its messages come from here; opened at its address,
	// from the page's.
	const fromBytes = checked.bytes;
	const pageOrigin = fromBytes && deps.origin ? deps.origin() : originOf(launch);
	let objectUrl: string | null = null;
	const revoke = () => {
		if (objectUrl === null) return;
		const url = objectUrl;
		objectUrl = null;
		try {
			deps.revokeUrl?.(url);
		} catch {
			/* gone already */
		}
	};
	revokeOnFinish = revoke;
	const id = `vs-${deps.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
	let handed = false;

	const onMessage = (event: MessageEvent) => {
		if (!win || event.source !== win || event.origin !== pageOrigin) return;
		const data = event.data as {
			vela?: string;
			id?: string;
			result?: unknown;
			code?: string;
		} | null;
		if (!data || typeof data !== 'object') return;
		if (data.vela === 'ready') {
			readyAt = null;
			if (handed) return;
			handed = true;
			win.postMessage({ vela: 'intent', v: 1, id, ...input.request }, pageOrigin);
			return;
		}
		if (data.id !== id) {
			if (data.vela === 'bye') finish(new PageHandoffError('closed'));
			return;
		}
		if (data.vela === 'error') {
			finish(new PageHandoffError('declined', null, data.code ?? 'user_rejected'));
			return;
		}
		if (data.vela === 'result') {
			let accepted: T;
			try {
				// A signature answers `{result: assertion}`; a key ceremony its
				// own fields (`{registration | assertion, origin}`, PROTOCOL.md
				// §10.3), which the core reads whole.
				accepted = input.accept(input.kind === 'ceremony' ? data : data.result);
			} catch (error) {
				withdraw(
					new PageHandoffError(
						'unverified',
						null,
						error instanceof Error ? error.message : String(error)
					)
				);
				return;
			}
			win.postMessage({ vela: 'bye', v: 1, reason: 'done' }, pageOrigin);
			finish(null, { ok: accepted });
			// The page has nothing left to do: a finished tab is not left
			// behind (each extension signature used to leave one open).
			closeWindow();
		}
	};

	const open = () => {
		if (finished) return;
		if (win && !win.closed) {
			win.focus();
			return;
		}
		// Inside the tap: a browser opens a window only for one. The bytes the
		// check hashed, when the core says so — a fresh object URL each time,
		// from the same bytes; else the checked address.
		revoke();
		if (fromBytes && deps.objectUrl) {
			objectUrl = deps.objectUrl(fromBytes.body);
			win = deps.open(objectUrl + fromBytes.fragment);
		} else {
			win = deps.open(launch);
		}
		if (!win) {
			revoke();
			return;
		}
		handed = false;
		stopListening?.();
		stopListening = deps.listen(onMessage);
		readyAt = deps.now();
		if (watch !== null) stop(watch);
		watch = every(() => {
			let closed: boolean;
			try {
				closed = !win || win.closed;
			} catch {
				closed = true;
			}
			if (closed) finish(new PageHandoffError('closed'));
			else if (readyAt !== null && deps.now() - readyAt >= READY_MS) {
				closeWindow();
				finish(new PageHandoffError('silent'));
			}
		}, 500);
		pageHandoff.current = { ...base, phase: 'waiting', line: checked.line, open, cancel };
	};
	if (input.keepAlive) {
		const keepAlive = input.keepAlive;
		alive = every(() => {
			void keepAlive().then(
				(live) => {
					if (live || finished) return;
					withdraw(new PageHandoffError('gone'));
					closeWindow();
				},
				() => {}
			);
		}, KEEP_ALIVE_MS);
	}
	pageHandoff.current = { ...base, phase: 'ready', line: checked.line, open, cancel };
	return done;
}
