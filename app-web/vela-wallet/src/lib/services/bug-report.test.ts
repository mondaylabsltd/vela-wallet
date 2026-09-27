/**
 * The report, and everything that must never be in it (spec 081 FR-016).
 *
 * The property under test is not "these five fields are present" — a payload
 * with the right shape and somebody's address inside it is exactly the failure
 * this feature exists to prevent. It is:
 *
 *   the consent line is literal (the preview IS the payload),
 *   nothing on the wallet's shelf can reach the wire, and
 *   a refused send still leaves the person a working road.
 *
 * The exclusion test seeds the device with the four forbidden classes —
 * an address, a balance, an RPC URL carrying an API key, and raw `vela.*`
 * values — and asserts none of them appear in the serialized payload.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
	AREA_OTHER,
	MAX_SCREENSHOTS,
	SCREENSHOT_TIMEOUT_MS,
	buildBugReport,
	describeBrowser,
	environmentLines,
	fingerprintOf,
	issueSummary,
	webPlatform,
	prefilledIssueURL,
	webClient,
	redact,
	reportTimeoutMs,
	sendBugReport,
	type DeviceFacts,
	type EnvironmentLabels
} from './bug-report';

const LABELS: EnvironmentLabels = {
	version: 'App version',
	platform: 'Platform',
	language: 'Language',
	rpc: 'Unreachable RPC',
	failures: 'Recent failures',
	none: 'None'
};

/** The four things a report may never carry, spelled out once. */
const SECRETS = {
	address: '0x44EEC06897ff7ab8C7f16819511A64bA168A6D33',
	balance: '1234.56789',
	rpcWithKey: 'https://eth-mainnet.g.alchemy.com/v2/SUPER-SECRET-KEY',
	storedValue: '{"accounts":[{"address":"0x44EEC06897ff7ab8C7f16819511A64bA168A6D33"}]}'
} as const;

const FACTS: DeviceFacts = {
	version: '1.0.0',
	client: 'web',
	os: 'Chrome 151 on macOS',
	commit: 'abc1234',
	platform: 'Web · Chrome 151 on macOS',
	language: 'en',
	unreachable: ['Gnosis'],
	failures: ['rpc:final_failure ×2']
};

afterEach(() => {
	vi.unstubAllGlobals();
});

describe('what a report is allowed to know', () => {
	it('carries the named fields and no other', () => {
		const payload = buildBugReport({
			what: 'Send froze',
			steps: '1. tap send',
			area: AREA_OTHER,
			labels: LABELS,
			facts: FACTS
		});
		expect(Object.keys(payload).sort()).toEqual([
			'appVersion',
			'area',
			'client',
			'environment',
			'fingerprint',
			'os',
			'steps',
			'what'
		]);
	});

	it('contains no address, balance, endpoint URL or stored value — even when the device is full of them', () => {
		// The whole shelf, present and readable, exactly as it is on a real
		// device. Nothing in the builder is given a way to reach it.
		vi.stubGlobal('localStorage', {
			length: 3,
			key: (i: number) => ['vela.accounts', 'vela.balanceCache', 'vela.networkConfig'][i] ?? null,
			getItem: () => SECRETS.storedValue,
			setItem: () => {},
			removeItem: () => {},
			clear: () => {}
		});

		const payload = buildBugReport({
			what: 'Balances stopped updating',
			steps: '1. open the wallet\n2. wait',
			area: AREA_OTHER,
			labels: LABELS,
			facts: FACTS
		});
		const wire = JSON.stringify(payload);

		expect(wire).not.toContain(SECRETS.address);
		expect(wire).not.toContain(SECRETS.balance);
		expect(wire).not.toContain(SECRETS.rpcWithKey);
		expect(wire).not.toContain('SUPER-SECRET-KEY');
		expect(wire).not.toContain(SECRETS.storedValue);
		// And no `vela.` key name either: a key list is a map of the shelf.
		expect(wire).not.toContain('vela.');
	});

	it('redacts an address or a URL that rode in on a name the person chose', () => {
		// A custom network can be named anything, including its own RPC URL —
		// which is how a key would otherwise reach a public issue tracker.
		const lines = environmentLines(LABELS, {
			...FACTS,
			unreachable: [SECRETS.rpcWithKey, SECRETS.address]
		});
		const joined = lines.join('\n');
		expect(joined).not.toContain('SUPER-SECRET-KEY');
		expect(joined).not.toContain(SECRETS.address);
		expect(joined).toContain('[url]');
		expect(joined).toContain('[address]');
	});

	it('redacts nothing it was not asked to', () => {
		expect(redact('Recent failures: rpc:final_failure ×2')).toBe(
			'Recent failures: rpc:final_failure ×2'
		);
	});

	it('says "none" rather than inventing a dash', () => {
		const lines = environmentLines(LABELS, { ...FACTS, unreachable: [], failures: [] });
		expect(lines[3]).toBe('Unreachable RPC: None');
		expect(lines[4]).toBe('Recent failures: None');
	});
});

