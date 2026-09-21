/**
 * Spec 022 gates for the signing layer.
 *
 * Two of these are product contracts, not style checks: the slide is the only
 * confirmation, and an unlimited approval can never be confirmed as requested.
 * They are asserted here so a later refactor has to break a test to break the
 * promise.
 */
import { describe, expect, it } from 'vitest';
import { rawResolve, resolveSigningMessages } from '$lib/i18n/engine.server';
import { SUPPORTED_LOCALES } from '$lib/i18n/locales';
import { ALL_STATES, buildSigningState } from './fixtures';
import type { Block, SigningModel } from './model';

const IDENTICON_STUB = (seed: string) => `<svg data-seed="${seed}"></svg>`;
const messages = resolveSigningMessages('zh');
const build = (state: (typeof ALL_STATES)[number]) =>
	buildSigningState(state, messages, IDENTICON_STUB);

const blocks = <K extends Block['kind']>(model: SigningModel, kind: K) =>
	model.blocks.filter((b): b is Extract<Block, { kind: K }> => b.kind === kind);

describe('signing messages', () => {
	/** The keys the manifest names, read back through the raw resolver. */
	const KEYS = Object.entries(resolveSigningMessages('en'));

	it.each(SUPPORTED_LOCALES)('no signing string is empty in %s', (locale) => {
		for (const [field] of KEYS) {
			const value = resolveSigningMessages(locale)[field as keyof typeof messages];
			// The speed control's words are a group (spec 069): every leaf of it.
			const leaves =
				typeof value === 'string'
					? [value]
					: [
							value.label,
							value.once,
							value.free,
							value.single,
							value.gasPriceLabel,
							...Object.values(value.names),
							...Object.values(value.hints)
						];
			for (const leaf of leaves) expect(leaf?.trim(), `${field} in ${locale}`).not.toBe('');
		}
	});

	it('the two keys the deepest rungs depend on resolve everywhere', () => {
		for (const locale of SUPPORTED_LOCALES) {
			for (const key of [
				'componentsUi.signing.simUnavailableWarning',
				'componentsUi.signing.drainWarning'
			]) {
				expect(rawResolve(locale, key), `${key} in ${locale}`).not.toBe(key);
			}
		}
	});
});

describe('the catalogue (data-model.md §3)', () => {
	it('is all 35 scenarios, each with its own id', () => {
		// 33 in data-model.md §3, plus cs34/cs35 (032 phase 39, the cap field).
		expect(ALL_STATES).toHaveLength(35);
		expect(new Set(ALL_STATES).size).toBe(35);
	});

	it.each(ALL_STATES)('%s builds, says something, and fills every template', (state) => {
		const model = build(state);
		expect(model.id).toBe(state);
		expect(model.blocks.length).toBeGreaterThan(0);
		expect(model.dapp.name.length).toBeGreaterThan(0);
		expect(model.confirm.action.length).toBeGreaterThan(0);
		// An unfilled `{{var}}` is the failure this whole layer is prone to:
		// it compiles, renders, and reads as gibberish.
		expect(JSON.stringify(model), state).not.toContain('{{');
	});

	it.each(ALL_STATES)('%s opens with an intent and never shows a reject button', (state) => {
		const model = build(state);
		expect(model.blocks[0].kind).toBe('intent');
		expect(JSON.stringify(model)).not.toContain(messages.confirmPlain + '”');
		expect(model.confirm.hint).toBe(messages.slideToConfirm);
	});
});

describe('never-unlimited (spec 022 §4)', () => {
	it('cs5 disables the requested chip AND the slide', () => {
		const model = build('cs5');
		const [editor] = blocks(model, 'allowance');
		expect(editor.chips.find((c) => c.id === 'requested')?.state).toBe('disabled');
		expect(model.confirm.enabled).toBe(false);
	});

	it('choosing a finite cap re-enables the slide (cs6, cs8)', () => {
		for (const state of ['cs6', 'cs8'] as const) {
			const model = build(state);
			expect(model.confirm.enabled, state).toBe(true);
			// The REQUEST was still unlimited, so its chip stays disabled — the
			// person picked one of the finite ones instead.
			const [editor] = blocks(model, 'allowance');
			expect(editor.chips.find((c) => c.id === 'requested')?.state, state).toBe('disabled');
		}
	});

	it('a finite request may be signed as asked (cs7 increaseAllowance)', () => {
		const model = build('cs7');
		const [editor] = blocks(model, 'allowance');
		expect(editor.chips.find((c) => c.id === 'requested')?.state).toBe('selected');
		expect(model.confirm.enabled).toBe(true);
		// An increment only means something next to the total it lands on.
		expect(editor.resultingTotal?.value).toBe('350 USDC');
	});

	it('the two uncappable requests are marked danger instead (cs10 setApprovalForAll, cs16 Permit2)', () => {
		for (const state of ['cs10', 'cs16'] as const) {
			const model = build(state);
			expect(blocks(model, 'intent')[0].tone, state).toBe('danger');
		}
		expect(blocks(build('cs16'), 'warning').some((w) => w.tone === 'danger')).toBe(true);
	});
});

describe('the degradation ladder', () => {
	it('cs23/cs30/cs31 promote the simulation to a block of its own', () => {
		for (const state of ['cs23', 'cs30', 'cs31'] as const) {
			expect(blocks(build(state), 'balances'), state).toHaveLength(1);
		}
	});

	it('cs24 (drain) and cs32 (deepest) both carry a danger warning', () => {
		for (const state of ['cs24', 'cs32'] as const) {
			expect(
				blocks(build(state), 'warning').some((w) => w.tone === 'danger'),
				state
			).toBe(true);
		}
	});

	it('cs32 states both failures and still shows the amount it does know', () => {
		const model = build('cs32');
		expect(blocks(model, 'warning')).toHaveLength(2);
		expect(blocks(model, 'rows')[0].rows[0].value).toContain('0.25 ETH');
	});
});

describe('fee shapes', () => {
	it('on-chain by default, off-chain for the permits, hidden for the message rungs', () => {
		expect(build('cs1').fee.kind).toBe('onchain');
		for (const state of ['cs16', 'cs17', 'cs18', 'cs19'] as const) {
			expect(build(state).fee.kind, state).toBe('offchain');
		}
		for (const state of ['cs20', 'cs21', 'cs22'] as const) {
			expect(build(state).fee.kind, state).toBe('hidden');
		}
	});

	it('cs33 is cs11 with the token selector open', () => {
		const fee = build('cs33').fee;
		expect(fee.kind === 'onchain' && fee.selector?.options).toHaveLength(2);
		expect(build('cs11').fee.kind === 'onchain' && build('cs11').fee).not.toHaveProperty(
			'selector.options'
		);
	});

	it('cs29 is cs1 with the technical panel open', () => {
		expect(build('cs29').techOpen).toBe(true);
		expect(build('cs1').techOpen).toBe(false);
		expect(build('cs29').tech.identities).toHaveLength(2);
	});
});
