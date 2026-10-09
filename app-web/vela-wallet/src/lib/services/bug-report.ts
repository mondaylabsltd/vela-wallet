/**
 * The in-app report, actually sent (spec 081 FR-016).
 *
 * ## What this module is for
 *
 * The feedback sheet has always drawn a consent line — *"only what you see is
 * sent"* — over a preview of five invented lines and a button wired to
 * nothing. Both halves of that were a lie, in opposite directions: nothing was
 * sent at all, and what the preview promised was a picture of somebody else's
 * phone. This module makes the sentence true in the only way it can be true:
 * the preview lines and the payload's `environment` field are **the same
 * strings**, built once, here.
 *
 * ## Two roads, and why the second one exists
 *
 * The first is `getvela.app/api/bug-report`: a server-side fine-grained PAT
 * files the issue, so somebody without a GitHub account can still report a
 * bug. That route answers **503 `not_configured`** when the token is not
 * provisioned, 429 when an IP has filed five reports in ten minutes, and 413
 * over 16 000 characters. None of those are the person's fault, and none of
 * them may end with their report on the floor — so every one of them falls
 * back to the second road, a prefilled GitHub issue form.
 *
 * ## Who is allowed to POST there
 *
 * The site answers CORS for `getvela.app`, any `*.getvela.app` subdomain and
 * localhost (`getvela.app/src/hooks.server.ts`), which covers the hosted
 * wallet and development. The EXTENSION build's origin is
 * `chrome-extension://…` and is not on that list — it gets through because
 * `https://getvela.app/*` is in its `host_permissions`, which is what lets an
 * MV3 page's `fetch` skip CORS entirely. Anywhere else, the request simply
 * fails and the fallback below carries the report, which is the same outcome
 * as a 503 and needs no special case.
 *
 * ## The GitHub gotcha this module exists to encode
 *
 * With `template=bug.yml`, GitHub **ignores `&body=`**. An issue-form prefill
 * addresses the form's own field ids — `what`, `steps`, `environment`, `area`
 * — and a URL that passes `body` silently opens an EMPTY form. Android's two
 * URLs did exactly that, which is why the fallback link is built here and
 * nowhere else.
 *
 * ## What may never be in a report
 *
 * The PERSON's addresses and balances, and endpoint or RPC URLs — the last
 * because a self-hosted endpoint routinely carries an API key in its path,
 * and a public issue tracker is the worst possible place for one. Raw
 * `vela.*` values are excluded for the same reason at one remove: they are
 * the wallet's whole shelf, and "include the storage for context" is how the
 * other three get in.
 *
 * One report names an address and a balance on purpose: a relay stop's
 * (issue 466). The treasury it names is the OPERATOR's — public, on chain,
 * and the whole point of telling them — and the core writes it into `what`,
 * text the person reads and can edit before Send. It never rides in
 * `environment`, the device's own lines, where the server would scrub it.
 *
 * The defence is not a scrub over a broad payload — it is that the payload is
 * ASSEMBLED from a named allowlist ({@link DeviceFacts}) that has no way to
 * reach any of them. {@link redact} is the second line, for the one field
 * whose content a person can choose: a custom network's display name reaches
 * `unreachable`, and somebody can name a network after its own RPC URL.
 */
import { fetchWithTimeout, NET_TIMEOUTS, isTimeoutError } from './net';

/** The site's proxy. The PAT lives there; nothing here has a token. */
export const BUG_REPORT_ENDPOINT = 'https://getvela.app/api/bug-report';

/** The repository's issue form, for the fallback road. */
export const GITHUB_ISSUE_FORM = 'https://github.com/mondaylabsltd/vela-wallet/issues/new';

/**
 * The endpoint's own cap (`MAX_BODY_CHARS`), honoured before the request. It
 * counts the TEXT of a report — the screenshots have caps of their own.
 */
export const MAX_REPORT_CHARS = 16_000;

/** At most this many screenshots per report (the endpoint's cap). */
export const MAX_SCREENSHOTS = 5;

/** Each screenshot, decoded, at most this many bytes (the endpoint's cap). */
export const MAX_SCREENSHOT_BYTES = 2_000_000;

