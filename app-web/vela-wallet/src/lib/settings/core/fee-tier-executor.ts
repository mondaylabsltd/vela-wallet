/**
 * The only place the `fee_tier_pref` core touches the outside world — WEB
 * (spec 068).
 *
 * Shaped on `currency-executor.ts`, deliberately: the same two sentences
 * (read a preference, persist a preference) against the same store, so there
 * is one way a committed preference reaches disk in this shell.
 *
 * The key lives under the `vela.` prefix and **survives sign-out** — the
 * standing ruling is that signing out clears `vela.accounts` and
 * `vela.activeAccountIndex` and nothing else, and a speed preference belongs
 * to the person and the device rather than to the account. It is not listed
 * in `device-storage.ts`, which means it counts as "your data" and no "clear
 * caches" sweep touches it: a preference is not a cache.
 *
 * The stored value is handed back RAW. Judging whether a string is a tier is
 * the core's job (`parse_stored`), so a value this build does not understand
 * reads as "never chose" — the factory `fast` — rather than being coerced
 * here into something that would then go on the wire.
 *
 * Failure contract (shared effect loop): nothing rejects.
 */

import { getItem, setItem } from '$lib/services/storage';
import type { FeeTierPrefOperation } from '$lib/core/generated/FeeTierPrefOperation';
import type { FeeTierPrefShellResult } from '$lib/core/generated/FeeTierPrefShellResult';

export type FeeTierPrefEffect = { id: number; operation: FeeTierPrefOperation };

/** The one storage home for the default transaction speed. */
export const FEE_TIER_KEY = 'vela.feeTier';

export async function executeFeeTierPrefOperation(
	effect: FeeTierPrefEffect
): Promise<FeeTierPrefShellResult> {
	const operation = effect.operation;
	switch (operation.type) {
		case 'read_stored_tier': {
			const raw = await getItem(FEE_TIER_KEY);
			return { type: 'stored_tier', raw: raw ?? null };
		}
		case 'write_stored_tier':
			await setItem(FEE_TIER_KEY, operation.tier);
			return { type: 'tier_written' };
		default: {
			const never: never = operation;
			throw new Error(`unhandled fee_tier_pref operation: ${JSON.stringify(never)}`);
		}
	}
}

export function feeTierPrefOperationFailure(effect: FeeTierPrefEffect): FeeTierPrefShellResult {
	const operation = effect.operation;
	switch (operation.type) {
		case 'read_stored_tier':
			// An unreadable preference means "the person never chose" — which is
			// the factory `fast`, i.e. exactly what this wallet did before the
			// preference existed. It must never read as a slower tier: a storage
			// hiccup is not a reason to make somebody's transaction cheaper and
			// later than the one they sent yesterday.
			return { type: 'stored_tier', raw: null };
		case 'write_stored_tier':
			// Best effort, as the currency's write always was. What is on screen
			// stays; the next launch simply reads the old value.
			return { type: 'tier_written' };
		default: {
			const never: never = operation;
			throw new Error(`unhandled fee_tier_pref operation: ${JSON.stringify(never)}`);
		}
	}
}
