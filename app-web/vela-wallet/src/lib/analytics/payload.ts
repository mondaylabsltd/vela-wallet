/**
 * What one analytics hit looks like on the wire — pure, so the privacy rules
 * are tested here rather than trusted at every call site.
 *
 * The shape is Rybbit's own (`tj.appsdata.org/api/script.js`,
 * `createBasePayload` + `track`), restricted to the fields this wallet is
 * willing to send. The server validates with a STRICT schema, so a field it
 * does not know is a rejected hit: nothing is added here that the script
 * itself would not send.
 *
 * Left out on purpose:
 *   - every query parameter but the five campaign tags (`UTM_PARAMS`): a
 *     contact's Send carries `?to=<address>` and a pay link
 *     `?to=…&amount=…`, and none of that may leave. The tags stay because
 *     they are how a visit is attributed — getvela.app's links to the wallet
 *     carry them — and how a test session marks itself
 *     (`utm_content=internal-test`);
 *   - `page_title`: a title can carry whatever a page put in it;
 *   - `anonymous_id` / `user_id`: no identifier is stored or sent — the
 *     server counts visitors the way it does for every cookieless site;
 *   - the script's bot-score fields (`_bs`, `_bsm`): it computes them by
 *     probing the browser, which this module does not do.
 */
import { localeOfPath, toLocale } from '$lib/i18n/locales';
import {
	AMBIENT_PROPERTIES,
	EVENTS,
	PROPERTY_RULES,
	isAnalyticsEvent,
	type AnalyticsEventName,
	type AnalyticsProperty
} from './catalog';

/** The Rybbit site the wallet reports to — the same site getvela.app uses. */
export const RYBBIT_SITE_ID = '51bb55d72d55';
/** Rybbit's ingestion endpoint (`${analyticsHost}/track` in its script). */
export const RYBBIT_TRACK_URL = 'https://tj.appsdata.org/api/track';

/** What the page knows about itself when a hit is built. */
export interface PageContext {
	/** The route id SvelteKit matched (`/[locale]/wallet`), or null. */
	routeId: string | null;
	/** `location.pathname` — only ever read through {@link pagePath}. */
	pathname: string;
	/** `location.search` — only ever read through {@link campaignQuery}. */
	search: string;
	hostname: string;
	/** The document's own origin, so a self-referrer reads as none. */
	origin: string;
	/** `navigator.language`. */
	language: string;
	screenWidth: number;
	screenHeight: number;
	/** `document.referrer`. */
	referrer: string;
	/** Which build is sending. */
	surface: 'web' | 'extension';
}

/** The body POSTed to {@link RYBBIT_TRACK_URL}. */
export interface RybbitPayload {
	site_id: string;
	type: 'pageview' | 'custom_event';
	hostname: string;
	pathname: string;
	/** The campaign tags alone (`?utm_source=…`), or empty. */
	querystring: string;
	screenWidth: number;
	screenHeight: number;
	language: string;
	referrer: string;
	event_name?: string;
	/** A JSON string, as Rybbit's schema requires. */
	properties?: string;
}

/**
 * Does `text` carry something that identifies a person or their money?
 *
 * - a `0x`-prefixed hex run of 8 or more digits: an address, a hash, calldata;
 * - a bare hex run of 32 or more: the same without its prefix;
 * - a run of 17 or more digits: an amount in base units (no chain id is that
 *   long — EIP-2294 caps them at 16 digits);
 * - an ENS-style name (`name.eth`).
 */
export function carriesIdentifier(text: string): boolean {
	return (
		/0x[0-9a-f]{8,}/i.test(text) ||
		/[0-9a-f]{32,}/i.test(text) ||
		/\d{17,}/.test(text) ||
		/[a-z0-9-]\.eth\b/i.test(text)
	);
}

/** A path segment that names a route, not a thing: lowercase words and hyphens. */
const ROUTE_WORD = /^[a-z][a-z-]{0,39}$/;
/** A route word that is also hex (`deadbeef`) could be an id cut short. */
const HEX_WORD = /^[a-f]{8,}$/;

function maskSegment(segment: string): string {
	const bare = segment.replace(/\.html$/, '');
	if (bare === '') return '';
	const locale = toLocale(bare);
	if (locale !== undefined) return locale;
	if (/^:[a-z]{1,32}$/.test(bare)) return bare;
	if (ROUTE_WORD.test(bare) && !HEX_WORD.test(bare)) return bare;
	return ':id';
}

/**
 * A path with every segment that is not a locale or a route word masked to
 * `:id`, and the packaged extension's `.html` dropped, so `/ja/wallet.html`
 * and `/ja/wallet` count as one page.
 */
