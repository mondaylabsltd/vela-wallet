/**
 * The Worker's half of spec 065: read the latest Release, and hand a person
 * its file — from R2 when it can, from GitHub when it cannot.
 *
 * Everything that touches the outside (GitHub, the edge cache, the bucket, the
 * clock) arrives through `deps`, so the rules below are tested as rules:
 *
 * 1. GitHub Releases is the source of truth; R2 is a mirror of it, filled on
 *    first request and never published to directly (A5).
 * 2. A mirrored file is served only if its recorded SHA-256 is the one the
 *    Release's own `SHA256SUMS*` names *now*. That one comparison is both the
 *    integrity check and the eviction: replace a file on the Release and the
 *    old object stops matching, so it is fetched again and overwritten.
 * 3. A file the Release gives no checksum for is never mirrored — nothing to
 *    verify it against — and is served by redirect.
 * 4. Whatever goes wrong on the mirror path ends in the GitHub redirect: a slow
 *    download beats none.
 * 5. GitHub being down does not take the page down: the last good list is kept
 *    and served stale (FR-002) — and SAYS SO in the log. v0.9.5 shipped its
 *    macOS images eleven minutes after its Release was created, and the page
 *    offered them as "coming shortly" for hours: the list fetched in between
 *    was kept, every refresh failed, and nothing anywhere said a word.
 * 6. The refresh does not depend on one rate-limited API. GitHub allows 60
 *    anonymous calls an hour PER IP, and a Worker's IP is shared with the rest
 *    of Cloudflare, so exhausting it is normal, not exceptional. Two defences:
 *    a conditional request (a 304 is free — GitHub does not count it), and a
 *    fallback that reads the release through plain file downloads, which have
 *    no such limit.
 */
import {
	matchFiles,
	type DownloadsManifest,
	type PlatformId,
	type ReleaseFile
} from '$lib/downloads/platforms';

const REPO = 'mondaylabsltd/vela-wallet';
const RELEASES_API = `https://api.github.com/repos/${REPO}/releases?per_page=10`;

/** How long a list is believed without asking GitHub again. */
export const FRESH_MS = 5 * 60_000;
/** Unauthenticated GitHub allows 60 requests/hour per address; a list request costs up to 3. */
const GITHUB_TIMEOUT_MS = 8_000;

export interface ReleaseInfo {
	tag: string;
	version: string;
	files: ReleaseFile[];
	/** file name → lowercase hex SHA-256, from every `SHA256SUMS*` on the Release. */
	checksums: Record<string, string>;
	fetchedAt: number;
	/** The API's ETag, so the next refresh can ask for a free 304. */
	etag?: string;
}

/** Somewhere a list outlives this isolate: the edge cache in production. */
export interface ReleaseStore {
	read(): Promise<ReleaseInfo | null>;
	write(info: ReleaseInfo): Promise<void>;
}

/** The slice of an R2 bucket this file uses — small enough to fake honestly. */
export interface MirrorObject {
	body: ReadableStream;
	size: number;
	customMetadata?: Record<string, string>;
	range?: { offset: number; length: number };
}
export interface Mirror {
	get(key: string, options?: { range?: Headers }): Promise<MirrorObject | null>;
	put(
		key: string,
		body: ReadableStream,
		options: { sha256: string; customMetadata: Record<string, string> }
	): Promise<unknown>;
}

export interface Deps {
	fetch: typeof fetch;
	now: () => number;
	store?: ReleaseStore;
	mirror?: Mirror;
	/** Workers' `ctx.waitUntil`: the mirror is filled after the person has their bytes. */
	waitUntil?: (work: Promise<unknown>) => void;
	/** R2 refuses a stream of unknown length; Workers' `FixedLengthStream` gives it one. */
	fixedLength?: (body: ReadableStream, size: number) => ReadableStream;
}

let remembered: ReleaseInfo | null = null;

/** Test seam: one isolate's memory must not leak between cases. */
export const forgetRelease = () => {
	remembered = null;
};

/** `<hex>  ./name` or `<hex>  name` or `<hex> *name`, one per line. */
export function parseChecksums(text: string): Record<string, string> {
	const out: Record<string, string> = {};
	for (const line of text.split('\n')) {
		const m = /^([0-9a-fA-F]{64})\s+\*?(?:\.\/)?(\S.*?)\s*$/.exec(line);
		if (m) out[m[2]] = m[1].toLowerCase();
	}
	return out;
}

interface GithubRelease {
	tag_name: string;
	draft: boolean;
	assets: { name: string; size: number; browser_download_url: string }[];
}

const GITHUB_HEADERS = { 'User-Agent': 'getvela.app', Accept: 'application/vnd.github+json' };

/** Every `SHA256SUMS*` on a release, merged. Plain downloads: no rate limit. */
async function fetchChecksums(
	deps: Deps,
	urls: { name: string; url: string }[]
): Promise<Record<string, string>> {
	const checksums: Record<string, string> = {};
	for (const { url } of urls) {
		const sums = await deps.fetch(url, {
			headers: GITHUB_HEADERS,
			signal: AbortSignal.timeout(GITHUB_TIMEOUT_MS)
		});
		// A checksum file that will not load only costs those files their mirror
		// (rule 3); it must not cost the page its list.
		if (sums.ok) Object.assign(checksums, parseChecksums(await sums.text()));
	}
	return checksums;
}

