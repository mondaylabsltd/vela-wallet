/**
 * A stand-in for the other device: the signer page's half of a relay room,
 * and the room itself (spec 075, relay.md).
 *
 * The wallet's half is the code under test (`clear-signer-channel.ts` +
 * `relay/secure-session.ts`, pinned to the core's vectors). This is the other
 * end of the same conversation, written the way the page writes it
 * (`app-web/clearsigning/lib/transport/{relay,secure}.js`): joined → the
 * signer's hello first → both derive → sealed frames only.
 *
 * Tests only; nothing in the app imports this. The real page, over a real
 * mock relay, is what `e2e/clear-signer-relay.e2e.ts` runs.
 */
import type { RelaySocket } from '../clear-signer-channel';
import { b64url, unb64url } from '../relay/secure-session';

const ECDH = { name: 'ECDH', namedCurve: 'P-256' } as const;
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

function iv(tag: string, counter: bigint): Uint8Array {
	const out = new Uint8Array(12);
	out.set(utf8(tag), 0);
	for (let i = 0; i < 8; i++) out[11 - i] = Number((counter >> BigInt(8 * i)) & 0xffn);
	return out;
}

/** The page's side of one session: it seals `c2p` and opens `p2c`. */
class PageSession {
	#key: CryptoKey;
	#label: string;
	#sent = 0n;
	#received = 0n;
	readonly code: string;

	constructor(key: CryptoKey, code: string, label: string) {
		this.#key = key;
		this.code = code;
		this.#label = label;
	}

	async seal(plaintext: Uint8Array): Promise<Uint8Array> {
		this.#sent += 1n;
		const nonce = iv('C2P.', this.#sent);
		const sealed = await crypto.subtle.encrypt(
			{
				name: 'AES-GCM',
				iv: nonce as BufferSource,
				additionalData: utf8(`${this.#label}|c2p|${this.#sent}`) as BufferSource
			},
			this.#key,
			plaintext as BufferSource
		);
		return concat([nonce, new Uint8Array(sealed)]);
	}

	async open(sealed: Uint8Array): Promise<Uint8Array> {
		const head = sealed.subarray(0, 12);
		let counter = 0n;
		for (let i = 4; i < 12; i++) counter = (counter << 8n) | BigInt(head[i]);
		if (counter <= this.#received) throw new Error('replayed');
		const plain = await crypto.subtle.decrypt(
			{
				name: 'AES-GCM',
				iv: head as BufferSource,
				additionalData: utf8(`${this.#label}|p2c|${counter}`) as BufferSource
			},
			this.#key,
			sealed.subarray(12) as BufferSource
		);
		this.#received = counter;
		return new Uint8Array(plain);
	}
}

export interface FakeRelay {
	/** The socket the wallet's channel is given. */
	socket: RelaySocket;
	/** Both ends are in the room: the page joins and speaks first. */
	join(): Promise<void>;
	/** The page dropped out; the room waits for it. */
	leave(): void;
	/** The relay itself closed the room (4408 expired, 4409 role taken, …). */
	shut(code: number): void;
	/** The six digits the page shows, once the two ends have derived them. */
	code(): string | null;
	/** Every message the page opened, in order. */
	received: Record<string, unknown>[];
	/** Answer the request `id` as the page answers a ceremony. */
	answer(id: string, body: Record<string, unknown>): Promise<void>;
	/** Answer with the page's error vocabulary (`user_rejected`, `refused`, …). */
	refuse(id: string, code: string): Promise<void>;
	/** Everything the relay forwarded: text frames are `string`, sealed ones are bytes. */
	frames: (string | Uint8Array)[];
}

/**
 * A room with the page already in it. `label` and the page's key material can
 * be pinned for a vector check; by default both ends are fresh.
 */
export function fakeRelay(options: { label?: string } = {}): FakeRelay {
	const label = options.label ?? 'vela-relay/1';
	const frames: (string | Uint8Array)[] = [];
	const received: Record<string, unknown>[] = [];
	let session: PageSession | null = null;
	let outgoing = 0;
	let lastSeen = 0;

	const socket: RelaySocket = {
		binaryType: 'blob',
		onopen: null,
		onmessage: null,
		onclose: null,
		onerror: null,
		send(data) {
			if (typeof data === 'string') {
				frames.push(data);
				void onText(data);
				return;
			}
			const bytes = new Uint8Array(
				data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength)
			);
			frames.push(bytes);
			void onSealed(bytes);
		},
		close() {
			socket.onclose?.({ code: 1000 });
		}
	};

	/** Deliver a frame to the wallet, as the relay would. */
	function toWallet(data: string | Uint8Array): void {
		socket.onmessage?.({
			data:
				typeof data === 'string'
					? data
					: data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength)
		});
	}

	async function onText(text: string): Promise<void> {
		const hello = JSON.parse(text) as Record<string, unknown>;
		if (hello.t !== 'hello' || pending === null) return;
		const peerPk = unb64url(String(hello.pk));
		const peerNonce = unb64url(String(hello.nonce));
		if (peerPk === null || peerNonce === null) throw new Error('bad wallet hello');
		const peerKey = await crypto.subtle.importKey('raw', peerPk as BufferSource, ECDH, false, []);
		const shared = new Uint8Array(
			await crypto.subtle.deriveBits({ name: 'ECDH', public: peerKey }, pending.privateKey, 256)
		);
		const salt = concat([pending.nonce, peerNonce]);
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
		session = new PageSession(aes, String(number % 1000000).padStart(6, '0'), label);
		outgoing = 0;
		lastSeen = 0;
		pending = null;
	}

	async function onSealed(bytes: Uint8Array): Promise<void> {
		if (session === null) return;
		const plain = await session.open(bytes);
		const message = JSON.parse(new TextDecoder().decode(plain)) as Record<string, unknown>;
		if (typeof message.n === 'number') lastSeen = Math.max(lastSeen, message.n);
		received.push(message);
	}

	let pending: { privateKey: CryptoKey; nonce: Uint8Array } | null = null;

	async function send(message: Record<string, unknown>): Promise<void> {
		if (session === null) throw new Error('the page has no session');
		outgoing = Math.max(outgoing, lastSeen) + 1;
		toWallet(await session.seal(utf8(JSON.stringify({ v: 1, ...message, n: outgoing }))));
	}

	return {
		socket,
		frames,
		received,
		async join() {
			session = null;
			toWallet(JSON.stringify({ v: 1, relay: 'joined' }));
			const pair = (await crypto.subtle.generateKey(ECDH, true, ['deriveBits'])) as CryptoKeyPair;
			const raw = new Uint8Array(await crypto.subtle.exportKey('raw', pair.publicKey));
			const nonce = new Uint8Array(16);
			crypto.getRandomValues(nonce);
			pending = { privateKey: pair.privateKey, nonce };
			// The page speaks first, in the clear (relay.md §2.1).
			toWallet(
				JSON.stringify({
					v: 1,
					t: 'hello',
					role: 'signer',
					pk: b64url(raw),
					nonce: b64url(nonce)
				})
			);
		},
		leave() {
			session = null;
			toWallet(JSON.stringify({ v: 1, relay: 'left' }));
		},
		shut(code: number) {
			socket.onclose?.({ code });
		},
		code() {
			return session?.code ?? null;
		},
		answer(id, body) {
			return send({ t: 'result', id, ...body });
		},
		refuse(id, code) {
			return send({ t: 'error', id, code });
		}
	};
}
