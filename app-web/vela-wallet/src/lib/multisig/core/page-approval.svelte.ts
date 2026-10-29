/**
 * A multisig member's approval on the member's trusted signing page
 * (2026-10-29, the owner's Bybit lesson: a signer must be able to review and
 * sign on a surface independent of the wallet app's code).
 *
 * When the member's account reviews and signs on a page
 * (`multisigApprovalRoute` — the venue the person chose), the approval is not
 * the wallet's passkey prompt. The wallet:
 *
 * 1. checks the page as every Vela app does (spec 076/102 R6: the core picks
 *    the version — `trustedSignerPageTarget` — this fetches exactly that URL
 *    as a browser navigates to it, hashes the bytes, and the core rules —
 *    `trustedSignerPageAdmit`). A page that does not match is not opened, and
 *    the person reads why (the core's integrity line);
 * 2. shows the hand-off card; Open opens the checked version in a window of
 *    its own (a tap: a browser opens a window only for one), and once the page
 *    says it is ready, hands it the request over `postMessage` — the raw
 *    SafeTx, the multisig, the chain, the member, and the core's safeTxHash
 *    for the page to compare (`trustedSignerApprovalRequest`);
 * 3. accepts the page's answer only as the core accepts it
 *    (`trustedSignerApprovalAccept`): a user-verified assertion over exactly
 *    the approval bytes the core computed, by one of the member's keys, that
 *    verifies the way the chain will. Nothing the page says about what it
 *    signed is read.
 *
 * The page decodes the SafeTx itself, from its fields: what the person
 * approves there does not depend on this file being honest about it.
 *
 * Messages come only from the window this opened, from the page's own origin
 * (`event.source`, `event.origin` — the browser's facts), and go only there.
 */
import type { Account } from '$lib/core/generated/Account';
import type { IntegrityLine } from '$lib/core/generated/IntegrityLine';
import type { KeyLabel } from '$lib/core/generated/KeyLabel';
import {
	trustedSignerApprovalAccept,
	trustedSignerApprovalRequest,
	trustedSignerPageAdmit,
	trustedSignerPageTarget
} from '$lib/core/kernels';
import { chainName, nativeSymbol } from '$lib/services/networks';
import type { PasskeyApproval, SafeTx } from './kernel';

/** What a browser sends when it navigates to a page (`launch::CHECK_HEADERS`). */
const CHECK_ACCEPT = 'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8';

/** How long the page has to say it is ready once opened. */
const READY_MS = 30_000;

/** Why an approval on the page did not come back. */
export type PageApprovalFailure =
	/** The page's check did not pass: it was not opened (`line` says why). */
	| 'not_opened'
	/** The person closed the page, or the hand-off, without approving. */
	| 'closed'
	/** The page refused to sign (it said why, on its own card). */
	| 'declined'
	/** The page answered, and the answer is not the member's approval. */
	| 'unverified';

export class PageApprovalError extends Error {
	constructor(
		readonly failure: PageApprovalFailure,
		readonly line: IntegrityLine | null = null,
		detail = ''
	) {
		super(`the signing page did not approve: ${failure}${detail ? ` (${detail})` : ''}`);
		this.name = 'PageApprovalError';
	}
}

/** The hand-off card, as the multisig host draws it. */
export interface PageHandoff {
	/** `checking` while the page is checked; `ready` to open; `waiting` once open; `refused` when it may not open. */
	phase: 'checking' | 'ready' | 'waiting' | 'refused';
	/** The page's host, and whether it is Vela's official page. */
	host: string;
	official: boolean;
	/** The member approving (its name on this device). */
	member: string;
	/** The key row: which key confirms it (`SigningPlan.key_label`). */
	keyLabel: KeyLabel | null;
	/** The core's integrity line, once the check ran. */
	line: IntegrityLine | null;
	/** Open the checked page (a tap); again, to bring it back. */
	open: () => void;
	/** Give up: nothing is approved. */
	cancel: () => void;
}

class Handoffs {
	current = $state<PageHandoff | null>(null);
}

/** The hand-off on screen, if any. */
export const pageHandoff = new Handoffs();

