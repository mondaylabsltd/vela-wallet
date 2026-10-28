/**
 * A multisig whose record has not landed is asked about again while a page
 * is up (ms-7) — when the browser is back online, when the page is shown
 * again, and on an interval — not only when the wallet page mounts.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const accounts = [{ address: '0x01', recorded: false }];
vi.mock('./store.svelte', () => ({ multisig: { accounts } }));
vi.mock('./chain', () => ({ hasCode: async () => false }));
vi.mock('./safe-service', () => ({}));
vi.mock('./kernel', () => ({}));
vi.mock('./owners', () => ({}));
vi.mock('./service', () => ({ ServiceUnavailableError: class extends Error {} }));

const win = new EventTarget();
const doc = Object.assign(new EventTarget(), { visibilityState: 'visible' });
vi.stubGlobal('window', win);
vi.stubGlobal('document', doc);

const { followUnrecorded, RECORD_RETRY_MS } = await import('./accounts');

beforeEach(() => {
	vi.useFakeTimers();
	accounts[0].recorded = false;
});
afterEach(() => vi.useRealTimers());

describe('a record that has not landed', () => {
	it('is retried at once, online, on focus, when shown, and on an interval — one at a time', async () => {
		let calls = 0;
		let finish: () => void = () => {};
		const retry = () => {
			calls += 1;
			return new Promise<void>((done) => (finish = done));
		};
		const stop = followUnrecorded(retry);
		expect(calls).toBe(1);
		// Still asking: a second event does not start a second retry.
		win.dispatchEvent(new Event('online'));
		expect(calls).toBe(1);
		finish();
		await vi.advanceTimersByTimeAsync(0);

		win.dispatchEvent(new Event('online'));
		expect(calls).toBe(2);
		finish();
		await vi.advanceTimersByTimeAsync(0);

		win.dispatchEvent(new Event('focus'));
		expect(calls).toBe(3);
		finish();
		await vi.advanceTimersByTimeAsync(0);

		doc.dispatchEvent(new Event('visibilitychange'));
		expect(calls).toBe(4);
		finish();
		await vi.advanceTimersByTimeAsync(RECORD_RETRY_MS);
		expect(calls).toBe(5);
		finish();
		await vi.advanceTimersByTimeAsync(0);

		stop();
		win.dispatchEvent(new Event('online'));
		await vi.advanceTimersByTimeAsync(RECORD_RETRY_MS);
		expect(calls).toBe(5);
	});

	it('is not asked about once every record has landed', async () => {
		accounts[0].recorded = true;
		const retry = vi.fn(async () => {});
		const stop = followUnrecorded(retry);
		win.dispatchEvent(new Event('online'));
		await vi.advanceTimersByTimeAsync(RECORD_RETRY_MS);
		stop();
		expect(retry).not.toHaveBeenCalled();
	});
});
