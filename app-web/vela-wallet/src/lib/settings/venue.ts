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
import type { VenueBlock } from '$lib/core/generated/VenueBlock';
import type { VenueChoice } from '$lib/core/generated/VenueChoice';
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

/** R1's reason, in the person's words. */
export function venueBlockText(block: VenueBlock, m: Words): string {
	return block.type === 'app_cannot_reach'
		? fill(m.venue.blockedApp, { domain: block.domain })
		: fill(m.venue.blockedPage, { pageDomain: block.page_domain, domain: block.domain });
}

/** `Keys on {{domain}}`. */
export function keysOnText(domain: string, m: Words): string {
	return fill(m.signing.keysOn, { domain });
}

/** How loud each state is drawn. `opens` decides the button; this only the colour. */
const INTEGRITY_TONE: Record<keyof SettingsMessages['integrity'], IntegrityLineModel['tone']> = {
	matches: 'ok',
	trusted: 'ok',
	checking: 'warn',
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
	m: Words
): IntegrityLineModel | undefined {
	const field = INTEGRITY_FIELD(line.key);
	if (field === undefined) return undefined;
	return {
		text: fill(m.integrity[field], { version: line.version, time }),
		tone: INTEGRITY_TONE[field]
	};
}

/** What a page is called on a row: "Official", the person's label, else its host. */
function pageName(official: boolean, name: string, url: string, m: Words): string {
	if (official) return m.signing.pageOfficial;
	return name.trim() !== '' ? name.trim() : hostOf(url);
}

export interface VenueInput {
	/** The account's signing domain (`SigningPlan.domain`). */
	domain: string;
	/** `signingVenueChoices(domain, venue, saved)` — the core's rows, in its order. */
	choices: VenueChoice[];
	/** Each page's integrity line, by address — where the shell checks pages. */
	integrity?: Partial<Record<string, IntegrityLineModel>>;
	/**
	 * The web: it opens no signing page (owner, 2026-09-23), so the only row it
	 * has is Vela's own sheet, shown as where signing happens HERE — whatever
	 * the account's stored venue says (P2-11: a page venue signs natively on
	 * the web) — or, for a custom-domain account, disabled with R1's reason.
	 */
	webOnly?: boolean;
}

/** "Where you review and sign", from the core's choices (R1, R2). */
export function venueModel(input: VenueInput, m: Words): VenueModel {
	const all = input.choices.map((choice): VenueRowModel => {
		const page =
			choice.venue.type === 'page'
				? {
						name: pageName(choice.official, choice.name, choice.venue.url, m),
						host: hostOf(choice.venue.url),
						official: choice.official
					}
				: undefined;
		return {
			id: choice.venue.type === 'page' ? choice.venue.url : 'in_vela',
			venue: choice.venue,
			icon: page === undefined ? 'wallet' : 'shield-check',
			title: page === undefined ? m.venue.inVela : m.venue.page,
			body: page === undefined ? m.venue.inVelaBody : m.venue.pageBody,
			page,
			keysOn: keysOnText(choice.domain, m),
			integrity: choice.venue.type === 'page' ? input.integrity?.[choice.venue.url] : undefined,
			active: choice.active,
			blocked: choice.blocked ? venueBlockText(choice.blocked, m) : undefined
		};
	});
	const rows = input.webOnly
		? all
				.filter((row) => row.venue.type === 'in_vela')
				.map((row) => ({ ...row, active: row.blocked === undefined }))
		: all;
	const active = rows.find((row) => row.active);
	const blocked = rows.length > 0 && rows.every((row) => row.blocked !== undefined);
	const value =
		active === undefined ? '' : active.page === undefined ? m.venue.inVela : m.venue.page;
	const note = blocked
		? rows[0].blocked
		: active?.page !== undefined
			? active.page.official
				? active.page.host
				: `${active.page.name} · ${active.page.host}`
			: undefined;
	return {
		title: m.venue.title,
		subtitle: m.venue.subtitle,
		value,
		note,
		// One line for the settings row: where it signs now, then on which
		// host — or, when nothing here can, why.
		summary: blocked
			? (note ?? '')
			: active?.page !== undefined
				? `${value} · ${active.page.host}`
				: value,
		domainLine: keysOnText(input.domain, m),
		rows,
		readOnly: input.webOnly === true ? true : undefined
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
		officialTag: m.signing.pageOfficial,
		rows: input.pages.map((row): SigningPageRowModel => ({
			url: row.url,
			name: pageName(row.official, row.name, row.url, m),
			host: hostOf(row.url),
			keysOn: keysOnText(row.domain, m),
			official: row.official,
			integrity: input.integrity?.[row.url]
		})),
		add: {
			id: 'signing-page-add',
			label: m.signing.pageAdd,
			value: input.draft ?? '',
			placeholder: 'https://',
			hint: refusal,
			tone: refusal === undefined ? 'default' : 'error'
		},
		addAction: m.signing.pageSave,
		renameLabel: m.signing.rename,
		removeLabel: m.signing.remove
	};
}
