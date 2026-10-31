/**
 * The PeerConnect relay rooms go through, and the token it asks of each peer.
 *
 * The public relay admits a connection only with a token, and each side of a
 * room brings its own: Vela here, and the owner's browser on the `/connect`
 * page. A device asks Vela's site for one (`getvela.app/api/peerconnect-token`,
 * which holds the app secret) under a random ID it keeps, and keeps the token
 * until it expires (a year). If the site cannot issue one, PeerConnect's
 * public trial gives a two-hour token instead, so signing never waits on it.
 *
 * r4C-1/r4C-4: a token the relay refuses (`4403`) is never followed by
 * another at once — the vendored client asks again on every refusal, which
 * was about once a second, forever. Each connection waits before its next
 * token (2 s, then 4 s), tries a fresh one from the site, then the trial's
 * once, and then stops and says so ({@link RelayWatch}: "Can't connect to
 * the signing relay", with Retry). Getting no token at all backs off the same
 * way (2 s … 30 s) and then stops, said as that; a relay that does not
 * answer is said too.
 *
 * Every one of those decisions is the core's (`multisig_rules::relay`, asked
 * through `multisigLinkCall("relay.<rule>")`): which relay asks for a token,
 * whether the kept one is still good, what an endpoint answered, which token
 * a refusal forgets, how long to wait and when to stop, where the
 * connection's way onto the relay stands. This file keeps only the I/O — the
 * socket, the timers, the fetches and the device's storage.
 *
 * A token only lets its holder use the relay: rooms are end-to-end
 * encrypted, and the relay reads nothing. Any other relay (a profile's
 * `vela.peerconnectRelay`, a self-hosted one) is given no token.
 */
import * as wasm from '../../../../../../rust/pkg-web/vela_core.js';
import { connect, type Room } from '$lib/vendor/peerconnect/index.js';

/** Ask one of the core's relay rules. */
function rule<T>(name: string, args: Record<string, unknown> = {}): T {
	return JSON.parse(wasm.multisigLinkCall(`relay.${name}`, JSON.stringify(args))) as T;
}

export const DEFAULT_RELAY = 'wss://relay.peerconnect.dev';
export const TOKEN_ENDPOINT = 'https://getvela.app/api/peerconnect-token';
export const TRIAL_ENDPOINT = 'https://peerconnect.dev/api/trial';

const RELAY_KEY = 'vela.peerconnectRelay';
const ENDPOINT_KEY = 'vela.peerconnectTokenEndpoint';
const DEVICE_KEY = 'vela.peerconnectDevice';
const TOKEN_KEY = 'vela.peerconnectToken.v1';

/** The relay new rooms go through; a profile may point at another. */
export function relayUrl(): string {
	try {
		return localStorage.getItem(RELAY_KEY) || DEFAULT_RELAY;
	} catch {
		return DEFAULT_RELAY;
	}
}

/** Where tokens are asked for; a profile may point at a local site. */
export function tokenEndpoint(): string {
	try {
		return localStorage.getItem(ENDPOINT_KEY) || TOKEN_ENDPOINT;
	} catch {
		return TOKEN_ENDPOINT;
	}
}

/** Whether `relay` is the public relay, the one that asks for tokens. */
export function needsToken(relay: string): boolean {
	return rule<boolean>('needsToken', { relay });
}

/** The core's schedule: the waits between tokens, and when offline is said as unreachable. */
export interface RelaySchedule {
	backoffMs: number[];
	refusals: number;
	unreachableAfterMs: number;
}

let schedule: RelaySchedule | null = null;
export function relaySchedule(): RelaySchedule {
	schedule ??= rule<RelaySchedule>('constants');
	return schedule;
}

interface Kept {
	token: string;
	/** Unix seconds. */
	expiresAt: number;
	/** Who issued it: Vela's site, or PeerConnect's public trial. */
	source?: 'site' | 'trial';
}

function read<T>(key: string): T | null {
	try {
		const raw = localStorage.getItem(key);
		return raw ? (JSON.parse(raw) as T) : null;
	} catch {
		return null;
	}
}

function write(key: string, value: unknown): void {
	try {
		if (value === null) localStorage.removeItem(key);
		else localStorage.setItem(key, JSON.stringify(value));
	} catch {
		// private mode: asked again next time
	}
}

/** This device's ID at Vela's site: 32 random hex characters, made once. */
function deviceId(): string {
	const kept = read<string>(DEVICE_KEY);
	if (typeof kept === 'string' && /^[a-z0-9]{26,64}$/.test(kept)) return kept;
	const bytes = new Uint8Array(16);
	crypto.getRandomValues(bytes);
	const id = Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
	write(DEVICE_KEY, id);
	return id;
}

