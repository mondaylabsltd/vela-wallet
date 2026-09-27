import { beforeEach, describe, expect, it } from 'vitest';
import {
	FRESH_MS,
	forgetRelease,
	loadRelease,
	manifestOf,
	parseChecksums,
	serveDownload,
	type Deps,
	type Mirror,
	type MirrorObject,
	type ReleaseInfo,
	type ReleaseStore
} from './downloads';

const EXE = 'VelaWallet-Setup-0.9.3-x64.exe';
const DMG = 'VelaWallet-0.9.3-macos-arm64.dmg';
const EXE_BYTES = new TextEncoder().encode('the installer, all of it');
const EXE_SHA = 'a'.repeat(64);
const asset = (name: string, size = EXE_BYTES.length) => ({
	name,
	size,
	browser_download_url: `https://github.test/dl/${name}`
});

/** A GitHub that can be switched off, rate-limited, or asked conditionally. */
function github(
	options: {
		assets?: ReturnType<typeof asset>[];
		sums?: string;
		etag?: string;
		apiStatus?: number;
	} = {}
) {
	const state = { down: false, notModified: false, conditional: false, calls: [] as string[] };
	const assets = options.assets ?? [asset(EXE), asset('SHA256SUMS', 90)];
	const sums = options.sums ?? `${EXE_SHA}  ./${EXE}\n`;
	const fetcher: typeof fetch = async (input, init) => {
		const url = String(input);
		state.calls.push(url);
		if (state.down) throw new Error('connect timeout');
		const headers = new Headers(init?.headers);
		if (url.includes('api.github.com')) {
			if (headers.has('If-None-Match')) state.conditional = true;
			if (state.notModified) return new Response(null, { status: 304 });
			if (options.apiStatus) return new Response('rate limited', { status: options.apiStatus });
			return Response.json(
				[
					{ tag_name: 'v0.9.4', draft: true, assets: [] },
					{ tag_name: 'desktop-v0.1.1', draft: false, assets: [] },
					{ tag_name: 'v0.9.3', draft: false, assets },
					{ tag_name: 'v0.9.2', draft: false, assets: [] }
				],
				options.etag ? { headers: { ETag: options.etag } } : undefined
			);
		}
		// The no-API route: `releases/latest` redirects to the tag's page, and
		// every file is reachable at a name-shaped URL under that tag.
		if (url.endsWith('/releases/latest')) {
			return Response.redirect(
				`https://github.com/${'mondaylabsltd/vela-wallet'}/releases/tag/v0.9.3`,
				302
			);
		}
		if (url.endsWith('/SHA256SUMS')) return new Response(sums);
		if (url.endsWith('/SHA256SUMS-macos')) return new Response('not found', { status: 404 });
		if (url.endsWith(`/${EXE}`)) {
			return init?.method === 'HEAD'
				? new Response(null, { headers: { 'Content-Length': String(EXE_BYTES.length) } })
				: new Response(EXE_BYTES as BodyInit);
		}
		return new Response('not found', { status: 404 });
	};
	return { state, fetcher };
}

function memoryStore(): ReleaseStore & { kept: ReleaseInfo | null } {
	return {
		kept: null,
		async read() {
			return this.kept;
		},
		async write(info) {
			this.kept = info;
		}
	};
}

/** A bucket that records what it was told to insist on. It does NOT verify the
 * hash itself — R2 does; that half is exercised against a real local R2, not here. */
function bucket() {
	const objects = new Map<string, { bytes: Uint8Array; customMetadata: Record<string, string> }>();
	const puts: { key: string; sha256: string }[] = [];
	const mirror: Mirror = {
		async get(key) {
			const o = objects.get(key);
			if (!o) return null;
			return {
				body: new Response(o.bytes as BodyInit).body!,
				size: o.bytes.length,
				customMetadata: o.customMetadata
			} satisfies MirrorObject;
		},
		async put(key, body, options) {
			puts.push({ key, sha256: options.sha256 });
			const bytes = new Uint8Array(await new Response(body).arrayBuffer());
			objects.set(key, { bytes, customMetadata: options.customMetadata });
		}
	};
	return { mirror, objects, puts };
}

function depsWith(
	over: Partial<Deps> & { fetch: typeof fetch }
): Deps & { clock: { t: number }; pending: Promise<unknown>[] } {
	const clock = { t: 1_000_000 };
	const pending: Promise<unknown>[] = [];
	return {
		now: () => clock.t,
		waitUntil: (work) => pending.push(work),
		fixedLength: (body) => body,
		clock,
		pending,
		...over
	};
}

const get = (headers?: Record<string, string>) =>
	new Request('https://getvela.app/download/x', { headers });

beforeEach(forgetRelease);

