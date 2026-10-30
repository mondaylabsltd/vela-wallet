/**
 * r4b-13: an address nobody knows — no record, no code, no transactions —
 * took ~20 s to answer (its record asked for four more times, 5 s apart).
 * It answers in a few seconds now: every network read at once, each one
 * bounded, the record asked for once more. A network that does not answer
 * in time is no "no": the answer is then "couldn't check", never "none".
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';

const seams = vi.hoisted(() => ({
	/** Chains whose code read never answers. */
	hanging: [] as number[],
	genesisReads: 0
}));
vi.mock('./store.svelte', () => ({ multisig: { find: () => undefined } }));
vi.mock('./authority', () => ({}));
vi.mock('./chain', () => ({
	hasCode: (chainId: number) =>
		seams.hanging.includes(chainId) ? new Promise(() => {}) : Promise.resolve(false)
}));
vi.mock('./safe-service', () => ({}));
vi.mock('./owners', () => ({}));
vi.mock('./service', () => ({
	ServiceUnavailableError: class extends Error {},
	fetchGenesis: async () => {
		seams.genesisReads += 1;
		return { found: false };
	},
	servedChains: async () => null
}));
vi.mock('$lib/services/networks', () => ({ networkForChainId: () => ({}) }));
vi.mock('$lib/services/rpc-pool', () => ({ poolRpcCall: async () => ({ result: '0x0' }) }));

const { findMultisig, LOOKUP_READ_MS } = await import('./accounts');

/** Seconds until `promise` settles, on fake time. */
async function secondsTo<T>(promise: Promise<T>): Promise<{ s: number; value: T }> {
	const start = Date.now();
	let done = false;
	let value: T | undefined;
	void promise.then((v) => {
		done = true;
		value = v;
	});
	while (!done) await vi.advanceTimersByTimeAsync(100);
	return { s: (Date.now() - start) / 1000, value: value as T };
}

beforeEach(() => {
	vi.useFakeTimers();
	seams.hanging = [];
	seams.genesisReads = 0;
});
afterEach(() => vi.useRealTimers());

describe('an unknown address (r4b-13)', () => {
	it('is "not found" within a few seconds — its record asked for once more, not four times', async () => {
		const { s, value } = await secondsTo(findMultisig(SAFE, [100, 5042002, 11155111]));
		expect(value).toEqual({ ok: false, reason: 'notFound' });
		// Was 4 × 5 s = 20 s of waits; now one 2 s wait.
		expect(s).toBeLessThanOrEqual(3);
		expect(seams.genesisReads).toBe(2);
	});

	it('a network that never answers is bounded, and said as "couldn’t check" — never "none"', async () => {
		seams.hanging = [5042002];
		const { s, value } = await secondsTo(findMultisig(SAFE, [100, 5042002, 11155111]));
		expect(value).toEqual({ ok: false, reason: 'unchecked' });
		// The bound, with the record's one more ask beside it: about four seconds, not forever.
		expect(s).toBeLessThanOrEqual(LOOKUP_READ_MS / 1000 + 0.5);
	});
});
