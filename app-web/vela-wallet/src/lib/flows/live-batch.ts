/**
 * The batch importer's live overlay (spec 026 T251).
 *
 * The drawn sheet, filled from `BatchView`. The parse, the duplicate check,
 * the fiat→token conversion, the cap and the apply gate are the core's — this
 * only words them.
 *
 * The rule that matters here is the one the Rust machine exists for: when no
 * source can price the chosen currency, the rate is UNKNOWN and the importer
 * refuses to convert. A 5,000 CNY payroll line converted at a defaulted 1:1
 * would pay ~7x. The core answers `rate_status: 'failed'` and `can_apply:
 * false`; this file shows that refusal instead of a number.
 */
import type { BatchPreviewRow } from '$lib/core/generated/BatchPreviewRow';
import type { BatchView } from '$lib/core/generated/BatchView';
import { fill } from '$lib/wallet/messages';
import type { WalletFlowMessages } from './messages';
import type { BatchImportModel } from './model';

export interface BatchLiveInputs {
	batch: BatchView;
	m: WalletFlowMessages;
	/** The token being split — its symbol words every converted row. */
	symbol: string;
}

/** One parsed line: who, and how much they end up receiving. */
function previewRow(
	row: BatchPreviewRow,
	symbol: string
): { ok: boolean; address: string; conversion: string } {
	return {
		ok: row.ok,
		address: row.name ?? row.address,
		// The core converted it; an unconvertible row carries no token amount,
		// and showing the raw fiat there would read as if it had.
		conversion: row.token_amount ? `${row.token_amount} ${symbol}` : row.raw_amount
	};
}

export function liveBatchImport(
	model: BatchImportModel,
	inputs: BatchLiveInputs
): BatchImportModel {
	const { batch, m, symbol } = inputs;
	const count = batch.recipient_count;
	return {
		...model,
		// The tabs, the rate label and the hint name the currency in force and
		// the token being split — the fixture's CNY/USDT were a picture.
		units: {
			fiat: fill(m['send.batchUnitFiat'], { code: batch.fiat_code }),
			token: fill(m['send.batchUnitToken'], { sym: symbol })
		},
		rateLabel: fill(m['send.batchRateLabel'], { sym: symbol }),
		rateHint: fill(m['send.batchRateHint'], { code: batch.fiat_code, sym: symbol }),
		unit: batch.unit,
		pasteValue: batch.raw_text,
		rateValue:
			batch.rate_status === 'ok'
				? `${batch.rate_input} ${batch.fiat_code}`
				: batch.rate_status === 'loading'
					? m['send.batchRateLoading']
					: // Unknown, and said so. The core has already refused to apply —
						// and the field below is where a rate can be typed (spec 038 #E6).
						m['send.batchRateFailed'],
		// The rate is the screen's real subject and it is editable in place
		// (`EditRate` / `ResetRateToAuto` have been the core's since 026; the
		// web drew the rate as a read-only span — spec 038 #E6).
		rateInput: batch.rate_input,
		rateEdited: batch.rate_edited,
		rateReset: m['send.batchRateReset'],
		parsedLabel: fill(m['send.batchParsedCount'], { n: batch.recipient_count }),
		rows: batch.preview.map((row) => previewRow(row, symbol)),
		rejectedText:
			batch.rejected > 0
				? fill(batch.rejected === 1 ? m['send.batchRejected_one'] : m['send.batchRejected_other'], {
						count: batch.rejected
					})
				: undefined,
		// The button offers what parsed, never the fixture's two.
		cta:
			count === 0
				? m['send.batchApplyEmpty']
				: fill(count === 1 ? m['send.batchApply_one'] : m['send.batchApply_other'], { count }),
		ctaDisabled: !batch.can_apply
	};
}
