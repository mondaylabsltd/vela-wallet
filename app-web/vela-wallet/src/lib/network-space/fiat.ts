/**
 * r18-9 (spec 105, r17-13): the testnet space shows NO fiat value anywhere —
 * no total, no worth on a row, no fiat beside a Send amount, none beside a
 * fee, none on a signing sheet or a detail. Nothing there has real value,
 * and a figure beside a test coin (a "stablecoin" priced at 1, a relay's
 * published fee price) says it has. The rule is the core's
 * (`network_space::shows_fiat`, wasm `networkSpaceShowsFiat`); every money
 * surface asks it here.
 */
import { networkSpaceShowsFiat } from '$lib/core/client';
import { networkSpace, type NetworkSpace } from './network-space.svelte';

/**
 * Whether `space` (default: the one in front on this device) shows fiat.
 * Before the core has loaded — a first frame — the stored value is read the
 * way `networkSpace` reads it: the testnet space shows none.
 */
export function showsFiat(space: NetworkSpace = networkSpace.current): boolean {
	try {
		return networkSpaceShowsFiat(space);
	} catch {
		return space !== 'testnet';
	}
}

/** A price (or any fiat figure) in `space`: itself where fiat is shown, else `null`. */
export function pricedIn<T extends number | string>(
	price: T | null | undefined,
	space?: NetworkSpace
): T | null {
	return price === null || price === undefined || !showsFiat(space) ? null : price;
}
