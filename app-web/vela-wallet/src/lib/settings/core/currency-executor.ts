/**
 * The only place the `display_currency` core touches the outside world — WEB.
 *
 * Ported from the currency arm of src/services/wallet-state-core/executors.ts
 * @ e78afdfa (spec 024). Same key (`vela.displayCurrency`), same value
 * format. Two web answers, per contracts/shell-operations.md:
 * - `read_device_currency` → `null`: the core's own rule already says "None
 *   on web and for regionless locales" — the browser has no region currency.
 * - `resolve_rate` → the rate waterfall (`services/currency-rate`: Chainlink
 *   feed → configured FX endpoint → `null`). `null` is NOT 1 by core rule —
 *   formatting degrades to the USD figure, conversion refuses. Live since
 *   spec 025 Phase 5.
 *
 * Failure contract (shared effect loop): nothing rejects.
 */

import { resolveRate } from '$lib/services/currency-rate';
import { getItem, setItem } from '$lib/services/storage';
import { announceWalletSync } from '$lib/wallet/core/wallet-sync';
import type { CurrencyShellResult } from '$lib/core/generated/CurrencyShellResult';
import type { CurrencyOperation } from '$lib/core/generated/CurrencyOperation';

export type CurrencyEffect = { id: number; operation: CurrencyOperation };

/** Same key `services/currency.ts` owns — the value format is unchanged. */
const CURRENCY_KEY = 'vela.displayCurrency';

export async function executeCurrencyOperation(
	effect: CurrencyEffect
): Promise<CurrencyShellResult> {
	const operation = effect.operation;
	switch (operation.type) {
		case 'read_stored_code': {
			const code = await getItem(CURRENCY_KEY);
			return { type: 'stored_code', code: code ?? null };
		}
		case 'write_stored_code':
			await setItem(CURRENCY_KEY, operation.code);
			// IndexedDB tells no other document: the wallet open elsewhere
			// (a tab, the side panel) reads the choice again (`wallet-sync.ts`).
			announceWalletSync({ topic: 'currency' });
			return { type: 'code_written' };
		case 'read_device_currency':
			// The core's rule: None on web. Answered, never skipped.
			return { type: 'device_currency', code: null };
		case 'resolve_rate': {
			// The source chain (Chainlink → FX endpoint); null = cannot price it
			// right now. The strict/fallback split lives in the core. null ≠ 1.
			const rate = await resolveRate(operation.code);
			return { type: 'rate_resolved', code: operation.code, rate };
		}
		default: {
			const never: never = operation;
			throw new Error(`unhandled display_currency operation: ${JSON.stringify(never)}`);
		}
	}
}

export function currencyOperationFailure(effect: CurrencyEffect): CurrencyShellResult {
	const operation = effect.operation;
	switch (operation.type) {
		case 'read_stored_code':
			// An unreadable preference means "the user never chose".
			return { type: 'stored_code', code: null };
		case 'write_stored_code':
			// Best-effort, as `setCurrency` always was.
			return { type: 'code_written' };
		case 'read_device_currency':
			return { type: 'device_currency', code: null };
		case 'resolve_rate':
			return { type: 'rate_resolved', code: operation.code, rate: null };
		default: {
			const never: never = operation;
			throw new Error(`unhandled display_currency operation: ${JSON.stringify(never)}`);
		}
	}
}