/** What the browser and the clock give, so a test can stand in for both. */
export interface PageApprovalDeps {
	fetch: typeof fetch;
	open: (url: string) => Window | null;
	now: () => number;
	lang: () => string;
	listen: (handler: (event: MessageEvent) => void) => () => void;
	sha256: (bytes: ArrayBuffer) => Promise<string>;
}

async function sha256Hex(bytes: ArrayBuffer): Promise<string> {
	const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', bytes));
	return Array.from(digest, (b) => b.toString(16).padStart(2, '0')).join('');
}

export const browserDeps: PageApprovalDeps = {
	fetch: (input, init) => fetch(input, init),
	// The page must know which window opened it (`window.opener`): no `noopener`.
	open: (url) => window.open(url, 'vela-signing-page'),
	now: () => Date.now(),
	lang: () => (typeof document === 'undefined' ? '' : document.documentElement.lang),
	listen: (handler) => {
		window.addEventListener('message', handler);
		return () => window.removeEventListener('message', handler);
	},
	sha256: sha256Hex
};

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

/**
 * R6, the web's way: the deployment's index, the version the core picks, the
 * bytes that version serves — fetched as a browser navigates — and the core's
 * ruling on them. The page must answer the wallet's origin's request
 * (`Access-Control-Allow-Origin`); one that does not is a check that could
 * not complete, and nothing opens.
 */
export async function checkPage(
	base: string,
	deps: PageApprovalDeps = browserDeps
): Promise<{ line: IntegrityLine; launch: string | null }> {
	const root = base.endsWith('/') ? base : `${base}/`;
	const index = await deps
		.fetch(`${root}index.json`, { cache: 'no-store', credentials: 'omit' })
		.then((r) => (r.ok ? r.text() : null))
		.catch(() => null);
	let observed: string | null = null;
	try {
		const target = trustedSignerPageTarget(root, index);
		const response = await deps.fetch(target.url, {
			headers: { Accept: CHECK_ACCEPT },
			cache: 'no-store',
			credentials: 'omit'
		});
		if (response.ok) observed = await deps.sha256(await response.arrayBuffer());
	} catch {
		observed = null;
	}
	return trustedSignerPageAdmit(root, index, observed, deps.now(), deps.lang());
}

export interface PageApprovalInput {
	/** The member's record as stored. */
	member: Account;
	/** The page the member reviews and signs on (its base address). */
	page: string;
	chainId: number;
	multisig: string;
	/** The multisig's name on this device: the page marks it "named by the app". */
	multisigName?: string | null;
	tx: SafeTx;
	/** The safeTxHash the core computed: what the approval is FOR. */
	safeTxHash: string;
	/** The member's founding keys. */
	keys: { credentialId: string; publicKeyHex: string }[];
	/** The member's key route (the core's `SigningPlan.key`), when the record names one. */
	keyRoute?: unknown;
	/** Which key confirms it, for the card's key row (`SigningPlan.key_label`). */
	keyLabel?: KeyLabel | null;
	/** The withdrawn proposal: nothing more is asked. */
	signal?: AbortSignal;
}

/**
 * The member approves `safeTxHash` on its page: the check, the card, the page,
 * the answer — and back, the approval the core accepted.
 */
/**
 * What a page answered, for this page's life, by member, chain and
 * safeTxHash: the same bytes approved twice are one approval, and the person
 * is not asked again (an execution that approves first and then stops before
 * its own passkey falls back to approving — it already has).
 */
// Not reactive state: nothing draws it, it is only looked up.
// eslint-disable-next-line svelte/prefer-svelte-reactivity
const answered = new Map<string, PasskeyApproval>();
const answerKey = (input: Pick<PageApprovalInput, 'member' | 'chainId' | 'safeTxHash'>) =>
	`${input.member.address.toLowerCase()}:${input.chainId}:${input.safeTxHash.toLowerCase()}`;

export async function approveOnPage(
	input: PageApprovalInput,
	deps: PageApprovalDeps = browserDeps
): Promise<PasskeyApproval> {
	const kept = answered.get(answerKey(input));
	if (kept) return kept;
	const approval = await handOff(input, deps);
	answered.set(answerKey(input), approval);
	return approval;
}

/** Forget what the pages answered (tests). */
export function forgetPageAnswers(): void {
	answered.clear();
}