describe('the preview is the payload (the consent line, made literal)', () => {
	it('sends exactly the lines the sheet showed', () => {
		const shown = environmentLines(LABELS, FACTS);
		const payload = buildBugReport({
			what: 'x',
			area: AREA_OTHER,
			labels: LABELS,
			facts: FACTS
		});
		expect(payload.environment).toBe(shown.join('\n'));
	});
});

describe('the fallback road', () => {
	it('prefills by the form’s FIELD IDS — `body` is ignored with template=bug.yml', () => {
		const payload = buildBugReport({
			what: 'Send froze',
			steps: '1. tap send',
			area: AREA_OTHER,
			labels: LABELS,
			facts: FACTS
		});
		const url = new URL(prefilledIssueURL(payload));
		expect(url.searchParams.get('template')).toBe('bug.yml');
		expect(url.searchParams.get('what')).toBe('Send froze');
		expect(url.searchParams.get('steps')).toBe('1. tap send');
		expect(url.searchParams.get('environment')).toBe(payload.environment);
		expect(url.searchParams.get('area')).toBe(AREA_OTHER);
		// The gotcha, asserted so it cannot come back: a `body` parameter here
		// would be silently dropped by GitHub and the form would open empty.
		expect(url.searchParams.get('body')).toBeNull();
	});

	it.each([
		[503, 'not_configured'],
		[429, 'rate_limited'],
		[413, 'too_large'],
		[500, 'rejected']
	])('falls back on %i rather than losing the report', async (status, reason) => {
		vi.stubGlobal(
			'fetch',
			vi.fn(async () => new Response('{}', { status }))
		);
		const payload = buildBugReport({ what: 'x', area: AREA_OTHER, labels: LABELS, facts: FACTS });
		const outcome = await sendBugReport(payload, 'https://example.test/api/bug-report');
		expect(outcome.ok).toBe(false);
		if (!outcome.ok) {
			expect(outcome.reason).toBe(reason);
			expect(outcome.fallbackUrl).toContain('what=');
		}
	});

	it('falls back when the endpoint is unreachable', async () => {
		vi.stubGlobal(
			'fetch',
			vi.fn(async () => {
				throw new TypeError('network down');
			})
		);
		const payload = buildBugReport({ what: 'x', area: AREA_OTHER, labels: LABELS, facts: FACTS });
		const outcome = await sendBugReport(payload, 'https://example.test/api/bug-report');
		expect(outcome).toMatchObject({ ok: false, reason: 'unreachable' });
	});

	it('reports the issue it became when the endpoint files it', async () => {
		vi.stubGlobal(
			'fetch',
			vi.fn(
				async () =>
					new Response(
						JSON.stringify({ number: 42, url: 'https://github.com/x/y/issues/42', deduped: true }),
						{ status: 200 }
					)
			)
		);
		const payload = buildBugReport({ what: 'x', area: AREA_OTHER, labels: LABELS, facts: FACTS });
		const outcome = await sendBugReport(payload, 'https://example.test/api/bug-report');
		expect(outcome).toEqual({
			ok: true,
			number: 42,
			url: 'https://github.com/x/y/issues/42',
			deduped: true,
			screenshotsDropped: 0
		});
	});
});

