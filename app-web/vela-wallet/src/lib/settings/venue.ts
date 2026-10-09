/**
 * Spec 102 — where an account reviews and signs, and the signing pages a device
 * trusts, as the settings screens draw them.
 *
 * Pure mappings from the core's own answers (`VenueChoice[]`, `SigningPageRow[]`,
 * `VenueBlock`, `IntegrityLine`) to the screens' models — no rule is decided
 * here. Which venue can reach which keys (R1), which choices an account has
 * (R2) and what an integrity check concluded (R6) are the core's; this file
 * only puts words on them.
 *
 * Shared by the live route and the gallery fixtures ON PURPOSE: the gallery is
 * the design the phones build to, and a board drawn by a mapping of its own
 * would be a picture of a screen nobody ships.
 */
import type { IntegrityLine } from '$lib/core/generated/IntegrityLine';
import type { SigningPageRow } from '$lib/core/generated/SigningPageRow';
import type { SigningPlan } from '$lib/core/generated/SigningPlan';
import type { SigningVenue } from '$lib/core/generated/SigningVenue';
import type { VenueBlock } from '$lib/core/generated/VenueBlock';
import type { VenueChoice } from '$lib/core/generated/VenueChoice';
import { venueBlockLine } from '$lib/core/kernels';
import { fill } from '$lib/wallet/messages';
import type { SettingsMessages } from './messages';
import type {
	IntegrityLineModel,
	SigningPageRowModel,
	SigningPagesModel,
	VenueModel,
	VenueRowModel
} from './model';

type Words = Pick<SettingsMessages, 'signing' | 'venue' | 'integrity'>;

/** The host a page lives on, for a row — the address itself if it does not parse. */
export function hostOf(url: string): string {
	try {
		return new URL(url).host;
	} catch {
		return url;
	}
}

/**
 * The refusals' words, by the corpus key the core names for each
 * (`VenueBlock::key()`) — Settings' and, for a refusal at sign time, the
 * sheets'. A manifest resolves exactly `VENUE_BLOCK_KEYS` (`./messages`).
 */
export type VenueBlockWords = Readonly<Record<string, string>>;

/**
 * Why a venue cannot be used, in the person's words: the core's whole
 * sentence (`venueBlockLine` — its corpus key and the values that fill it),
 * translated. No variant is read here. Empty only for a line this build has
 * no words for, which `VENUE_BLOCK_KEYS` and `venue.test.ts` rule out.
 */
export function venueBlockText(block: VenueBlock, words: VenueBlockWords): string {
	const line = venueBlockLine(block);
	const template = line === null ? undefined : words[line.key];
	return line === null || template === undefined ? '' : fill(template, line.vars);
}

/** `Keys on {{domain}}`. */
export function keysOnText(domain: string, m: Words): string {
	return fill(m.signing.keysOn, { domain });
}

/** How loud each state is drawn. `opens` decides the button; this only the colour. */
const INTEGRITY_TONE: Record<keyof SettingsMessages['integrity'], IntegrityLineModel['tone']> = {
	matches: 'ok',
	trusted: 'ok',
	checking: 'checking',
	unchecked: 'warn',
	askTrust: 'warn',
	mismatch: 'error',
	blocked: 'error',
	couldNotCheck: 'error',
	noVersion: 'error',
	allBlocked: 'error'
};

/** The corpus key's last segment → the manifest's field (`…integrity.askTrust` → `askTrust`). */
const INTEGRITY_FIELD = (key: string): keyof SettingsMessages['integrity'] | undefined => {
	const field = key.split('.').at(-1);
	return field !== undefined && field in INTEGRITY_TONE
		? (field as keyof SettingsMessages['integrity'])
		: undefined;
};

/**
 * The core's integrity line, in words. `time` is `checked_at_ms` as the
 * person reads times (their own format) — the shell's to format, the core's
 * to supply. `undefined` for a key this build has no words for.
 */
