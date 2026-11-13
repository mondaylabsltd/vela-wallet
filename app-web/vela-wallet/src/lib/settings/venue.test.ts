/**
 * Spec 102 — "Where you review and sign" and Settings → Signing pages, against
 * the REAL core.
 *
 * Two promises are pinned here:
 *
 * 1. The gallery boards are screens the core can produce. Their inputs are
 *    literal fixtures (the boards are prerendered from data), so each is asked
 *    of the core here — `signingVenueChoices` for the venue rows, the
 *    `SigningPagesCore` machine for the pages list — and must come back equal.
 *    A board that drifted from the core would be a design nobody can build.
 * 2. The web's and the extension's own reading (owner, 2026-10-30: the
 *    page, everywhere): the apps' rows — R1 alone decides — a real choice for
 *    every account, with what a signing page protects against and what it
 *    does not said under it.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import { SigningPagesCore } from '../../../../../rust/pkg-web/vela_core.js';
import {
	signingPageDomain,
	signingPlan,
	signingVenueBlock,
	signingVenueChoices,
	venueBlockLine
} from '$lib/core/kernels';
import type { VenueBlock } from '$lib/core/generated/VenueBlock';
import { resolveSettingsMessages } from '$lib/i18n/engine.server';
import { SUPPORTED_LOCALES } from '$lib/i18n/locales';
import type { SigningPagesView } from '$lib/core/generated/SigningPagesView';
import { BOARD_CHECK, BOARD_CHECK_TIME, boardCheckTime } from './board-check';
import {
	NEW_PAGE,
	NEW_PAGE_VERSION,
	OFFICIAL_PAGE,
	OWN_PAGE,
	SAVED_PAGES,
	SIGNING_PAGE_ROWS,
	VENUE_ACCOUNTS,
	buildDesktopState,
	buildMobileState,
	webVenue
} from './fixtures';
import { VENUE_BLOCK_KEYS } from './messages';
import { integrityLineModel, signingPagesModel, venueBlockText, venueModel } from './venue';

const en = resolveSettingsMessages('en');
const zh = resolveSettingsMessages('zh');
const IDENTICON = (seed: string) => `<svg data-seed="${seed}"></svg>`;

/** Run the signing-pages machine over a store holding `pages`, and return its view. */
function signingPagesView(pages: unknown): SigningPagesView {
	const core = new SigningPagesCore();
	try {
		const started = JSON.parse(core.dispatch(JSON.stringify({ type: 'refresh' }))) as {
			effects: { id: number; operation: { type: string } }[];
		};
		const read = started.effects.find((effect) => effect.operation.type === 'read_stored');
		if (read === undefined) throw new Error('the machine did not ask for the stored pages');
		core.resolve_effect(
			BigInt(read.id),
			JSON.stringify({ type: 'stored', pages_json: JSON.stringify(pages), legacy_url: null })
		);
		return JSON.parse(core.view()) as SigningPagesView;
	} finally {
		core.free();
	}
}

describe('the boards are screens the core can produce', () => {
	it('each drawn account’s venue rows are the core’s own answer', () => {
		for (const [which, account] of Object.entries(VENUE_ACCOUNTS)) {
			expect(signingVenueChoices(account.domain, account.venue, SAVED_PAGES), which).toEqual(
				account.choices
			);
			// The web's: the apps' rows with no saved pages of its own.
			expect(signingVenueChoices(account.domain, account.venue, []), `${which} on the web`).toEqual(
				account.web.choices
			);
		}
	});

	it('each drawn account’s plan on the web is the apps’ plan', () => {
		for (const [which, account] of Object.entries(VENUE_ACCOUNTS)) {
			const plan = signingPlan(storedAccount(account.domain, account.venue));
			expect(plan, which).not.toBeNull();
			expect(plan?.venue, which).toEqual(account.venue);
			expect(plan?.blocked ?? null, which).toBeNull();
		}
	});

	it('the boards’ check time is the core’s, in every language (D-13)', () => {
		// A check from today reads as its clock time — the literal the browser-built
		// fixtures draw is exactly what the core says, everywhere.
		for (const locale of SUPPORTED_LOCALES) {
			expect(boardCheckTime(locale), locale).toBe(BOARD_CHECK_TIME);
		}
		expect(BOARD_CHECK.now - BOARD_CHECK.at).toBe(30 * 60 * 1000);
	});

	it('the pages list is what the signing-pages machine shows for the saved pages', () => {
		const view = signingPagesView(SAVED_PAGES);
		expect(view.loaded).toBe(true);
		expect(view.pages).toEqual(SIGNING_PAGE_ROWS);
		expect(view.saved).toEqual(SAVED_PAGES);
		// The official page is never stored, and always first.
		expect(view.pages[0]).toMatchObject({ url: OFFICIAL_PAGE, official: true });
	});

	it('the drawn pages live where the core says', () => {
		expect(signingPageDomain(OFFICIAL_PAGE)).toBe('getvela.app');
		expect(signingPageDomain(OWN_PAGE)).toBe('sign.example.com');
	});
});