// 078 round 3: screenshots ride in the same JSON body, as plain base64.
describe('screenshots', () => {
	/** A tiny "JPEG": SOI + EOI, base64 — enough to ride the wire. */
	const JPEG = btoa(String.fromCharCode(0xff, 0xd8, 0xff, 0xd9));

	it('are ABSENT from a text-only report — the payload is what it always was', () => {
		const none = buildBugReport({ what: 'x', area: AREA_OTHER, labels: LABELS, facts: FACTS });
		const empty = buildBugReport({
			what: 'x',
			area: AREA_OTHER,
			labels: LABELS,
			facts: FACTS,
			screenshots: []
		});
		expect('screenshots' in none).toBe(false);
		expect('screenshots' in empty).toBe(false);
	});

	it('go in tile order, at most five, and never into the GitHub form', () => {
		const shots = ['a', 'b', 'c', 'd', 'e', 'f'].map((tag) => `${JPEG}${tag}`);
		const payload = buildBugReport({
			what: 'x',
			area: AREA_OTHER,
			labels: LABELS,
			facts: FACTS,
			screenshots: shots
		});
		expect(payload.screenshots).toEqual(shots.slice(0, MAX_SCREENSHOTS));
		expect(prefilledIssueURL(payload)).not.toContain(JPEG.slice(0, 4));
	});

	it('get the 30 s timeout; text alone keeps the usual one', () => {
		const text = buildBugReport({ what: 'x', area: AREA_OTHER, labels: LABELS, facts: FACTS });
		const withShots = { ...text, screenshots: [JPEG] };
		expect(reportTimeoutMs(withShots)).toBe(SCREENSHOT_TIMEOUT_MS);
		expect(reportTimeoutMs(text)).toBeLessThan(SCREENSHOT_TIMEOUT_MS);
	});

	it('do not count against the text cap — images have caps of their own', async () => {
		let sent = '';
		vi.stubGlobal(
			'fetch',
			vi.fn(async (_url: string, init: RequestInit) => {
				sent = String(init.body);
				return new Response(JSON.stringify({ number: 7, url: 'https://github.com/x/y/issues/7' }), {
					status: 200
				});
			})
		);
		const big = 'A'.repeat(200_000);
		const payload = {
			...buildBugReport({ what: 'x', area: AREA_OTHER, labels: LABELS, facts: FACTS }),
			screenshots: [big]
		};
		const outcome = await sendBugReport(payload, 'https://example.test/api/bug-report');
		expect(outcome.ok).toBe(true);
		expect(JSON.parse(sent).screenshots).toEqual([big]);
	});

	it('says how many images the endpoint could not store', async () => {
		vi.stubGlobal(
			'fetch',
			vi.fn(
				async () =>
					new Response(
						JSON.stringify({
							number: 9,
							url: 'https://github.com/x/y/issues/9',
							deduped: false,
							screenshots: 1,
							screenshotsDropped: 1
						}),
						{ status: 200 }
					)
			)
		);
		const payload = buildBugReport({
			what: 'x',
			area: AREA_OTHER,
			labels: LABELS,
			facts: FACTS,
			screenshots: [JPEG, JPEG]
		});
		const outcome = await sendBugReport(payload, 'https://example.test/api/bug-report');
		expect(outcome).toMatchObject({ ok: true, number: 9, screenshotsDropped: 1 });
	});

	it.each([
		[413, 'too_large'],
		[415, 'rejected'],
		[400, 'rejected']
	] as const)(
		'an image refusal (%i) falls back to the form like any other',
		async (status, reason) => {
			vi.stubGlobal(
				'fetch',
				vi.fn(async () => new Response('{}', { status }))
			);
			const payload = buildBugReport({
				what: 'x',
				area: AREA_OTHER,
				labels: LABELS,
				facts: FACTS,
				screenshots: [JPEG]
			});
			const outcome = await sendBugReport(payload, 'https://example.test/api/bug-report');
			expect(outcome).toMatchObject({ ok: false, reason });
			if (!outcome.ok) expect(outcome.fallbackUrl).toContain('what=x');
		}
	);
});

