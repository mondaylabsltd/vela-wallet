/**
 * cs-12: a member reviewing a multisig's call to another contract sees what
 * the signing sheet would show for that call sent by a site — the decoded
 * intent, what it grants, the guard's unlimited warning — and, for a call
 * nobody can read, the blind warning and the coin it moves. Never only
 * "Contract call".
 *
 * Both machines are the real cores; what they would fetch is answered in
 * `call-review.testing.ts`.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import type { CurrencyView } from '$lib/core/generated/CurrencyView';
import { resolveSigningMessages } from '$lib/i18n/engine.server';
import { INITIAL_CLEAR_VIEW, INITIAL_GUARD_VIEW } from '$lib/signing/core/sheet.svelte';
import { fill } from '$lib/wallet/messages';
import { callParamsJson, callReviewBlocks, callSource, callTitle } from './call-review.svelte';
import { APPROVE_MAX, BLIND, SPENDER, TOKEN, read } from './call-review.testing';

const m = resolveSigningMessages('en');
const CURRENCY: CurrencyView = { code: 'USD', rate: 1, committed: true, pending: null };

const texts = (blocks: ReturnType<typeof callReviewBlocks>) =>
	blocks.flatMap((b) =>
		b.kind === 'intent' || b.kind === 'warning' || b.kind === 'sentence'
			? [b.text]
			: b.kind === 'rows'
				? b.rows.map((r) => `${r.label}: ${r.value}`)
				: b.kind === 'party'
					? [`${b.label}: ${b.address ?? b.name}`]
					: []
	);

describe('a multisig contract call reads as the signing sheet reads it (cs-12)', () => {
	it('sends the call as a site would: the value as a hex quantity', () => {
		expect(
			JSON.parse(callParamsJson({ to: TOKEN, value: '1000000000000000000', data: '0x' }))
		).toEqual([{ to: TOKEN, data: '0x', value: '0xde0b6b3a7640000' }]);
	});

	it('approve(spender, MAX): "Approve", the spender, unlimited in red, the guard’s warning', () => {
		const reading = read(APPROVE_MAX);
		expect(reading.clear.surface).toBe('clear_sign');
		expect(reading.guard.unlimited_warning).toBe(true);
		const blocks = callReviewBlocks(reading, m, CURRENCY);
		const intent = blocks[0];
		expect(intent).toMatchObject({ kind: 'intent', tone: 'danger' });
		expect(intent.kind === 'intent' && intent.text.toLowerCase()).toContain('approve');
		expect(blocks).toContainEqual({ kind: 'warning', tone: 'danger', text: m.warnUnlimited });
		// What it grants, as a row — a member has no cap to choose.
		expect(blocks.some((b) => b.kind === 'allowance')).toBe(false);
		expect(blocks).toContainEqual({
			kind: 'rows',
			rows: [
				{
					label: fill(m.labelSpendingCap, { symbol: 'USDC' }),
					value: m.valueUnlimited,
					valueTone: 'danger'
				}
			]
		});
		expect(texts(blocks).some((t) => t.toLowerCase().includes(SPENDER.toLowerCase()))).toBe(true);
		// The proposal's line is the core's verb, not "Contract call".
		expect(callTitle(reading.clear, m)).toBe(intent.kind === 'intent' ? intent.text : null);
		// And its row says who read it: Vela, from the transaction's own bytes.
		expect(callSource(reading.clear, m)).toBe(`${m.certain.labelSource} ${m.certain.byVela}`);
	});

	it('a call nobody can read: the blind warning in red, and the coin it moves', () => {
		const reading = read(BLIND);
		expect(reading.clear.surface).toBe('blind_transaction');
		const blocks = callReviewBlocks(reading, m, CURRENCY);
		expect(blocks[0]).toEqual({ kind: 'intent', text: m.intentBlind, tone: 'danger' });
		expect(blocks).toContainEqual({
			kind: 'warning',
			tone: 'danger',
			text: fill(m.warnBlindDecode, { bytes: '8' })
		});
		expect(blocks).toContainEqual({
			kind: 'amount',
			line: { sign: '−', value: '1.5', symbol: 'xDAI', tone: 'neutral' }
		});
		// Nothing to title it with: it stays "Contract call", with no source.
		expect(callTitle(reading.clear, m)).toBeNull();
		expect(callSource(reading.clear, m)).toBeNull();
	});

	it('says it is reading until the core has read it', () => {
		const reading = read(BLIND);
		const loading = [m.loading];
		// The core not loaded yet, then the call being read: never an empty review.
		const before = { ...reading, clear: INITIAL_CLEAR_VIEW, guard: INITIAL_GUARD_VIEW };
		expect(texts(callReviewBlocks(before, m, CURRENCY))).toEqual(loading);
		const out = { ...reading, clear: { ...reading.clear, resolving: true } };
		expect(texts(callReviewBlocks(out, m, CURRENCY))).toEqual(loading);
		expect(callTitle(out.clear, m)).toBeNull();
	});
});
