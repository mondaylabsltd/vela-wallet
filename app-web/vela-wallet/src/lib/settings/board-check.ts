/**
 * When the gallery boards' signing pages were checked (spec 102, D-13) — one
 * moment for every board that draws an integrity line ("… · checked
 * {{time}}"): Settings → Signing pages, "Where you review and sign", the
 * integrity-state board and the hand-off card.
 *
 * `{{time}}` is the core's (`launch::checked_time`, `signerIntegrityTime`):
 * the clock time in the person's format for a check from today, else the date
 * and the time. The boards' check is half an hour old, the same day, read on
 * a 24-hour clock — so every language says "14:32".
 *
 * The settings fixtures are built in the browser too (component tests), where
 * the wasm core is not loaded, so they draw {@link BOARD_CHECK_TIME}; the
 * prerendered boards ask the core ({@link boardCheckTime}); and
 * `venue.test.ts` holds the two equal in every locale — the same pattern as
 * the boards' venue rows, which are literal fixtures the core is asked to
 * produce.
 */
import { signerIntegrityTime } from '$lib/core/kernels';

/** 14:32 UTC, read at 15:02 the same day, in UTC. */
export const BOARD_CHECK = {
	at: Date.UTC(2026, 9, 9, 14, 32),
	now: Date.UTC(2026, 9, 9, 15, 2),
	utcOffsetMinutes: 0,
	formats: { date: 'dmy_slash', time: 'h24' }
} as const;

/** What {@link boardCheckTime} answers in every language (pinned by `venue.test.ts`). */
export const BOARD_CHECK_TIME = '14:32';

/** `{{time}}` of the boards' integrity lines, in `language`, as the core draws it. */
export function boardCheckTime(language: string): string {
	return signerIntegrityTime(
		BOARD_CHECK.at,
		BOARD_CHECK.now,
		BOARD_CHECK.utcOffsetMinutes,
		BOARD_CHECK.formats,
		language
	);
}
