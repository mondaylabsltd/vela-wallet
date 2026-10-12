/**
 * The simulation verdict's boards (SV1–SV8). What must hold, in every locale:
 * each board is its CS board's own request with the verdict's place after it
 * and nothing else changed; what stands in the place is what the real core
 * gave for the board's reply, built by the live sheet's own builder; and an
 * unverified token's figure — the site's number — is nowhere on any board.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import { resolveSigningMessages } from '$lib/i18n/engine.server';
import { SUPPORTED_LOCALES, type Locale } from '$lib/i18n/locales';
import { fill } from '$lib/wallet/messages';
import { buildSigningState } from './fixtures';
import type { Block, SigningModel } from './model';
import {
	buildVerdictState,
	VERDICT_STATES,
	verdictBoardOver,
	type VerdictStateId
} from './verdict-boards';
import { verdictRooms } from './verdict';

const identicon = (seed: string) => `<svg data-seed="${seed}"></svg>`;
const en = resolveSigningMessages('en');

function board(state: VerdictStateId, locale: Locale = 'en') {
	const m = resolveSigningMessages(locale);
	const base = buildSigningState(verdictBoardOver(state), m, identicon);
	return { m, base, model: buildVerdictState(state, base, m) };
}
type Place = Extract<Block, { kind: 'verdict' }>;
const placeOf = (model: SigningModel) => model.blocks.at(-1) as Place;

describe('the verdict boards', () => {
	it('each is its CS board’s request with the place after it — in every locale, with no template left', () => {
		for (const locale of SUPPORTED_LOCALES) {
			for (const state of VERDICT_STATES) {
				const { m, base, model } = board(state, locale);
				const place = placeOf(model);
				expect(place.kind, `${locale} ${state}`).toBe('verdict');
				// The request above it is the CS board's, less any card a mock drew
				// of a simulation; the place is the one voice.
				expect(model.blocks.slice(0, -1), `${locale} ${state}`).toEqual(
					base.blocks.filter((block) => block.kind !== 'balances')
				);
				expect(model.blocks.filter((block) => block.kind === 'balances')).toEqual([]);
				expect(model.tech.simResult).toBeUndefined();
				// The rooms the live sheet keeps, and one block in the place.
				expect(place.rooms, `${locale} ${state}`).toEqual(verdictRooms(m));
				expect(place.inner, `${locale} ${state}`).toHaveLength(1);
				expect(JSON.stringify(place.inner), `${locale} ${state}`).not.toContain('{{');
				expect(JSON.stringify(place.inner), `${locale} ${state}`).not.toContain('componentsUi.');
				// Everything else is the CS board's.
				expect(model.dapp).toEqual(base.dapp);
				expect(model.network).toEqual(base.network);
				expect(model.fee).toEqual(base.fee);
				expect(model.signer).toEqual(base.signer);
				expect(model.confirm.action).toBe(base.confirm.action);
			}
		}
	});

	it('SV1: the simulation is out — "Checking…" in the place, the confirm held with the core’s line', () => {
		const { model } = board('sv1');
		expect(placeOf(model)).toMatchObject({
			inner: [{ kind: 'balances', title: 'Balance changes', rows: [], note: 'Checking…' }],
			landed: false
		});
		expect(model.confirm).toEqual({
			action: en.confirmSend,
			enabled: false,
			note: 'Checking what this transaction does…'
		});
		const zh = board('sv1', 'zh').model;
		expect(placeOf(zh).inner[0]).toMatchObject({ title: '余额变化', note: '正在检查…' });
		expect(zh.confirm.note).toBe(
			resolveSigningMessages('zh').confirmBlock['componentsUi.signing.confirmBlock.simChecking']
		);
	});

	it('every other board has landed, and its confirm is open beside its verdict', () => {
		for (const state of VERDICT_STATES.filter((state) => state !== 'sv1')) {
			const { model } = board(state);
			expect(placeOf(model).landed, state).toBe(true);
			expect(model.confirm, state).toEqual({ action: model.confirm.action, enabled: true });
		}
	});

	it('SV2 a send, SV3 a swap: the rows the real core judged, with its figures', () => {
		expect(placeOf(board('sv2').model).inner).toEqual([
			{
				kind: 'balances',
				title: 'Balance changes',
				rows: [{ symbol: 'USDC', delta: '−1,000.00', tone: 'neutral' }]
			}
		]);
		expect(placeOf(board('sv3').model).inner).toEqual([
			{
				kind: 'balances',
				title: 'Balance changes',
				rows: [
					{ symbol: 'USDC', delta: '−1,000.00', tone: 'neutral' },
					// The chain's wrapped coin: an arrival the wallet vouches for.
					{ symbol: 'WETH', delta: '+0.3051', tone: 'success' }
				]
			}
		]);
	});

	it('SV4, SV5: an unverified token is a direction with one warning — and its figure is on no board', () => {
		expect(placeOf(board('sv4').model).inner).toEqual([
			{
				kind: 'balances',
				title: 'Balance changes',
				rows: [
					{ symbol: 'ETH', delta: '−10', tone: 'neutral' },
					{ symbol: 'Unverified token', delta: '+', tone: 'caution' }
				],
				note: en.warnUnverifiedAmount,
				noteTone: 'caution'
			}
		]);
		const tall = placeOf(board('sv5').model).inner[0];
		expect(tall).toMatchObject({
			rows: [
				{ symbol: 'USDC', delta: '−1,000.00', tone: 'neutral' },
				{ symbol: 'WETH', delta: '+0.3051', tone: 'success' },
				{ symbol: 'Unverified token', delta: '+', tone: 'caution' },
				{ symbol: 'DAI', delta: '−18.5', tone: 'neutral' }
			],
			note: en.warnUnverifiedAmount,
			noteTone: 'caution'
		});
		// The site emitted 5,000 of a token that calls itself USDC. Neither the
		// number nor a fifth "USDC" is anywhere on any board, in any language.
		for (const locale of SUPPORTED_LOCALES) {
			for (const state of ['sv4', 'sv5'] as const) {
				const said = JSON.stringify(placeOf(board(state, locale).model).inner);
				expect(said, `${locale} ${state}`).not.toMatch(/5[,.\s]?000/);
				expect(said.split('USDC').length - 1, `${locale} ${state}`).toBe(state === 'sv5' ? 1 : 0);
			}
		}
		expect(placeOf(board('sv4', 'zh').model).inner[0]).toMatchObject({
			rows: [
				{ symbol: 'ETH', delta: '−10' },
				{ symbol: '未验证代币', delta: '+' }
			]
		});
	});

	it('SV6 nothing moves, SV7 the call fails with its reason, SV8 nothing could be checked', () => {
		expect(placeOf(board('sv6').model).inner).toEqual([
			{ kind: 'balances', title: 'Balance changes', rows: [], note: 'No asset changes' }
		]);
		expect(placeOf(board('sv7').model).inner).toEqual([
			{
				kind: 'warning',
				tone: 'danger',
				text: fill(en.warnWillFailReason, { reason: 'ERC20: transfer amount exceeds balance' })
			}
		]);
		expect(placeOf(board('sv8').model).inner).toEqual([
			{ kind: 'warning', tone: 'caution', text: en.warnSimUnavailable }
		]);
		expect(placeOf(board('sv6', 'zh').model).inner[0]).toMatchObject({ note: '无资产变动' });
	});
});
