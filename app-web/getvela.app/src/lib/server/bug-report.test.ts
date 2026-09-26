import { describe, expect, it } from 'vitest';
import {
	ATTACHMENT_ORIGIN,
	ATTACHMENT_PREFIX,
	MAX_BODY_BYTES,
	MAX_REPORT_CHARS,
	MAX_SCREENSHOTS,
	MAX_SCREENSHOT_BYTES,
	attachmentKey,
	fileReport,
	isAttachmentKey,
	parseReport,
	serveAttachment,
	sniffImage,
	type AttachmentBucket,
	type FilingDeps,
	type ReportText
} from './bug-report';

const JPEG = new Uint8Array([0xff, 0xd8, 0xff, 0xe0, 1, 2, 3, 4]);
const PNG = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13]);
const WEBP = new TextEncoder().encode('RIFF\0\0\0\0WEBPVP8 ');
const GIF = new TextEncoder().encode('GIF89a......');
const SVG = new TextEncoder().encode('<svg onload="alert(1)"/>');
const UUID = '0f8fad5b-d9cb-469f-a165-70867728950e';

const b64 = (bytes: Uint8Array) => btoa(String.fromCharCode(...bytes));

function post(body: unknown, headers: Record<string, string> = {}): Request {
	return new Request('https://getvela.app/api/bug-report', {
		method: 'POST',
		headers: { 'Content-Type': 'application/json', ...headers },
		body: typeof body === 'string' ? body : JSON.stringify(body)
	});
}

describe('sniffImage', () => {
	it('knows a JPEG, a PNG and a WebP by their bytes', () => {
		expect(sniffImage(JPEG)).toBe('jpg');
		expect(sniffImage(PNG)).toBe('png');
		expect(sniffImage(WEBP)).toBe('webp');
	});

	it('refuses everything else — a GIF, an SVG, nothing at all', () => {
		expect(sniffImage(GIF)).toBeNull();
		expect(sniffImage(SVG)).toBeNull();
		expect(sniffImage(new Uint8Array())).toBeNull();
	});
});

describe('attachment keys', () => {
	it('are minted by month and uuid, and read back', () => {
		const key = attachmentKey('png', new Date(Date.UTC(2026, 8, 30, 23, 59)), UUID);
		expect(key).toBe(`2026-09/${UUID}.png`);
		expect(isAttachmentKey(key)).toBe(true);
	});

	it('never match anything else in the bucket', () => {
		for (const key of [
			`../downloads/VelaWallet.dmg`,
			`2026-09/../../${UUID}.png`,
			`2026-09/${UUID}.svg`,
			`2026-09/${UUID.toUpperCase()}.png`,
			`bug-attachments/2026-09/${UUID}.png`,
			`2026-09/${UUID}.png/x`,
			''
		]) {
			expect(isAttachmentKey(key), key).toBe(false);
		}
	});
});