async function handOff(input: PageApprovalInput, deps: PageApprovalDeps): Promise<PasskeyApproval> {
	const official = /(^|\.)getvela\.app$/.test(hostOf(input.page).split(':')[0] ?? '');
	const base: Omit<PageHandoff, 'phase' | 'line' | 'open' | 'cancel'> = {
		host: hostOf(input.page),
		official,
		member: input.member.name,
		keyLabel: input.keyLabel ?? null
	};
	let settle: { resolve: (a: PasskeyApproval) => void; reject: (e: unknown) => void } | null = null;
	const done = new Promise<PasskeyApproval>((resolve, reject) => {
		settle = { resolve, reject };
	});
	let win: Window | null = null;
	let stopListening: (() => void) | null = null;
	let watch: ReturnType<typeof setInterval> | null = null;
	let readyTimer: ReturnType<typeof setTimeout> | null = null;
	let finished = false;

	const finish = (error: unknown, approval?: PasskeyApproval) => {
		if (finished) return;
		finished = true;
		stopListening?.();
		if (watch) clearInterval(watch);
		if (readyTimer) clearTimeout(readyTimer);
		pageHandoff.current = null;
		if (approval) settle?.resolve(approval);
		else settle?.reject(error);
	};
	const cancel = () => {
		try {
			win?.close();
		} catch {
			/* gone already */
		}
		finish(new PageApprovalError('closed'));
	};
	input.signal?.addEventListener('abort', cancel, { once: true });

	pageHandoff.current = { ...base, phase: 'checking', line: null, open: () => {}, cancel };
	const checked = await checkPage(input.page, deps);
	if (finished) return done;
	if (checked.launch === null) {
		const line = checked.line;
		pageHandoff.current = {
			...base,
			phase: 'refused',
			line,
			open: () => {},
			cancel: () => finish(new PageApprovalError('not_opened', line))
		};
		return done;
	}
	const launch = checked.launch;
	const pageOrigin = originOf(launch);
	const request = trustedSignerApprovalRequest({
		chainId: input.chainId,
		chainName: chainName(input.chainId) || null,
		nativeSymbol: nativeSymbol(input.chainId) || null,
		multisig: input.multisig,
		multisigName: input.multisigName ?? null,
		tx: input.tx,
		member: input.member.address,
		memberName: input.member.name,
		credentialIdsHex: input.keys.map((k) => k.credentialId),
		keyRoute: input.keyRoute
	});
	const id = `ms-${deps.now().toString(36)}`;

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
			if (readyTimer) clearTimeout(readyTimer);
			win.postMessage({ vela: 'intent', v: 1, id, ...request }, pageOrigin);
			return;
		}
		if (data.id !== id) {
			if (data.vela === 'bye') finish(new PageApprovalError('closed'));
			return;
		}
		if (data.vela === 'error') {
			finish(new PageApprovalError('declined', null, data.code ?? ''));
			return;
		}
		if (data.vela === 'result') {
			try {
				const approval = trustedSignerApprovalAccept<PasskeyApproval>({
					result: data.result,
					chainId: input.chainId,
					member: input.member.address,
					safeTxHash: input.safeTxHash,
					keys: input.keys
				});
				win.postMessage({ vela: 'bye', v: 1, reason: 'done' }, pageOrigin);
				finish(null, approval);
			} catch (error) {
				finish(
					new PageApprovalError(
						'unverified',
						null,
						error instanceof Error ? error.message : String(error)
					)
				);
			}
		}
	};

	const open = () => {
		if (finished) return;
		if (win && !win.closed) {
			win.focus();
			return;
		}
		// Inside the tap: a browser opens a window only for one.
		win = deps.open(launch);
		if (!win) return;
		stopListening?.();
		stopListening = deps.listen(onMessage);
		readyTimer = setTimeout(
			() => finish(new PageApprovalError('closed', null, 'silent')),
			READY_MS
		);
		if (watch) clearInterval(watch);
		watch = setInterval(() => {
			let closed: boolean;
			try {
				closed = !win || win.closed;
			} catch {
				closed = true;
			}
			if (closed) finish(new PageApprovalError('closed'));
		}, 500);
		pageHandoff.current = { ...base, phase: 'waiting', line: checked.line, open, cancel };
	};
	pageHandoff.current = { ...base, phase: 'ready', line: checked.line, open, cancel };
	return done;
}
