/**
 * The speed a submission names on the wire (spec 068's relay contract).
 *
 * The core puts the tier on the quoted fee it hands the submit path, taken
 * from the same estimate as the amount (spec 069), and already filtered to a
 * name the relay accepts. This is only the narrowing the type system needs:
 * the relay refuses the dead `rapid` with -32602, so a value that somehow
 * carried it names nothing — the pre-068 wire — rather than a neighbouring
 * tier the fee was not priced at.
 */
import type { FeeTier } from '$lib/core/generated/FeeTier';

export function wireTier(tier: FeeTier | null | undefined): Exclude<FeeTier, 'rapid'> | undefined {
	return tier == null || tier === 'rapid' ? undefined : tier;
}
