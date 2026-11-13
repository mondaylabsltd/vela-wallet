/**
 * r5reg-3: the wait before the passkey is bounded and said; a wait given up
 * never opens a passkey after it; once the passkey opens the bound is over.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	BeforePasskeyError,
	boundedUntilPasskey,
	PREPARE_LIMIT_MS,
	PREPARE_SLOW_MS,
	PrepareTimeoutError
} from './prepare-bound';

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

const never = () => new Promise<never>(() => {});

describe('boundedUntilPasskey', () => {
	it('says how long and why past the slow mark, and gives up at the limit', async () => {
		const slow: [number, boolean][] = [];
		let busy = false;
		const run = boundedUntilPasskey(never, {
			busy: () => busy,
			onSlow: (ms, b) => slow.push([ms, b])
		});
		const settled = run.catch((e: unknown) => e);
		await vi.advanceTimersByTimeAsync(PREPARE_SLOW_MS - 1);
		expect(slow).toEqual([]);
		await vi.advanceTimersByTimeAsync(1);
		expect(slow).toEqual([[PREPARE_SLOW_MS, false]]);
		busy = true; // the nodes start answering 429
		await vi.advanceTimersByTimeAsync(3_000);
		expect(slow.at(-1)).toEqual([PREPARE_SLOW_MS + 3_000, true]);
		await vi.advanceTimersByTimeAsync(PREPARE_LIMIT_MS);
		const error = await settled;
		expect(error).toBeInstanceOf(BeforePasskeyError);
		expect((error as BeforePasskeyError).busy).toBe(true);
		expect((error as BeforePasskeyError).cause).toBeInstanceOf(PrepareTimeoutError);
		// Nothing more is said once it gave up.
		const said = slow.length;
		await vi.advanceTimersByTimeAsync(10_000);
		expect(slow).toHaveLength(said);
	});

	it('a passkey never opens after the wait was given up', async () => {
		let gate: () => void = () => {};
		let release: () => void = () => {};
		const opened = vi.fn();
		const run = boundedUntilPasskey(
			async (g) => {
				gate = g;
				await new Promise<void>((r) => (release = r));
				gate();
				opened();
			},
			{ busy: () => false, onSlow: () => {} }
		);
		const settled = run.catch((e: unknown) => e);
		await vi.advanceTimersByTimeAsync(PREPARE_LIMIT_MS);
		expect(await settled).toBeInstanceOf(BeforePasskeyError);
		release(); // the reads answer at last…
		await vi.advanceTimersByTimeAsync(0);
		expect(opened).not.toHaveBeenCalled(); // …and the gate stays shut
		expect(() => gate()).toThrow(PrepareTimeoutError);
	});

	it('once the passkey opens, the bound is over: a long prompt is the prompt’s', async () => {
		const slow = vi.fn();
		const run = boundedUntilPasskey(
			async (gate) => {
				await new Promise((r) => setTimeout(r, 2_000));
				gate();
				await new Promise((r) => setTimeout(r, 200_000)); // the ceremony
				return 'signed';
			},
			{ busy: () => false, onSlow: slow }
		);
		await vi.advanceTimersByTimeAsync(210_000);
		await expect(run).resolves.toBe('signed');
		expect(slow).not.toHaveBeenCalled();
	});

	it('a failure before the passkey is marked as such; after it, or passed through, as thrown', async () => {
		const boom = new Error('nonce read failed');
		await expect(
			boundedUntilPasskey(
				async () => {
					throw boom;
				},
				{ busy: () => false, onSlow: () => {} }
			)
		).rejects.toMatchObject({ name: 'BeforePasskeyError', cause: boom });
		const late = new Error('prompt closed');
		await expect(
			boundedUntilPasskey(
				async (gate) => {
					gate();
					throw late;
				},
				{ busy: () => false, onSlow: () => {} }
			)
		).rejects.toBe(late);
		const refusal = new Error('refused');
		await expect(
			boundedUntilPasskey(
				async () => {
					throw refusal;
				},
				{ busy: () => false, onSlow: () => {}, passThrough: (e) => e === refusal }
			)
		).rejects.toBe(refusal);
	});
});
