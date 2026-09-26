/**
 * One-click bug-report proxy.
 *
 * The wallet app POSTs a scrubbed report here — its text and up to five
 * screenshots; this route creates (or +1s) a GitHub issue using a server-side
 * fine-grained PAT, so users who don't have a GitHub account can still file a
 * bug. The token NEVER reaches the client — it lives only as a Cloudflare secret
 * (`wrangler secret put GITHUB_BUG_TOKEN`). Since spec 081 deleted the five
 * dormant proxy routes it is the ONLY secret this Worker reads —
 * `ALCHEMY_API_KEY`, `PIMLICO_API_KEY` and `BUNDLER_PROVIDER` went with them,
 * and `GITHUB_BUG_REPO` is a default, not a secret.
 *
 * If the token isn't configured the route returns 503 `{ error: 'not_configured' }`
 * so the client transparently falls back to the prefilled-GitHub-URL path.
 *
 * What a report is, where its screenshots go (R2, the `DOWNLOADS` bucket under
 * `bug-attachments/`) and how it becomes an issue live in
 * `$lib/server/bug-report`. This file owns the secret and the rate limit.
 */
import { env } from '$env/dynamic/private';
import type { RequestHandler } from './$types';
import { fileReport, parseReport, type AttachmentBucket } from '$lib/server/bug-report';

const GITHUB_BUG_TOKEN = env.GITHUB_BUG_TOKEN ?? '';
const GITHUB_BUG_REPO = env.GITHUB_BUG_REPO ?? 'mondaylabsltd/vela-wallet';
const GITHUB_TIMEOUT_MS = 12_000;

/** Per-IP: at most N reports per window (best-effort, isolate-local). */
const RATE_LIMIT = 5;
const RATE_WINDOW_MS = 10 * 60 * 1000;

// Best-effort in-memory limiter. Cloudflare isolates are short-lived and not
// shared, so this caps a single abusive burst; a durable KV/DO limiter is the
// production upgrade if abuse becomes real (noted, not silently assumed).
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
		headers: { 'Content-Type': 'application/json' }
	});
}

export const POST: RequestHandler = async ({ request, getClientAddress, platform }) => {
	if (!GITHUB_BUG_TOKEN) {
		// Not provisioned yet → tell the client to use its URL fallback.
		return json({ error: 'not_configured' }, 503);
	}

	let ip = 'unknown';
	try {
		ip = getClientAddress();
	} catch {
		/* getClientAddress can throw in some adapters; degrade to shared bucket */
	}
	if (rateLimited(ip)) {
		return json({ error: 'rate_limited' }, 429);
	}

	const report = await parseReport(request);
	if (!report.ok) {
		return json({ error: report.error }, report.status);
	}

	const filed = await fileReport(report, {
		fetch: (input, init) => fetch(input, init),
		token: GITHUB_BUG_TOKEN,
		repo: GITHUB_BUG_REPO,
		timeoutMs: GITHUB_TIMEOUT_MS,
		bucket: platform?.env?.DOWNLOADS as AttachmentBucket | undefined,
		now: () => new Date(),
		uuid: () => crypto.randomUUID(),
		waitUntil: platform?.ctx ? (work) => platform.ctx.waitUntil(work) : undefined
	});
	return json(filed.body, filed.status);
};
