/**
 * The amount codec — fund-safety-critical, so it lives alone and is pinned.
 *
 * Ported from src/services/wallet-state-core/send-types.ts:170-195 @ f9bcb278.
 * The core states base units as DECIMAL strings; every consumer in
 * `safe-transaction.ts` reads `value` as HEX. Getting this wrong does not fail
 * loudly — it signs a different number.
 */
import type { FeeCall } from '#lib/core/generated/FeeCall.ts';

export function toShellCall(call: FeeCall): { to: string; value: string; data: string } {
	return { to: call.to, value: decimalToHex(call.value), data: call.data };
}

/** A decimal base-unit string as `0x…`. An unparsable value is `0x0`, never NaN. */
export function decimalToHex(value: string): string {
	try {
		return `0x${BigInt(value.trim() || '0').toString(16)}`;
	} catch {
		return '0x0';
	}
}

/** A decimal wire amount back to a bigint; a malformed one reads as `0n`. */
export function fromWireAmount(value: string): bigint {
	try {
		return BigInt(value.trim() || '0');
	} catch {
		return 0n;
	}
}