/**
 * How long a send may take. Text alone keeps the REST timeout every other
 * relay call has; a report carrying up to five images gets 30 s, because a
 * few megabytes on a slow uplink is not a dead endpoint.
 */
export const SCREENSHOT_TIMEOUT_MS = 30_000;

/**
 * The issue form's `area` dropdown, spelled exactly as `.github/ISSUE_TEMPLATE/
 * bug.yml` spells it — a prefill that does not match an option leaves the
 * dropdown unset, which is the same as not prefilling it.
 *
 * The sheet has no area picker, so the wallet answers the one option that is
 * true of a report filed from a settings sheet about anything: the person's
 * own words above are where the area actually is.
 */
export const AREA_OTHER = 'Other (explain above)';

/**
 * Which app sent the report — the endpoint puts it first in the issue title,
 * `[Web] …` / `[Extension] …` (078 §E, founder 2026-09-27, after the team's
 * own `[iOS · Function] …` in issue 318). The two a browser can be; the native
 * shells send `ios` / `android` / `desktop`.
 */
export type ReportClient = 'web' | 'extension';

/** The title tag for each, spelled as the endpoint spells it (`CLIENT_TAGS` there). */
export const CLIENT_TAGS: Record<ReportClient, string> = { web: 'Web', extension: 'Extension' };

/**
 * The text fields the endpoint accepts, and the images the person chose to
 * attach. Nothing else is sent, and this type is the reason another field
 * cannot appear by accident.
 */
export interface BugReportPayload {
	/** What the person typed. They wrote it; they can see it. */
	what: string;
	/** The optional reproduction steps, same rule. */
	steps: string;
	/** One of the issue form's `area` options — a label, never free text. */
	area: string;
	/** The preview lines, joined. IDENTICAL to what the sheet showed. */
	environment: string;
	/** Dedup marker: stable for the same complaint, meaningless on its own. */
	fingerprint: string;
	/** `web`, or `extension` inside the MV3 extension — the issue title's tag. */
	client: ReportClient;
	/**
	 * The browser and the OS, one short line — "Chrome 151 on macOS" — for the
	 * issue's "Platform:" line. Never the user agent (that stays in
	 * `environment`, where the person saw it), never a URL or an address.
	 */
	os: string;
	/** The build's version, no leading "v" — "0.9.4". */
	appVersion: string;
	/**
	 * Up to {@link MAX_SCREENSHOTS} images the person attached and saw as
	 * tiles, in tile order: plain base64 (no `data:` prefix) of JPEGs this
	 * device re-encoded — which is what strips their EXIF and location
	 * (`screenshot-prep.ts`). ABSENT, not empty, when there are none, so a
	 * text-only report is byte-for-byte what it always was.
	 */
	screenshots?: string[];
}

/**
 * Everything about the device a report is allowed to know.
 *
 * Named fields, because an object with an index signature is how a balance
 * ends up in a bug report six months from now.
 */
export interface DeviceFacts {
	/** `1.0.0` — the build's own, never the mock's. */
	version: string;
	/** Which app this is — see {@link webClient}. */
	client: ReportClient;
	/** "Chrome 151 on macOS" — see {@link webOs}. */
	os: string;
	/** Short commit, or `unknown`. */
	commit: string;
	/** `Web · Chrome 151 on macOS` — see {@link webPlatform}. */
	platform: string;
	/** The UI language tag in use. */
	language: string;
	/** Display NAMES of networks whose RPC is unreachable. Never their URLs. */
	unreachable: readonly string[];
	/** Recent failure summaries: counters and classes, never values. */
	failures: readonly string[];
}

/**
 * The extension worker's counters in `storage.session` — `extension/lib/
 * swlog.js`'s `SW_COUNTS_KEY`, declared here because the app bundle must not
 * import the worker's modules; `one-surface.test.ts` pins the two together.
 */
export const SW_COUNTS_KEY = 'vela.sw.counts';

