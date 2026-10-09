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
	footer_key: 'componentsUi.signing.confirmBlock.feeRetrying',
	tap: 'retry',
	chain_id: 1,
	fee_token: null
};
/** The send form's chain in these tests: the failure's own. */
const FORM = 1;
const quoted = { busy: false, failure: null };
const failed = { busy: false, failure: FAILURE };
const retrying = { busy: true, failure: { ...FAILURE, retrying: true } };
const measuring = { busy: true, failure: null };

describe('the bridge’s busy and failed words (FeeStateWord)', () => {
	it('a journey’s first word of the failure always goes; "not busy" is the machine’s start', () => {
		const word = new FeeStateWord();
		expect(word.news(quoted, FORM)).toEqual([{ type: 'fee_failed_changed', failed: false }]);
		expect(word.news(quoted, FORM)).toEqual([]);
	});

	it('says the failure once, and holds it through the core’s re-asks — no flicker', () => {
		const word = new FeeStateWord();
		word.news(quoted, FORM);
		expect(word.news(failed, FORM)).toEqual([{ type: 'fee_failed_changed', failed: true }]);
		// The core's re-ask: busy, the failure kept — only the busy word moves.
		expect(word.news(retrying, FORM)).toEqual([{ type: 'fee_busy_changed', busy: true }]);
		expect(word.news(failed, FORM)).toEqual([{ type: 'fee_busy_changed', busy: false }]);
		expect(word.news(failed, FORM)).toEqual([]);
		// A quote lands: no failure any more.
		expect(word.news(measuring, FORM)).toEqual([
			{ type: 'fee_busy_changed', busy: true },
			{ type: 'fee_failed_changed', failed: false }
		]);
		expect(word.news(quoted, FORM)).toEqual([{ type: 'fee_busy_changed', busy: false }]);
	});

	it('a new journey has been told nothing', () => {
		const word = new FeeStateWord();
		word.news(failed, FORM);
		word.forget();
		expect(word.news(failed, FORM)).toEqual([{ type: 'fee_failed_changed', failed: true }]);
	});
});

/**
 * PR 2 polish: a failure is for one question. Right after a token switch the
 * form names another chain before the fee machine has been asked about it;
 * the old chain's failure is not this form's, and must not hold its confirm.
 */
describe('only the form’s own chain’s failure is "failed" (FeeFailureView::is_for_chain)', () => {
	it('another chain’s failure is no failure for this form', () => {
		const word = new FeeStateWord();
		expect(word.news(failed, 137)).toEqual([{ type: 'fee_failed_changed', failed: false }]);
		expect(word.news(failed, 137)).toEqual([]);
	});

	it('the switch drops the old chain’s failure at once, and the new chain’s own is said', () => {
		const word = new FeeStateWord();
		expect(word.news(failed, FORM)).toEqual([{ type: 'fee_failed_changed', failed: true }]);
		// The token moved to Polygon: the Ethereum failure is gone from the form.
		expect(word.news(failed, 137)).toEqual([{ type: 'fee_failed_changed', failed: false }]);
		// Polygon's own failure, once the machine was asked about it.
		const polygon = { ...failed, failure: { ...FAILURE, chain_id: 137 } };
		expect(word.news(polygon, 137)).toEqual([{ type: 'fee_failed_changed', failed: true }]);
	});

	it('a failure built without a run (no chain) is every form’s, and a form with no chain drops a tagged one', () => {
		const untagged = { ...failed, failure: { ...FAILURE, chain_id: null } };
		expect(new FeeStateWord().news(untagged, 137)).toEqual([
			{ type: 'fee_failed_changed', failed: true }
		]);
		expect(new FeeStateWord().news(failed, null)).toEqual([
			{ type: 'fee_failed_changed', failed: false }
		]);
	});
});
