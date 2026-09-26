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
 * Addresses, balances, and endpoint or RPC URLs — the last because a
 * self-hosted endpoint routinely carries an API key in its path, and a public
 * issue tracker is the worst possible place for one. Raw `vela.*` values are
 * excluded for the same reason at one remove: they are the wallet's whole
 * shelf, and "include the storage for context" is how the other three get in.
 *
 * The defence is not a scrub over a broad payload — it is that the payload is
 * ASSEMBLED from a five-field allowlist ({@link DeviceFacts}) that has no way
 * to reach any of them. {@link redact} is the second line, for the one field
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
 * The five text fields the endpoint accepts, and the images the person chose
 * to attach. Nothing else is sent, and this type is the reason another field
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
 * Five fields, by name, because an object with an index signature is how a
 * balance ends up in a bug report six months from now.
 */
export interface DeviceFacts {
	/** `1.0.0` — the build's own, never the mock's. */
	version: string;
	/** Short commit, or `unknown`. */
	commit: string;
	/** `Web · <user agent>` — see {@link webPlatform}. */
	platform: string;
	/** The UI language tag in use. */
	language: string;
	/** Display NAMES of networks whose RPC is unreachable. Never their URLs. */
	unreachable: readonly string[];
	/** Recent failure summaries: counters and classes, never values. */
	failures: readonly string[];
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

/** A coarse, honest platform string for a browser. */
export function webPlatform(): string {
	if (typeof navigator === 'undefined') return 'Web';
	const extension =
		(globalThis as { chrome?: { runtime?: { id?: string } } }).chrome?.runtime?.id !== undefined;
	const agent = navigator.userAgent.trim();
	return `Web${extension ? ' (extension)' : ''}${agent === '' ? '' : ` · ${agent}`}`;
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
		fingerprint: fingerprintOf(what, draft.area, draft.facts.version),
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
 */
export function prefilledIssueURL(payload: BugReportPayload): string {
	const params = new URLSearchParams({
		template: 'bug.yml',
		title: `[bug] ${payload.what.slice(0, 80)}`,
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