const assetUrl = (tag: string, name: string) =>
	`https://github.com/${REPO}/releases/download/${tag}/${encodeURIComponent(name)}`;

/**
 * The release, read WITHOUT the API (rule 6): the `releases/latest` redirect
 * names the tag, and the tag's own `SHA256SUMS*` name every file on it — which
 * is exactly the set this page is allowed to offer anyway, since a file with no
 * published checksum is never mirrored.
 *
 * `latest` skips pre-releases, and every release so far is one; when it has
 * nothing to point at, the caller's stale list is still better than an empty
 * page, so this throws rather than inventing one.
 */
async function fetchWithoutApi(deps: Deps): Promise<ReleaseInfo> {
	// `redirect: 'manual'`, and the tag read from the Location header rather than
	// from the final URL: the header is the answer itself, and it is there
	// whether or not anything followed it.
	const res = await deps.fetch(`https://github.com/${REPO}/releases/latest`, {
		redirect: 'manual',
		signal: AbortSignal.timeout(GITHUB_TIMEOUT_MS)
	});
	const where = res.headers.get('Location') ?? res.url;
	const tag = /\/releases\/tag\/(v\d+\.\d+\.\d+)/.exec(where)?.[1];
	if (!tag) throw new Error(`GitHub releases/latest: no vX.Y.Z tag in "${where}"`);

	const checksums = await fetchChecksums(
		deps,
		['SHA256SUMS', 'SHA256SUMS-macos'].map((name) => ({ name, url: assetUrl(tag, name) }))
	);
	const names = Object.keys(checksums).filter((name) => !name.startsWith('SHA256SUMS'));
	if (!names.length) throw new Error(`GitHub ${tag}: no checksummed files`);

	// The size is shown beside each download, so it is worth one HEAD each —
	// and a size that cannot be had is 0, not a reason to hide the file.
	const files = await Promise.all(
		names.map(async (name) => {
			const url = assetUrl(tag, name);
			let size = 0;
			try {
				const head = await deps.fetch(url, {
					method: 'HEAD',
					redirect: 'follow',
					signal: AbortSignal.timeout(GITHUB_TIMEOUT_MS)
				});
				size = Number(head.headers.get('Content-Length')) || 0;
			} catch {
				// Leave it at 0.
			}
			return { name, size, url };
		})
	);
	return { tag, version: tag.slice(1), files, checksums, fetchedAt: deps.now() };
}

async function fetchFromGithub(deps: Deps, previous: ReleaseInfo | null): Promise<ReleaseInfo> {
	const headers: Record<string, string> = { ...GITHUB_HEADERS };
	// A conditional request costs nothing against the hourly limit when the
	// answer is 304, which it is most of the time: releases are rare.
	if (previous?.etag) headers['If-None-Match'] = previous.etag;

	const res = await deps.fetch(RELEASES_API, {
		headers,
		signal: AbortSignal.timeout(GITHUB_TIMEOUT_MS)
	});
	if (res.status === 304 && previous) return { ...previous, fetchedAt: deps.now() };
	if (!res.ok) throw new Error(`GitHub releases: ${res.status}`);

	// "Latest" includes pre-releases — every release so far is one — so this is
	// the list's first published entry, not `/releases/latest`, which skips them.
	// The tag shape also skips the per-shell tags (`desktop-v*`) of before 064.
	const releases = (await res.json()) as GithubRelease[];
	const latest = releases.find((r) => !r.draft && /^v\d+\.\d+\.\d+$/.test(r.tag_name));
	if (!latest) throw new Error('GitHub releases: no published vX.Y.Z release');

	const checksums = await fetchChecksums(
		deps,
		latest.assets
			.filter((a) => a.name.startsWith('SHA256SUMS'))
			.map((a) => ({ name: a.name, url: a.browser_download_url }))
	);

	return {
		tag: latest.tag_name,
		version: latest.tag_name.slice(1),
		files: latest.assets
			.filter((a) => !a.name.startsWith('SHA256SUMS'))
			.map((a) => ({ name: a.name, size: a.size, url: a.browser_download_url })),
		checksums,
		fetchedAt: deps.now(),
		etag: res.headers.get('ETag') ?? undefined
	};
}

