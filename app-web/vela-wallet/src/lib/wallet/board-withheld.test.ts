/**
 * The boards' "currency on its way" twins (0.8) — the drawn states with every
 * fiat figure withheld, so the frame before the display currency commits can
 * be laid beside the frame after it.
 *
 * The twin is made by a transform over the fixture models (`withheldBoard`),
 * and what it must be is what the live builders make: no fiat figure in any
 * money, the pending mark where each stood, the same lines as the drawn
 * board — and the hero its skeleton.
 */
import { describe, expect, it } from 'vitest';
import {
	resolveSettingsMessages,
	resolveSigningMessages,
	resolveWalletFlowMessages,
	resolveWalletMessages
} from '$lib/i18n/engine.server';
import { buildDesktopFlowState, buildFlowState } from '$lib/flows/fixtures';
import { buildMobileState as buildSettingsMobileState } from '$lib/settings/fixtures';
import { buildSigningState } from '$lib/signing/fixtures';
import { withheldBoard, withheldModel } from './board-withheld';
import { buildDesktopState, buildMobileState } from './fixtures';
import { MONEY_PENDING } from './live';
import { FIAT_FIGURE, shapeOf } from './testing/fiat-withheld';

const identicon = (seed: string) => `<svg data-seed="${seed}"></svg>`;

for (const locale of ['en', 'zh'] as const) {
	const m = resolveWalletMessages(locale);
	const fm = resolveWalletFlowMessages(locale);
	const sm = resolveSettingsMessages(locale);
	const gm = resolveSigningMessages(locale);

	describe(`the boards with the currency on its way (${locale})`, () => {
		/** The four the round asked for, and the screens beside them. */
		const boards: [string, unknown][] = [
			['sd2 · the send form', buildFlowState('sd2', fm, identicon)],
			['sd3 · the review', buildFlowState('sd3', fm, identicon)],
			['sd1 · the coin list', buildFlowState('sd1', fm, identicon)],
			['t2 · the token page', buildFlowState('t2', fm, identicon)],
			['t1 · assets', buildFlowState('t1', fm, identicon)],
			['a2 · a transfer’s detail', buildFlowState('a2', fm, identicon)],
			['dsd2 · the send form, wide', buildDesktopFlowState('dsd2', fm, identicon)],
			['sr3 · the balance detail sheet', buildSettingsMobileState('sr3', sm, identicon)],
			['st2 · the account switcher', buildSettingsMobileState('st2', sm, identicon)],
			['cs1 · the signing sheet', buildSigningState('cs1', gm, identicon)],
			['cs12 · the signing sheet, a swap', buildSigningState('cs12', gm, identicon)]
		];

		it.each(boards)('%s: no figure, the pending mark, the same lines', (_name, drawn) => {
			// Not vacuous: the board as drawn does carry a fiat figure.
			expect(JSON.stringify(drawn)).toMatch(FIAT_FIGURE);
			const twin = withheldModel(drawn);
			const text = JSON.stringify(twin);
			expect(text).not.toMatch(FIAT_FIGURE);
			expect(text).toContain(MONEY_PENDING);
			// The same lines: a fee row is told its money is withheld, and
			// nothing else is added or dropped.
			const lines = (value: unknown) =>
				JSON.stringify(shapeOf(value), (key, entry: unknown) =>
					key === 'valueFiatWithheld' ? undefined : entry
				);
			expect(lines(twin)).toBe(lines(drawn));
			// The drawn board itself is untouched.
			expect(JSON.stringify(drawn)).toMatch(FIAT_FIGURE);
		});

		it('the home: the hero is its skeleton with the currency still named; holdings wait', () => {
			for (const drawn of [
				buildMobileState('h1', m, identicon),
				buildDesktopState('d1', m, identicon)
			]) {
				expect(drawn.balance.state).toBe('normal');
				const twin = withheldModel(drawn);
				expect(twin.balance).toMatchObject({ state: 'loading', label: drawn.balance.label });
				expect(twin.balance.currency).toBe(drawn.balance.currency);
				expect(twin.balance.integer).toBeUndefined();
				expect(twin.balance.decimals).toBeUndefined();
				expect(JSON.stringify(twin)).not.toMatch(FIAT_FIGURE);
				// A token amount is not fiat: the rows' balances are as drawn.
				expect(twin.assetRows.map((row) => row.balance)).toEqual(
					drawn.assetRows.map((row) => row.balance)
				);
				expect(twin.assetRows.every((row) => JSON.stringify(row.fiat).includes('…'))).toBe(true);
			}
		});

		it('the send form’s fee row is told, so it holds the figure’s room as the live one does', () => {
			const drawn = buildFlowState('sd2', fm, identicon);
			const twin = withheldModel(drawn);
			if (twin.base.kind !== 'send-form' || drawn.base.kind !== 'send-form') throw new Error('sd2');
			expect(drawn.base.model.fee.valueFiat).toMatch(FIAT_FIGURE);
			expect(twin.base.model.fee.valueFiat).toBe(
				drawn.base.model.fee.valueFiat?.replace(/[$¥]\s?[\d.,]+/, MONEY_PENDING)
			);
			expect(twin.base.model.fee.valueFiatWithheld).toBe(true);
			// The coin half is a token amount.
			expect(twin.base.model.fee.value).toBe(drawn.base.model.fee.value);
		});

		it('a page’s data: the drawn models are withheld, the words beside them are not', () => {
			const data = {
				kind: 'flow-mobile' as const,
				model: buildFlowState('sd2', fm, identicon),
				copy: { example: 'A fee of about $0.70' }
			};
			const twin = withheldBoard(data);
			expect(JSON.stringify(twin.model)).not.toMatch(FIAT_FIGURE);
			expect(twin.copy).toEqual(data.copy);
			expect(twin.kind).toBe('flow-mobile');
		});

		it('artwork is not copy: an identicon’s markup passes through untouched', () => {
			const art = '<svg viewBox="0 0 8 8"><text>$5</text></svg>';
			expect(withheldModel({ identiconSvg: art, fiat: '≈ $5.00' })).toEqual({
				identiconSvg: art,
				fiat: `≈ ${MONEY_PENDING}`
			});
		});
	});
}
