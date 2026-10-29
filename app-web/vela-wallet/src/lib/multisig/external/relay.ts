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
 * A token only lets its holder use the relay: rooms are end-to-end
 * encrypted, and the relay reads nothing. Any other relay (a profile's
 * `vela.peerconnectRelay`, a self-hosted one) is given no token.
 */
import { connect, type Room } from '$lib/vendor/peerconnect/index.js';

export const DEFAULT_RELAY = 'wss://relay.peerconnect.dev';
export const TOKEN_ENDPOINT = 'https://getvela.app/api/peerconnect-token';
export const TRIAL_ENDPOINT = 'https://peerconnect.dev/api/trial';

const RELAY_KEY = 'vela.peerconnectRelay';
const ENDPOINT_KEY = 'vela.peerconnectTokenEndpoint';
const DEVICE_KEY = 'vela.peerconnectDevice';
const TOKEN_KEY = 'vela.peerconnectToken.v1';
/** A token this close to expiring is replaced before it is used. */
const MARGIN_S = 10 * 60;

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
	return relay.replace(/\/+$/, '') === DEFAULT_RELAY;
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

/** Waits between tokens, refused or not given: each longer, at most 30 s. */
export const BACKOFF_MS = [2_000, 4_000, 8_000, 16_000, 30_000] as const;
/** Tokens refused in a row before it stops: the kept one, a fresh one, the trial's. */
export const REFUSALS = 3;
/** With a token and still offline this long: the relay is said not to answer. */
export const UNREACHABLE_AFTER_MS = 10_000;

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

async function ask(url: string, body?: unknown): Promise<Kept | string> {
	try {
		const res = await fetch(url, {
			method: 'POST',
			headers: body === undefined ? undefined : { 'content-type': 'application/json' },
			body: body === undefined ? undefined : JSON.stringify(body)
		});
		if (!res.ok) return `HTTP ${res.status}`;
		const answer = (await res.json()) as { token?: unknown; expires_at?: unknown };
		return typeof answer.token === 'string' && typeof answer.expires_at === 'number'
			? { token: answer.token, expiresAt: answer.expires_at }
			: 'no token in the answer';
	} catch {
		return 'no answer';
	}
}

function hostOf(url: string): string {
	try {
		return new URL(url).host;
	} catch {
		return url;
	}
}

let asking: Promise<Issued> | null = null;

/**
 * A token for the public relay: the one kept, if it has time left, else a new
 * one from Vela's site, else a trial one (`trial`: the trial's, unless the one
 * kept is the trial's already). Throws {@link NoTokenError} when neither
 * answers with one.
 */
export function relayToken(trial = false): Promise<Issued> {
	const kept = read<Kept>(TOKEN_KEY);
	if (
		kept &&
		typeof kept.token === 'string' &&
		kept.expiresAt - MARGIN_S > Date.now() / 1000 &&
		(!trial || kept.source === 'trial')
	) {
		return Promise.resolve({ token: kept.token, source: kept.source ?? 'site' });
	}
	// Two rooms opening at once share one request.
	asking ??= (async (): Promise<Issued> => {
		const site = trial ? null : await ask(tokenEndpoint(), { device: deviceId() });
		let fresh: Kept;
		if (site !== null && typeof site !== 'string') {
			fresh = { ...site, source: 'site' };
		} else {
			const fallback = await ask(TRIAL_ENDPOINT);
			if (typeof fallback === 'string') {
				const said = [
					...(site === null ? [] : [`${hostOf(tokenEndpoint())}: ${site}`]),
					`${hostOf(TRIAL_ENDPOINT)}: ${fallback}`
				];
				throw new NoTokenError(said.join(' · '));
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
	const kept = read<Kept>(TOKEN_KEY);
	if (!refused || kept?.token === refused) write(TOKEN_KEY, null);
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
	let source: Issued['source'] | undefined;
	/** Tokens refused in a row, and whether the trial's was one of them. */
	let refused = 0;
	let trialTried = false;
	/** The relay refused the last one: the next ask waits first. */
	let pending = false;
	const connection = connect(room, {
		token: async () => {
			if (pending) {
				pending = false;
				refused += 1;
				if (source === 'trial') trialTried = true;
				if (refused >= REFUSALS || (refused >= REFUSALS - 1 && trialTried)) {
					await watch.stop('refused', 'HTTP 4403');
					refused = 0;
					trialTried = false;
				} else {
					await watch.wait('refused', 'HTTP 4403', BACKOFF_MS[refused - 1]);
				}
			}
			for (let tries = 0; ; tries += 1) {
				watch.set({ kind: 'connecting' });
				try {
					// The last chance before stopping is the trial's, once.
					const issued = await relayToken(refused === REFUSALS - 1 && !trialTried);
					last = issued.token;
					source = issued.source;
					return issued.token;
				} catch (error) {
					const detail = error instanceof NoTokenError ? error.detail : String(error);
					if (tries < BACKOFF_MS.length) {
						await watch.wait('noToken', detail, BACKOFF_MS[tries]);
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
		pending = true;
	});
	const watched = Object.assign(connection, { relay: watch });
	followState(watched, watch, () => {
		refused = 0;
		trialTried = false;
	});
	return watched;
}

/**
 * On the relay: said as up (`accepted` is told, to forget the refusals). A
 * token in hand and still offline after {@link UNREACHABLE_AFTER_MS}: the
 * relay does not answer, said — the connection keeps trying on its own.
 */
function followState(connection: RelayConnection, watch: RelayWatch, accepted?: () => void) {
	let timer: ReturnType<typeof setTimeout> | undefined;
	const check = () => {
		clearTimeout(timer);
		const state = connection.state;
		if (state === 'waiting' || state === 'connected') {
			accepted?.();
			if (watch.status.kind !== 'up') watch.set({ kind: 'up' });
			return;
		}
		if (state === 'closed') return;
		if (watch.status.kind === 'up') watch.set({ kind: 'connecting' });
		timer = setTimeout(() => {
			if (connection.state === 'offline' && watch.status.kind === 'connecting') {
				watch.set({
					kind: 'retrying',
					why: 'unreachable',
					detail: hostOf(connection.room.relay),
					at: null
				});
			}
		}, UNREACHABLE_AFTER_MS);
	};
	connection.addEventListener('state', check);
	queueMicrotask(check);
}