/** A counter name the worker writes: `<area>.<event>[.<cause>]`, lower-case words only. */
const SW_COUNTER = /^[a-z]+\.[a-z_]+(?:\.[a-z][a-z_]*\d{0,3})?$/;

/**
 * The worker's events that are a failure when they carry a cause: a request
 * that ended without a decision, a claim refused, an endpoint that failed.
 * Not every counter with a third segment is one — swlog names a counter by
 * its cause OR its `kind`, and `req.arrived.sign` is an arrival.
 */
const SW_FAILURE_EVENTS = new Set(['req.settled', 'req.claim', 'read.fail']);

/**
 * The worker's failure counters as report lines — `sw:req.settled.page_left
 * ×2` (spec 082 RB14): the failure events that carry a cause, plus a read
 * that reached no node at all. Counters and classes only: a counter's name is
 * a closed word list, and nothing the worker logs next to it (a host, a tab)
 * is read here.
 */
export function workerFailureLines(counts: unknown): string[] {
	if (!counts || typeof counts !== 'object') return [];
	const lines: string[] = [];
	for (const [key, value] of Object.entries(counts as Record<string, unknown>)) {
		if (!SW_COUNTER.test(key)) continue;
		if (typeof value !== 'number' || !Number.isInteger(value) || value <= 0) continue;
		const [area, event, cause] = key.split('.');
		const failure =
			key === 'read.exhausted' ||
			(cause !== undefined && SW_FAILURE_EVENTS.has(`${area}.${event}`));
		if (!failure) continue;
		lines.push(`sw:${key} ×${value}`);
	}
	return lines.sort();
}

/**
 * The failures the wallet PAGE saw itself (spec 082 G61) — a submit that may
 * have been sent, one that was not sent, one the relay refused, a fee quote
 * that failed, by cause. The worker's counters only cover what the worker
 * does; a panel whose payment ended "not sent" left no trace a report could
 * carry. Closed names, like the worker's: `<area>.<event>[.<cause>]`.
 */
export type PanelFailure =
	'submit.maybe_sent' | 'submit.not_sent' | 'submit.refused' | `fee.quote_failed.${string}`;

/** Where the page's counts live for the tab's life (a panel reload keeps them). */
export const PANEL_COUNTS_KEY = 'vela.panel.counts';

const panelCounts = new Map<string, number>();

/** The counts this tab kept, once per load — `sessionStorage` may be absent or refuse. */
function panelStore(): Storage | null {
	try {
		return typeof sessionStorage === 'undefined' ? null : sessionStorage;
	} catch {
		return null;
	}
}

let panelCountsLoaded = false;
function loadPanelCounts(): void {
	if (panelCountsLoaded) return;
	panelCountsLoaded = true;
	try {
		const raw = panelStore()?.getItem(PANEL_COUNTS_KEY);
		const parsed: unknown = raw ? JSON.parse(raw) : null;
		if (!parsed || typeof parsed !== 'object') return;
		for (const [key, value] of Object.entries(parsed as Record<string, unknown>)) {
			if (SW_COUNTER.test(key) && typeof value === 'number' && Number.isInteger(value)) {
				panelCounts.set(key, Math.max(value, panelCounts.get(key) ?? 0));
			}
		}
	} catch {
		/* nothing kept: the counts start from this load */
	}
}

/**
 * Count one failure the page saw. A name outside the closed shape is dropped:
 * a counter is a class, never a value (no hash, address or URL can ride on it).
 */
export function countPanelFailure(event: PanelFailure): void {
	if (!SW_COUNTER.test(event)) return;
	loadPanelCounts();
	panelCounts.set(event, (panelCounts.get(event) ?? 0) + 1);
	try {
		panelStore()?.setItem(PANEL_COUNTS_KEY, JSON.stringify(Object.fromEntries(panelCounts)));
	} catch {
		/* kept in memory for this load */
	}
}

/** The page's failure counts as report lines — `panel:submit.not_sent ×1`. */
export function panelFailureLines(): string[] {
	loadPanelCounts();
	const lines: string[] = [];
	for (const [key, value] of panelCounts) {
		if (value > 0) lines.push(`panel:${key} ×${value}`);
	}
	return lines.sort();
}

