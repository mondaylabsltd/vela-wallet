/**
 * Spec 082 round 2 (T227): the new words resolve in every locale — the
 * refusal (RJ3/RJ6), "提交至网络…" for a may-have-been-sent op (G56), and every
 * reason key the core's `feeFailureReasonKey` can name (RJ13).
 */
import '$lib/i18n/wasm-init.server';
import { readdirSync, readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import {
	CONFIRM_BLOCK_KEYS,
	FEE_REASON_KEYS,
	REFUSAL_KEYS,
	rawResolve,
	resolveSigningMessages
} from '$lib/i18n/engine.server';
import { SUPPORTED_LOCALES } from '$lib/i18n/locales';
import { feeFailureReasonKey } from '$lib/core/kernels';
import type { FeeFailure } from '$lib/core/generated/FeeFailure';

describe('the refusal resolves (RJ3, RJ6)', () => {
	it.each(SUPPORTED_LOCALES)(
		'%s: `componentsUi.signing.refused` is a sentence, not its key',
		(locale) => {
			const m = resolveSigningMessages(locale);
			expect(m.receipt.refused).toBe(rawResolve(locale, 'componentsUi.signing.refused'));
			expect(m.receipt.refused).not.toBe('componentsUi.signing.refused');
			expect(m.receipt.refused.trim()).not.toBe('');
		}
	);

	it('says the network refused it and nothing was sent — and never "try again"', () => {
		expect(resolveSigningMessages('en').receipt.refused).toBe(
			'The network refused it — nothing was sent.'
		);
		expect(resolveSigningMessages('zh').receipt.refused).toBe(
			'网络拒绝了这笔交易，什么都没有发出。'
		);
		for (const locale of ['en', 'zh'] as const) {
			const m = resolveSigningMessages(locale);
			expect(m.receipt.refused).not.toBe(m.status.failedHint);
		}
		expect(resolveSigningMessages('zh').receipt.refused).not.toContain('重试');
		expect(resolveSigningMessages('en').receipt.refused.toLowerCase()).not.toContain('try again');
	});

	it('a may-have-been-sent op is titled "提交至网络…" (G56)', () => {
		expect(resolveSigningMessages('zh').receipt.submitting).toBe(
			rawResolve('zh', 'send.txSubmitting')
		);
		expect(resolveSigningMessages('zh').receipt.submitting).toContain('提交至网络');
	});
});

describe('every fee reason the core can name resolves (RJ13)', () => {
	const failures: FeeFailure[] = [
		'quote_unavailable',
		'fee_token_unavailable',
		'estimate_failed',
		'gas_quote_too_high',
		'missing_public_key',
		'calculation_failed',
		'would_fail',
		'internal',
		{ chain_read: { rate_limited: true } },
		{ chain_read: { rate_limited: false } }
	];

	it('the core’s keys are the ones the build resolves', () => {
		for (const failure of failures) {
			const key = feeFailureReasonKey(failure);
			if (key !== null) expect(FEE_REASON_KEYS, JSON.stringify(failure)).toContain(key);
		}
		expect(feeFailureReasonKey('missing_public_key')).toBeNull();
		expect(feeFailureReasonKey({ chain_read: { rate_limited: true } })).toBe(
			'home.balanceDetailStatusRetrying'
		);
		// Issue 483: the fee's own words — a chain out of reach, and a fault
		// inside Vela, which is never "can't reach the chain".
		expect(feeFailureReasonKey({ chain_read: { rate_limited: false } })).toBe(
			'componentsUi.gas.reasonChainDown'
		);
		expect(feeFailureReasonKey('internal')).toBe('componentsUi.gas.reasonInternal');
	});

	it.each(SUPPORTED_LOCALES)('%s: each reason is a sentence', (locale) => {
		const m = resolveSigningMessages(locale);
		for (const key of FEE_REASON_KEYS) {
			expect(m.feeReasons[key], `${key} in ${locale}`).not.toBe(key);
			expect(m.feeReasons[key]?.trim(), `${key} in ${locale}`).not.toBe('');
		}
		// `{{chain}}` is left for the sheet to fill with the chain's name.
		expect(m.feeReasons['componentsUi.gas.reasonChainDown']).toContain('{{chain}}');
		expect(m.feeReasons['componentsUi.gas.reasonInternal']).not.toContain('{{chain}}');
	});

	it('the revert warning with its reason resolves (RJ19)', () => {
		const m = resolveSigningMessages('en');
		expect(m.warnWillFailReason).toContain('{{reason}}');
		expect(m.warnWillFail).not.toBe('componentsUi.signing.simWillFail');
	});
});

/**
 * Correctness batch item 3: a refusal is told by its reason, and the held
 * confirm says the previous transaction is still on its way — every sentence
 * the core can name resolves in every locale.
 */
describe('the refusal reasons and the held confirm resolve', () => {
	it.each(SUPPORTED_LOCALES)('%s: each is a sentence, not its key', (locale) => {
		const m = resolveSigningMessages(locale);
		for (const key of REFUSAL_KEYS) {
			expect(m.receipt.refusals[key], `${key} in ${locale}`).toBe(rawResolve(locale, key));
			expect(m.receipt.refusals[key], `${key} in ${locale}`).not.toBe(key);
		}
		const held = 'componentsUi.signing.confirmBlock.previousPending';
		expect(m.confirmBlock[held], locale).toBe(rawResolve(locale, held));
		expect(m.confirmBlock[held], locale).not.toBe(held);
	});
});

/**
 * PR 2 polish: "not sent yet" for a held nonce at submit, the failed row's
 * "Pay with another coin", and the would-fail line under the held confirm —
 * every word the sheet draws for them resolves in every locale.
 */
describe('the not-sent and would-fail words resolve', () => {
	it.each(SUPPORTED_LOCALES)('%s: each is a sentence, not its key', (locale) => {
		const m = resolveSigningMessages(locale);
		const said = (value: string | undefined, key: string) => {
			expect(value, `${key} in ${locale}`).toBe(rawResolve(locale, key));
			expect(value, `${key} in ${locale}`).not.toBe(key);
		};
		said(m.status.notSentTitle, 'componentsUi.signing.notSentTitle');
		said(
			m.receipt.refusals['componentsUi.signing.notSentBody'],
			'componentsUi.signing.notSentBody'
		);
		said(m.feePayWithAnotherCoin, 'componentsUi.gas.payWithAnotherCoin');
		const wouldFail = 'componentsUi.signing.confirmBlock.feeWouldFail';
		said(m.confirmBlock[wouldFail], wouldFail);
	});
});

/**
 * PR 3 — the confirm waits for the simulation's verdict. The core names one
 * new line under the held confirm (`ConfirmBlock::SimChecking`); the sheet
 * looks a block's key up in `confirmBlock`, and a key missing from that map
 * is a held confirm that does not say why.
 */
describe('the line under a confirm that waits for the simulation (PR 3)', () => {
	const KEY = 'componentsUi.signing.confirmBlock.simChecking';

	it.each(SUPPORTED_LOCALES)('%s: it is a sentence, not its key', (locale) => {
		const m = resolveSigningMessages(locale);
		expect(m.confirmBlock[KEY], locale).toBe(rawResolve(locale, KEY));
		expect(m.confirmBlock[KEY], locale).not.toBe(KEY);
		expect(m.confirmBlock[KEY].trim(), locale).not.toBe('');
	});

	it('says what is happening, in the present tense of the could-not-check sentence', () => {
		expect(resolveSigningMessages('en').confirmBlock[KEY]).toBe(
			'Checking what this transaction does…'
		);
		expect(resolveSigningMessages('zh').confirmBlock[KEY]).toBe('正在检查这笔交易的结果…');
		expect(resolveSigningMessages('en').warnSimUnavailable).toBe(
			'Vela couldn’t check what this transaction does. Review it before you sign.'
		);
	});

	it('every line the core can put under a held confirm has its words here, and no other', () => {
		const dir = '../../rust/crates/vela-core/src/app';
		const named = new Set<string>();
		for (const file of readdirSync(dir).filter((name) => name.endsWith('.rs'))) {
			const source = readFileSync(`${dir}/${file}`, 'utf8');
			for (const [, key] of source.matchAll(
				/"(componentsUi\.signing\.confirmBlock\.[A-Za-z]+)"/g
			)) {
				named.add(key);
			}
		}
		expect(named.has(KEY)).toBe(true);
		expect([...named].sort()).toEqual([...CONFIRM_BLOCK_KEYS].sort());
	});
});