describe('How you sign', () => {
	it('a getvela.app account: Vela’s sheet, then the pages under one heading', () => {
		const model = venueModel({ domain: 'getvela.app', choices: VENUE_ACCOUNTS.app.choices }, en);
		expect(model.rows.map((row) => row.id)).toEqual(['in_vela', OFFICIAL_PAGE, OWN_PAGE, NEW_PAGE]);
		expect(model.rows.map((row) => row.active)).toEqual([false, true, false, false]);
		// The page on another domain stays listed, disabled, with BOTH domains.
		expect(model.rows[2].blocked).toBe(
			"This page is on sign.example.com, but this account's passkeys sign only for getvela.app."
		);
		// D6 / D-19: the official page is NAMED "Vela's official signing page".
		expect(model.rows[1].page).toEqual({
			name: "Vela's official signing page",
			host: 'sign.getvela.app',
			official: true,
			hostShown: true
		});
		// A page the person named keeps its name; an unnamed one is "Self-hosted · domain".
		expect(model.rows[2].page).toMatchObject({ name: 'Home server', hostShown: true });
		expect(model.rows[3].page).toMatchObject({
			name: 'Self-hosted\u00a0· sign.example.org',
			hostShown: false
		});
		expect(model.value).toBe(en.venue.page);
		expect(model.note).toBe('sign.getvela.app');
		expect(model.summary).toBe('Sign on a trusted signing page · sign.getvela.app');
		expect(model.domainLine).toBe(
			'Your passkeys stay on your devices and sign only for getvela.app'
		);
		expect(model.readOnly).toBeUndefined();
	});

	it('an account on its own domain is locked to its page; Vela’s sheet says why not', () => {
		const model = venueModel(
			{ domain: 'sign.example.com', choices: VENUE_ACCOUNTS.own.choices },
			en
		);
		expect(model.rows[0].blocked).toBe(
			"This account's passkeys sign only for sign.example.com, so Vela can't use them here."
		);
		expect(model.rows[1].blocked).toContain('getvela.app');
		expect(model.rows[2]).toMatchObject({ active: true, blocked: undefined });
		expect(model.note).toBe('Home server · sign.example.com');
	});

	const BLOCKS = {
		app: { type: 'app_cannot_reach', domain: 'x.example' },
		page: { type: 'page_on_other_domain', page_domain: 'p.example', domain: 'getvela.app' },
		web: { type: 'not_on_web' }
	} satisfies Record<string, VenueBlock>;

	it('a refusal’s sentence is the core’s: its line and the values that fill it', () => {
		// `VenueBlock::key()` + `vars()` — which fact fills which placeholder
		// is the core's, not a switch here.
		expect(venueBlockLine(BLOCKS.app)).toEqual({
			key: 'settings.venue.blockedApp',
			vars: { domain: 'x.example' }
		});
		expect(venueBlockLine(BLOCKS.page)).toEqual({
			key: 'settings.venue.blockedPage',
			vars: { pageDomain: 'p.example', domain: 'getvela.app' }
		});
		expect(venueBlockLine(BLOCKS.web)).toEqual({ key: 'settings.venue.blockedWeb', vars: {} });
		// Every line the core can name is one the manifests resolve.
		expect(
			Object.values(BLOCKS)
				.map((block) => venueBlockLine(block)?.key)
				.sort()
		).toEqual([...VENUE_BLOCK_KEYS].sort());
	});

	it('every reason is said in every locale, with its domains filled in', () => {
		for (const locale of SUPPORTED_LOCALES) {
			const m = resolveSettingsMessages(locale);
			const app = venueBlockText(BLOCKS.app, m.venue.blocked);
			const page = venueBlockText(BLOCKS.page, m.venue.blocked);
			const web = venueBlockText(BLOCKS.web, m.venue.blocked);
			expect(app, locale).toContain('x.example');
			expect(page, locale).toContain('p.example');
			expect(page, locale).toContain('getvela.app');
			expect(web, locale).toBe(m.venue.blocked['settings.venue.blockedWeb']);
			expect(web.length, locale).toBeGreaterThan(0);
			expect(`${app}${page}${web}`, locale).not.toContain('{{');
		}
		expect(venueBlockText(BLOCKS.web, en.venue.blocked)).toBe(
			'Not available on the web. Open it from the Vela app.'
		);
		expect(venueBlockText(BLOCKS.web, zh.venue.blocked)).toBe(
			'网页版不能用，请在 Vela 应用里打开。'
		);
		expect(venueBlockText(BLOCKS.page, en.venue.blocked)).toBe(
			"This page is on p.example, but this account's passkeys sign only for getvela.app."
		);
	});
});