describe('parseChecksums', () => {
	it('reads the three spellings sha256sum and shasum produce', () => {
		expect(
			parseChecksums(
				`${'A'.repeat(64)}  ./a.exe\n${'b'.repeat(64)}  b.dmg\n${'c'.repeat(64)} *c.deb\n\nnoise\n`
			)
		).toEqual({ 'a.exe': 'a'.repeat(64), 'b.dmg': 'b'.repeat(64), 'c.deb': 'c'.repeat(64) });
	});
});

describe('loadRelease', () => {
	it('takes the first PUBLISHED vX.Y.Z — a pre-release counts, a draft and an old per-shell tag do not', async () => {
		const gh = github();
		const info = await loadRelease(depsWith({ fetch: gh.fetcher }));
		expect(info.tag).toBe('v0.9.3');
		expect(info.version).toBe('0.9.3');
		expect(info.files.map((f) => f.name)).toEqual([EXE]);
		expect(info.checksums[EXE]).toBe(EXE_SHA);
	});

	it('does not ask GitHub again while the list is fresh, and does once it is not', async () => {
		const gh = github();
		const deps = depsWith({ fetch: gh.fetcher });
		await loadRelease(deps);
		const afterFirst = gh.state.calls.length;
		await loadRelease(deps);
		expect(gh.state.calls.length).toBe(afterFirst);
		deps.clock.t += FRESH_MS + 1;
		await loadRelease(deps);
		expect(gh.state.calls.length).toBeGreaterThan(afterFirst);
	});

	it('GitHub down: serves the last good list, from another isolate if need be (FR-002)', async () => {
		const gh = github();
		const store = memoryStore();
		const deps = depsWith({ fetch: gh.fetcher, store });
		await loadRelease(deps);

		forgetRelease(); // a new isolate: only the store remembers
		gh.state.down = true;
		deps.clock.t += FRESH_MS * 100;
		const info = await loadRelease(deps);
		expect(info.tag).toBe('v0.9.3');
	});

	it('GitHub down with nothing remembered: says so rather than inventing a list', async () => {
		const gh = github();
		gh.state.down = true;
		await expect(loadRelease(depsWith({ fetch: gh.fetcher }))).rejects.toThrow();
	});

	it('asks conditionally once it has an ETag, and a 304 keeps the list without re-reading it', async () => {
		const gh = github({ etag: 'W/"abc"' });
		const deps = depsWith({ fetch: gh.fetcher });
		const first = await loadRelease(deps);
		expect(first.etag).toBe('W/"abc"');

		gh.state.notModified = true;
		deps.clock.t += FRESH_MS + 1;
		const second = await loadRelease(deps);
		expect(second.files.map((f) => f.name)).toEqual([EXE]);
		expect(second.fetchedAt).toBe(deps.clock.t);
		expect(gh.state.conditional).toBe(true);
	});

	it('rate-limited API: the release is read through plain downloads instead (rule 6)', async () => {
		const gh = github({ apiStatus: 403 });
		const info = await loadRelease(depsWith({ fetch: gh.fetcher }));
		expect(info.tag).toBe('v0.9.3');
		expect(info.files.map((f) => f.name)).toEqual([EXE]);
		expect(info.files[0].size).toBe(EXE_BYTES.length);
		expect(info.checksums[EXE]).toBe(EXE_SHA);
	});

	it('both routes gone: the stale list is served, and says so out loud', async () => {
		const gh = github();
		const deps = depsWith({ fetch: gh.fetcher });
		await loadRelease(deps);

		const said: string[] = [];
		const wasError = console.error;
		console.error = (...args: unknown[]) => said.push(args.join(' '));
		try {
			gh.state.down = true;
			deps.clock.t += FRESH_MS + 30 * 60_000;
			const info = await loadRelease(deps);
			expect(info.tag).toBe('v0.9.3');
		} finally {
			console.error = wasError;
		}
		expect(said.join(' ')).toMatch(/could not refresh/);
		expect(said.join(' ')).toMatch(/35 minutes ago/);
	});

	it('an unreadable checksum file costs the mirror, not the list', async () => {
		const gh = github({ assets: [asset(EXE), asset('SHA256SUMS-macos', 90)] });
		const info = await loadRelease(depsWith({ fetch: gh.fetcher }));
		expect(info.files).toHaveLength(1);
		expect(info.checksums).toEqual({});
	});
});

describe('manifestOf', () => {
	it('lists only what the Release carries, and takes the version from its tag', async () => {
		const gh = github();
		const manifest = manifestOf(await loadRelease(depsWith({ fetch: gh.fetcher })));
		expect(manifest).toEqual({
			version: '0.9.3',
			tag: 'v0.9.3',
			files: { 'windows-x64': { name: EXE, size: EXE_BYTES.length } }
		});
	});
});

