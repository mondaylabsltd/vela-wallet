/**
 * A PeerConnect relay token for one device of the web wallet.
 *
 * What it is for and why it needs no sign-in live in
 * `$lib/server/peerconnect-token`. This file owns the secret and the rate
 * limit. Without the secret the route answers 503 `{ error: 'not_configured' }`
 * and the wallet falls back to PeerConnect's public trial token.
 */
import { env } from '$env/dynamic/private';
import type { RequestHandler } from './$types';
import { issueToken, parseDevice } from '$lib/server/peerconnect-token';

/** Per-IP: at most N token requests per window (best-effort, isolate-local). */
const RATE_LIMIT = 6;
const RATE_WINDOW_MS = 10 * 60 * 1000;

// The same best-effort limiter as bug-report's: isolates are short-lived and
// not shared, so it caps a burst, not a patient abuser. A device asks once a
// year; a durable limiter is the upgrade if the app's user count says so.
const hits = new Map<string, number[]>();

function rateLimited(ip: string): boolean {
	const now = Date.now();
	const arr = (hits.get(ip) ?? []).filter((t) => now - t < RATE_WINDOW_MS);
	if (arr.length >= RATE_LIMIT) {
		hits.set(ip, arr);
		return true;
	}
	arr.push(now);
	hits.set(ip, arr);
	return false;
}

function json(body: unknown, status = 200): Response {
	return new Response(JSON.stringify(body), {
		status,
		headers: { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' }
	});
}

export const POST: RequestHandler = async ({ request, getClientAddress }) => {
	const secret = env.PEERCONNECT_APP_SECRET ?? '';
	if (!secret) return json({ error: 'not_configured' }, 503);

	let ip = 'unknown';
	try {
		ip = getClientAddress();
	} catch {
		/* getClientAddress can throw in some adapters; degrade to shared bucket */
	}
	if (rateLimited(ip)) return json({ error: 'rate_limited' }, 429);

	const parsed = await parseDevice(request);
	if (!parsed.ok) return json({ error: parsed.error }, parsed.status);

	const issued = await issueToken(parsed.device, {
		fetch: (input, init) => fetch(input, init),
		secret,
		appId: env.PEERCONNECT_APP_ID || undefined
	});
	return json(issued.body, issued.status);
};
