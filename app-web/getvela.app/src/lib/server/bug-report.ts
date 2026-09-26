/**
 * The one-click bug report's rules — what a report is, where its screenshots
 * go, and how it becomes a GitHub issue.
 *
 * Kept out of the route so every rule is testable without GitHub, R2 or the
 * SvelteKit runtime: the route (`routes/api/bug-report/+server.ts`) only reads
 * the secret, applies the rate limit and hands the platform's pieces in.
 *
 * Screenshots: GitHub's REST API cannot attach an image to an issue — the
 * drag-and-drop upload on github.com is an internal endpoint no token can
 * call. So each screenshot is stored in R2 under {@link ATTACHMENT_PREFIX},
 * served by `routes/api/bug-report/attachments/[...key]`, and shown inline in
 * the issue as a markdown image. The founder chose public screenshots
 * (2026-09-26); every client says so beside the picker before anything is sent.
 *
 * Wire shape: still ONE `application/json` body — the report's text fields
 * plus `screenshots`, an array of plain base64 strings (no `data:` prefix).
 * Not multipart on purpose: SvelteKit refuses a cross-site `multipart/form-data`
 * POST in production (its CSRF origin check, skipped under `vite dev`), and a
 * native app sends no Origin at all. JSON is exempt from that check, is already
 * allowed through the CORS preflight, and is what every client sent before.
 * Base64 costs a third more bytes; a re-encoded phone screenshot is ~300 KB.
 */

/** The report's text, all fields together — a blob pasted into it is refused. */
export const MAX_REPORT_CHARS = 16_000;
/** At most this many screenshots per report. */
export const MAX_SCREENSHOTS = 5;
/**
 * Per screenshot, decoded, after the client re-encoded it (longest edge
 * ≤ 1920 px as JPEG, which also drops EXIF — a photo's location never ships).
 */
export const MAX_SCREENSHOT_BYTES = 2_000_000;
/**
 * The whole body: the text (worst case 4 bytes a char) + every screenshot in
 * base64 + a twentieth more, because some JSON writers escape `/` as `\/`
 * (Android's org.json does; base64 is ~1/64 slashes) + slack.
 */
export const MAX_BODY_BYTES =
	MAX_REPORT_CHARS * 4 +
	Math.ceil(((MAX_SCREENSHOT_BYTES * 4) / 3) * MAX_SCREENSHOTS * 1.05) +
	16_000;
/** Where screenshots live in the bucket. */
export const ATTACHMENT_PREFIX = 'bug-attachments/';
/** Where the issue links them — the one origin whose route serves them. */
export const ATTACHMENT_ORIGIN = 'https://getvela.app';

export interface ReportText {
	what: string;
	steps: string;
	area: string;
	environment: string;
	diagnostics: string;
	fingerprint: string;
}

export type ImageKind = 'jpg' | 'png' | 'webp';

export const CONTENT_TYPES: Record<ImageKind, string> = {
	jpg: 'image/jpeg',
	png: 'image/png',
	webp: 'image/webp'
};

/**
 * What the bytes ARE, from their magic number — never a type the client
 * claims. `null` for anything that is not a JPEG, PNG or WebP (an SVG, which
 * can carry script, never gets through).
 */
export function sniffImage(bytes: Uint8Array): ImageKind | null {
	if (bytes.length >= 3 && bytes[0] === 0xff && bytes[1] === 0xd8 && bytes[2] === 0xff)
		return 'jpg';
	const png = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
	if (bytes.length >= 8 && png.every((b, i) => bytes[i] === b)) return 'png';
	const ascii = (from: number, to: number) => String.fromCharCode(...bytes.slice(from, to));
	if (bytes.length >= 12 && ascii(0, 4) === 'RIFF' && ascii(8, 12) === 'WEBP') return 'webp';
	return null;
}

/** `2026-09/<uuid>.jpg` — the month keeps the bucket browsable; the uuid is unguessable. */
export function attachmentKey(kind: ImageKind, now: Date, uuid: string): string {
	const month = `${now.getUTCFullYear()}-${String(now.getUTCMonth() + 1).padStart(2, '0')}`;
	return `${month}/${uuid}.${kind}`;
}

const KEY_SHAPE =
	/^\d{4}-\d{2}\/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\.(jpg|png|webp)$/;

/** Only keys {@link attachmentKey} mints may be read back — nothing else in the bucket. */
export function isAttachmentKey(key: string): boolean {
	return KEY_SHAPE.test(key);
}

