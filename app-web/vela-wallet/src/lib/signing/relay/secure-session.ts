/**
 * The end-to-end session a cross-device pairing runs — the REQUESTER's half
 * (spec 075, `contracts/relay.md` §2; PROTOCOL.md §3).
 *
 * P-256 ECDH → HKDF-SHA256 → AES-256-GCM, and a six-digit code both screens
 * show: the one place a stand-in on the path is caught. The relay only ever
 * forwards ciphertext, so everything that matters is derived here and on the
 * page — nothing is trusted to the server in the middle.
 *
 * Why this is TypeScript and not the core: the Rust session is exported to the
 * phones over UniFFI, but linking it into wasm cost ~120 KB on a module already
 * at its size ceiling (see `vela-core-wasm/src/clear_signer.rs`). So the web
 * runs the session on the browser's own WebCrypto, exactly as the signer page
 * does (`app-web/clearsigning/lib/transport/secure.js`) — and
 * `secure-session.test.ts` pins every byte of it against the shared vectors,
 * `rust/crates/vela-core/tests/clear-signer/secure-session.json`. If those
 * vectors and this file ever disagree, this file is wrong.
 *
 * Roles are BLE's, as the contract keeps them: the SIGNER is the page and
 * speaks first; the REQUESTER is the wallet. `c2p` is signer → requester,
 * `p2c` requester → signer. This module only ever plays the requester.
 */

/** The label is a parameter: one session implementation, two channels. */
export const RELAY_LABEL = 'vela-relay/1';

const ECDH = { name: 'ECDH', namedCurve: 'P-256' } as const;

/**
 * PKCS#8 `PrivateKeyInfo` for a P-256 key with no public half — the importer
 * derives it. Only used when a caller injects a bare 32-byte scalar, which is
 * how the vectors are checked; a real pairing generates its key pair.
 */
const PKCS8_PREFIX = Uint8Array.from([
	0x30, 0x41, 0x02, 0x01, 0x00, 0x30, 0x13, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01,
	0x06, 0x08, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07, 0x04, 0x27, 0x30, 0x25, 0x02, 0x01,
	0x01, 0x04, 0x20
]);

const utf8 = (text: string) => new TextEncoder().encode(text);

function concat(parts: Uint8Array[]): Uint8Array {
	const out = new Uint8Array(parts.reduce((n, part) => n + part.length, 0));
	let at = 0;
	for (const part of parts) {
		out.set(part, at);
		at += part.length;
	}
	return out;
}