// 078 §E: the endpoint titles the issue "[Web] …" and opens it with
// "Platform: Chrome 151 on macOS. App v0.9.4." — only when these arrive.
describe('the platform in the issue title', () => {
	it('carries client, os and appVersion — the version without a "v", the os one short line', () => {
		const facts: DeviceFacts = { ...FACTS, version: 'v0.9.4', os: '  Chrome 151\n on   macOS ' };
		const payload = buildBugReport({ what: 'Send froze', area: AREA_OTHER, labels: LABELS, facts });
		expect(payload.client).toBe('web');
		expect(payload.os).toBe('Chrome 151 on macOS');
		expect(payload.appVersion).toBe('0.9.4');
		// The environment is still exactly the lines the sheet showed.
		expect(payload.environment).toBe(environmentLines(LABELS, facts).join('\n'));
	});

	it('says "extension" inside the MV3 extension, the same test the preview uses', () => {
		expect(webClient()).toBe('web');
		vi.stubGlobal('chrome', { runtime: { id: 'abcdefghijklmnop' } });
		expect(webClient()).toBe('extension');
		// A page some extension can message has a runtime — but no id.
		vi.stubGlobal('chrome', { runtime: {} });
		expect(webClient()).toBe('web');
	});

	it('sends the three fields on the wire', async () => {
		let sent: Record<string, unknown> = {};
		vi.stubGlobal(
			'fetch',
			vi.fn(async (_url: string, init: RequestInit) => {
				sent = JSON.parse(String(init.body));
				return new Response(JSON.stringify({ number: 1, url: 'https://github.com/x/y/issues/1' }), {
					status: 200
				});
			})
		);
		const payload = buildBugReport({
			what: 'x',
			area: AREA_OTHER,
			labels: LABELS,
			facts: { ...FACTS, client: 'extension', version: '0.9.4' }
		});
		await sendBugReport(payload, 'https://example.test/api/bug-report');
		expect(sent).toMatchObject({
			client: 'extension',
			os: 'Chrome 151 on macOS',
			appVersion: '0.9.4'
		});
	});

	it.each([
		['web', '[Web] '],
		['extension', '[Extension] ']
	] as const)('the fallback form is titled with the tag (%s)', (client, tag) => {
		const payload = buildBugReport({
			what: 'Send froze after Confirm\nthen nothing',
			area: AREA_OTHER,
			labels: LABELS,
			facts: { ...FACTS, client }
		});
		const title = new URL(prefilledIssueURL(payload)).searchParams.get('title');
		expect(title).toBe(`${tag}Send froze after Confirm`);
		expect(title?.startsWith('[bug]')).toBe(false);
	});

	it('titles with the first line of what, at most 80 characters, cut at a space', () => {
		expect(issueSummary('\n\n  Balance   shows 0  \nsecond')).toBe('Balance shows 0');
		const long =
			'The send button stays grey after I pick USDC on Gnosis and type an amount then wait';
		const cut = issueSummary(long);
		expect(cut.length).toBeLessThanOrEqual(80);
		expect(cut.endsWith('…')).toBe(true);
		expect(long.startsWith(cut.slice(0, -1))).toBe(true);
		expect(cut.slice(0, -1).endsWith(' ')).toBe(false);
		// One unbroken word still fits.
		expect(issueSummary('x'.repeat(200))).toHaveLength(80);
	});

	it.each([
		[
			'Chrome on macOS (client hints)',
			{
				userAgent:
					'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/151.0.0.0 Safari/537.36',
				brands: [
					{ brand: 'Not)A;Brand', version: '8' },
					{ brand: 'Chromium', version: '151' },
					{ brand: 'Google Chrome', version: '151' }
				],
				platform: 'macOS'
			},
			'Chrome 151 on macOS'
		],
		[
			'Brave says so in its brands',
			{
				userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/151.0.0.0 Safari/537.36',
				brands: [
					{ brand: 'Brave', version: '151' },
					{ brand: 'Chromium', version: '151' },
					{ brand: 'Not_A Brand', version: '24' }
				],
				platform: 'Windows'
			},
			'Brave 151 on Windows'
		],
		[
			'Edge, from the user agent',
			{
				userAgent:
					'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/150.0.0.0 Safari/537.36 Edg/150.0.1',
				platform: undefined
			},
			'Edge 150 on Windows'
		],
		[
			'Safari on iPhone',
			{
				userAgent:
					'Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Mobile/15E148 Safari/604.1'
			},
			'Safari 26 on iOS'
		],
		[
			'an iPad asking for the desktop site',
			{
				userAgent:
					'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15',
				touchPoints: 5
			},
			'Safari 26 on iPadOS'
		],
		[
			'Firefox on Android',
			{ userAgent: 'Mozilla/5.0 (Android 14; Mobile; rv:140.0) Gecko/140.0 Firefox/140.0' },
			'Firefox 140 on Android'
		],
		['nothing to go on', { userAgent: '' }, '']
	])('describes %s in one short line', (_name, signals, expected) => {
		expect(describeBrowser(signals)).toBe(expected);
	});

	it('words the preview’s platform line from the short description — never the user agent', () => {
		const agent =
			'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/151.0.7922.34 Safari/537.36';
		vi.stubGlobal('navigator', { userAgent: agent, maxTouchPoints: 0 });
		expect(webPlatform()).toBe('Web · Chrome 151 on macOS');
		vi.stubGlobal('chrome', { runtime: { id: 'abcdefghijklmnop' } });
		expect(webPlatform()).toBe('Web (extension) · Chrome 151 on macOS');
		// And so the payload: the environment says it the same way, one line.
		const payload = buildBugReport({
			what: 'x',
			area: AREA_OTHER,
			labels: LABELS,
			facts: { ...FACTS, platform: webPlatform() }
		});
		expect(payload.environment).toContain('Platform: Web (extension) · Chrome 151 on macOS');
		expect(payload.environment).not.toContain('Mozilla');
		expect(payload.environment).not.toContain('AppleWebKit');
		expect(payload.environment).not.toContain('7922');
	});

	it('says just "Web" when the browser cannot be told', () => {
		vi.stubGlobal('navigator', { userAgent: '', maxTouchPoints: 0 });
		expect(webPlatform()).toBe('Web');
	});

	it('never carries the user agent itself', () => {
		const agent =
			'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/151.0.0.0 Safari/537.36';
		const line = describeBrowser({ userAgent: agent });
		expect(line).toBe('Chrome 151 on Linux');
		expect(line).not.toContain('Mozilla');
		expect(line).not.toContain('AppleWebKit');
	});
});

describe('the dedup marker', () => {
	it('is stable for the same complaint and different for another', () => {
		const a = fingerprintOf('Send froze', AREA_OTHER, '1.0.0');
		expect(fingerprintOf('  SEND FROZE ', AREA_OTHER, '1.0.0')).toBe(a);
		expect(fingerprintOf('Receive froze', AREA_OTHER, '1.0.0')).not.toBe(a);
	});
});