export function maskPath(pathname: string): string {
	const path = pathname.split(/[?#]/)[0] ?? '';
	const masked = path.split('/').map(maskSegment).join('/');
	const normal = masked.length > 1 ? masked.replace(/\/+$/, '') : masked;
	return normal.startsWith('/') ? normal : `/${normal}`;
}

/**
 * The path a hit reports: the matched ROUTE PATTERN with the page's locale put
 * in (`/[locale]/wallet` → `/ja/wallet`), every other parameter left as its
 * name. Without a route id (a page the extension loads fresh has none yet),
 * the masked path.
 */
export function pagePath(routeId: string | null, pathname: string): string {
	if (routeId === null || routeId === '') return maskPath(pathname);
	const locale = localeOfPath(pathname);
	const pattern = routeId
		.split('/')
		.filter((segment) => !/^\(.*\)$/.test(segment))
		.map((segment) => {
			if (segment === '[locale]') return locale ?? ':locale';
			const param = /^\[+(?:\.\.\.)?([a-zA-Z]+)(?:=[a-zA-Z]+)?\]+$/.exec(segment);
			return param ? `:${param[1].toLowerCase()}` : segment;
		})
		.join('/');
	return maskPath(pattern);
}

/**
 * Developer surfaces are not usage: the fixture galleries, the parallel space
 * and the dev routes never report.
 */
export function isTrackedPath(path: string): boolean {
	const segments = path.split('/').filter((segment) => segment !== '');
	const first = toLocale(segments[0] ?? '') !== undefined ? segments[1] : segments[0];
	return first !== 'dev' && first !== 'gallery' && first !== 'parallel';
}

/** The site that linked here, as its origin — never its path — or nothing. */
export function coarseReferrer(referrer: string, ownOrigin: string): string {
	if (!referrer) return '';
	try {
		const url = new URL(referrer);
		if (url.protocol !== 'https:' && url.protocol !== 'http:') return '';
		if (url.origin === ownOrigin) return '';
		if (carriesIdentifier(url.hostname)) return '';
		return `${url.origin}/`;
	} catch {
		return '';
	}
}

/** The only query parameters that ever leave: the campaign tags. */
export const UTM_PARAMS = [
	'utm_source',
	'utm_medium',
	'utm_campaign',
	'utm_content',
	'utm_term'
] as const;

/**
 * The campaign tags of `search`, in a fixed order, as a query string
 * (`?utm_source=getvela.app&utm_medium=site`) — everything else dropped. A
 * tag is a short label: one longer than 100 characters, or one shaped like an
 * identifier, is dropped too.
 */
export function campaignQuery(search: string): string {
	let params: URLSearchParams;
	try {
		params = new URLSearchParams(search);
	} catch {
		return '';
	}
	const kept = new URLSearchParams();
	for (const name of UTM_PARAMS) {
		const value = params.get(name)?.trim();
		if (!value || value.length > 100 || carriesIdentifier(value)) continue;
		kept.set(name, value);
	}
	const query = kept.toString();
	return query === '' ? '' : `?${query}`;
}

/** `navigator.language` if it is a language tag, else nothing. */
function languageTag(language: string): string {
	return /^[A-Za-z]{2,3}(?:-[A-Za-z0-9]{2,8}){0,3}$/.test(language) ? language : '';
}

function hostnameOf(hostname: string): string {
	return /^[a-z0-9.-]{1,253}$/i.test(hostname) && !carriesIdentifier(hostname) ? hostname : '';
}

function dimension(value: number): number {
	return Number.isSafeInteger(value) && value >= 0 && value <= 100_000 ? value : 0;
}

/**
 * The properties an event may carry, cleaned: keys outside the event's list
 * (and the ambient pair) are dropped, and so is every value that fails its
 * rule (`catalog.ts`).
 */
export function cleanProperties(
	name: AnalyticsEventName,
	properties: Readonly<Record<string, unknown>>
): Record<string, string | number> {
	const allowed: readonly AnalyticsProperty[] = [...EVENTS[name], ...AMBIENT_PROPERTIES];
	const clean: Record<string, string | number> = {};
	for (const key of allowed) {
		if (!Object.hasOwn(properties, key)) continue;
		const value = properties[key];
		if (PROPERTY_RULES[key](value)) clean[key] = value as string | number;
	}
	return clean;
}

export type Hit =
	| { type: 'pageview' }
	| { type: 'custom_event'; name: string; properties?: Readonly<Record<string, unknown>> };

/**
 * Build the body for one hit, or `null` when it must not be sent: an event
 * the catalog does not name, a developer page, or a payload that — after
 * every rule above — still carries something identifier-shaped. The last is
 * the backstop: whatever a future edit lets through, an address does not
 * leave.
 */
export function buildPayload(hit: Hit, context: PageContext): RybbitPayload | null {
	const pathname = pagePath(context.routeId, context.pathname);
	if (!isTrackedPath(pathname)) return null;
	const payload: RybbitPayload = {
		site_id: RYBBIT_SITE_ID,
		type: hit.type,
		hostname: hostnameOf(context.hostname),
		pathname,
		querystring: campaignQuery(context.search),
		screenWidth: dimension(context.screenWidth),
		screenHeight: dimension(context.screenHeight),
		language: languageTag(context.language),
		referrer: coarseReferrer(context.referrer, context.origin)
	};
	if (hit.type === 'custom_event') {
		if (!isAnalyticsEvent(hit.name)) return null;
		// The page's locale unless the event names one itself (a language
		// change names the NEW one); the surface is always the sender's.
		const locale = localeOfPath(context.pathname);
		const properties = cleanProperties(hit.name, {
			...(locale !== undefined ? { locale } : {}),
			...hit.properties,
			surface: context.surface
		});
		payload.event_name = hit.name;
		payload.properties = JSON.stringify(properties);
	}
	return carriesIdentifier(JSON.stringify(payload)) ? null : payload;
}
