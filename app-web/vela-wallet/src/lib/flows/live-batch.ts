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
 *
 * And the rule this file broke until issue 204: every reason `can_apply` is
 * false is a field on the view, and a shell that reads the gate without the
 * reasons draws a dark button and says nothing. `over_balance`, `over_cap`,
 * `total_token`, `total_fiat`, `priced`, `busy`, `file_error`,
 * `template_saved`, `file_name` and each row's `dup` went unread here while
 * their sentences sat in the corpus in fifteen languages. They are all read
 * below; nothing is decided below.
 */
import type { BatchPreviewRow } from '$lib/core/generated/BatchPreviewRow';
import type { BatchView } from '$lib/core/generated/BatchView';
import { amountToInput, groupDigits, numberSeparators } from '$lib/services/locale-format';
import { shortenAddress } from '$lib/wallet/identity';
import { exactAmount as exact, tokenAmountText } from '$lib/wallet/live';
import { fill } from '$lib/wallet/messages';
import type { WalletFlowMessages } from './messages';
import type { BatchImportModel, BatchRefusedModel, BatchRowModel } from './model';

export interface BatchLiveInputs {
	batch: BatchView;
	m: WalletFlowMessages;
	/** The token being split — its symbol words every converted row. */
	symbol: string;
	/** What the account holds of it: the figure the total is read against. */
	balance: string;
	identicon: (seed: string) => string;
	/** There is already someone on the split form for this import to add to. */
	formHasRows?: boolean;
	/** The person chose to REPLACE them with this import instead. */
	replaces?: boolean;
	/**
	 * What the form has NOT yet given out (the core's `split_remaining`). When
	 * this import adds to people already on the form, that — not the whole
	 * balance — is what it draws from, so that is the figure beside the total.
	 */
	remaining?: string | null;
}

/**
 * Artwork is drawn by the core, one call per address, and this overlay is
 * rebuilt on every keystroke in the paste box and the rate field — sixty
 * people would be sixty drawings per character. The rows rarely change between
 * two keystrokes, so the drawings are kept for as long as one list is on
 * screen and dropped when the importer is empty again (which every open is).
 */
const artwork = new Map<string, string>();

function art(inputs: BatchLiveInputs, address: string): string {
	let svg = artwork.get(address);
	if (svg === undefined) {
		svg = inputs.identicon(address);
		artwork.set(address, svg);
	}
	return svg;
}

/** The extensions the picker takes. Not words, so not corpus. */
const FILE_FORMATS = 'xlsx · csv · txt';

/** "13000.5" → "13,000.5": the sheet's own sum, grouped the person's way. */
function groupedFigure(figure: string): string {
	const [whole, frac] = figure.split('.');
	const grouped = groupDigits(whole);
	return frac === undefined ? grouped : `${grouped}${numberSeparators().decimal}${frac}`;
}

/**
 * A figure from the sheet. A plain number is grouped like the sum beneath it
 * ("5,000" over "13,000"); anything else — a currency sign, the sheet's own
 * separators — is left exactly as it was written, so it can be found again.
 */
function sheetFigure(raw: string): string {
	return /^\d+(\.\d+)?$/.test(raw) ? groupedFigure(raw) : raw;
}

/** One parsed line: who, how much they end up receiving, and why not. */
function previewRow(row: BatchPreviewRow, inputs: BatchLiveInputs): BatchRowModel {
	const { batch, m, symbol } = inputs;
	const fiat = batch.unit === 'fiat';
	// The core converted it; an unconvertible row carries no token amount,
	// and showing the raw fiat there would read as if it had.
	const sendable = row.token_amount !== '' && row.token_amount !== '0';
	return {
		kind: 'row',
		ok: row.ok,
		name: row.name ?? undefined,
		address: shortenAddress(row.address),
		addressFull: row.address,
		identiconSvg: art(inputs, row.address),
		amount: sendable ? `${exact(row.token_amount)} ${symbol}` : '—',
		// The figure exactly as the sheet wrote it, so it can be read back
		// against the sheet — beside the unit the importer took it to be in.
		source: fiat ? `${sheetFigure(row.raw_amount)} ${batch.fiat_code}` : undefined,
		note: row.dup ? m['send.batchDup'] : !row.valid ? m['send.batchBadAddress'] : undefined
	};
}

/**
 * The list as the sheet had it: parsed rows and the lines the parser refused,
 * in source order (both carry the same line numbering), so a refused line sits
 * between the neighbours it has in the sheet. `errors` is absent from a core
 * built before it existed — read as none.
 */
function previewRows(inputs: BatchLiveInputs): (BatchRowModel | BatchRefusedModel)[] {
	const { batch, m } = inputs;
	const rows = batch.preview.map((row) => ({ line: row.line, model: previewRow(row, inputs) }));
	const refused = (batch.errors ?? []).map((error) => ({
		line: error.line,
		model: {
			kind: 'refused' as const,
			text: error.raw,
			note: error.reason === 'no_address' ? m['send.batchBadAddress'] : m['send.badAmount']
		}
	}));
	return [...rows, ...refused].sort((a, b) => a.line - b.line).map((entry) => entry.model);
}

/**
 * The sentence under the rate. While the figure is known it explains what the
 * figure does; while it is not, it says why and what to do — which is the only
 * thing between the person and a button that will not arm.
 */
