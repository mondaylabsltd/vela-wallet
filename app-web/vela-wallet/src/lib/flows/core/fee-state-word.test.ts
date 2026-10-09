/**
 * PR 2 integration: the send machine hears the fee card's failure.
 *
 * Between two of the core's re-asks — busy false, `failure` set — the send
 * confirm opened on the figure the send machine had kept from Continue, which
 * the fee machine had already discarded, and shut again with the next re-ask:
 * an enabled↔held flicker every 3–8 s. The bridge now says the card's failure
 * too (`fee_failed_changed`), beside its busy word, each when it changes; the
 * send machine holds the confirm while it stands (its own suite proves that).
 */
import { describe, expect, it } from 'vitest';
import type { FeeFailureView } from '$lib/core/generated/FeeFailureView';
import { FeeStateWord } from './send-estimates';

const FAILURE: FeeFailureView = {
	failure: { chain_read: { rate_limited: false } },
	reason_key: 'componentsUi.gas.reasonChainDown',
	auto_retry: true,
	retrying: false,
	figure_key: null,
	footer_key: 'componentsUi.signing.confirmBlock.feeRetrying'
};
const quoted = { busy: false, failure: null };
const failed = { busy: false, failure: FAILURE };
const retrying = { busy: true, failure: { ...FAILURE, retrying: true } };
const measuring = { busy: true, failure: null };

describe('the bridge’s busy and failed words (FeeStateWord)', () => {
	it('a journey’s first word of the failure always goes; "not busy" is the machine’s start', () => {
		const word = new FeeStateWord();
		expect(word.news(quoted)).toEqual([{ type: 'fee_failed_changed', failed: false }]);
		expect(word.news(quoted)).toEqual([]);
	});

	it('says the failure once, and holds it through the core’s re-asks — no flicker', () => {
		const word = new FeeStateWord();
		word.news(quoted);
		expect(word.news(failed)).toEqual([{ type: 'fee_failed_changed', failed: true }]);
		// The core's re-ask: busy, the failure kept — only the busy word moves.
		expect(word.news(retrying)).toEqual([{ type: 'fee_busy_changed', busy: true }]);
		expect(word.news(failed)).toEqual([{ type: 'fee_busy_changed', busy: false }]);
		expect(word.news(failed)).toEqual([]);
		// A quote lands: no failure any more.
		expect(word.news(measuring)).toEqual([
			{ type: 'fee_busy_changed', busy: true },
			{ type: 'fee_failed_changed', failed: false }
		]);
		expect(word.news(quoted)).toEqual([{ type: 'fee_busy_changed', busy: false }]);
	});

	it('a new journey has been told nothing', () => {
		const word = new FeeStateWord();
		word.news(failed);
		word.forget();
		expect(word.news(failed)).toEqual([{ type: 'fee_failed_changed', failed: true }]);
	});
});
