/**
 * A Multicall3 that answers PER CALL (spec 028 T445).
 *
 * The 025/026 suites answer every `aggregate3` with N copies of one blob,
 * which is enough when every slot decodes the same two words. A sweep needs
 * two tokens with two balances, and an add-token probe needs `name()` and
 * `symbol()` to come back as STRINGS — so this walks the calldata, finds each
 * inner call's target and selector, and lets the test answer each one.
 *
 * Layout (the inverse of the app's `encAggregate3`): selector, head offset,
 * array length, N element offsets (relative to the elements area), then each
 * element as [target][allowFailure][bytes offset = 0x60][bytes length][data].
 */
import { encodeAggregate3Result } from './stub-chain';

export interface InnerCall {
	target: string;
	selector: string;
	/** The call's own calldata, `0x`-less, selector included. */
	data: string;
}

const SEL = {
	aggregate3: '82ad56cb',
	getEthBalance: '4d2301cc',
	balanceOf: '70a08231',
	decimals: '313ce567',
	symbol: '95d89b41',
	name: '06fdde03',
	latestRoundData: 'feaf968c'
} as const;

export { SEL as MULTICALL_SEL };

/**
 * Whether this `eth_call` is an `aggregate3` at all. A plain call — the Safe's
 * `nonce()`, a `balanceOf` outside the batch — expects ONE word back, and an
 * aggregate3-shaped answer parses as a uint too large for any target type.
 */
export function isAggregate3(calldata: string): boolean {
	const hex = calldata.startsWith('0x') ? calldata.slice(2) : calldata;
	return hex.startsWith(SEL.aggregate3);
}

/** Every `Call3` inside an `aggregate3` calldata, in order. */
export function aggregate3Calls(calldata: string): InnerCall[] {
	const hex = calldata.startsWith('0x') ? calldata.slice(2) : calldata;
	if (!hex.startsWith(SEL.aggregate3)) return [];
	const word = (pos: number) => parseInt(hex.slice(pos, pos + 64), 16);
	const headOffset = word(8) * 2;
	const lengthPos = 8 + headOffset;
	const count = word(lengthPos);
	const elementsStart = lengthPos + 64;
	const calls: InnerCall[] = [];
	for (let i = 0; i < count; i++) {
		const elementPos = elementsStart + word(elementsStart + i * 64) * 2;
		const target = '0x' + hex.slice(elementPos + 24, elementPos + 64);
		const bytesPos = elementPos + word(elementPos + 128) * 2;
		const length = word(bytesPos);
		const data = hex.slice(bytesPos + 64, bytesPos + 64 + length * 2);
		calls.push({ target: target.toLowerCase(), selector: data.slice(0, 8), data });
	}
	return calls;
}

/** A 32-byte word. */
export function word(n: number | bigint): string {
	return BigInt(n).toString(16).padStart(64, '0');
}

/** ABI-encode a `string` return: offset, length, right-padded bytes. */
export function abiString(text: string): string {
	const bytes = new TextEncoder().encode(text);
	const hex = [...bytes].map((b) => b.toString(16).padStart(2, '0')).join('');
	return '0x' + word(0x20) + word(bytes.length) + hex.padEnd(Math.ceil(hex.length / 64) * 64, '0');
}

/**
 * Chainlink `latestRoundData()`: (roundId, answer, startedAt, updatedAt,
 * answeredInRound). The app reads `answer` (8 decimals) from word 1.
 */
export function roundData(usd8dp: bigint): string {
	return '0x' + word(1) + word(usd8dp) + word(0) + word(0) + word(0);
}

export type CallAnswer = { success: true; data: string } | { success: false };

/**
 * Build one `aggregate3` result from a per-call answerer. A call the
 * answerer declines (returns `undefined`) fails, which is what a contract
 * that does not exist does — and is exactly what the price ladder and the
 * add-token probe are built to survive.
 */
export function answerAggregate3(
	calldata: string,
	answer: (call: InnerCall) => CallAnswer | undefined
): string {
	return encodeAggregate3Result(
		aggregate3Calls(calldata).map((call) => {
			const reply = answer(call);
			return reply?.success ? { success: true, data: reply.data } : { success: false, data: '0x' };
		})
	);
}
