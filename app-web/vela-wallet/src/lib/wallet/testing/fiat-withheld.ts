/**
 * TESTS ONLY — never imported by product code.
 *
 * The core's withhold rule (`display_currency.rs`, module doc): while
 * `CurrencyView.committed` is false the view is the USD/1 placeholder, and NO
 * fiat figure is drawn on ANY surface — and withholding never moves the
 * layout. `expectWithheld` is the one assertion every surface's test makes,
 * so the surfaces cannot drift apart in what "withheld" means:
 *
 * 1. under both views the display-currency machine can emit before a commit,
 *    the surface's model carries no fiat figure in any money;
 * 2. committed, it does (so the check above is not vacuous) — unless the
 *    surface draws no fiat on the web at all, which the caller says;
 * 3. the two models have the same SHAPE: every line present after the commit
 *    was already there before it. A line that exists only once the figure
 *    lands is a row that grows when it arrives — the jump the rule ends.
 *
 * `fiat-withheld.test.ts` holds the list of surfaces to the core's own
 * (`FIAT_SURFACES`) and checks that every one of them calls this somewhere.
 */
import { expect } from 'vitest';
import type { CurrencyView } from '$lib/core/generated/CurrencyView';
import { MONEY_PENDING } from '../live';

/**
 * The twelve surfaces the core names (`display_currency::FIAT_SURFACES`),
 * pinned here so a call that misspells one does not compile; the test reads
 * the Rust source and fails if the two lists differ.
 */
export const FIAT_SURFACES = [
	'home_total',
	'holdings',
	'account_switcher',
	'token_detail',
	'assets',
	'balance_detail',
	'activity_row',
	'activity_detail',
	'send_coin_list',
	'send_form',
	'signing_sheet',
	'settings_total'
] as const;

export type FiatSurface = (typeof FIAT_SURFACES)[number];

/** The machine's first view: nothing read yet. */
export const NOT_READ: CurrencyView = { code: 'USD', rate: 1, committed: false, pending: null };
/** A stored choice whose rate is still being fetched. */
export const ON_ITS_WAY: CurrencyView = { code: 'USD', rate: 1, committed: false, pending: 'CNY' };
/** The pair landed. */
export const COMMITTED: CurrencyView = { code: 'CNY', rate: 7.2, committed: true, pending: null };

/**
 * A fiat figure in ANY money: a currency's glyph, or its code where the
 * catalog has no glyph, and then a digit. "≈ …" is not one; neither is
 * "418.25 USDT" (a token amount is not in the display currency).
 */
export const FIAT_FIGURE = /(?:[$¥€£₫₩₹₽₺₴₱฿]|\b(?:USD|CNY|EUR|JPY|VND|KRW|HKD|GBP|IDR)\b)\s?\d/;

/**
 * A model's shape: its keys and lengths, with every value reduced to whether
 * anything is drawn there. Two models of one shape have the same lines.
 */
export function shapeOf(value: unknown): unknown {
	if (value === null || value === undefined) return null;
	if (typeof value === 'string') return value === '' ? 'empty' : 'text';
	if (typeof value !== 'object') return typeof value;
	if (Array.isArray(value)) return value.map(shapeOf);
	return Object.fromEntries(
		Object.entries(value as Record<string, unknown>)
			.filter(([, v]) => v !== undefined)
			.sort(([a], [b]) => a.localeCompare(b))
			.map(([k, v]) => [k, shapeOf(v)])
	);
}

export interface WithheldOptions {
	/**
	 * Does this surface draw a fiat figure on the web once committed? `false`
	 * for one that draws none in either state (the web's activity rows).
	 */
	figure?: boolean;
	/**
	 * Is the withheld stand-in the pending mark in the model itself? `false`
	 * for the hero, whose stand-in is its skeleton (`state: 'loading'`) — its
	 * room is the component's to keep, and `BalanceDisplay.svelte.test.ts`
	 * measures that.
	 */
	sameShape?: boolean;
}

/** What `build` draws for `surface`, withheld and committed, held to the rule. */
export function expectWithheld(
	surface: FiatSurface,
	build: (currency: CurrencyView) => unknown,
	{ figure = true, sameShape = true }: WithheldOptions = {}
): void {
	const landed = build(COMMITTED);
	const landedText = JSON.stringify(landed);
	if (figure) {
		expect(landedText, `${surface}: committed, a figure is drawn`).toMatch(/¥\s?\d/);
	} else {
		expect(landedText, `${surface}: no fiat on this surface`).not.toMatch(FIAT_FIGURE);
	}
	for (const [name, view] of [
		['nothing read yet', NOT_READ],
		['a stored choice on its way', ON_ITS_WAY]
	] as const) {
		const withheld = build(view);
		const text = JSON.stringify(withheld);
		expect(text, `${surface}, ${name}: no fiat figure`).not.toMatch(FIAT_FIGURE);
		if (!sameShape) continue;
		if (figure) expect(text, `${surface}, ${name}: the pending mark`).toContain(MONEY_PENDING);
		expect(shapeOf(withheld), `${surface}, ${name}: the same lines as once committed`).toEqual(
			shapeOf(landed)
		);
	}
}