export interface Screenshot {
	bytes: Uint8Array;
	kind: ImageKind;
}

export type ParsedReport =
	| { ok: true; text: ReportText; screenshots: Screenshot[] }
	| { ok: false; status: number; error: string };

const refuse = (status: number, error: string): ParsedReport => ({ ok: false, status, error });

/** The body as text, or `null` past `cap` bytes — read no further than that. */
async function readCapped(request: Request, cap: number): Promise<string | null> {
	const declared = Number(request.headers.get('content-length') ?? '0');
	if (declared > cap) return null;
	if (!request.body) return '';
	const reader = request.body.getReader();
	const chunks: Uint8Array[] = [];
	let size = 0;
	for (;;) {
		const { done, value } = await reader.read();
		if (done) break;
		size += value.byteLength;
		if (size > cap) {
			await reader.cancel().catch(() => {});
			return null;
		}
		chunks.push(value);
	}
	const all = new Uint8Array(size);
	let at = 0;
	for (const chunk of chunks) {
		all.set(chunk, at);
		at += chunk.byteLength;
	}
	return new TextDecoder().decode(all);
}

function decodeBase64(data: string): Uint8Array | null {
	if (!/^[A-Za-z0-9+/]*={0,2}$/.test(data)) return null;
	try {
		const binary = atob(data);
		const bytes = new Uint8Array(binary.length);
		for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
		return bytes;
	} catch {
		return null;
	}
}

const field = (value: unknown): string => (typeof value === 'string' ? value : '');

/**
 * The server's own scrub of the fields the APP writes — `environment`,
 * `diagnostics`, `area` — the same rule the clients apply
 * (`app-web/vela-wallet/src/lib/services/bug-report.ts` `redact`): an address
 * becomes `[address]`, anything with a scheme becomes `[url]` (a self-hosted
 * RPC URL routinely carries an API key). The issue is public, so this does not
 * trust that every client, of every version, scrubbed first. What the person
 * typed (`what`, `steps`) is left as written: they can see it, and "sending to
 * 0x… failed" is often the report.
 */
export function redact(line: string): string {
	return line
		.replace(/0x[0-9a-fA-F]{40}\b/g, '[address]')
		.replace(/\b[a-zA-Z][a-zA-Z0-9+.-]*:\/\/\S+/g, '[url]');
}

/** A report from the request body, or the refusal the client gets back. */
export async function parseReport(request: Request): Promise<ParsedReport> {
	const raw = await readCapped(request, MAX_BODY_BYTES);
	if (raw === null) return refuse(413, 'too_large');
	let parsed: Record<string, unknown>;
	try {
		const value = JSON.parse(raw) as unknown;
		if (value === null || typeof value !== 'object' || Array.isArray(value))
			return refuse(400, 'invalid_json');
		parsed = value as Record<string, unknown>;
	} catch {
		return refuse(400, 'invalid_json');
	}

	const text: ReportText = {
		what: field(parsed.what).trim(),
		steps: field(parsed.steps),
		area: redact(field(parsed.area)),
		environment: redact(field(parsed.environment)),
		diagnostics: redact(field(parsed.diagnostics)),
		fingerprint: field(parsed.fingerprint)
	};
	const chars = Object.values(text).reduce((sum, value) => sum + value.length, 0);
	if (chars > MAX_REPORT_CHARS) return refuse(413, 'too_large');
	if (!text.what) return refuse(400, 'missing_description');

	const sent = parsed.screenshots ?? [];
	if (!Array.isArray(sent)) return refuse(400, 'invalid_screenshot');
	if (sent.length > MAX_SCREENSHOTS) return refuse(400, 'too_many_screenshots');
	const screenshots: Screenshot[] = [];
	for (const item of sent) {
		if (typeof item !== 'string') return refuse(400, 'invalid_screenshot');
		if (Math.floor((item.length * 3) / 4) > MAX_SCREENSHOT_BYTES + 2)
			return refuse(413, 'screenshot_too_large');
		const bytes = decodeBase64(item);
		if (bytes === null) return refuse(400, 'invalid_screenshot');
		if (bytes.length > MAX_SCREENSHOT_BYTES) return refuse(413, 'screenshot_too_large');
		const kind = sniffImage(bytes);
		if (kind === null) return refuse(415, 'unsupported_screenshot');
		screenshots.push({ bytes, kind });
	}
	return { ok: true, text, screenshots };
}

/** Keep a fingerprint to a short safe token for the dedup marker. */
export function safeFingerprint(fp: string): string {
	return fp.replace(/[^a-zA-Z0-9_-]/g, '').slice(0, 64) || 'none';
}

