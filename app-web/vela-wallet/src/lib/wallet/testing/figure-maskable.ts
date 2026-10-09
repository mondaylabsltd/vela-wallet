/**
 * TESTS ONLY — never imported by product code.
 *
 * Hand-built feed rows need the field the core sets on every real one,
 * `FeedItem.figure_maskable` (`app::privacy::figure_maskable`). This is that
 * rule, for synthetic rows: the shared fixture (`privacy-fixture.test.ts`)
 * pins the core's own answers, so a drift between the two shows there.
 */
import type { FeedItem } from '$lib/core/generated/FeedItem';

export function coreFigureMaskable(item: Omit<FeedItem, 'figure_maskable'>): boolean {
	if (item.value !== null) return true;
	if (item.dapp != null) {
		const allowance = item.dapp.allowance;
		return allowance != null && !allowance.unlimited && allowance.value !== null;
	}
	return item.batch !== null;
}

/** A hand-built row with the core's `figure_maskable`, unless the test sets it. */
export function withFigureMaskable(
	item: Omit<FeedItem, 'figure_maskable'> & { figure_maskable?: boolean }
): FeedItem {
	return { ...item, figure_maskable: item.figure_maskable ?? coreFigureMaskable(item) };
}
