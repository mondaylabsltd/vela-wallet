/**
 * Round 16 leftover 4: a failed action is worded from its KIND — the core's
 * (`review.failure`) — never from its text. The core's own failures are
 * kinds (`passkey_check`, `unknown`), plain text is "something went wrong",
 * and a message a shell kept beside a kind is never words.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import { failureWords } from './failure';
import { detailError } from './proposal-failure';
import { PasskeyCheckError } from '../core/owners';
import { ExecutionFeeMovedError } from '../core/execution';

const proposal = {
	notSent: 'not sent',
	reason: {
		passkeyClosed: 'closed',
		passkeyTimedOut: 'timed out',
		passkeyMissing: 'missing',
		passkeyFailed: 'did not check out',
		serviceKept: 'kept',
		serviceRefused: 'refused',
		offline: 'offline',
		chainUnreachable: 'cannot reach {{network}}',
		unknown: 'something went wrong'
	}
};

describe('a failure is a kind the web words, never its text (r16 leftover 4)', () => {
	it('reads the core’s passkey_check as a passkey that did not check out', () => {
		expect(detailError(JSON.stringify({ kind: 'passkey_check' }))).toBeInstanceOf(
			PasskeyCheckError
		);
	});

	it('never matches English: plain text is unknown, whatever it says', () => {
		const error = detailError("the passkey's signature did not check out: bad s");
		expect(error).not.toBeInstanceOf(PasskeyCheckError);
		expect(error.message).not.toContain('check out');
	});

	it('a kind it does not know is still an error (said as unknown), never undefined', () => {
		const error = detailError(JSON.stringify({ kind: 'something_new' }));
		expect(error).toBeInstanceOf(Error);
		expect(error.message).not.toContain('something_new');
	});

	it('keeps the facts of the kinds worded from them', () => {
		const moved = detailError(
			JSON.stringify({ kind: 'fee_moved', amount: '12', chainId: 100, executor: '0xab' })
		);
		expect(moved).toBeInstanceOf(ExecutionFeeMovedError);
	});

	it('words create and share-again failures from the kind', () => {
		expect(failureWords('Error: relay said 500', proposal)).toBe('something went wrong');
		expect(failureWords('', proposal)).toBe('something went wrong');
		expect(
			failureWords(
				JSON.stringify({
					kind: 'passkey',
					failure: 'cancelled',
					timedOut: false,
					message: 'NotAllowedError: The operation either timed out or was not allowed.'
				}),
				proposal
			)
		).toBe('closed');
		expect(
			failureWords(
				JSON.stringify({ kind: 'unknown', message: 'TypeError: x is undefined' }),
				proposal
			)
		).toBe('something went wrong');
		expect(failureWords(JSON.stringify({ kind: 'passkey_check' }), proposal)).toBe(
			'did not check out'
		);
		expect(failureWords(JSON.stringify({ kind: 'not_sent' }), proposal)).toBe('not sent');
		expect(
			failureWords(JSON.stringify({ kind: 'chain_unreachable', chain_id: 100 }), proposal, 'Gnosis')
		).toBe('cannot reach Gnosis');
	});
});

/**
 * r18-8 (§28, §32): the core's own failure for a write this test build
 * refused (`production_write_refused`, a share-again's) is said in the test
 * build's words — never "can't be reached", "didn't answer" or "offline".
 */
describe('a write the test build refused', () => {
	it('is said as the test build’s refusal, never the network', () => {
		const refused = 'This test build won’t write to the live service.';
		const said = failureWords(
			JSON.stringify({ kind: 'production_write_refused' }),
			proposal,
			'',
			refused
		);
		expect(said).toBe(refused);
		// Without resolved words: the corpus' English, still never the network.
		const english = failureWords(JSON.stringify({ kind: 'production_write_refused' }), proposal);
		expect(english).toBe(
			"This test build won't write to the live service; point it at a local one."
		);
		for (const text of [said, english]) {
			expect(text).not.toMatch(/didn.t answer|can.t be reached|offline|went wrong/i);
		}
	});
});