/** Markdown for the stored screenshots, one image per line. */
function screenshotsMarkdown(urls: string[], dropped: number): string[] {
	const lines =
		urls.length > 0
			? ['', '### Screenshots', ...urls.map((url, i) => `![Screenshot ${i + 1}](${url})`)]
			: [];
	if (dropped > 0) lines.push('', `_${dropped} screenshot(s) were sent but could not be stored._`);
	return lines;
}

/** The issue as GitHub will show it. */
export function issueBody(
	marker: string,
	text: ReportText,
	urls: string[],
	dropped: number
): string {
	return [
		marker,
		'> Filed from the in-app one-click reporter.',
		'',
		'### What happened',
		text.what,
		...(text.steps ? ['', '### Steps to reproduce', text.steps] : []),
		...screenshotsMarkdown(urls, dropped),
		...(text.area ? ['', `**Area:** ${text.area}`] : []),
		...(text.environment ? ['', '### Environment', text.environment] : []),
		...(text.diagnostics ? ['', '### Diagnostics', '```', text.diagnostics, '```'] : [])
	].join('\n');
}

/** The +1 comment on the open issue this report repeats — its screenshots included. */
export function dedupComment(text: ReportText, urls: string[], dropped: number): string {
	return [
		'➕ Another in-app report for the same issue.',
		'',
		text.what,
		...screenshotsMarkdown(urls, dropped),
		...(text.environment ? ['', text.environment] : [])
	].join('\n');
}

/** The slice of an R2 bucket the reporter uses. */
export interface AttachmentBucket {
	put(
		key: string,
		value: Uint8Array,
		options: { httpMetadata: { contentType: string } }
	): Promise<unknown>;
	delete(key: string): Promise<unknown>;
	get(key: string): Promise<{ body: ReadableStream; size: number } | null>;
}

export interface FilingDeps {
	fetch: typeof fetch;
	token: string;
	repo: string;
	timeoutMs: number;
	/** Absent in `vite dev` and on a Worker without the binding: screenshots are then dropped, the report still goes. */
	bucket?: AttachmentBucket;
	now: () => Date;
	uuid: () => string;
	/** For clean-up work that should outlive the response. */
	waitUntil?: (work: Promise<unknown>) => void;
}

export interface Filed {
	status: number;
	body:
		| {
				number: number;
				url: string;
				deduped: boolean;
				screenshots: number;
				screenshotsDropped: number;
		  }
		| { error: string };
}

/** Put each screenshot in the bucket; what could not be stored is counted, never fatal. */
async function storeScreenshots(
	screenshots: Screenshot[],
	deps: FilingDeps
): Promise<{ keys: string[]; dropped: number }> {
	if (screenshots.length === 0) return { keys: [], dropped: 0 };
	const bucket = deps.bucket;
	if (!bucket) return { keys: [], dropped: screenshots.length };
	const now = deps.now();
	const stored = await Promise.all(
		screenshots.map(async ({ bytes, kind }) => {
			const key = attachmentKey(kind, now, deps.uuid());
			try {
				await bucket.put(ATTACHMENT_PREFIX + key, bytes, {
					httpMetadata: { contentType: CONTENT_TYPES[kind] }
				});
				return key;
			} catch {
				return null;
			}
		})
	);
	const keys = stored.filter((key): key is string => key !== null);
	if (keys.length < screenshots.length)
		console.error(`[bug-report] ${screenshots.length - keys.length} screenshot(s) not stored`);
	return { keys, dropped: screenshots.length - keys.length };
}

/**
 * The report onto GitHub: a +1 comment on the open issue with the same
 * fingerprint, else a new issue. Screenshots are stored first so the issue can
 * show them; if the issue then fails they are deleted again — nothing public
 * is left behind for a report that never landed.
 */