describe('serveDownload', () => {
	it('a platform the Release does not carry answers with the page, never a 404 (FR-001)', async () => {
		const gh = github();
		const res = await serveDownload('macos-arm64', get(), depsWith({ fetch: gh.fetcher }));
		expect(res.status).toBe(302);
		expect(res.headers.get('Location')).toBe('/get-started?unavailable=macos-arm64#downloads');
	});

	it('…and with the page they were reading, if that is where they came from', async () => {
		const gh = github();
		const deps = depsWith({ fetch: gh.fetcher });
		const from = (Referer: string) => serveDownload('macos-arm64', get({ Referer }), deps);
		expect((await from('https://getvela.app/ja/get-started')).headers.get('Location')).toBe(
			'/ja/get-started?unavailable=macos-arm64#downloads'
		);
		expect((await from('https://evil.example/ja/get-started')).headers.get('Location')).toBe(
			'/get-started?unavailable=macos-arm64#downloads'
		);
		expect(
			(await from('https://getvela.app/ja/get-started/../../pay')).headers.get('Location')
		).toBe('/get-started?unavailable=macos-arm64#downloads');
	});

	it('no bucket bound: redirects to GitHub', async () => {
		const gh = github();
		const res = await serveDownload('windows-x64', get(), depsWith({ fetch: gh.fetcher }));
		expect(res.status).toBe(302);
		expect(res.headers.get('Location')).toBe(`https://github.test/dl/${EXE}`);
	});

	it('first request: the person gets the bytes AND the mirror is filled, told which SHA-256 to insist on', async () => {
		const gh = github();
		const b = bucket();
		const deps = depsWith({ fetch: gh.fetcher, mirror: b.mirror });
		const res = await serveDownload('windows-x64', get(), deps);

		expect(res.status).toBe(200);
		expect(res.headers.get('Content-Disposition')).toBe(`attachment; filename="${EXE}"`);
		expect(new Uint8Array(await res.arrayBuffer())).toEqual(EXE_BYTES);

		await Promise.all(deps.pending);
		expect(b.puts).toEqual([{ key: `v0.9.3/${EXE}`, sha256: EXE_SHA }]);
		expect(b.objects.get(`v0.9.3/${EXE}`)!.bytes).toEqual(EXE_BYTES);
	});

	it('second request: R2 alone — GitHub is not asked for the file', async () => {
		const gh = github();
		const b = bucket();
		const deps = depsWith({ fetch: gh.fetcher, mirror: b.mirror });
		await (await serveDownload('windows-x64', get(), deps)).arrayBuffer();
		await Promise.all(deps.pending);

		const before = gh.state.calls.filter((u) => u.endsWith(EXE)).length;
		const res = await serveDownload('windows-x64', get(), deps);
		expect(res.status).toBe(200);
		expect(new Uint8Array(await res.arrayBuffer())).toEqual(EXE_BYTES);
		expect(gh.state.calls.filter((u) => u.endsWith(EXE)).length).toBe(before);
		expect(b.puts).toHaveLength(1);
	});

	it('a file replaced on the Release evicts its mirror: the old object is not served, and is overwritten', async () => {
		const b = bucket();
		b.objects.set(`v0.9.3/${EXE}`, {
			bytes: new TextEncoder().encode('the OLD installer'),
			customMetadata: { sha256: 'f'.repeat(64) }
		});
		const gh = github();
		const deps = depsWith({ fetch: gh.fetcher, mirror: b.mirror });
		const res = await serveDownload('windows-x64', get(), deps);
		expect(new Uint8Array(await res.arrayBuffer())).toEqual(EXE_BYTES);
		await Promise.all(deps.pending);
		expect(b.objects.get(`v0.9.3/${EXE}`)!.customMetadata.sha256).toBe(EXE_SHA);
	});

	it('no published checksum for the file: never mirrored, served by redirect', async () => {
		const gh = github({ assets: [asset(DMG), asset(EXE), asset('SHA256SUMS', 90)] });
		const b = bucket();
		const res = await serveDownload(
			'macos-arm64',
			get(),
			depsWith({ fetch: gh.fetcher, mirror: b.mirror })
		);
		expect(res.status).toBe(302);
		expect(res.headers.get('Location')).toBe(`https://github.test/dl/${DMG}`);
		expect(b.puts).toHaveLength(0);
	});

	it('the bucket throwing falls back to the GitHub redirect — a slow download beats none', async () => {
		const gh = github();
		const mirror: Mirror = {
			get: async () => {
				throw new Error('R2 is having a day');
			},
			put: async () => {}
		};
		const res = await serveDownload('windows-x64', get(), depsWith({ fetch: gh.fetcher, mirror }));
		expect(res.status).toBe(302);
		expect(res.headers.get('Location')).toBe(`https://github.test/dl/${EXE}`);
	});

	it('no list and none remembered: GitHub’s releases page is what is left', async () => {
		const gh = github();
		gh.state.down = true;
		const res = await serveDownload('windows-x64', get(), depsWith({ fetch: gh.fetcher }));
		expect(res.status).toBe(302);
		expect(res.headers.get('Location')).toBe(
			'https://github.com/mondaylabsltd/vela-wallet/releases'
		);
	});
});