/** A stored record on `domain` reviewing at `venue` — what the web plan is asked of. */
function storedAccount(domain: string, venue: unknown) {
	return {
		id: 'aa01',
		name: 'Ann',
		address: '0x2222222222222222222222222222222222222222',
		public_key_hex: '04' + '11'.repeat(64),
		created_at_iso: '2026-10-09T00:00:00.000Z',
		keys: [
			{ credential_id: 'aa01', public_key_hex: '04' + '11'.repeat(64), name: 'Mac', transports: '' }
		],
		sign_in_key: { credential_id: 'aa01', method: 'platform', transports: 'internal' },
		signing_domain: domain,
		signing_venue: venue
	};
}

/** 2026-10-30: the page, everywhere — the web and the extension offer a real choice. */
describe('the web’s venue', () => {
	it('a getvela.app account reviewing on the official page: the page is chosen, Vela offered', () => {
		const model = webVenue(en, 'app');
		expect(model.readOnly).toBeUndefined();
		expect(model.rows.map((row) => row.id)).toEqual(['in_vela', OFFICIAL_PAGE]);
		expect(model.rows[0]).toMatchObject({ active: false, blocked: undefined });
		expect(model.rows[1]).toMatchObject({ active: true, blocked: undefined });
		expect(model.value).toBe(en.venue.page);
		expect(model.summary).toBe('Sign on a trusted signing page · sign.getvela.app');
	});

	it('says what a page protects against, and what it does not — in every language', () => {
		const model = webVenue(en, 'app');
		expect(model.pageNotes).toEqual([
			"A signing page reads the request itself and signs exactly what it shows you, so a tampered Vela screen can't trick you into signing something else.",
			"It can't protect you if this wallet itself is compromised: your passkeys sign for getvela.app, and code running there could still ask them for a signature."
		]);
		for (const locale of SUPPORTED_LOCALES) {
			const notes = webVenue(resolveSettingsMessages(locale), 'own').pageNotes ?? [];
			expect(notes.length, locale).toBe(2);
			expect(notes[1], locale).toContain('sign.example.com');
			expect(notes.join(''), locale).not.toContain('{{');
		}
		// The apps' boards say nothing of it (they carry their own copy).
		expect(
			venueModel({ domain: 'getvela.app', choices: VENUE_ACCOUNTS.app.choices }, en).pageNotes
		).toBeUndefined();
	});

	it('a custom-domain account: its own page, the only place its keys answer, is chosen', () => {
		const model = webVenue(en, 'own');
		expect(model.rows.map((row) => row.id)).toEqual(['in_vela', OFFICIAL_PAGE, OWN_PAGE]);
		expect(model.rows[0].blocked).toBe(
			venueBlockText(signingVenueBlock('sign.example.com', { type: 'in_vela' })!, en.venue.blocked)
		);
		expect(model.rows[1].blocked).toContain('getvela.app');
		expect(model.rows[2]).toMatchObject({
			active: true,
			blocked: undefined,
			page: { name: 'Self-hosted\u00a0· sign.example.com', hostShown: false }
		});
		expect(model.value).toBe(en.venue.page);
	});

	it('the live route’s reading is the fixture’s: the core asked with no saved pages', () => {
		const model = venueModel(
			{
				domain: 'getvela.app',
				choices: signingVenueChoices('getvela.app', { type: 'page', url: OFFICIAL_PAGE }, []),
				honest: true
			},
			en
		);
		expect(model).toEqual(webVenue(en, 'app'));
	});
});

