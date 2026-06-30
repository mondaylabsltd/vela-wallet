/**
 * One-click bug-report proxy.
 *
 * The wallet app POSTs a scrubbed report here; this route creates (or +1s) a
 * GitHub issue using a server-side fine-grained PAT, so users who don't have a
 * GitHub account can still file a bug. The token NEVER reaches the client — it
 * lives only as a Cloudflare secret (`wrangler secret put GITHUB_BUG_TOKEN`),
 * exactly like ALCHEMY_API_KEY in the sibling routes.
 *
 * If the token isn't configured the route returns 503 `{ error: 'not_configured' }`
 * so the client transparently falls back to the prefilled-GitHub-URL path.
 *
 * Hardening: per-IP rate limit, body size cap, fingerprint dedup (+1 comment on
 * an existing open issue instead of creating duplicates), upstream timeouts, and
 * stable safe error messages (never leaks the token or GitHub internals).
 */
import { env } from '$env/dynamic/private';
import type { RequestHandler } from './$types';
import { fetchWithTimeout } from '$lib/server/net';

const GITHUB_BUG_TOKEN = env.GITHUB_BUG_TOKEN ?? '';
const GITHUB_BUG_REPO = env.GITHUB_BUG_REPO ?? 'mondaylabsltd/vela-wallet';
const GITHUB_TIMEOUT_MS = 12_000;

/** Max accepted request body (bytes-ish) — a bug report is small; reject blobs. */
const MAX_BODY_CHARS = 16_000;
/** Per-IP: at most N reports per window (best-effort, isolate-local). */
const RATE_LIMIT = 5;
const RATE_WINDOW_MS = 10 * 60 * 1000;

interface ReportBody {
	what?: string;
	steps?: string;
	area?: string;
	environment?: string;
	diagnostics?: string;
	fingerprint?: string;
}

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

/** Keep a fingerprint to a short safe token for the dedup marker. */
function safeFingerprint(fp: string | undefined): string {
	return (fp ?? '').replace(/[^a-zA-Z0-9_-]/g, '').slice(0, 64) || 'none';
}

const GH_HEADERS = () => ({
	Authorization: `Bearer ${GITHUB_BUG_TOKEN}`,
	Accept: 'application/vnd.github+json',
	'X-GitHub-Api-Version': '2022-11-28',
	'User-Agent': 'vela-wallet-bug-reporter',
	'Content-Type': 'application/json'
});

export const POST: RequestHandler = async ({ request, getClientAddress }) => {
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

	const raw = await request.text();
	if (raw.length > MAX_BODY_CHARS) {
		return json({ error: 'too_large' }, 413);
	}
	let body: ReportBody;
	try {
		body = JSON.parse(raw);
	} catch {
		return json({ error: 'invalid_json' }, 400);
	}

	const what = (body.what ?? '').toString().trim();
	if (!what) {
		return json({ error: 'missing_description' }, 400);
	}

	const fp = safeFingerprint(body.fingerprint);
	const marker = `<!-- vela-fp:${fp} -->`;
	const title = `[bug] ${what.slice(0, 80)}${what.length > 80 ? '…' : ''}`;
	const issueBody = [
		marker,
		'> Filed from the in-app one-click reporter.',
		'',
		'### What happened',
		what,
		...(body.steps ? ['', '### Steps to reproduce', body.steps] : []),
		...(body.area ? ['', `**Area:** ${body.area}`] : []),
		...(body.environment ? ['', '### Environment', body.environment] : []),
		...(body.diagnostics ? ['', '### Diagnostics', '```', body.diagnostics, '```'] : [])
	].join('\n');

	try {
		// Dedup: if an open issue with this fingerprint exists, +1 it instead of
		// creating a duplicate — keeps the tracker usable as volume grows.
		// Dedup is best-effort: a search failure (flaky/timeout/permission) must NOT
		// block creating the report, so it has its own try/catch and falls through.
		if (fp !== 'none') {
			try {
				const q = encodeURIComponent(`repo:${GITHUB_BUG_REPO} is:issue is:open in:body "vela-fp:${fp}"`);
				const searchRes = await fetchWithTimeout(
					`https://api.github.com/search/issues?q=${q}&per_page=1`,
					{ headers: GH_HEADERS() },
					GITHUB_TIMEOUT_MS
				);
				if (searchRes.ok) {
					const data = (await searchRes.json()) as { items?: { number: number; html_url: string }[] };
					const existing = data.items?.[0];
					if (existing) {
						await fetchWithTimeout(
							`https://api.github.com/repos/${GITHUB_BUG_REPO}/issues/${existing.number}/comments`,
							{
								method: 'POST',
								headers: GH_HEADERS(),
								body: JSON.stringify({
									body: `➕ Another in-app report for the same issue.\n\n${what}${body.environment ? `\n\n${body.environment}` : ''}`
								})
							},
							GITHUB_TIMEOUT_MS
						);
						return json({ number: existing.number, url: existing.html_url, deduped: true });
					}
				} else {
					console.error(`[bug-report] dedup search non-ok: ${searchRes.status} — skipping dedup`);
				}
			} catch {
				console.error('[bug-report] dedup search failed — skipping dedup, will still create');
			}
		}

		const createUrl = `https://api.github.com/repos/${GITHUB_BUG_REPO}/issues`;
		const create = (labels?: string[]) =>
			fetchWithTimeout(
				createUrl,
				{ method: 'POST', headers: GH_HEADERS(), body: JSON.stringify({ title, body: issueBody, ...(labels ? { labels } : {}) }) },
				GITHUB_TIMEOUT_MS
			);

		let createRes = await create(['bug', 'in-app-report']);
		// A 422 here is usually a label the repo doesn't have (or the token can't
		// create). The report matters more than the labels — retry without them so
		// the issue still lands instead of silently dropping the user's report.
		if (createRes.status === 422) {
			console.error('[bug-report] create 422 (labels?) — retrying without labels');
			createRes = await create(undefined);
		}

		if (!createRes.ok) {
			// Log status only (never the token / full response) and return a stable error.
			console.error(`[bug-report] GitHub create failed: ${createRes.status}`);
			return json({ error: 'upstream_failed' }, 502);
		}
		const issue = (await createRes.json()) as { number: number; html_url: string };
		return json({ number: issue.number, url: issue.html_url, deduped: false });
	} catch (err) {
		const timedOut = err instanceof Error && err.name === 'TimeoutError';
		console.error(`[bug-report] failed: ${timedOut ? 'timeout' : 'network error'}`);
		return json({ error: timedOut ? 'upstream_timeout' : 'upstream_failed' }, 502);
	}
};
