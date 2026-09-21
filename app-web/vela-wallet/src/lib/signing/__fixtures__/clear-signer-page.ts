/**
 * Test stand-ins for the Clear Signer (spec 071): a passkey that is a P-256
 * key, the answer the page sends for a digest — built exactly as the core's
 * own suite builds it (`rust/crates/vela-core/tests/clear_signer.rs`) — and a
 * browser whose `window.open` hands back a popup the test speaks for.
 * Tests only; nothing in the app imports this.
 */
import { p256 } from '@noble/curves/p256';
import { sha256 } from '@noble/hashes/sha256';
import type { ClearSignerKey } from '$lib/core/kernels';
import type { ClearSignerHost } from '../clear-signer';

export const SIGNER_ORIGIN = 'https://sign.getvela.app';

export const hex = (bytes: Uint8Array) =>
	Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');

export const b64url = (bytes: Uint8Array) =>
	btoa(String.fromCharCode(...bytes))
		.replace(/\+/g, '-')
		.replace(/\//g, '_')
		.replace(/=+$/, '');

/** A passkey stand-in: a P-256 key and the credential id it answers as. */
export function passkey(seed: number, credential: number[]) {
	const priv = new Uint8Array(32).fill(seed);
	return {
		priv,
		credential: Uint8Array.from(credential),
		key: {
			credentialId: hex(Uint8Array.from(credential)),
			publicKeyHex: hex(p256.getPublicKey(priv, false))
		} satisfies ClearSignerKey
	};
}

/** What the page returns for `digest`, signed by `signer`. `flags` 0x05 = UP + UV. */
export function answer(
	signer: ReturnType<typeof passkey>,
	digest: Uint8Array,
	options: { flags?: number; origin?: string } = {}
) {
	const authenticatorData = Uint8Array.from([
		...sha256(new TextEncoder().encode('getvela.app')),
		options.flags ?? 0x05,
		0,
		0,
		0,
		7
	]);
	const clientDataJSON = new TextEncoder().encode(
		`{"type":"webauthn.get","challenge":"${b64url(digest)}","origin":"${options.origin ?? SIGNER_ORIGIN}","crossOrigin":false}`
	);
	const signed = sha256(Uint8Array.from([...authenticatorData, ...sha256(clientDataJSON)]));
	const signature = p256.sign(signed, signer.priv).toCompactRawBytes();
	return {
		credentialId: b64url(signer.credential),
		signature: '0x' + hex(signature),
		authenticatorData: '0x' + hex(authenticatorData),
		clientDataJSON: '0x' + hex(clientDataJSON),
		userVerified: true
	};
}

/**
 * The browser around the channel: `open` hands out a popup whose
 * `postMessage` is recorded, and the page speaks by `say` — a message event
 * from whichever origin and window the test chooses (the signer's and the
 * popup, unless told otherwise).
 */
export function fakeBrowser(options: { blocked?: boolean; origin?: string } = {}) {
	const origin = options.origin ?? SIGNER_ORIGIN;
	const listeners = new Set<(event: MessageEvent) => void>();
	const posted: { message: Record<string, unknown>; targetOrigin: string }[] = [];
	const opened: string[] = [];
	let blocked = options.blocked ?? false;
	const popup = {
		closed: false,
		focused: 0,
		postMessage(message: Record<string, unknown>, targetOrigin: string) {
			posted.push({ message, targetOrigin });
		},
		focus() {
			popup.focused += 1;
		},
		close() {
			popup.closed = true;
		}
	};
	const host: ClearSignerHost = {
		open(url) {
			opened.push(url);
			if (blocked) return null;
			popup.closed = false;
			return popup as unknown as Window;
		},
		addEventListener: (_type, listener) => void listeners.add(listener),
		removeEventListener: (_type, listener) => void listeners.delete(listener)
	};
	return {
		host,
		popup,
		posted,
		opened,
		listeners,
		unblock: () => (blocked = false),
		say(data: unknown, from: { origin?: string; source?: unknown } = {}) {
			const event = {
				data,
				origin: from.origin ?? origin,
				source: 'source' in from ? from.source : popup
			} as unknown as MessageEvent;
			for (const listener of [...listeners]) listener(event);
		},
		/** The intent the wallet posted, once the page said ready. */
		intent: () =>
			posted.find((entry) => entry.message.vela === 'intent')?.message as
				| { id: string; intent: Record<string, unknown>; context: Record<string, unknown> }
				| undefined,
		/** The id the wallet gave this request. */
		id: () => posted.find((entry) => entry.message.vela === 'intent')?.message.id as string
	};
}