function rateHint(inputs: BatchLiveInputs): { hint: string; hintTone: 'plain' | 'warning' } {
	const { batch, m, symbol } = inputs;
	if (batch.rate_input !== '')
		return {
			hint: fill(m['send.batchRateHint'], { code: batch.fiat_code, sym: symbol }),
			hintTone: 'plain'
		};
	if (batch.rate_status === 'loading')
		return { hint: m['send.batchRateLoading'], hintTone: 'plain' };
	return {
		hint: batch.priced ? m['send.batchRateFailed'] : m['send.batchNoPrice'],
		hintTone: 'warning'
	};
}

export function liveBatchImport(
	model: BatchImportModel,
	inputs: BatchLiveInputs
): BatchImportModel {
	const { batch, m, symbol, balance } = inputs;
	const count = batch.recipient_count;
	const fiat = batch.unit === 'fiat';
	// Lines READ: the ones that became rows and the ones that were refused.
	const seen = batch.preview.length + (batch.errors ?? []).length;
	if (batch.preview.length === 0) artwork.clear();

	const notices: string[] = [];
	if (batch.rejected > 0)
		notices.push(
			fill(batch.rejected === 1 ? m['send.batchRejected_one'] : m['send.batchRejected_other'], {
				count: batch.rejected
			})
		);
	// Past the cap the first rows are kept and the rest are not sent. It does
	// not block the import, so it is a notice and never the refusal.
	if (batch.over_cap) notices.push(fill(m['send.batchOverCap'], { n: count }));

	return {
		...model,
		// The tabs, the rate and the hint name the currency in force and the
		// token being split — the fixture's CNY/USDT were a picture.
		unitCaption: m['send.batchUnitCaption'],
		units: {
			fiat: fill(m['send.batchUnitFiat'], { code: batch.fiat_code }),
			token: fill(m['send.batchUnitToken'], { sym: symbol })
		},
		unit: batch.unit,
		pasteValue: batch.raw_text,
		tools: {
			file: {
				label: batch.busy ? m['send.batchReading'] : m['send.batchImportFile'],
				busy: batch.busy
			},
			template: {
				label: batch.template_saved ? m['send.batchTemplateSaved'] : m['send.batchTemplate'],
				saved: batch.template_saved
			},
			formats: FILE_FORMATS,
			fileName: batch.file_name ?? undefined,
			// `file_error` outlives a paste in the core (only the next pick
			// clears it), and an error about a file above a list that parsed
			// is an error about nothing.
			error:
				batch.file_error && seen === 0
					? `${m['send.batchImportFailedTitle']}. ${m['send.batchImportFailedBody']}`
					: undefined
		},
		// The rate is the screen's real subject in fiat mode and it is editable
		// in place (`EditRate` / `ResetRateToAuto`, spec 038 E6). In token mode
		// the core ignores it entirely, so there is no row.
		rate: fiat
			? {
					section: m['send.batchRateSection'],
					lead: `1 ${symbol}`,
					sign: batch.rate_edited ? '=' : '≈',
					// The figure that converts every row, digit for digit — only
					// its decimal mark is the person's, because "7.558" is seven
					// thousand to anyone who groups with a dot.
					value: amountToInput(batch.rate_input),
					code: batch.fiat_code,
					editable: true,
					edited: batch.rate_edited,
					reset: m['send.batchRateReset'],
					...rateHint(inputs)
				}
			: undefined,
		unitHint: fiat ? undefined : fill(m['send.batchTokenHint'], { sym: symbol }),
		preview:
			seen > 0
				? {
						// Rows READ, not rows kept: the count above a list is the
						// length of that list.
						label: fill(m['send.batchParsedCount'], { n: seen }),
						rows: previewRows(inputs)
					}
				: undefined,
		notices,
		total:
			count > 0
				? {
						label: `${m['send.splitTotalLabel']} · ${fill(
							count === 1 ? m['send.recipientCount_one'] : m['send.recipientCount_other'],
							{ count }
						)}`,
						value: `${exact(batch.total_token)} ${symbol}`,
						detail:
							batch.total_fiat === null
								? undefined
								: `${groupedFigure(batch.total_fiat)} ${batch.fiat_code}`,
						balance:
							inputs.formHasRows && !inputs.replaces && inputs.remaining != null
								? fill(m['send.splitRemaining'], {
										amount: `${tokenAmountText(inputs.remaining)} ${symbol}`
									})
								: fill(m['send.balanceLabel'], {
										amount: `${tokenAmountText(balance)} ${symbol}`
									}),
						over: batch.over_balance,
						overText: batch.over_balance
							? fill(m['send.batchOverBalance'], { sym: symbol })
							: undefined
					}
				: undefined,
		// Said once the import can happen, beside the button that does it — and
		// with the way to choose the other, because either can be what is meant.
		merge:
			inputs.formHasRows && batch.can_apply
				? inputs.replaces
					? { note: m['send.batchReplacesRows'], action: m['send.batchAddInstead'] }
					: { note: m['send.batchAddsToRows'], action: m['send.batchReplaceInstead'] }
				: undefined,
		// The button offers what parsed, never the fixture's two.
		cta:
			count === 0
				? m['send.batchApplyEmpty']
				: fill(count === 1 ? m['send.batchApply_one'] : m['send.batchApply_other'], { count }),
		ctaDisabled: !batch.can_apply
	};
}