describe('parseReport', () => {
	it('scrubs addresses and URLs from what the app wrote, never from what the person typed', async () => {
		const address = '0x88cCA0e5bA1D5b6D1b8a0bB7e1c3F0e5b5E26894';
		const parsed = await parseReport(
			post({
				what: `sending to ${address} failed`,
				environment: `RPC: https://eth.example/v2/SECRET · ${address}`,
				diagnostics: `wss://node.example/key=abc`
			})
		);
		expect(parsed).toMatchObject({
			ok: true,
			text: {
				what: `sending to ${address} failed`,
				environment: 'RPC: [url] · [address]',
				diagnostics: '[url]'
			}
		});
	});

	it('takes a report with no screenshots — the shape every client sent before', async () => {
		const parsed = await parseReport(
			post({ what: '  Send stuck  ', steps: '1. tap', fingerprint: 'abc' })
		);
		expect(parsed).toMatchObject({
			ok: true,
			text: { what: 'Send stuck', steps: '1. tap' },
			screenshots: []
		});
	});

	it('decodes screenshots and knows their type from the bytes', async () => {
		const parsed = await parseReport(
			post({ what: 'x', screenshots: [b64(JPEG), b64(PNG), b64(WEBP)] })
		);
		expect(parsed.ok && parsed.screenshots.map((s) => s.kind)).toEqual(['jpg', 'png', 'webp']);
		expect(parsed.ok && parsed.screenshots[0].bytes).toEqual(JPEG);
	});

	it('refuses a report with no description', async () => {
		expect(await parseReport(post({ what: '   ' }))).toEqual({
			ok: false,
			status: 400,
			error: 'missing_description'
		});
	});

	it('refuses what is not a JSON object', async () => {
		expect(await parseReport(post('not json'))).toMatchObject({
			status: 400,
			error: 'invalid_json'
		});
		expect(await parseReport(post('[1]'))).toMatchObject({ status: 400, error: 'invalid_json' });
		expect(await parseReport(post('null'))).toMatchObject({ status: 400, error: 'invalid_json' });
	});

	it('caps the text, all fields together', async () => {
		const half = 'a'.repeat(MAX_REPORT_CHARS / 2);
		expect(await parseReport(post({ what: half, diagnostics: half }))).toMatchObject({ ok: true });
		expect(await parseReport(post({ what: half, diagnostics: `${half}a` }))).toMatchObject({
			status: 413,
			error: 'too_large'
		});
	});

	it('refuses a body larger than the cap before reading it', async () => {
		const parsed = await parseReport(
			post({ what: 'x' }, { 'Content-Length': String(MAX_BODY_BYTES + 1) })
		);
		expect(parsed).toMatchObject({ status: 413, error: 'too_large' });
	});

	it('refuses more than five screenshots', async () => {
		const six = Array.from({ length: MAX_SCREENSHOTS + 1 }, () => b64(JPEG));
		expect(await parseReport(post({ what: 'x', screenshots: six }))).toMatchObject({
			status: 400,
			error: 'too_many_screenshots'
		});
	});

	it('refuses a screenshot that is not an image it can show safely', async () => {
		expect(await parseReport(post({ what: 'x', screenshots: [b64(SVG)] }))).toMatchObject({
			status: 415,
			error: 'unsupported_screenshot'
		});
		expect(await parseReport(post({ what: 'x', screenshots: [b64(GIF)] }))).toMatchObject({
			status: 415
		});
	});

	it('refuses a screenshot that is not base64, or not a string', async () => {
		expect(
			await parseReport(post({ what: 'x', screenshots: ['data:image/png;base64,AAAA'] }))
		).toMatchObject({ error: 'invalid_screenshot' });
		expect(await parseReport(post({ what: 'x', screenshots: [42] }))).toMatchObject({
			error: 'invalid_screenshot'
		});
		expect(await parseReport(post({ what: 'x', screenshots: 'AAAA' }))).toMatchObject({
			error: 'invalid_screenshot'
		});
	});

	it('refuses a screenshot over 2 MB', async () => {
		const big = new Uint8Array(MAX_SCREENSHOT_BYTES + 1);
		big.set(JPEG);
		expect(
			await parseReport(post({ what: 'x', screenshots: [Buffer.from(big).toString('base64')] }))
		).toMatchObject({
			status: 413,
			error: 'screenshot_too_large'
		});
	});

	it('takes five 2 MB screenshots from a JSON writer that escapes every slash', async () => {
		// Random bytes are what a JPEG looks like to base64: ~1/64 of it is `/`.
		const image = () => {
			const bytes = new Uint8Array(MAX_SCREENSHOT_BYTES);
			for (let at = 0; at < bytes.length; at += 65_536)
				crypto.getRandomValues(bytes.subarray(at, at + 65_536));
			bytes.set(JPEG);
			return Buffer.from(bytes).toString('base64');
		};
		const body = JSON.stringify({
			what: 'x'.repeat(MAX_REPORT_CHARS - 1),
			screenshots: Array.from({ length: MAX_SCREENSHOTS }, image)
		}).replaceAll('/', '\\/');
		expect(body.length).toBeGreaterThan(
			Math.ceil((MAX_SCREENSHOT_BYTES * 4) / 3) * MAX_SCREENSHOTS
		);
		const parsed = await parseReport(post(body));
		expect(parsed.ok && parsed.screenshots.length).toBe(MAX_SCREENSHOTS);
	});
});

/** An R2 bucket in memory, which can be told to fail. */
function bucket(options: { failPuts?: number } = {}) {
	const objects = new Map<string, { bytes: Uint8Array; contentType: string }>();
	let failPuts = options.failPuts ?? 0;
	const store: AttachmentBucket = {
		async put(key, value, { httpMetadata }) {
			if (failPuts > 0) {
				failPuts--;
				throw new Error('r2 down');
			}
			objects.set(key, { bytes: value, contentType: httpMetadata.contentType });
		},
		async delete(key) {
			objects.delete(key);
		},
		async get(key) {
			const hit = objects.get(key);
			return hit
				? { body: new Blob([hit.bytes as BlobPart]).stream(), size: hit.bytes.length }
				: null;
		}
	};
	return { store, objects };
}

interface Call {
	url: string;
	method: string;
	body: { title?: string; body?: string; labels?: string[] } | null;
}

