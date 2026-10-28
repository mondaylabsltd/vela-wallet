/**
 * Relay tokens for PeerConnect, issued to Vela's own users.
 *
 * The public relay (`wss://relay.peerconnect.dev`) admits a connection only
 * with a token. The web wallet opens PeerConnect rooms when an outside owner
 * of a multisig signs with their own wallet: Vela on one side, the wallet's
 * browser on Vela's `/connect` page on the other, each bringing a token. The
 * people never see PeerConnect, so Vela's server asks for a token per device
 * with the app secret (PeerConnect's docs/self-serve.md, "Tokens from the
 * developer's server"). The secret is a Worker secret
 * (`wrangler secret put PEERCONNECT_APP_SECRET`) and never reaches a browser.
 *
 * The device names itself with a random ID it keeps; the same ID gets the
 * same token (valid a year) until it is rotated. A token lets its holder use
 * the relay and nothing else: rooms are end-to-end encrypted, the relay reads
 * nothing. So the route needs no sign-in — but every new ID counts against
 * the app's user limit, which is why each address may introduce only a few.
 *
 * Pure: the route supplies `fetch`, the secret and the clock, so the whole
 * exchange runs under test without the network.
 */

export const PEERCONNECT_SITE = 'https://peerconnect.dev';
export const PEERCONNECT_APP_ID = 'app_ezb5erytt5mrzkyy';
export const UPSTREAM_TIMEOUT_MS = 8_000;

/** What a device sends: its own random ID, lowercase base32/hex, 26–64 characters. */
const DEVICE = /^[a-z0-9]{26,64}$/;

export type DeviceRequest =
	| { ok: true; device: string }
	| { ok: false; status: number; error: string };

export async function parseDevice(request: Request): Promise<DeviceRequest> {
	const body: unknown = await request.json().catch(() => null);
	const device =
		body && typeof body === 'object' ? (body as Record<string, unknown>).device : undefined;
	if (typeof device !== 'string' || !DEVICE.test(device)) {
		return { ok: false, status: 400, error: 'bad_device' };
	}
	return { ok: true, device };
}

/** PeerConnect's user ID for a device: ours, namespaced so other kinds can follow. */
export function userId(device: string): string {
	return `web:${device}`;
}

export interface IssueDeps {
	fetch: typeof fetch;
	secret: string;
	appId?: string;
	site?: string;
	timeoutMs?: number;
}

export type Issued =
	| { status: 200; body: { token: string; expires_at: number; relay_url: string } }
	| { status: 409 | 502 | 503; body: { error: string } };

/** Ask PeerConnect for the device's token. */
export async function issueToken(device: string, deps: IssueDeps): Promise<Issued> {
	const site = deps.site ?? PEERCONNECT_SITE;
	const appId = deps.appId ?? PEERCONNECT_APP_ID;
	let res: Response;
	try {
		res = await deps.fetch(`${site}/api/apps/${appId}/tokens`, {
			method: 'POST',
			headers: {
				authorization: `Bearer ${deps.secret}`,
				'content-type': 'application/json'
			},
			body: JSON.stringify({ user: userId(device) }),
			signal: AbortSignal.timeout(deps.timeoutMs ?? UPSTREAM_TIMEOUT_MS)
		});
	} catch {
		return { status: 503, body: { error: 'unavailable' } };
	}
	// The app's user limit is reached: the client falls back to a trial token.
	if (res.status === 409) return { status: 409, body: { error: 'user_limit' } };
	if (!res.ok) return { status: res.status >= 500 ? 503 : 502, body: { error: 'upstream' } };
	const answer = (await res.json().catch(() => null)) as Record<string, unknown> | null;
	if (
		!answer ||
		typeof answer.token !== 'string' ||
		typeof answer.expires_at !== 'number' ||
		typeof answer.relay_url !== 'string'
	) {
		return { status: 502, body: { error: 'upstream' } };
	}
	// Only what the device needs: not the client ID PeerConnect keeps for it.
	return {
		status: 200,
		body: { token: answer.token, expires_at: answer.expires_at, relay_url: answer.relay_url }
	};
}
