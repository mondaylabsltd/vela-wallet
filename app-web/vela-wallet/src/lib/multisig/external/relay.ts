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

async function ask(url: string, body?: unknown): Promise<Kept | null> {
	try {
		const res = await fetch(url, {
			method: 'POST',
			headers: body === undefined ? undefined : { 'content-type': 'application/json' },
			body: body === undefined ? undefined : JSON.stringify(body)
		});
		if (!res.ok) return null;
		const answer = (await res.json()) as { token?: unknown; expires_at?: unknown };
		return typeof answer.token === 'string' && typeof answer.expires_at === 'number'
			? { token: answer.token, expiresAt: answer.expires_at }
			: null;
	} catch {
		return null;
	}
}

let asking: Promise<string> | null = null;

/**
 * A token for the public relay: the one kept, if it has time left, else a new
 * one from Vela's site, else a trial one. Throws when neither answers; the
 * connection then asks again in a little while.
 */
export function relayToken(): Promise<string> {
	const kept = read<Kept>(TOKEN_KEY);
	if (kept && typeof kept.token === 'string' && kept.expiresAt - MARGIN_S > Date.now() / 1000) {
		return Promise.resolve(kept.token);
	}
	// Two rooms opening at once share one request.
	asking ??= (async () => {
		const fresh =
			(await ask(tokenEndpoint(), { device: deviceId() })) ?? (await ask(TRIAL_ENDPOINT));
		if (!fresh) throw new Error('No PeerConnect relay token');
		write(TOKEN_KEY, fresh);
		return fresh.token;
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

/** Connect to `room`, with a token when its relay asks for one. */
export function connectRoom(room: Room): ReturnType<typeof connect> {
	if (!needsToken(room.relay)) return connect(room);
	let last: string | undefined;
	const connection = connect(room, {
		token: async () => {
			last = await relayToken();
			return last;
		}
	});
	// Dispatched before the connection asks again: that ask must not get the same one.
	connection.addEventListener('unauthorized', () => forgetRelayToken(last));
	return connection;
}