/** A GitHub that answers from a script and records every call. */
function github(
	script: { search?: number | object; comment?: number; create?: number[]; throws?: Error } = {}
) {
	const calls: Call[] = [];
	const creates = [...(script.create ?? [201])];
	const fetcher: typeof fetch = async (input, init) => {
		const url = String(input);
		calls.push({
			url,
			method: init?.method ?? 'GET',
			body: init?.body ? JSON.parse(String(init.body)) : null
		});
		if (script.throws) throw script.throws;
		if (url.includes('/search/issues')) {
			const search = script.search ?? { items: [] };
			return typeof search === 'number'
				? new Response('', { status: search })
				: Response.json(search);
		}
		if (url.endsWith('/comments')) return new Response('{}', { status: script.comment ?? 201 });
		const status = creates.shift() ?? 201;
		return status === 201
			? Response.json({ number: 42, html_url: 'https://github.com/o/r/issues/42' }, { status })
			: new Response('{}', { status });
	};
	return { fetcher, calls };
}

const TEXT: ReportText = {
	what: 'Send stuck',
	steps: '1. tap Send',
	area: 'send',
	environment: 'iOS 18',
	diagnostics: 'log',
	fingerprint: 'fp-1'
};

function deps(
	fetcher: typeof fetch,
	store?: AttachmentBucket,
	extra: Partial<FilingDeps> = {}
): FilingDeps {
	let n = 0;
	return {
		fetch: fetcher,
		token: 't0ken',
		repo: 'o/r',
		timeoutMs: 1000,
		bucket: store,
		now: () => new Date(Date.UTC(2026, 8, 26)),
		uuid: () => `0f8fad5b-d9cb-469f-a165-70867728950${n++}`,
		...extra
	};
}