/** Tests only: forget every count. */
export function _resetPanelFailuresForTest(): void {
	panelCounts.clear();
	panelCountsLoaded = false;
	try {
		panelStore()?.removeItem(PANEL_COUNTS_KEY);
	} catch {
		/* nothing to forget */
	}
}

/**
 * The failure lines a report carries beside the net counters: the page's own
 * ({@link panelFailureLines}, spec 082 G61) and — in the extension — the
 * worker's ({@link workerFailureLines}; `[]` on the hosted wallet, which has no
 * worker, and when the session store cannot be read).
 */
export async function readWorkerFailureLines(): Promise<string[]> {
	const panel = panelFailureLines();
	const session = (
		globalThis as {
			chrome?: { storage?: { session?: { get(key: string): Promise<Record<string, unknown>> } } };
		}
	).chrome?.storage?.session;
	if (!isExtensionPage() || typeof session?.get !== 'function') return panel;
	try {
		const all = await session.get(SW_COUNTS_KEY);
		return [...panel, ...workerFailureLines(all?.[SW_COUNTS_KEY])];
	} catch {
		return panel;
	}
}

/** The corpus labels the preview lines wear. */
export interface EnvironmentLabels {
	version: string;
	platform: string;
	language: string;
	rpc: string;
	failures: string;
	none: string;
}

/**
 * The second line of defence, applied to every generated line.
 *
 * Addresses and URLs are replaced rather than dropped, so a reader of the
 * issue can see that something was removed instead of quietly reading a
 * shorter sentence. `0x` + 40 hex is an address; anything with a scheme is a
 * URL and may carry a key in its path or query.
 */
export function redact(line: string): string {
	return line
		.replace(/0x[0-9a-fA-F]{40}\b/g, '[address]')
		.replace(/\b[a-zA-Z][a-zA-Z0-9+.-]*:\/\/\S+/g, '[url]');
}

/**
 * This page is one of the MV3 extension's own. `chrome.runtime.id` is set
 * only there: an ordinary page gets a `chrome.runtime` at most (when some
 * extension lets the site message it), never an id.
 */
export function isExtensionPage(): boolean {
	return (
		(globalThis as { chrome?: { runtime?: { id?: string } } }).chrome?.runtime?.id !== undefined
	);
}

/**
 * A coarse, honest platform line for a browser: "Web · Chrome 151 on macOS",
 * "Web (extension) · …" — the preview's line and the payload's, the same
 * string.
 *
 * NOT the user agent (078 design review, DECIDED): the raw agent was four to
 * seven wrapped lines on the sheet and, worse, a device fingerprint posted in
 * a public issue. The browser's name, its major version and the OS's name
 * say what a triager needs and nothing that tells one person from another.
 */
export function webPlatform(): string {
	const described = webOs();
	return `Web${isExtensionPage() ? ' (extension)' : ''}${described === '' ? '' : ` · ${described}`}`;
}

/** Which app is sending: the same test {@link webPlatform} words for the preview. */
export function webClient(): ReportClient {
	return isExtensionPage() ? 'extension' : 'web';
}

/** What a browser says about itself — the inputs {@link describeBrowser} reads. */
export interface BrowserSignals {
	userAgent: string;
	/** `navigator.userAgentData.brands` (Chromium only): the honest name, Brave included. */
	brands?: readonly { brand: string; version: string }[];
	/** `navigator.userAgentData.platform` (Chromium only): "macOS", "Windows", "Android"… */
	platform?: string;
	/** `navigator.maxTouchPoints`: an iPad asks for the desktop site and says "Macintosh". */
	touchPoints?: number;
}

const BRAND_NAMES: Record<string, string> = {
	'Google Chrome': 'Chrome',
	'Microsoft Edge': 'Edge',
	'Opera GX': 'Opera'
};

const PLATFORM_NAMES: Record<string, string> = {
	macOS: 'macOS',
	Windows: 'Windows',
	Android: 'Android',
	Linux: 'Linux',
	iOS: 'iOS',
	'Chrome OS': 'ChromeOS',
	'Chromium OS': 'ChromeOS'
};