export function integrityLineModel(
	line: Pick<IntegrityLine, 'key' | 'version'>,
	time: string,
	m: Pick<Words, 'integrity'>
): IntegrityLineModel | undefined {
	const field = INTEGRITY_FIELD(line.key);
	if (field === undefined) return undefined;
	return {
		text: fill(m.integrity[field], { version: line.version, time }),
		tone: INTEGRITY_TONE[field]
	};
}

/**
 * What a page is called on a row (D6, D-19): "Vela's official signing page",
 * the person's own label, else "Self-hosted · {{domain}}" — never "my own
 * page", and never a bare host that reads like an address to type.
 */
function pageName(official: boolean, name: string, domain: string, m: Words): string {
	if (official) return m.signing.pageOfficial;
	return name.trim() !== '' ? name.trim() : fill(m.signing.pageSelfHosted, { domain });
}

/**
 * The host to draw with a page's name — `undefined` when the name already
 * says it ("Self-hosted · sign.example.com"), so a row never names the same
 * address twice.
 */
function hostLine(name: string, host: string): string | undefined {
	return name.includes(host) ? undefined : host;
}

/**
 * "Keys on {{domain}}" for a page — only when its keys live on a domain its
 * address does not already say: Vela's official page (sign.getvela.app, keys
 * on getvela.app) says it; a self-hosted page whose keys are its own host's
 * would only repeat the address.
 */
function pageKeysOn(url: string, domain: string, m: Words): string | undefined {
	let hostname = url;
	try {
		hostname = new URL(url).hostname;
	} catch {
		// Not an address: say whose keys, as for any other page.
	}
	return hostname === domain ? undefined : keysOnText(domain, m);
}

/** A page row's name and host, for "Where you review and sign"; `undefined` for Vela's own sheet. */
function pageOf(choice: VenueChoice, m: Words): VenueRowModel['page'] {
	if (choice.venue.type !== 'page') return undefined;
	const name = pageName(choice.official, choice.name, choice.domain, m);
	const host = hostOf(choice.venue.url);
	return { name, host, official: choice.official, hostShown: hostLine(name, host) !== undefined };
}

/** Two venues the core named alike (it normalises a page's address). */
function sameVenue(a: SigningVenue, b: SigningVenue): boolean {
	return a.type === 'page' ? b.type === 'page' && a.url === b.url : b.type === a.type;
}

export interface VenueInput {
	/** The account's signing domain (`SigningPlan.domain`). */
	domain: string;
	/** `signingVenueChoices(domain, venue, saved)` — the core's rows, in its order. */
	choices: VenueChoice[];
	/** Each page's integrity line, by address — where the shell checks pages. */
	integrity?: Partial<Record<string, IntegrityLineModel>>;
	/**
	 * The web (P2-09, D-16): it opens no signing page, so the list is a
	 * statement, not a choice. `choices` are then `signingVenueChoices(…,
	 * 'web')` — every page row disabled with its reason — and this is the
	 * core's web plan (`signingPlan(record, 'web')`): the row it signs with is
	 * the one marked (Vela's sheet, for a `getvela.app` account whatever its
	 * stored venue, P2-11), and a `blocked` plan marks none and says why.
	 */
	web?: Pick<SigningPlan, 'venue' | 'blocked'>;
}

