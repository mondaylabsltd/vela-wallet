/**
 * The relay requester, against the shared vectors (spec 075).
 *
 * `rust/crates/vela-core/tests/clear-signer/secure-session.json` is the one
 * source both ends are pinned to: the Rust session the phones run, the signer
 * page's `lib/transport/secure.js`, and this module. Every case is replayed
 * here as the REQUESTER — the wallet's side — so what is checked is not "the
 * code runs" but "the bytes are the same bytes":
 *
 * - the public key derived from the secret;
 * - `rk`, the fingerprint the pairing link carries;
 * - the hello JSON, field for field;
 * - the six-digit code both screens show;
 * - every sealed frame this side sends, byte for byte, and every frame the
 *   page sends, opened.
 *
 * A replay and a frame from our own direction are refused without decrypting,
 * which is the other half of the guarantee: the relay cannot reorder a session.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import {
	SecureSessionError,
	b64url,
	completeSession,
	keyFingerprint,
	requesterHello,
	requesterKey,
	unb64url
} from './secure-session';

interface Case {
	name: string;
	label: string;
	code: string;
	rk: string;
	requester: {
		secretHex: string;
		publicKeyHex: string;
		nonceHex: string;
		hello: string;
		app: string;
	};
	signer: { secretHex: string; publicKeyHex: string; nonceHex: string; hello: string };
	messages: { from: 'requester' | 'signer'; plaintext: string; sealedHex: string }[];
}

const VECTORS = join(
	import.meta.dirname,
	'..',
	'..',
	'..',
	'..',
	'..',
	'..',
	'rust',
	'crates',
	'vela-core',
	'tests',
	'clear-signer',
	'secure-session.json'
);

const cases: Case[] = JSON.parse(readFileSync(VECTORS, 'utf8')).cases;

const fromHex = (hex: string) =>
	Uint8Array.from((hex.match(/../g) ?? []).map((pair) => Number.parseInt(pair, 16)));
const toHex = (bytes: Uint8Array) =>
	Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');

describe('the relay session, byte for byte with the core', () => {
	it('has vectors to check against at all', () => {
		expect(cases.length).toBeGreaterThan(0);
		expect(cases.some((one) => one.label === 'vela-relay/1')).toBe(true);
	});

	// Only the relay's cases carry frames this module can reproduce: BLE binds
	// the FRAME's msgId into the AAD instead of the IV's counter (PROTOCOL.md
	// §3.5), and this channel has no frames — a WebSocket message is already
	// whole. The key, `rk`, hello and code are the same derivation on both, and
	// the last test below reads the BLE case for exactly that.
	for (const one of cases.filter((candidate) => candidate.label === 'vela-relay/1')) {
		it(`${one.name} (${one.label}): key, rk, hello, code and every frame`, async () => {
			const key = await requesterKey(fromHex(one.requester.secretHex));
			expect(toHex(key.publicKey)).toBe(one.requester.publicKeyHex);
			// The fingerprint in the pairing link — how the page knows the
			// requester is the wallet that drew the QR.
			expect(await keyFingerprint(key.publicKey)).toBe(one.rk);

			const nonce = fromHex(one.requester.nonceHex);
			expect(requesterHello(key, nonce, one.requester.app)).toEqual(
				JSON.parse(one.requester.hello)
			);

			const session = await completeSession(key, nonce, one.signer.hello, one.label);
			expect(session.code).toBe(one.code);

			for (const message of one.messages) {
				const plaintext = new TextEncoder().encode(message.plaintext);
				if (message.from === 'requester') {
					expect(toHex(await session.seal(plaintext))).toBe(message.sealedHex);
				} else {
					const opened = await session.open(fromHex(message.sealedHex));
					expect(new TextDecoder().decode(opened)).toBe(message.plaintext);
				}
			}
		});
	}
});

describe('what a session refuses', () => {
	const relay = cases.find((one) => one.label === 'vela-relay/1') as Case;

	const paired = async () => {
		const key = await requesterKey(fromHex(relay.requester.secretHex));
		return completeSession(key, fromHex(relay.requester.nonceHex), relay.signer.hello, relay.label);
	};

	it('a frame it already opened, and one from its own direction', async () => {
		const session = await paired();
		const first = relay.messages.find((m) => m.from === 'signer') as Case['messages'][0];
		await session.open(fromHex(first.sealedHex));
		await expect(session.open(fromHex(first.sealedHex))).rejects.toMatchObject({
			code: 'replayed'
		});
		const own = relay.messages.find((m) => m.from === 'requester') as Case['messages'][0];
		await expect(session.open(fromHex(own.sealedHex))).rejects.toMatchObject({
			code: 'replayed'
		});
	});

	it('a frame whose bytes were touched', async () => {
		const session = await paired();
		const signed = relay.messages.find((m) => m.from === 'signer') as Case['messages'][0];
		const sealed = fromHex(signed.sealedHex);
		sealed[sealed.length - 1] ^= 0x01;
		await expect(session.open(sealed)).rejects.toMatchObject({ code: 'unreadable' });
	});

	it('a hello that is not one, and a peer speaking in our own role', async () => {
		const key = await requesterKey(fromHex(relay.requester.secretHex));
		const nonce = fromHex(relay.requester.nonceHex);
		await expect(completeSession(key, nonce, '{}', relay.label)).rejects.toBeInstanceOf(
			SecureSessionError
		);
		const hello = JSON.parse(relay.signer.hello) as Record<string, unknown>;
		await expect(
			completeSession(key, nonce, { ...hello, role: 'requester' }, relay.label)
		).rejects.toMatchObject({ code: 'wrong_role' });
		await expect(
			completeSession(key, nonce, { ...hello, nonce: b64url(new Uint8Array(8)) }, relay.label)
		).rejects.toMatchObject({ code: 'bad_hello' });
	});

	it('the label is in the derivation: the same keys give each channel its own code', async () => {
		const ble = cases.find((one) => one.label === 'vela-ble/1');
		const sameKeys = cases.find((one) => one.label === 'vela-relay/1' && one.rk === ble?.rk);
		expect(ble && sameKeys).toBeTruthy();
		if (!ble || !sameKeys) return;
		const key = await requesterKey(fromHex(ble.requester.secretHex));
		const nonce = fromHex(ble.requester.nonceHex);
		// Same secrets, same nonces, different label — and the vectors say the
		// code differs, so a session of one channel can never be one of the other.
		const overBle = await completeSession(key, nonce, ble.signer.hello, ble.label);
		expect(overBle.code).toBe(ble.code);
		expect(overBle.code).not.toBe(sameKeys.code);
	});
});

describe('the bytes helpers', () => {
	it('round-trip base64url without padding, and refuse what is not', () => {
		const bytes = Uint8Array.from([0xfb, 0xff, 0x00, 0x10]);
		expect(b64url(bytes)).toBe('-_8AEA');
		expect(unb64url(b64url(bytes))).toEqual(bytes);
		expect(unb64url('not base64!')).toBeNull();
	});
});
