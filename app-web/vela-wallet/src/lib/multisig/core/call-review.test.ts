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
import {
	batchParamsJson,
	callParamsJson,
	callReviewBlocks,
	callSource,
	callTitle,
	readableCalls,
	type InnerCall
} from './call-review.svelte';
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

	it('r5reg-10: a plain coin send is titled in the reader’s words, never the record’s English "Send"', () => {
		const reading = read({ to: SPENDER, value: '50000000000000000', data: '0x' });
		expect(reading.clear.record_intent).not.toBeNull();
		const zh = resolveSigningMessages('zh');
		const title = callTitle(reading.clear, zh);
		expect(title).toBe(zh.terms.intentSend);
		expect(title).not.toBe(m.terms.intentSend);
		expect(callTitle(reading.clear, m)).toBe(m.terms.intentSend);
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

/**
 * Owner 2026-10-28: a multisig's batch of calls (a site's `wallet_sendCalls`,
 * one MultiSendCallOnly proposal) reads as the signing sheet reads that
 * batch — every call decoded on its own card, each saying whose reading it
 * is — before the proposer's passkey and for every member after.
 */
describe('a multisig batch of calls reads call by call', () => {
	it('goes over as the site would send it: every call, atomically, from the multisig', () => {
		expect(JSON.parse(batchParamsJson([APPROVE_MAX, BLIND], 100, '0xabc'))).toEqual([
			{
				version: '2.0.0',
				chainId: '0x64',
				from: '0xabc',
				atomicRequired: true,
				calls: [
					{ to: APPROVE_MAX.to, data: APPROVE_MAX.data, value: '0x0' },
					{ to: BLIND.to, data: BLIND.data, value: '0x14d1120d7b160000' }
				]
			}
		]);
	});

	it('every call its own card, the unlimited approval warned, the coin it moves', () => {
		const reading = read([APPROVE_MAX, BLIND]);
		expect(reading.method).toBe('wallet_sendCalls');
		expect(reading.clear.surface).toBe('batch');
		expect(reading.clear.batch?.calls).toHaveLength(2);
		const blocks = callReviewBlocks(reading, m, CURRENCY);
		expect(blocks[0]).toMatchObject({ kind: 'intent', text: m.intentBatch });
		const cards = blocks.filter((b) => b.kind === 'card');
		expect(cards).toHaveLength(2);
		// The first call is read (an approval), and says whose reading it is.
		const first = cards[0];
		expect(first.kind === 'card' && first.rows.some((r) => r.label === m.certain.labelSource)).toBe(
			true
		);
		// The second nobody can read: its title says so, with what it moves.
		const second = cards[1];
		expect(second.kind === 'card' && second.title).toContain(
			fill(m.warnBlindDecode, { bytes: '8' })
		);
		expect(texts(blocks).some((t) => t.includes('1.5'))).toBe(true);
		expect(blocks).toContainEqual({ kind: 'warning', tone: 'danger', text: m.warnUnlimited });
	});

	it('a batch of transfers alone keeps its own lines; one with a call is read', () => {
		const transfer = {
			kind: 'transfer' as const,
			token: null,
			to: SPENDER,
			amount: '1'
		};
		const leg = (action: typeof transfer | (InnerCall & { kind: 'call' })) => ({
			to: action.to,
			value: '0',
			data: '0x',
			action
		});
		expect(readableCalls({ kind: 'batch', calls: [leg(transfer), leg(transfer)] })).toBeNull();
		const mixed = readableCalls({
			kind: 'batch',
			calls: [leg(transfer), leg({ kind: 'call', ...APPROVE_MAX })]
		});
		expect(mixed).toHaveLength(2);
		expect(readableCalls({ kind: 'call', ...BLIND })).toEqual([
			{ to: BLIND.to, value: BLIND.value, data: BLIND.data }
		]);
		expect(readableCalls(null)).toBeNull();
	});
});