/** Why a device's own side cannot get onto the relay. */
export type RelayTrouble =
	/** The relay refused the token (`4403`: invalid, revoked, expired). */
	| 'refused'
	/** Neither Vela's site nor the trial gave a token (`detail`: what they answered). */
	| 'noToken'
	/** A token, and still no connection: the relay does not answer. */
	| 'unreachable';

/** Where this side's connection to the relay stands, for the screen to say. */
export type RelayStatus =
	/** Getting a token, or connecting with one. */
	| { kind: 'connecting' }
	/** On the relay (waiting for the other side, or with it). */
	| { kind: 'up' }
	/** Not on it; tried again at `at` (ms since epoch; `null`: on its own, soon). */
	| { kind: 'retrying'; why: RelayTrouble; detail: string; at: number | null }
	/** Stopped trying: said, until the person taps Retry ({@link RelayWatch.retry}). */
	| { kind: 'failed'; why: RelayTrouble; detail: string };

/** What a connection does before it asks for its next token (the core's `Before`). */
type Before = { kind: 'go'; trial: boolean } | { kind: 'wait'; ms: number } | { kind: 'stop' };

/** A token, with who issued it. */
export interface Issued {
	token: string;
	source: 'site' | 'trial';
}

/** No token: what each place answered ("getvela.app: HTTP 429 · peerconnect.dev: HTTP 429"). */
export class NoTokenError extends Error {
	constructor(readonly detail: string) {
		super(`No PeerConnect relay token (${detail})`);
		this.name = 'NoTokenError';
	}
}

/** One endpoint asked; what it answered is read by the core: a token, or what to say. */
async function ask(url: string, body?: unknown): Promise<Kept | string> {
	let status: number | null = null;
	let text = '';
	try {
		const res = await fetch(url, {
			method: 'POST',
			headers: body === undefined ? undefined : { 'content-type': 'application/json' },
			body: body === undefined ? undefined : JSON.stringify(body)
		});
		status = res.status;
		text = await res.text();
	} catch {
		// No answer: said as that, by the core.
	}
	const answer = rule<{ ok: Kept } | { err: string }>('tokenAnswer', { status, body: text });
	return 'ok' in answer ? answer.ok : answer.err;
}

let asking: Promise<Issued> | null = null;

/**
 * A token for the public relay: the one kept, if the core finds it still
 * good, else a new one from Vela's site, else a trial one (`trial`: the
 * trial's, unless the one kept is the trial's already). Throws
 * {@link NoTokenError} when neither answers with one.
 */
export function relayToken(trial = false): Promise<Issued> {
	const usable = rule<Issued | null>('usable', {
		kept: read<Kept>(TOKEN_KEY),
		nowS: Date.now() / 1000,
		trial
	});
	if (usable) return Promise.resolve(usable);
	// Two rooms opening at once share one request.
	asking ??= (async (): Promise<Issued> => {
		const site = trial ? null : await ask(tokenEndpoint(), { device: deviceId() });
		let fresh: Kept;
		if (site !== null && typeof site !== 'string') {
			fresh = { ...site, source: 'site' };
		} else {
			const fallback = await ask(TRIAL_ENDPOINT);
			if (typeof fallback === 'string') {
				throw new NoTokenError(
					rule<string>('noTokenDetail', { endpoint: tokenEndpoint(), site, trial: fallback })
				);
			}
			fresh = { ...fallback, source: 'trial' };
		}
		write(TOKEN_KEY, fresh);
		return { token: fresh.token, source: fresh.source ?? 'site' };
	})().finally(() => {
		asking = null;
	});
	return asking;
}

/** The relay refused the kept token (revoked, expired): the next ask gets a new one. */
export function forgetRelayToken(refused?: string): void {
	if (rule<boolean>('forgets', { kept: read<Kept>(TOKEN_KEY), refused: refused ?? null })) {
		write(TOKEN_KEY, null);
	}
}

/**
 * One connection's way onto the relay, as the screen says it
 * ({@link RelayStatus}; event `change`). A wait it counts down is cut short,
 * and a stop undone, by {@link retry}.
 */
export class RelayWatch extends EventTarget {
	status: RelayStatus = { kind: 'connecting' };
	#wake: (() => void) | null = null;

	set(status: RelayStatus): void {
		this.status = status;
		this.dispatchEvent(new Event('change'));
	}

	/** Wait `ms`, said as retrying for `why` — less, if Retry is tapped. */
	wait(why: RelayTrouble, detail: string, ms: number): Promise<void> {
		this.set({ kind: 'retrying', why, detail, at: Date.now() + ms });
		return new Promise<void>((resolve) => {
			const timer = setTimeout(() => {
				this.#wake = null;
				resolve();
			}, ms);
			this.#wake = () => {
				clearTimeout(timer);
				this.#wake = null;
				resolve();
			};
		});
	}

	/** Stop: said as failed for `why`, until Retry is tapped. */
	stop(why: RelayTrouble, detail: string): Promise<void> {
		this.set({ kind: 'failed', why, detail });
		return new Promise<void>((resolve) => {
			this.#wake = () => {
				this.#wake = null;
				this.set({ kind: 'connecting' });
				resolve();
			};
		});
	}

	/** Try again now: a wait cut short, a stop undone. */
	retry(): void {
		this.#wake?.();
	}
}

