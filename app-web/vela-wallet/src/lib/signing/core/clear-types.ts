/**
 * Platform-neutral types and the locale codec for the `clear_signing` core
 * (spec 026 Phase 5; `rust/crates/vela-core/src/app/clear_signing.rs`).
 *
 * Ported from src/services/wallet-state-core/clear-types.ts @ f9bcb278, minus
 * its RESULT codec. That codec existed to translate the core's fields back
 * into the shapes Expo's own 1,321-line TypeScript twin declared; on the web
 * that twin is not ported (Rust owns the ladder), so `signing/live.ts` reads
 * the generated view directly. One shape, not two that can drift.
 *
 * What remains is the locale preset: the core owns WHICH number is shown, the
 * preset says how digits group, and `auto` is resolved here because detecting
 * it reads `Intl` — a shell capability.
 */
import type { ClearLocale } from '#lib/core/generated/ClearLocale.ts';
import type { ClearOperation } from '#lib/core/generated/ClearOperation.ts';
import type { ClearSigningView } from '#lib/core/generated/ClearSigningView.ts';
import type { SessionOptions } from '#lib/core/types.ts';

/** One request from the core, carrying the id it will be answered by. */
export type ClearEffect = { id: number; operation: ClearOperation };

export type ClearSigningSessionOptions = SessionOptions<ClearSigningView>;

/**
 * The shell's resolved presets for one resolution run.
 *
 * `tz_offset_minutes` is minutes to ADD to UTC (the negation of JS
 * `getTimezoneOffset()`), sampled once per request — the core owns the
 * deadline verdict but no clock and no timezone database.
 */
export function toClearLocale(keys: {
	number: ClearLocale['number_format'];
	date: ClearLocale['date_format'];
	time: ClearLocale['time_format'];
}): ClearLocale {
	return {
		number_format: keys.number,
		date_format: keys.date,
		time_format: keys.time,
		tz_offset_minutes: -new Date().getTimezoneOffset()
	};
}