/** The browser's name and major version — "Chrome 151" — or '' when it cannot be told. */
function browserName(signals: BrowserSignals): string {
	const brands = (signals.brands ?? []).filter(
		(b) => !/not.?a.?brand/i.test(b.brand) && b.brand !== 'Chromium'
	);
	const branded = brands[0] ?? signals.brands?.find((b) => b.brand === 'Chromium');
	if (branded !== undefined) {
		const name = (BRAND_NAMES[branded.brand] ?? branded.brand).replace(/[^\w .-]/g, '').trim();
		const major = /^\d+/.exec(branded.version)?.[0];
		if (name !== '') return major === undefined ? name : `${name} ${major}`;
	}
	const agent = signals.userAgent;
	const rules: [RegExp, string][] = [
		[/\bEdg(?:e|A|iOS)?\/(\d+)/, 'Edge'],
		[/\bOPR\/(\d+)/, 'Opera'],
		[/\bSamsungBrowser\/(\d+)/, 'Samsung Internet'],
		[/\b(?:Firefox|FxiOS)\/(\d+)/, 'Firefox'],
		[/\bCriOS\/(\d+)/, 'Chrome'],
		[/\bChrome\/(\d+)/, 'Chrome'],
		[/\bVersion\/(\d+)[\d.]*(?: Mobile\/\S+)? Safari\//, 'Safari']
	];
	for (const [pattern, name] of rules) {
		const hit = pattern.exec(agent);
		if (hit) return `${name} ${hit[1]}`;
	}
	return '';
}

/**
 * The OS by NAME only. Browsers freeze the version they report (Chrome says
 * "Android 10" and "Mac OS X 10_15_7" on every device, Safari 26 says iOS
 * 18), so a version here would be a confident wrong answer.
 */
function osName(signals: BrowserSignals): string {
	const platform = signals.platform === undefined ? undefined : PLATFORM_NAMES[signals.platform];
	if (platform !== undefined) return platform;
	const agent = signals.userAgent;
	if (/\biPad\b/.test(agent)) return 'iPadOS';
	if (/\b(?:iPhone|iPod)\b/.test(agent)) return 'iOS';
	if (/\bAndroid\b/.test(agent)) return 'Android';
	if (/\bCrOS\b/.test(agent)) return 'ChromeOS';
	if (/\bWindows\b/.test(agent)) return 'Windows';
	if (/\b(?:Macintosh|Mac OS X)\b/.test(agent)) {
		return (signals.touchPoints ?? 0) > 1 ? 'iPadOS' : 'macOS';
	}
	if (/\bLinux\b/.test(agent)) return 'Linux';
	return '';
}

/**
 * "Chrome 151 on macOS" — the browser and the OS in one short line, for the
 * issue's "Platform:" line (078 §E). Built from a closed set of names and a
 * number, so nothing the person has — a profile name, a URL — can ride along;
 * '' when neither can be told (the endpoint then says just "Web").
 */
export function describeBrowser(signals: BrowserSignals): string {
	const browser = browserName(signals);
	const os = osName(signals);
	return browser !== '' && os !== '' ? `${browser} on ${os}` : browser || os;
}

/** {@link describeBrowser} for this browser. */
export function webOs(): string {
	if (typeof navigator === 'undefined') return '';
	const data = (
		navigator as Navigator & {
			userAgentData?: { brands?: { brand: string; version: string }[]; platform?: string };
		}
	).userAgentData;
	return describeBrowser({
		userAgent: navigator.userAgent,
		brands: data?.brands,
		platform: data?.platform,
		touchPoints: navigator.maxTouchPoints
	});
}

/**
 * The first line of what the person wrote, for a title: whitespace
 * collapsed, at most `max` characters — the ellipsis included — cut at a space
 * when one is near the end. The endpoint's `summary()` does the same for the
 * issue it files, so the two roads title a report alike.
 */
export function issueSummary(what: string, max = 80): string {
	const first = what.split('\n').find((line) => line.trim() !== '') ?? what;
	const line = first.replace(/\s+/g, ' ').trim();
	if (line.length <= max) return line;
	const cut = line.slice(0, max - 1);
	const space = cut.lastIndexOf(' ');
	return `${space > (max - 1) * 0.6 ? cut.slice(0, space) : cut}…`;
}

/**
 * The preview, and the payload's `environment`, as one list.
 *
 * Called by the settings live layer to fill `FeedbackModel.previewLines` and
 * by {@link buildBugReport} to fill the field — one function, so the two can
 * never drift and make the consent line false again.
 */
export function environmentLines(labels: EnvironmentLabels, facts: DeviceFacts): string[] {
	const list = (items: readonly string[], separator: string): string =>
		items.length === 0 ? labels.none : items.join(separator);
	return [
		`${labels.version}: v${facts.version} (${facts.commit})`,
		`${labels.platform}: ${facts.platform}`,
		`${labels.language}: ${facts.language}`,
		`${labels.rpc}: ${list(facts.unreachable, ', ')}`,
		`${labels.failures}: ${list(facts.failures, '; ')}`
	].map(redact);
}

/**
 * A stable short marker for "the same complaint".
 *
 * The endpoint uses it to add a comment to an open issue rather than filing a
 * duplicate. It is derived from the words and the build, never from anything
 * identifying: two people hitting the same bug SHOULD collide here, which is
 * the whole point, and that is only safe because the value says nothing about
 * either of them.
 */
export function fingerprintOf(what: string, area: string, version: string): string {
	const seed = `${what.trim().toLowerCase().slice(0, 120)}|${area}|${version}`;
	// FNV-1a, 32-bit. Not a security primitive and not asked to be one: the
	// endpoint already strips it to `[A-Za-z0-9_-]{0,64}`.
	let hash = 0x811c9dc5;
	for (let i = 0; i < seed.length; i++) {
		hash ^= seed.charCodeAt(i);
		hash = Math.imul(hash, 0x01000193) >>> 0;
	}
	return hash.toString(16).padStart(8, '0');
}

export interface BugReportDraft {
	what: string;
	steps?: string;
	area: string;
	/**
	 * The dedup marker, when somebody other than the words decides "the same
	 * complaint": a relay stop's report carries the core's (`relay-gas-130`),
	 * so every report of one outage lands on one issue whatever the person
	 * typed, and whatever version or balance it was sent at (issue 466).
	 * Absent, it is derived from the words ({@link fingerprintOf}).
	 */
	fingerprint?: string;
	labels: EnvironmentLabels;
	facts: DeviceFacts;
	/** Base64 JPEGs from `screenshot-prep.ts`, in tile order. */
	screenshots?: readonly string[];
}

/** The whole payload, from the allowlist and nothing else. */
export function buildBugReport(draft: BugReportDraft): BugReportPayload {
	const what = draft.what.trim();
	const steps = (draft.steps ?? '').trim();
	const environment = environmentLines(draft.labels, draft.facts).join('\n');
	const shots = (draft.screenshots ?? []).slice(0, MAX_SCREENSHOTS);
	return {
		what,
		steps,
		area: draft.area,
		environment,
		fingerprint: draft.fingerprint ?? fingerprintOf(what, draft.area, draft.facts.version),
		client: draft.facts.client,
		// One line, scrubbed like every generated line, and short: it lands in
		// the issue's header line.
		os: redact(draft.facts.os.replace(/\s+/g, ' ').trim()).slice(0, 120),
		appVersion: draft.facts.version.trim().replace(/^v/i, ''),
		...(shots.length > 0 ? { screenshots: [...shots] } : {})
	};
}

/** How long {@link sendBugReport} waits for this payload. */
export function reportTimeoutMs(payload: BugReportPayload): number {
	return (payload.screenshots?.length ?? 0) > 0 ? SCREENSHOT_TIMEOUT_MS : NET_TIMEOUTS.bundlerRest;
}

/**
 * The prefilled issue form — the fallback road.
 *
 * Field ids, not `body`. See the module doc: `template=bug.yml` makes GitHub
 * ignore `body` entirely, so a URL built the obvious way opens a blank form
 * and the person's typing is gone.
 *
 * The title wears the platform tag the endpoint's issues wear — `[Web] …`,
 * `[Extension] …` — in place of bug.yml's default `[bug] ` (078 §E), so a
 * report that took this road is triaged like one that did not.
 */
export function prefilledIssueURL(payload: BugReportPayload): string {
	const params = new URLSearchParams({
		template: 'bug.yml',
		title: `[${CLIENT_TAGS[payload.client]}] ${issueSummary(payload.what)}`,
		what: payload.what,
		// The form marks steps required; an empty box is better than a missing
		// one, because the person can see the cursor sitting in it.
		steps: payload.steps,
		environment: payload.environment
	});
	if (payload.area !== '') params.set('area', payload.area);
	return `${GITHUB_ISSUE_FORM}?${params.toString()}`;
}

/** Filed on the tracker, either as a new issue or as a +1 on an open one. */
export interface BugReportFiled {
	ok: true;
	number: number;
	url: string;
	deduped: boolean;
	/** Screenshots the endpoint could not store; the report filed without them. */
	screenshotsDropped: number;
}

/**
 * The endpoint could not file it. `fallbackUrl` is the prefilled form, and a
 * caller that shows anything else is dropping the person's report.
 */
export interface BugReportFallback {
	ok: false;
	/** For the log and the tests; never shown raw to a person. */
	reason: 'not_configured' | 'rate_limited' | 'too_large' | 'rejected' | 'unreachable';
	fallbackUrl: string;
}

export type BugReportOutcome = BugReportFiled | BugReportFallback;

/**
 * Send it, or hand back the road that still works.
 *
 * Every non-2xx and every network fault falls back, deliberately — 503
 * `not_configured` is the DESIGNED one (the endpoint says so in its own module
 * doc), but a person whose report hit a 500 is owed the same second chance as
 * one who hit an unprovisioned server.
 */
export async function sendBugReport(
	payload: BugReportPayload,
	endpoint: string = BUG_REPORT_ENDPOINT
): Promise<BugReportOutcome> {
	const fallbackUrl = prefilledIssueURL(payload);
	// Checked here as well as there: a 413 round trip costs the person a
	// spinner to learn what this line knows before the request leaves. The
	// cap is on the TEXT; the screenshots are capped by count and size.
	const { screenshots: _images, ...text } = payload;
	void _images;
	if (JSON.stringify(text).length > MAX_REPORT_CHARS) {
		return { ok: false, reason: 'too_large', fallbackUrl };
	}
	const body = JSON.stringify(payload);

	let response: Response;
	try {
		response = await fetchWithTimeout(
			endpoint,
			{ method: 'POST', headers: { 'Content-Type': 'application/json' }, body },
			{ timeoutMs: reportTimeoutMs(payload) }
		);
	} catch (error) {
		console.error('[bug-report]', isTimeoutError(error) ? 'timed out' : 'network error');
		return { ok: false, reason: 'unreachable', fallbackUrl };
	}

	if (!response.ok) {
		const reason =
			response.status === 503
				? 'not_configured'
				: response.status === 429
					? 'rate_limited'
					: response.status === 413
						? 'too_large'
						: 'rejected';
		console.error(`[bug-report] endpoint answered ${response.status} — falling back`);
		return { ok: false, reason, fallbackUrl };
	}

	try {
		const filed = (await response.json()) as {
			number?: number;
			url?: string;
			deduped?: boolean;
			screenshotsDropped?: number;
		};
		if (typeof filed.number !== 'number' || typeof filed.url !== 'string') {
			return { ok: false, reason: 'rejected', fallbackUrl };
		}
		const dropped = filed.screenshotsDropped;
		return {
			ok: true,
			number: filed.number,
			url: filed.url,
			deduped: filed.deduped === true,
			screenshotsDropped:
				typeof dropped === 'number' && Number.isFinite(dropped) && dropped > 0 ? dropped : 0
		};
	} catch {
		// A 200 this client cannot read is not a filed report it can point at.
		return { ok: false, reason: 'rejected', fallbackUrl };
	}
}