describe('fileReport', () => {
	it('stores each screenshot and shows it inline in the new issue', async () => {
		const { store, objects } = bucket();
		const { fetcher, calls } = github();
		const filed = await fileReport(
			{
				text: TEXT,
				screenshots: [
					{ bytes: JPEG, kind: 'jpg' },
					{ bytes: PNG, kind: 'png' }
				]
			},
			deps(fetcher, store)
		);

		expect(filed).toEqual({
			status: 200,
			body: {
				number: 42,
				url: 'https://github.com/o/r/issues/42',
				deduped: false,
				screenshots: 2,
				screenshotsDropped: 0
			}
		});
		expect([...objects.keys()]).toEqual([
			`${ATTACHMENT_PREFIX}2026-09/0f8fad5b-d9cb-469f-a165-708677289500.jpg`,
			`${ATTACHMENT_PREFIX}2026-09/0f8fad5b-d9cb-469f-a165-708677289501.png`
		]);
		expect(
			objects.get(`${ATTACHMENT_PREFIX}2026-09/0f8fad5b-d9cb-469f-a165-708677289501.png`)
				?.contentType
		).toBe('image/png');
		const create = calls.find((c) => c.method === 'POST' && c.url.endsWith('/issues'))!;
		expect(create.body?.labels).toEqual(['bug', 'in-app-report']);
		expect(create.body?.body).toContain('<!-- vela-fp:fp-1 -->');
		expect(create.body?.body).toContain(
			`![Screenshot 1](${ATTACHMENT_ORIGIN}/api/bug-report/attachments/2026-09/0f8fad5b-d9cb-469f-a165-708677289500.jpg)`
		);
		expect(create.body?.body).toContain('![Screenshot 2](');
	});

	it('never sends the token anywhere but GitHub, and only in the header', async () => {
		const { fetcher, calls } = github();
		await fileReport({ text: TEXT, screenshots: [] }, deps(fetcher));
		expect(calls.every((c) => c.url.startsWith('https://api.github.com/'))).toBe(true);
		expect(JSON.stringify(calls)).not.toContain('t0ken');
	});

	it('files the report without screenshots when there is no bucket, and says so', async () => {
		const { fetcher, calls } = github();
		const filed = await fileReport(
			{ text: TEXT, screenshots: [{ bytes: JPEG, kind: 'jpg' }] },
			deps(fetcher)
		);
		expect(filed.body).toMatchObject({ number: 42, screenshots: 0, screenshotsDropped: 1 });
		const body = calls.at(-1)!.body!.body!;
		expect(body).not.toContain('![Screenshot');
		expect(body).toContain('1 screenshot(s) were sent but could not be stored');
	});

	it('keeps the screenshots that did store when one put fails', async () => {
		const { store, objects } = bucket({ failPuts: 1 });
		const { fetcher } = github();
		const filed = await fileReport(
			{
				text: TEXT,
				screenshots: [
					{ bytes: JPEG, kind: 'jpg' },
					{ bytes: PNG, kind: 'png' }
				]
			},
			deps(fetcher, store)
		);
		expect(filed.body).toMatchObject({ screenshots: 1, screenshotsDropped: 1 });
		expect(objects.size).toBe(1);
	});

	it('+1s the open issue with the same fingerprint, screenshots included', async () => {
		const { store } = bucket();
		const { fetcher, calls } = github({
			search: { items: [{ number: 7, html_url: 'https://github.com/o/r/issues/7' }] }
		});
		const filed = await fileReport(
			{ text: TEXT, screenshots: [{ bytes: JPEG, kind: 'jpg' }] },
			deps(fetcher, store)
		);
		expect(filed.body).toMatchObject({ number: 7, deduped: true, screenshots: 1 });
		const comment = calls.find((c) => c.url.endsWith('/issues/7/comments'))!;
		expect(comment.body?.body).toContain('➕ Another in-app report');
		expect(comment.body?.body).toContain('![Screenshot 1](');
		expect(calls.some((c) => c.url.endsWith('/o/r/issues'))).toBe(false);
	});

	it('files a new issue when the +1 comment does not land', async () => {
		const { fetcher, calls } = github({
			search: { items: [{ number: 7, html_url: 'u' }] },
			comment: 403
		});
		const filed = await fileReport({ text: TEXT, screenshots: [] }, deps(fetcher));
		expect(filed.body).toMatchObject({ number: 42, deduped: false });
		expect(calls.map((c) => c.method)).toEqual(['GET', 'POST', 'POST']);
	});

	it('still files when the dedup search fails', async () => {
		const { fetcher } = github({ search: 500 });
		expect((await fileReport({ text: TEXT, screenshots: [] }, deps(fetcher))).body).toMatchObject({
			number: 42
		});
	});

	it('skips the search for a report with no fingerprint', async () => {
		const { fetcher, calls } = github();
		await fileReport({ text: { ...TEXT, fingerprint: '../' }, screenshots: [] }, deps(fetcher));
		expect(calls.some((c) => c.url.includes('/search/'))).toBe(false);
	});

	it('retries without labels when GitHub refuses them', async () => {
		const { fetcher, calls } = github({ create: [422, 201] });
		const filed = await fileReport({ text: TEXT, screenshots: [] }, deps(fetcher));
		expect(filed.body).toMatchObject({ number: 42 });
		expect(calls.at(-1)!.body?.labels).toBeUndefined();
	});

	it('deletes its screenshots again when the issue is not created', async () => {
		const { store, objects } = bucket();
		const { fetcher } = github({ create: [500] });
		const pending: Promise<unknown>[] = [];
		const filed = await fileReport(
			{ text: TEXT, screenshots: [{ bytes: JPEG, kind: 'jpg' }] },
			deps(fetcher, store, { waitUntil: (work) => pending.push(work) })
		);
		await Promise.all(pending);
		expect(filed).toEqual({ status: 502, body: { error: 'upstream_failed' } });
		expect(objects.size).toBe(0);
	});

	it('says a timeout is a timeout', async () => {
		const timeout = Object.assign(new Error('slow'), { name: 'TimeoutError' });
		const { fetcher } = github({ throws: timeout });
		expect(
			await fileReport({ text: { ...TEXT, fingerprint: '' }, screenshots: [] }, deps(fetcher))
		).toEqual({
			status: 502,
			body: { error: 'upstream_timeout' }
		});
	});
});

describe('serveAttachment', () => {
	it('serves a stored screenshot as its image type, cached for good, never sniffed', async () => {
		const { store } = bucket();
		const key = `2026-09/${UUID}.png`;
		await store.put(ATTACHMENT_PREFIX + key, PNG, { httpMetadata: { contentType: 'image/png' } });
		const res = await serveAttachment(key, store);
		expect(res.status).toBe(200);
		expect(res.headers.get('Content-Type')).toBe('image/png');
		expect(res.headers.get('Cache-Control')).toBe('public, max-age=31536000, immutable');
		expect(res.headers.get('X-Content-Type-Options')).toBe('nosniff');
		expect(res.headers.get('Content-Security-Policy')).toContain('sandbox');
		expect(new Uint8Array(await res.arrayBuffer())).toEqual(PNG);
	});

	it('is 404 for a key it did not mint, a missing object, or no bucket', async () => {
		const { store } = bucket();
		await store.put('downloads/VelaWallet.dmg', JPEG, { httpMetadata: { contentType: 'x' } });
		expect((await serveAttachment('../downloads/VelaWallet.dmg', store)).status).toBe(404);
		expect((await serveAttachment(`2026-09/${UUID}.jpg`, store)).status).toBe(404);
		expect((await serveAttachment(`2026-09/${UUID}.jpg`, undefined)).status).toBe(404);
	});
});
