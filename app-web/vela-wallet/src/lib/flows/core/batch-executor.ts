// Ported from src/services/wallet-state-core/batch-import-executor.ts @ f9bcb278 — RN seams rewritten to the web modules; logic verbatim.
/**
 * The only place the `batch_import` core touches the outside world.
 *
 * Three operations, three existing service calls: the USD→fiat rate source,
 * the file picker (+ the lazy SheetJS flattening an `.xlsx` into a cell
 * matrix), and the template save. Nothing here branches on business meaning —
 * picking the `BatchShellResult` variant is *reporting what was observed*, and
 * every rule that reads those observations (what a cancelled pick does to
 * `busy`, whether a rate for a stale currency counts) lives in Rust.
 *
 * Failure contract (shared effect loop): nothing rejects. Every rejection is
 * converted into the result variant that operation answers with, so the core
 * keeps ownership of classification.
 */

import { resolveRate } from '$lib/services/currency-rate';
import { pickTable, saveTextFile } from '$lib/services/file-io';
import { readWorkbookMatrix } from '$lib/services/file-io';

import type { BatchShellResult } from '$lib/core/generated/BatchShellResult';
import type { BatchEffect } from './batch-types';

/**
 * SheetJS cells arrive as `any` (numbers survive `raw: false` in odd sheets,
 * ragged rows come back short). The core's wire type is `Vec<Vec<String>>`, so
 * the matrix is normalized here exactly as `interpretRows` does today
 * (`String(c ?? '')`) — the trim itself stays in the core.
 */
function toStringMatrix(rows: string[][]): string[][] {
	return rows.map((row) => (row ?? []).map((cell) => String(cell ?? '')));
}

export async function executeBatchOperation(effect: BatchEffect): Promise<BatchShellResult> {
	const operation = effect.operation;
	switch (operation.type) {
		case 'fetch_usd_fiat_rate': {
			// `resolveRate`, NOT `getRate`: `getRate` is the DISPLAY helper and ends
			// in `?? 1`, so an unpriceable code would arrive at the core as "the rate
			// really is 1" and `BatchRateStatus::Failed` would only ever be reachable
			// when the source *throws*. A CNY payroll row would then be split at
			// 1 CNY = 1 token — ~7x the intended payout — with `can_apply` still
			// true, because the core's guardrail had been answered a lie.
			// `null` is the honest observation ("no source can price it"); the core
			// turns it into Failed → empty rate → `can_apply = false`. Same choice as
			// the display-currency executor (`executors.web.ts::resolve_rate`).
			const rate = await resolveRate(operation.code);
			return { type: 'rate_resolved', code: operation.code, rate };
		}
		case 'pick_file': {
			const picked = await pickTable();
			if (!picked) return { type: 'file_pick_cancelled' };
			if (picked.excel) {
				// Excel only: ~1MB of SheetJS stays off the startup path (lazy import
				// inside `readWorkbookMatrix`), and the core parses the matrix.
				return {
					type: 'file_picked',
					name: picked.name,
					content: { type: 'matrix', rows: toStringMatrix(await readWorkbookMatrix(picked.bytes)) }
				};
			}
			// A text table's bytes, undecoded — the core decodes and parses
			// them (087, issue 333's twin).
			return {
				type: 'file_picked',
				name: picked.name,
				content: { type: 'bytes', bytes: Array.from(picked.bytes) }
			};
		}
		case 'save_template_file':
			await saveTextFile(operation.name, operation.contents, operation.mime);
			return { type: 'template_saved' };
	}
}

export function batchOperationFailure(effect: BatchEffect): BatchShellResult {
	switch (effect.operation.type) {
		case 'fetch_usd_fiat_rate':
			// No source could price it — the core turns this into the "enter one
			// manually" hint, never a silent zero.
			return { type: 'rate_resolved', code: effect.operation.code, rate: null };
		case 'pick_file':
			// Unreadable / unparseable file — the controller shows today's alert.
			return { type: 'file_pick_failed' };
		case 'save_template_file':
			// Share sheet dismissed or unavailable — silently keep the plain label.
			return { type: 'template_save_failed' };
	}
}
