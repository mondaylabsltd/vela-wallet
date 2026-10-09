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
 * 2. The web's own reading (P2-11): it opens no page, so its one venue is
 *    Vela's sheet — shown as where signing happens for a `getvela.app`
 *    account whatever its stored venue, and disabled with R1's reason for an
 *    account on its own domain. The reason is the core's, asked of `in_vela`.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import { SigningPagesCore } from '../../../../../rust/pkg-web/vela_core.js';
import { signingPageDomain, signingVenueBlock, signingVenueChoices } from '$lib/core/kernels';
import { resolveSettingsMessages } from '$lib/i18n/engine.server';
import { SUPPORTED_LOCALES } from '$lib/i18n/locales';
import type { SigningPagesView } from '$lib/core/generated/SigningPagesView';
import {
	OFFICIAL_PAGE,
	OWN_PAGE,
	SAVED_PAGES,
	SIGNING_PAGE_ROWS,
	VENUE_ACCOUNTS,
	buildDesktopState,
	buildMobileState
} from './fixtures';
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
		}
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

describe('Where you review and sign', () => {
	it('a getvela.app account: Vela’s sheet, then the pages under one heading', () => {
		const model = venueModel({ domain: 'getvela.app', choices: VENUE_ACCOUNTS.app.choices }, en);
		expect(model.rows.map((row) => row.id)).toEqual(['in_vela', OFFICIAL_PAGE, OWN_PAGE]);
		expect(model.rows.map((row) => row.active)).toEqual([false, true, false]);
		// The page on another domain stays listed, disabled, with BOTH domains.
		expect(model.rows[2].blocked).toBe(
			"This page is on sign.example.com; this account's keys are on getvela.app."
		);
		expect(model.rows[1].page).toEqual({
			name: en.signing.pageOfficial,
			host: 'sign.getvela.app',
			official: true
		});
		expect(model.value).toBe(en.venue.page);
		expect(model.note).toBe('sign.getvela.app');
		expect(model.summary).toBe('On a trusted page · sign.getvela.app');
		expect(model.domainLine).toBe('Keys on getvela.app');
		expect(model.readOnly).toBeUndefined();
	});

	it('an account on its own domain is locked to its page; Vela’s sheet says why not', () => {
		const model = venueModel(
			{ domain: 'sign.example.com', choices: VENUE_ACCOUNTS.own.choices },
			en
		);
		expect(model.rows[0].blocked).toBe("Vela can't reach keys on sign.example.com.");
		expect(model.rows[1].blocked).toContain('getvela.app');
		expect(model.rows[2]).toMatchObject({ active: true, blocked: undefined });
		expect(model.note).toBe('My page · sign.example.com');
	});

	it('every reason is said in every locale, with its domains filled in', () => {
		for (const locale of SUPPORTED_LOCALES) {
			const m = resolveSettingsMessages(locale);
			const app = venueBlockText({ type: 'app_cannot_reach', domain: 'x.example' }, m);
			const page = venueBlockText(
				{ type: 'page_on_other_domain', page_domain: 'p.example', domain: 'getvela.app' },
				m
			);
			expect(app, locale).toContain('x.example');
			expect(page, locale).toContain('p.example');
			expect(page, locale).toContain('getvela.app');
			expect(`${app}${page}`, locale).not.toContain('{{');
		}
	});
});

/** P2-11: the web opens no page. */
describe('the web’s venue', () => {
	it('a getvela.app account reviewing on the trusted page signs in Vela HERE — said so', () => {
		const model = venueModel(
			{
				domain: 'getvela.app',
				choices: signingVenueChoices('getvela.app', { type: 'page', url: OFFICIAL_PAGE }, []),
				webOnly: true
			},
			en
		);
		expect(model.readOnly).toBe(true);
		expect(model.rows).toHaveLength(1);
		expect(model.rows[0]).toMatchObject({ id: 'in_vela', active: true, blocked: undefined });
		expect(model.value).toBe(en.venue.inVela);
		expect(model.note).toBeUndefined();
		expect(model.summary).toBe('In Vela');
	});

	it('a custom-domain account: nothing here can sign, and the reason is the core’s', () => {
		const block = signingVenueBlock('sign.example.com', { type: 'in_vela' });
		expect(block).toEqual({ type: 'app_cannot_reach', domain: 'sign.example.com' });
		const model = venueModel(
			{
				domain: 'sign.example.com',
				choices: signingVenueChoices('sign.example.com', { type: 'page', url: OWN_PAGE }, []),
				webOnly: true
			},
			en
		);
		expect(model.rows).toHaveLength(1);
		expect(model.rows[0].active).toBe(false);
		expect(model.rows[0].blocked).toBe(venueBlockText(block!, en));
		expect(model.value).toBe('');
		expect(model.note).toBe(venueBlockText(block!, en));
		expect(model.summary).toBe(venueBlockText(block!, en));
	});
});

describe('Settings → Signing pages', () => {
	it('says which keys each page can reach, and why an address was not added', () => {
		const model = signingPagesModel(
			{ pages: SIGNING_PAGE_ROWS, addError: 'duplicate', draft: OWN_PAGE },
			en
		);
		expect(model.rows.map((row) => [row.name, row.host, row.keysOn])).toEqual([
			[en.signing.pageOfficial, 'sign.getvela.app', 'Keys on getvela.app'],
			['My page', 'sign.example.com', 'Keys on sign.example.com']
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
			['checking', 'warn'],
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
			"Version 0ba8ee8c · matches Vela's published build list · checked 14:32"
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
		expect(buildDesktopState('dst1', zh, IDENTICON).account.venue?.rows).toHaveLength(3);
	});

	it('the official page is checked against the published list; the person’s own is trusted here', () => {
		const rows = buildMobileState('st18', en, IDENTICON).signingPages!.rows;
		expect(rows.map((row) => row.integrity?.tone)).toEqual(['ok', 'ok']);
		expect(rows[0].integrity?.text).toContain('published build list');
		expect(rows[1].integrity?.text).toContain('trusted on this device');
		// ST18b: a page whose bytes are not the version named will not open.
		const refused = buildMobileState('st18b', en, IDENTICON).signingPages!.rows;
		expect(refused[1].integrity?.tone).toBe('error');
	});
});
