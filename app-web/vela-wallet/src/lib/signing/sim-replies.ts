/**
 * `eth_simulateV1` replies, written out — a node's answer as it comes over
 * the wire, for a drawn board and a test to hand the REAL core
 * (`simOutcome`), so neither writes a verdict by hand.
 *
 * The app's own replies come from a node. It writes one itself: the over-long
 * revert `verdict.ts` hands the core to learn where the core cuts a reason
 * (`longestRevertReason`), so the cap is never restated on this side.
 */

/** keccak256("Transfer(address,address,uint256)"). */
export const TRANSFER_TOPIC = '0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef';

/**
 * The sender of the log a node writes for native value under
 * `traceTransfers` — one of the core's native sentinels.
 */
export const NATIVE_SENTINEL = '0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee';

const word = (value: bigint | number) => BigInt(value).toString(16).padStart(64, '0');
const topic = (address: string) => '0x' + address.slice(2).toLowerCase().padStart(64, '0');

/** One `Transfer(from, to, amount)` log, emitted by `token`. */
export function transferLog(token: string, from: string, to: string, amount: bigint) {
	return {
		address: token.toLowerCase(),
		topics: [TRANSFER_TOPIC, topic(from), topic(to)],
		data: '0x' + word(amount)
	};
}

/** Native value moving `from` → `to`, as a node's trace reports it. */
export function nativeLog(from: string, to: string, amount: bigint) {
	return transferLog(NATIVE_SENTINEL, from, to, amount);
}

/** Solidity's `Error(string)` revert payload for `reason`. */
export function errorString(reason: string): string {
	const bytes = new TextEncoder().encode(reason);
	const text = [...bytes].map((byte) => byte.toString(16).padStart(2, '0')).join('');
	return (
		'0x08c379a0' +
		word(32) +
		word(bytes.length) +
		text.padEnd(Math.ceil(text.length / 64) * 64, '0')
	);
}

/** The JSON-RPC `result` of a run: one simulated block carrying `calls`. */
export function simulatedBlock(...calls: object[]): object[] {
	return [{ calls }];
}

/** A whole reply envelope, as text: every call of one simulated block. */
export function answered(...calls: object[]): string {
	return JSON.stringify({ result: simulatedBlock(...calls) });
}

/** A run whose calls all succeeded, emitting `logs`. */
export function ran(...logs: object[]): string {
	return answered({ status: '0x1', logs });
}

/** A run whose call fails — with `reason` as its `Error(string)`, or with none. */
export function reverted(reason?: string): string {
	return answered({
		status: '0x0',
		...(reason === undefined ? {} : { returnData: errorString(reason) }),
		error: { code: 3, message: 'execution reverted' }
	});
}

/** A node that does not offer the method. */
export const NOT_OFFERED = JSON.stringify({
	error: { code: -32601, message: 'the method eth_simulateV1 does not exist/is not available' }
});

/** No node answered (the pool's wire form for it). */
export const UNREACHABLE = JSON.stringify({ unreachable: true });