describe('Settings → Signing pages', () => {
	it('says which keys each page can reach, and why an address was not added', () => {
		const model = signingPagesModel(
			{ pages: SIGNING_PAGE_ROWS, addError: 'duplicate', draft: OWN_PAGE },
			en
		);
		// Each page is named once: no host the name already says, and "Keys on"
		// only where the keys are not the page's own host's (the official page
		// on sign.getvela.app signs with getvela.app's keys).
		expect(model.rows.map((row) => [row.name, row.host, row.keysOn])).toEqual([
			[
				"Vela's official signing page",
				'sign.getvela.app',
				'Your passkeys stay on your devices and sign only for getvela.app'
			],
			['Home server', 'sign.example.com', undefined],
			['Self-hosted\u00a0· sign.example.org', undefined, undefined]
		]);
		// The signing pages' own words (D6): no borrowed "Rename" / "Remove".
		expect([model.renameLabel, model.removeLabel]).toEqual([
			en.signing.pageRename,
			en.signing.pageRemove
		]);
		expect(model.add).toMatchObject({ hint: en.signing.pageDuplicate, tone: 'error' });
		for (const error of ['invalid', 'insecure'] as const) {
			expect(signingPagesModel({ pages: [], addError: error }, en).add.hint).toBe(
				error === 'invalid' ? en.signing.pageInvalid : en.signing.pageInsecure
			);
		}
		expect(signingPagesModel({ pages: [] }, en).add.hint).toBeUndefined();
	});

	it('the integrity line is the core’s key in words — and never says "certified"', () => {
		const states = [
			['checking', 'checking'],
			['matches', 'ok'],
			['trusted', 'ok'],
			['unchecked', 'warn'],
			['mismatch', 'error'],
			['blocked', 'error'],
			['askTrust', 'warn'],
			['couldNotCheck', 'error'],
			['noVersion', 'error'],
			['allBlocked', 'error']
		] as const;
		for (const locale of SUPPORTED_LOCALES) {
			const m = resolveSettingsMessages(locale);
			for (const [state, tone] of states) {
				const line = integrityLineModel(
					{ key: `componentsUi.signing.integrity.${state}`, version: '0ba8ee8c' },
					'14:32',
					m
				);
				expect(line?.tone, `${locale} ${state}`).toBe(tone);
				expect(line?.text, `${locale} ${state}`).not.toContain('{{');
			}
		}
		const matches = integrityLineModel(
			{ key: 'componentsUi.signing.integrity.matches', version: '0ba8ee8c' },
			'14:32',
			en
		);
		expect(matches?.text).toBe(
			"Version 0ba8ee8c\u00a0· matches Vela's published build list\u00a0· checked 14:32"
		);
		expect(matches?.text).not.toMatch(/certif|untamper/i);
		expect(integrityLineModel({ key: 'componentsUi.signing.nope', version: '' }, '', en)).toBe(
			undefined
		);
	});
});

describe('the boards', () => {
	it('ST17 / ST17b / ST18 / ST18b / DST9 are the pages they say', () => {
		expect(buildMobileState('st17', zh, IDENTICON).page).toBe('signing-venue');
		expect(buildMobileState('st17b', zh, IDENTICON).venue?.rows[2].active).toBe(true);
		expect(buildMobileState('st18', zh, IDENTICON).page).toBe('signing-pages');
		expect(buildMobileState('st18b', zh, IDENTICON).signingPages?.add.tone).toBe('error');
		expect(buildDesktopState('dst9', zh, IDENTICON).page).toBe('signing-pages');
		// The desktop account panel carries the venue under the keys.
		expect(buildDesktopState('dst1', zh, IDENTICON).account.venue?.rows).toHaveLength(4);
	});

	it('the official page is checked against the published list; a self-hosted one is trusted here or asks', () => {
		const rows = buildMobileState('st18', en, IDENTICON).signingPages!.rows;
		expect(rows.map((row) => row.integrity?.tone)).toEqual(['ok', 'ok', 'warn']);
		expect(rows[0].integrity?.text).toContain('published build list');
		expect(rows[1].integrity?.text).toContain('trusted on this device');
		// The time on every line is the core's `checked_time` (D-13).
		expect(rows[0].integrity?.text).toContain('14:32');
		// An unknown version of a self-hosted page asks — and is answered in place (D-15).
		expect(rows.map((row) => row.trust)).toEqual([
			undefined,
			undefined,
			{ label: en.signing.pageTrust, version: NEW_PAGE_VERSION }
		]);
		// ST18b: a page whose bytes are not the version named will not open.
		const refused = buildMobileState('st18b', en, IDENTICON).signingPages!.rows;
		expect(refused[1].integrity?.tone).toBe('error');
	});

	it('the official page is never asked to be trusted, whatever the shell passes', () => {
		const model = signingPagesModel(
			{ pages: SIGNING_PAGE_ROWS, askTrust: { [OFFICIAL_PAGE]: NEW_PAGE_VERSION } },
			en
		);
		expect(model.rows[0].trust).toBeUndefined();
	});
});