export function b64url(bytes: Uint8Array): string {
	let binary = '';
	for (const byte of bytes) binary += String.fromCharCode(byte);
	return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

export function unb64url(text: string): Uint8Array | null {
	if (typeof text !== 'string' || !/^[A-Za-z0-9_-]*={0,2}$/.test(text)) return null;
	let padded = text.replace(/=+$/, '').replace(/-/g, '+').replace(/_/g, '/');
	while (padded.length % 4) padded += '=';
	try {
		const binary = atob(padded);
		const out = new Uint8Array(binary.length);
		for (let i = 0; i < binary.length; i++) out[i] = binary.charCodeAt(i);
		return out;
	} catch {
		return null;
	}
}

/** Why a session could not be established, or a message could not be opened. */
export class SecureSessionError extends Error {
	readonly code: 'bad_hello' | 'wrong_role' | 'foreign_peer' | 'replayed' | 'unreadable';
	constructor(code: SecureSessionError['code'], message: string) {
		super(message);
		this.name = 'SecureSessionError';
		this.code = code;
	}
}

/** The wallet's pairing key: static for one pairing, since the link's `rk` is its fingerprint. */
export interface RequesterKey {
	privateKey: CryptoKey;
	/** Uncompressed P-256 point, `04‖x‖y`, 65 bytes — what the hello carries. */
	publicKey: Uint8Array;
}

/**
 * A key pair for one pairing. `secret` (32 bytes) is for the vectors only;
 * without it the private key is generated non-extractable by the browser.
 */
export async function requesterKey(secret?: Uint8Array): Promise<RequesterKey> {
	if (secret === undefined) {
		const pair = (await crypto.subtle.generateKey(ECDH, false, ['deriveBits'])) as CryptoKeyPair;
		const raw = await crypto.subtle.exportKey('raw', pair.publicKey);
		return { privateKey: pair.privateKey, publicKey: new Uint8Array(raw) };
	}
	if (secret.length !== 32) throw new SecureSessionError('bad_hello', 'the secret is not 32 bytes');
	const privateKey = await crypto.subtle.importKey(
		'pkcs8',
		concat([PKCS8_PREFIX, secret]) as BufferSource,
		ECDH,
		true,
		['deriveBits']
	);
	const jwk = await crypto.subtle.exportKey('jwk', privateKey);
	const x = unb64url(jwk.x ?? '');
	const y = unb64url(jwk.y ?? '');
	if (x === null || y === null)
		throw new SecureSessionError('bad_hello', 'the secret is not a P-256 scalar');
	return { privateKey, publicKey: concat([Uint8Array.from([0x04]), x, y]) };
}

/** `rk` in the pairing link: `b64url(SHA-256(pk)[0..16])`. */
export async function keyFingerprint(publicKey: Uint8Array): Promise<string> {
	const digest = await crypto.subtle.digest('SHA-256', publicKey as BufferSource);
	return b64url(new Uint8Array(digest).subarray(0, 16));
}

export interface Hello {
	v: 1;
	t: 'hello';
	role: 'requester' | 'signer';
	pk: string;
	nonce: string;
	app?: string;
}

/** This wallet's hello — the only frame it ever sends in the clear. */
export function requesterHello(key: RequesterKey, nonce: Uint8Array, app: string): Hello {
	const hello: Hello = {
		v: 1,
		t: 'hello',
		role: 'requester',
		pk: b64url(key.publicKey),
		nonce: b64url(nonce)
	};
	if (app) hello.app = app;
	return hello;
}

async function hkdf(
	secret: Uint8Array,
	salt: Uint8Array,
	info: string,
	length: number
): Promise<Uint8Array> {
	const key = await crypto.subtle.importKey('raw', secret as BufferSource, 'HKDF', false, [
		'deriveBits'
	]);
	const bits = await crypto.subtle.deriveBits(
		{ name: 'HKDF', hash: 'SHA-256', salt: salt as BufferSource, info: utf8(info) as BufferSource },
		key,
		length * 8
	);
	return new Uint8Array(bits);
}

/**
 * Finish the handshake with the page's hello and derive the session.
 *
 * `label` is the channel (`vela-relay/1`); `nonce` is this side's 16 bytes,
 * fresh for every handshake — a wallet that dropped out and came back is a new
 * session with a new code, checked again from scratch.
 */
export async function completeSession(
	key: RequesterKey,
	nonce: Uint8Array,
	peerHello: unknown,
	label: string = RELAY_LABEL
): Promise<SecureSession> {
	const hello = (
		typeof peerHello === 'string' ? safeParse(peerHello) : peerHello
	) as Partial<Hello> | null;
	if (!hello || typeof hello !== 'object' || hello.t !== 'hello') {
		throw new SecureSessionError('bad_hello', 'the peer did not answer the handshake');
	}
	if (hello.role !== 'signer' && hello.role !== 'requester') {
		throw new SecureSessionError('bad_hello', 'the peer named no role');
	}
	// Our own role back is a stand-in, not a peer: two requesters derive
	// nothing, and pretending otherwise would put a code on screen.
	if (hello.role === 'requester') {
		throw new SecureSessionError('wrong_role', 'the peer spoke in our own role');
	}
	const peerPk = unb64url(hello.pk ?? '');
	const peerNonce = unb64url(hello.nonce ?? '');
	if (peerPk === null || peerNonce === null || peerNonce.length !== 16) {
		throw new SecureSessionError('bad_hello', 'the peer hello is malformed');
	}
	let peerKey: CryptoKey;
	try {
		peerKey = await crypto.subtle.importKey('raw', peerPk as BufferSource, ECDH, false, []);
	} catch {
		throw new SecureSessionError('bad_hello', 'the peer key is not a P-256 point');
	}
	const shared = new Uint8Array(
		await crypto.subtle.deriveBits({ name: 'ECDH', public: peerKey }, key.privateKey, 256)
	);
	// The signer's nonce first, whichever side is deriving (relay.md §2.3).
	const salt = concat([peerNonce, nonce]);
	const [material, codeBytes] = await Promise.all([
		hkdf(shared, salt, `${label} key`, 32),
		hkdf(shared, salt, `${label} code`, 4)
	]);
	const aes = await crypto.subtle.importKey('raw', material as BufferSource, 'AES-GCM', false, [
		'encrypt',
		'decrypt'
	]);
	const number =
		((codeBytes[0] << 24) >>> 0) + (codeBytes[1] << 16) + (codeBytes[2] << 8) + codeBytes[3];
	return new SecureSession(aes, String(number % 1000000).padStart(6, '0'), label, peerPk);
}

function safeParse(text: string): unknown {
	try {
		return JSON.parse(text);
	} catch {
		return null;
	}
}

/**
 * One established session. Every message after the two hellos is a binary
 * frame `IV(12) ‖ AES-GCM(ciphertext‖tag)`, and the counters only go up, so a
 * replayed or reordered frame is refused before it is decrypted.
 */
export class SecureSession {
	/** The six digits both screens show; the person confirms them on the wallet. */
	readonly code: string;
	readonly label: string;
	/** The page's pairing key, as the hello carried it. */
	readonly peerPublicKey: Uint8Array;

	#key: CryptoKey;
	#sent = 0n;
	#received = 0n;
	/** Seals and opens run in call order: a counter must not be spent out of turn. */
	#queue: Promise<unknown> = Promise.resolve();

	constructor(key: CryptoKey, code: string, label: string, peerPublicKey: Uint8Array) {
		this.#key = key;
		this.code = code;
		this.label = label;
		this.peerPublicKey = peerPublicKey;
	}

	/** `p2c` — this wallet's direction. */
	get outgoing(): 'p2c' {
		return 'p2c';
	}

	/** `c2p` — the page's. */
	get incoming(): 'c2p' {
		return 'c2p';
	}

	async seal(plaintext: Uint8Array): Promise<Uint8Array> {
		this.#sent += 1n;
		const counter = this.#sent;
		return this.#serial(async () => {
			const iv = ivOf(this.outgoing, counter);
			const sealed = await crypto.subtle.encrypt(
				{
					name: 'AES-GCM',
					iv: iv as BufferSource,
					additionalData: this.#additionalData(this.outgoing, counter) as BufferSource
				},
				this.#key,
				plaintext as BufferSource
			);
			return concat([iv, new Uint8Array(sealed)]);
		});
	}

	async open(sealed: Uint8Array): Promise<Uint8Array> {
		return this.#serial(async () => {
			if (!(sealed instanceof Uint8Array) || sealed.length < 12 + 16) {
				throw new SecureSessionError('unreadable', 'the message is too short');
			}
			const head = sealed.subarray(0, 12);
			const tag = directionTag(this.incoming);
			for (let i = 0; i < 4; i++) {
				if (head[i] !== tag[i]) {
					throw new SecureSessionError('replayed', 'the message came from our own direction');
				}
			}
			let counter = 0n;
			for (let i = 4; i < 12; i++) counter = (counter << 8n) | BigInt(head[i]);
			if (counter <= this.#received) {
				throw new SecureSessionError('replayed', `replayed message (counter ${counter})`);
			}
			let plain: ArrayBuffer;
			try {
				plain = await crypto.subtle.decrypt(
					{
						name: 'AES-GCM',
						iv: head as BufferSource,
						additionalData: this.#additionalData(this.incoming, counter) as BufferSource
					},
					this.#key,
					sealed.subarray(12) as BufferSource
				);
			} catch {
				throw new SecureSessionError('unreadable', 'the message could not be opened');
			}
			this.#received = counter;
			return new Uint8Array(plain);
		});
	}

	/** The relay binds the IV's counter into the AAD (relay.md §2.5). */
	#additionalData(direction: string, counter: bigint): Uint8Array {
		return utf8(`${this.label}|${direction}|${counter.toString()}`);
	}

	#serial<T>(work: () => Promise<T>): Promise<T> {
		const run = this.#queue.then(work, work);
		this.#queue = run.catch(() => {
			/* the next one still runs */
		});
		return run;
	}
}

function directionTag(direction: 'c2p' | 'p2c'): Uint8Array {
	return utf8(direction === 'c2p' ? 'C2P.' : 'P2C.');
}

/** IV = 4-byte direction tag ‖ 8-byte big-endian counter, from 1, never reused. */
function ivOf(direction: 'c2p' | 'p2c', counter: bigint): Uint8Array {
	const out = new Uint8Array(12);
	out.set(directionTag(direction), 0);
	for (let i = 0; i < 8; i++) out[11 - i] = Number((counter >> BigInt(8 * i)) & 0xffn);
	return out;
}
