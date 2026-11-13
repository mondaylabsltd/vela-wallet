// Round 3 (send-15): one send at a time per network and account across tabs.
import { afterEach, describe, expect, it, vi } from 'vitest';

import { acquireSendLock, releaseWhen, sendLockName } from './send-lock';

/** A LockManager as small as the tests need: exclusive locks, FIFO waiters. */
function fakeLocks() {
	const held = new Set<string>();
	const waiting = new Map<string, Array<() => void>>();
	const run = async (name: string, callback: (lock: unknown) => unknown) => {
		held.add(name);
		try {
			return await callback({ name });
		} finally {
			held.delete(name);
			waiting.get(name)?.shift()?.();
		}
	};
	return {
		held,
		request(
			name: string,
			options: { ifAvailable?: boolean; signal?: AbortSignal },
			callback: (lock: unknown) => unknown
		): Promise<unknown> {
			if (options.ifAvailable && held.has(name)) return Promise.resolve(callback(null));
			if (!held.has(name)) return run(name, callback);
			return new Promise((resolve, reject) => {
				const queue = waiting.get(name) ?? [];
				queue.push(() => resolve(run(name, callback)));
				waiting.set(name, queue);
				options.signal?.addEventListener('abort', () => reject(options.signal?.reason), {
					once: true
				});
			});
		}
	};
}

afterEach(() => {
	vi.unstubAllGlobals();
});

describe('the send lock (round 3, send-15)', () => {
	const NAME = sendLockName(100, '0xAbC0000000000000000000000000000000000001');

	it('is one per network and account, whatever the case of the address', () => {
		expect(NAME).toBe('vela.send:100:0xabc0000000000000000000000000000000000001');
		expect(sendLockName(1, '0xABC0000000000000000000000000000000000001')).not.toBe(NAME);
	});

	it('without Web Locks there is no lock, and the send goes on as before', async () => {
		vi.stubGlobal('navigator', {});
		await expect(acquireSendLock(NAME)).resolves.toBeNull();
	});

	it('a second tab waits its turn, says so once, and goes when the first lets go', async () => {
		const locks = fakeLocks();
		vi.stubGlobal('navigator', { locks });
		const first = await acquireSendLock(NAME);
		expect(first).toBeTypeOf('function');
		const queued = vi.fn();
		let second: (() => void) | null = null;
		const waiting = acquireSendLock(NAME, { onQueued: queued }).then((release) => {
			second = release;
			return release;
		});
		await vi.waitFor(() => expect(queued).toHaveBeenCalledTimes(1));
		await new Promise((resolve) => setTimeout(resolve, 10));
		expect(second).toBeNull();
		first?.();
		await expect(waiting).resolves.toBeTypeOf('function');
		expect(queued).toHaveBeenCalledTimes(1);
		expect(locks.held.has(NAME)).toBe(true);
		(second as unknown as () => void)();
		await vi.waitFor(() => expect(locks.held.has(NAME)).toBe(false));
	});

	it('a wait the send gave up rejects, and holds nothing', async () => {
		const locks = fakeLocks();
		vi.stubGlobal('navigator', { locks });
		const first = await acquireSendLock(NAME);
		const abort = new AbortController();
		const waiting = acquireSendLock(NAME, { signal: abort.signal });
		await new Promise((resolve) => setTimeout(resolve, 0));
		abort.abort(new DOMException('gone', 'AbortError'));
		await expect(waiting).rejects.toThrow('gone');
		first?.();
	});

	it('an operation that went out keeps it until it is settled, or the hold runs out', async () => {
		vi.useFakeTimers();
		try {
			let settled = false;
			const listeners = new Set<() => void>();
			const subscribe = (listener: () => void) => {
				listeners.add(listener);
				return () => listeners.delete(listener);
			};
			const release = vi.fn();
			releaseWhen(release, () => settled, subscribe, 1_000);
			for (const listener of listeners) listener();
			expect(release).not.toHaveBeenCalled();
			settled = true;
			for (const listener of [...listeners]) listener();
			expect(release).toHaveBeenCalledTimes(1);
			expect(listeners.size).toBe(0);

			const late = vi.fn();
			releaseWhen(late, () => false, subscribe, 1_000);
			vi.advanceTimersByTime(1_000);
			expect(late).toHaveBeenCalledTimes(1);

			const already = vi.fn();
			releaseWhen(already, () => true, subscribe, 1_000);
			expect(already).toHaveBeenCalledTimes(1);
		} finally {
			vi.useRealTimers();
		}
	});
});