export async function fileReport(
	report: { text: ReportText; screenshots: Screenshot[] },
	deps: FilingDeps
): Promise<Filed> {
	const { text } = report;
	const headers = {
		Authorization: `Bearer ${deps.token}`,
		Accept: 'application/vnd.github+json',
		'X-GitHub-Api-Version': '2022-11-28',
		'User-Agent': 'vela-wallet-bug-reporter',
		'Content-Type': 'application/json'
	};
	const call = (url: string, init: RequestInit = {}) =>
		deps.fetch(url, { ...init, headers, signal: AbortSignal.timeout(deps.timeoutMs) });

	const fp = safeFingerprint(text.fingerprint);
	const marker = `<!-- vela-fp:${fp} -->`;
	const title = `[bug] ${text.what.slice(0, 80)}${text.what.length > 80 ? '…' : ''}`;
	const { keys, dropped } = await storeScreenshots(report.screenshots, deps);
	const urls = keys.map((key) => `${ATTACHMENT_ORIGIN}/api/bug-report/attachments/${key}`);
	const filed = (number: number, url: string, deduped: boolean): Filed => ({
		status: 200,
		body: { number, url, deduped, screenshots: keys.length, screenshotsDropped: dropped }
	});
	const failed = (error: string): Filed => {
		const bucket = deps.bucket;
		if (bucket && keys.length > 0) {
			const cleanup = Promise.all(
				keys.map((key) => bucket.delete(ATTACHMENT_PREFIX + key).catch(() => {}))
			);
			if (deps.waitUntil) deps.waitUntil(cleanup);
		}
		return { status: 502, body: { error } };
	};

	try {
		// Dedup is best-effort: a failed search must not stop the report, so it
		// has its own try/catch and falls through to creating the issue.
		if (fp !== 'none') {
			try {
				const q = encodeURIComponent(`repo:${deps.repo} is:issue is:open in:body "vela-fp:${fp}"`);
				const searchRes = await call(`https://api.github.com/search/issues?q=${q}&per_page=1`);
				if (searchRes.ok) {
					const data = (await searchRes.json()) as {
						items?: { number: number; html_url: string }[];
					};
					const existing = data.items?.[0];
					if (existing) {
						const commentRes = await call(
							`https://api.github.com/repos/${deps.repo}/issues/${existing.number}/comments`,
							{
								method: 'POST',
								body: JSON.stringify({ body: dedupComment(text, urls, dropped) })
							}
						);
						// A comment that did not land is not a filed report — file it as a new issue instead.
						if (commentRes.ok) return filed(existing.number, existing.html_url, true);
						console.error(
							`[bug-report] dedup comment non-ok: ${commentRes.status} — filing a new issue`
						);
					}
				} else {
					console.error(`[bug-report] dedup search non-ok: ${searchRes.status} — skipping dedup`);
				}
			} catch {
				console.error('[bug-report] dedup failed — skipping dedup, will still create');
			}
		}

		const body = issueBody(marker, text, urls, dropped);
		const create = (labels?: string[]) =>
			call(`https://api.github.com/repos/${deps.repo}/issues`, {
				method: 'POST',
				body: JSON.stringify({ title, body, ...(labels ? { labels } : {}) })
			});
		let createRes = await create(['bug', 'in-app-report']);
		// A 422 is usually a label the repo lacks. The report matters more than
		// its labels — retry without them rather than drop it.
		if (createRes.status === 422) {
			console.error('[bug-report] create 422 (labels?) — retrying without labels');
			createRes = await create(undefined);
		}
		if (!createRes.ok) {
			// Status only — never the token or GitHub's response.
			console.error(`[bug-report] GitHub create failed: ${createRes.status}`);
			return failed('upstream_failed');
		}
		const issue = (await createRes.json()) as { number: number; html_url: string };
		return filed(issue.number, issue.html_url, false);
	} catch (err) {
		const timedOut = err instanceof Error && err.name === 'TimeoutError';
		console.error(`[bug-report] failed: ${timedOut ? 'timeout' : 'network error'}`);
		return failed(timedOut ? 'upstream_timeout' : 'upstream_failed');
	}
}

/** A stored screenshot, served as the image it was sniffed to be — or 404. */
export async function serveAttachment(
	key: string,
	bucket: AttachmentBucket | undefined
): Promise<Response> {
	const notFound = () =>
		new Response('Not found', { status: 404, headers: { 'Cache-Control': 'no-store' } });
	if (!bucket || !isAttachmentKey(key)) return notFound();
	const object = await bucket.get(ATTACHMENT_PREFIX + key);
	if (!object) return notFound();
	const kind = key.slice(key.lastIndexOf('.') + 1) as ImageKind;
	return new Response(object.body, {
		headers: {
			'Content-Type': CONTENT_TYPES[kind],
			'Content-Length': String(object.size),
			// A key is never reused: the bytes behind it never change.
			'Cache-Control': 'public, max-age=31536000, immutable',
			'X-Content-Type-Options': 'nosniff',
			'Content-Security-Policy': "default-src 'none'; sandbox",
			'Content-Disposition': 'inline'
		}
	});
}
