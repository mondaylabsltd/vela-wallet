/**
 * EIP-55 for display: the core's `checksumAddress`, the one rule every
 * surface uses. The /connect page loads the core before it shows any address
 * (`boot()` awaits `loadCore()` before a member or an account exists), so
 * this keeps no keccak of its own. Should the core not be aboard, the address
 * comes back as given: its letter case is lost, never its value.
 */
import { checksumAddress } from '$lib/core/kernels';

/** `address` in its checksummed case, or as given when it is not an address. */
export function eip55(address: string): string {
	if (!/^0x[0-9a-fA-F]{40}$/.test(address)) return address;
	try {
		return checksumAddress(address);
	} catch {
		return address;
	}
}