export type RelayConnection = ReturnType<typeof connect> & { relay: RelayWatch };

/**
 * Connect to `room`, with a token when its relay asks for one. The
 * connection's `relay` says where getting onto the relay stands.
 */
export function connectRoom(room: Room): RelayConnection {
	const watch = new RelayWatch();
	if (!needsToken(room.relay)) {
		const plain = Object.assign(connect(room), { relay: watch });
		followState(plain, watch);
		return plain;
	}
	let last: string | undefined;
	/** The core's count of this connection's refusals, kept here between questions. */
	let refusals: unknown = null;
	const refused = (on: string, source: Issued['source'] | null = null): Before | null => {
		const answer = rule<{ state: unknown; before: Before | null }>('refusals', {
			state: refusals,
			on,
			source
		});
		refusals = answer.state;
		return answer.before;
	};
	const connection = connect(room, {
		token: async () => {
			// Refused before: waited out, or stopped until Retry, as the core says.
			let before = refused('beforeToken');
			while (before && before.kind !== 'go') {
				if (before.kind === 'wait') {
					await watch.wait('refused', 'HTTP 4403', before.ms);
				} else {
					await watch.stop('refused', 'HTTP 4403');
					refused('retried');
				}
				before = refused('beforeToken');
			}
			// The last chance before stopping is the trial's, once.
			const trial = before?.kind === 'go' && before.trial;
			for (let tries = 0; ; tries += 1) {
				watch.set({ kind: 'connecting' });
				try {
					const issued = await relayToken(trial);
					last = issued.token;
					refused('issued', issued.source);
					return issued.token;
				} catch (error) {
					const detail = error instanceof NoTokenError ? error.detail : String(error);
					const step = rule<Before>('noTokenStep', { tries });
					if (step.kind === 'wait') {
						await watch.wait('noToken', detail, step.ms);
					} else {
						await watch.stop('noToken', detail);
						tries = -1;
					}
				}
			}
		}
	});
	// Dispatched before the connection asks again: that ask must not get the same one.
	connection.addEventListener('unauthorized', () => {
		forgetRelayToken(last);
		refused('unauthorized');
	});
	const watched = Object.assign(connection, { relay: watch });
	followState(watched, watch, () => refused('accepted'));
	return watched;
}

/**
 * Where the connection's way onto the relay stands, as the core reads its
 * state: up once on the relay (`accepted` is told, to forget the refusals);
 * offline with a token for the core's while, said as the relay not answering
 * — the connection keeps trying on its own.
 */
function followState(connection: RelayConnection, watch: RelayWatch, accepted?: () => void) {
	let timer: ReturnType<typeof setTimeout> | undefined;
	const said = (offlineMs: number) => {
		const next = rule<{ status: RelayStatus; accepted: boolean }>('onState', {
			status: watch.status,
			state: connection.state,
			offlineMs,
			relay: connection.room.relay
		});
		if (next.accepted) accepted?.();
		if (JSON.stringify(next.status) !== JSON.stringify(watch.status)) watch.set(next.status);
		return next;
	};
	const check = () => {
		clearTimeout(timer);
		const next = said(0);
		if (next.accepted || connection.state === 'closed') return;
		const after = relaySchedule().unreachableAfterMs;
		timer = setTimeout(() => said(after), after);
	};
	connection.addEventListener('state', check);
	queueMicrotask(check);
	backOnline(connection);
}

/**
 * r4C-15: the network back (or the page in front again): a connection
 * waiting out its backoff — up to 30 s between tries — tries the relay at
 * once, so the other side sees it back within a second or two rather than
 * "went offline — reconnecting…" for half a minute. Stops once it is left or closed.
 */
function backOnline(connection: RelayConnection) {
	if (typeof window === 'undefined') return;
	const over = () => connection.state === 'closed' || connection.reason === 'left';
	const nudge = () => {
		if (over()) return stop();
		if (typeof document !== 'undefined' && document.visibilityState === 'hidden') return;
		connection.reconnectNow?.();
	};
	const stop = () => {
		window.removeEventListener('online', nudge);
		document.removeEventListener('visibilitychange', nudge);
	};
	window.addEventListener('online', nudge);
	document.addEventListener('visibilitychange', nudge);
	connection.addEventListener('state', () => {
		if (over()) stop();
	});
}