export async function loadRelease(deps: Deps): Promise<ReleaseInfo> {
	const fresh = (info: ReleaseInfo | null) => info && deps.now() - info.fetchedAt < FRESH_MS;

	if (fresh(remembered)) return remembered!;
	const stored = (await deps.store?.read().catch(() => null)) ?? null;
	if (stored && (!remembered || stored.fetchedAt > remembered.fetchedAt)) remembered = stored;
	if (fresh(remembered)) return remembered!;

	const reasons: string[] = [];
	// The API first — it is the one source that carries sizes and knows a draft
	// from a pre-release — then the plain-download route, which no rate limit
	// touches (rule 6).
	for (const attempt of [() => fetchFromGithub(deps, remembered), () => fetchWithoutApi(deps)]) {
		try {
			remembered = await attempt();
			await deps.store?.write(remembered).catch(() => {});
			return remembered;
		} catch (error) {
			reasons.push(String(error));
		}
	}

	if (remembered) {
		// Serving a stale list is the right answer and a silent one is not: this
		// is how a page goes on saying "coming shortly" about a file that has
		// been published for hours.
		const minutes = Math.round((deps.now() - remembered.fetchedAt) / 60_000);
		console.error(
			`[downloads] could not refresh the release list (${reasons.join('; ')}) — serving ${remembered.tag} as read ${minutes} minutes ago`
		);
		return remembered;
	}
	throw new Error(`no release list: ${reasons.join('; ')}`);
}

export function manifestOf(info: ReleaseInfo): DownloadsManifest {
	const files: DownloadsManifest['files'] = {};
	for (const [id, file] of Object.entries(matchFiles(info.files))) {
		files[id as PlatformId] = { name: file.name, size: file.size };
	}
	return { version: info.version, tag: info.tag, files };
}

const redirect = (location: string) =>
	new Response(null, { status: 302, headers: { Location: location, 'Cache-Control': 'no-store' } });

/**
 * Where a person lands when the file is not there: the page, saying so (FR-001)
 * — and the page they came from, so someone reading 日本語 is not answered in
 * English. Only a same-origin `/…/get-started` referrer is believed.
 */
export function unavailable(id: string, request?: Request): Response {
	let page = '/get-started';
	try {
		const from = new URL(request?.headers.get('Referer') ?? '');
		if (
			request &&
			from.origin === new URL(request.url).origin &&
			/^(\/[A-Za-z-]+)?\/get-started\/?$/.test(from.pathname)
		) {
			page = from.pathname;
		}
	} catch {
		// No referrer, or not a URL: the English page.
	}
	return redirect(`${page}?unavailable=${encodeURIComponent(id)}#downloads`);
}

function fileHeaders(file: ReleaseFile, sha256: string, length: number): Headers {
	return new Headers({
		'Content-Type': 'application/octet-stream',
		'Content-Length': String(length),
		'Content-Disposition': `attachment; filename="${file.name}"`,
		'Accept-Ranges': 'bytes',
		ETag: `"${sha256}"`,
		// `/download/<platform>` is the same URL for every version (and through
		// 0.9.3 so was the Flatpak's file name) — so the URL is never cached; the
		// object is.
		'Cache-Control': 'no-cache'
	});
}

export async function serveDownload(
	id: PlatformId,
	request: Request,
	deps: Deps
): Promise<Response> {
	let info: ReleaseInfo;
	try {
		info = await loadRelease(deps);
	} catch {
		// No list now and none remembered: GitHub's own page is all that is left.
		return redirect(`https://github.com/${REPO}/releases`);
	}

	const file = matchFiles(info.files)[id];
	if (!file) return unavailable(id, request);

	const sha256 = info.checksums[file.name];
	if (!deps.mirror || !sha256) return redirect(file.url);

	const key = `${info.tag}/${file.name}`;
	try {
		const wantsRange = request.headers.has('Range');
		const kept = await deps.mirror.get(key, wantsRange ? { range: request.headers } : undefined);
		if (kept && kept.customMetadata?.sha256 === sha256) {
			if (wantsRange && kept.range && kept.range.length < kept.size) {
				const { offset, length } = kept.range;
				const headers = fileHeaders(file, sha256, length);
				headers.set('Content-Range', `bytes ${offset}-${offset + length - 1}/${kept.size}`);
				return new Response(kept.body, { status: 206, headers });
			}
			return new Response(kept.body, { headers: fileHeaders(file, sha256, kept.size) });
		}
		// A kept object that no longer matches is NOT served; its body is dropped.
		await kept?.body.cancel().catch(() => {});

		const upstream = await deps.fetch(file.url, { redirect: 'follow' });
		if (!upstream.ok || !upstream.body) return redirect(file.url);

		if (!deps.waitUntil || !deps.fixedLength) {
			return new Response(upstream.body, { headers: fileHeaders(file, sha256, file.size) });
		}
		const [toPerson, toMirror] = upstream.body.tee();
		deps.waitUntil(
			deps.mirror
				// `sha256` makes R2 itself refuse the object if the bytes that
				// arrived are not the published ones — a truncated fetch must not
				// become the file everyone downloads from then on.
				.put(key, deps.fixedLength(toMirror, file.size), { sha256, customMetadata: { sha256 } })
				.catch((error) => console.error(`[downloads] mirror of ${key} not kept:`, error))
		);
		return new Response(toPerson, { headers: fileHeaders(file, sha256, file.size) });
	} catch (error) {
		console.error(`[downloads] mirror path failed for ${key}:`, error);
		return redirect(file.url);
	}
}