/** "Where you review and sign", from the core's choices (R1, R2). */
export function venueModel(input: VenueInput, m: Words): VenueModel {
	const web = input.web;
	const rows = input.choices.map((choice): VenueRowModel => {
		const page = pageOf(choice, m);
		return {
			id: choice.venue.type === 'page' ? choice.venue.url : 'in_vela',
			venue: choice.venue,
			icon: page === undefined ? 'wallet' : 'shield-check',
			title: page === undefined ? m.venue.inVela : m.venue.page,
			body: page === undefined ? m.venue.inVelaBody : m.venue.pageBody,
			page,
			keysOn: keysOnText(choice.domain, m),
			integrity: choice.venue.type === 'page' ? input.integrity?.[choice.venue.url] : undefined,
			// The web marks where IT signs (the core's web plan), not the
			// stored choice it cannot open.
			active:
				web === undefined
					? choice.active
					: web.blocked == null && sameVenue(choice.venue, web.venue),
			blocked: choice.blocked ? venueBlockText(choice.blocked, m.venue.blocked) : undefined
		};
	});
	const active = rows.find((row) => row.active);
	// Nothing here can sign: the web's plan says why (D-16), else R1's reason
	// under the first row.
	const blocked =
		web?.blocked != null
			? venueBlockText(web.blocked, m.venue.blocked)
			: rows.length > 0 && rows.every((row) => row.blocked !== undefined)
				? rows[0].blocked
				: undefined;
	const value =
		active === undefined ? '' : active.page === undefined ? m.venue.inVela : m.venue.page;
	const note =
		blocked !== undefined
			? blocked
			: active?.page !== undefined
				? active.page.official
					? active.page.host
					: active.page.hostShown
						? `${active.page.name} · ${active.page.host}`
						: active.page.name
				: undefined;
	return {
		title: m.venue.title,
		subtitle: m.venue.subtitle,
		value,
		note,
		// One line for the settings row: where it signs now, then on which
		// host — or, when nothing here can, why.
		summary:
			blocked !== undefined
				? blocked
				: active?.page !== undefined
					? `${value} · ${active.page.host}`
					: value,
		domainLine: keysOnText(input.domain, m),
		rows,
		readOnly: web !== undefined ? true : undefined
	};
}

export interface SigningPagesInput {
	/** `SigningPagesView.pages` — the official page first. */
	pages: SigningPageRow[];
	/** `SigningPagesView.add_error`: `invalid` | `insecure` | `duplicate`. */
	addError?: string | null;
	/** What is typed in the add field. */
	draft?: string;
	integrity?: Partial<Record<string, IntegrityLineModel>>;
	/**
	 * A self-hosted page whose check asks to trust an unknown version
	 * (`IntegrityState::AskToTrust`), by address → the version to trust
	 * (`admission.version_to_trust()`).
	 */
	askTrust?: Partial<Record<string, string>>;
}

/** Settings → Signing pages, from the core's view. */
export function signingPagesModel(input: SigningPagesInput, m: Words): SigningPagesModel {
	const refusal =
		input.addError === 'invalid'
			? m.signing.pageInvalid
			: input.addError === 'insecure'
				? m.signing.pageInsecure
				: input.addError === 'duplicate'
					? m.signing.pageDuplicate
					: undefined;
	return {
		title: m.signing.title,
		subtitle: m.signing.subtitle,
		rows: input.pages.map((row): SigningPageRowModel => {
			const name = pageName(row.official, row.name, row.domain, m);
			return {
				url: row.url,
				name,
				// Named once (polish 3): no host the name already says, and whose
				// keys only where that is not the page's own host.
				host: hostLine(name, hostOf(row.url)),
				keysOn: pageKeysOn(row.url, row.domain, m),
				official: row.official,
				integrity: input.integrity?.[row.url],
				// The core's question (`integrity.askTrust`) gets its own answer:
				// "Trust this version" sends `version_trusted {url, version}` (D-15).
				trust:
					input.askTrust?.[row.url] !== undefined && !row.official
						? { label: m.signing.pageTrust, version: input.askTrust[row.url] as string }
						: undefined
			};
		}),
		add: {
			id: 'signing-page-add',
			label: m.signing.pageAdd,
			value: input.draft ?? '',
			placeholder: 'https://',
			hint: refusal,
			tone: refusal === undefined ? 'default' : 'error'
		},
		addAction: m.signing.pageSave,
		renameLabel: m.signing.pageRename,
		removeLabel: m.signing.pageRemove
	};
}
