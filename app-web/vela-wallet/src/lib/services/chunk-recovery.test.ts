/**
 * A lazy chunk failed to load — and a reload is the answer to exactly one
 * reason it can: the build moved, nobody is leaving, nothing open would be
 * dropped, and this tab has not reloaded away from this build before.
 *
 * The first rule is the one that was missing. The layout reloaded on EVERY
 * `vite:preloadError`, and a page that is being left reports one for each
 * lazy chunk the browser cancels: on Firefox that reload cancelled the very
 * navigation that caused it (`page.reload: NS_BINDING_ABORTED` in three e2e
 * suites, a different test each run).
 */
import { describe, expect, it, vi } from 'vitest';
import {
	RECOVERY_KEY,
	installChunkRecovery,
	reloadRecovers,
	type ChunkRecoveryPorts,
	type PreloadFailureFacts
} from './chunk-recovery';

const STALE: PreloadFailureFacts = {
	leaving: false,
	moved: true,
	version: 'v1',
	recoveredFrom: null,
	busy: false
};

describe('whether a fresh document is the answer', () => {
	it('the build moved, nobody is leaving, nothing is open, never recovered: yes', () => {
		expect(reloadRecovers(STALE)).toBe(true);
	});

	it('the document is being left: never — whatever else is true', () => {
		expect(reloadRecovers({ ...STALE, leaving: true })).toBe(false);
	});

	it('the build did not move — the network dropped the chunk: no', () => {
		expect(reloadRecovers({ ...STALE, moved: false })).toBe(false);
	});

	it('a sheet is open: not now', () => {
		expect(reloadRecovers({ ...STALE, busy: true })).toBe(false);
	});

	it('this tab already reloaded away from THIS build: not again', () => {
		expect(reloadRecovers({ ...STALE, recoveredFrom: 'v1' })).toBe(false);
		// From an earlier build: this is a new one, and a new recovery.
		expect(reloadRecovers({ ...STALE, version: 'v2', recoveredFrom: 'v1' })).toBe(true);
	});

	it('a tab with no memory never reloads by itself: it could not know it was looping', () => {
		expect(reloadRecovers({ ...STALE, recoveredFrom: undefined })).toBe(false);
	});
});

/** A window, a server and a tab's memory, each the test's to move. */
function world(over: Partial<ChunkRecoveryPorts> = {}) {
	const target = new EventTarget();
	const kept = new Map<string, string>();
	const memory = {
		getItem: (key: string) => kept.get(key) ?? null,
		setItem: (key: string, value: string) => void kept.set(key, value)
	};
	const reload = vi.fn();
	const moved = vi.fn(async () => true);
	const ports: ChunkRecoveryPorts = {
		version: 'v1',
		moved,
		busy: () => false,
		reload,
		target: target as unknown as ChunkRecoveryPorts['target'],
		memory,
		...over
	};
	const stop = installChunkRecovery(ports);
	const fire = (type: string) => target.dispatchEvent(new Event(type));
	/** A lazy import failed; and the question it raised has been answered. */
	const fail = async () => {
		fire('vite:preloadError');
		await vi.waitFor(async () => {
			await Promise.resolve();
		});
		// The check is a promise chain: let it run out.
		for (let turn = 0; turn < 5; turn++) await Promise.resolve();
	};
	return { fire, fail, reload, moved, kept, stop };
}

describe('listening for failed lazy imports', () => {
	it('a page being left reports its cancelled imports — and nothing is done, nothing even asked', async () => {
		const w = world();
		// What Firefox does on a reload: the unload is announced, then every
		// lazy chunk in flight is cancelled and reported.
		w.fire('beforeunload');
		for (let chunk = 0; chunk < 4; chunk++) await w.fail();
		expect(w.reload).not.toHaveBeenCalled();
		expect(w.moved).not.toHaveBeenCalled();
		// The same after the page has gone into hiding.
		w.fire('pagehide');
		await w.fail();
		expect(w.reload).not.toHaveBeenCalled();
		expect(w.kept.size).toBe(0);
	});

	it('the build is where it was — a dropped connection: no reload, and nothing remembered', async () => {
		const w = world({ moved: vi.fn(async () => false) });
		await w.fail();
		await w.fail();
		expect(w.reload).not.toHaveBeenCalled();
		expect(w.kept.size).toBe(0);
	});

	it('the build moved: one reload, and the tab remembers which build it left', async () => {
		const w = world();
		await w.fail();
		expect(w.reload).toHaveBeenCalledOnce();
		expect(w.kept.get(RECOVERY_KEY)).toBe('v1');
	});

	it('a burst of failures is one question and one reload', async () => {
		let answer: (moved: boolean) => void = () => {};
		const moved = vi.fn(() => new Promise<boolean>((resolve) => (answer = resolve)));
		const w = world({ moved });
		// A route's chunks fail together, before the server has answered.
		w.fire('vite:preloadError');
		w.fire('vite:preloadError');
		w.fire('vite:preloadError');
		expect(moved).toHaveBeenCalledOnce();
		answer(true);
		await vi.waitFor(() => expect(w.reload).toHaveBeenCalledOnce());
	});

	it('NO LOOP: the document that comes back is still this build, still failing — it does not reload again', async () => {
		// The first document: recovers once.
		const first = world();
		await first.fail();
		expect(first.reload).toHaveBeenCalledOnce();
		// What came back is the same build (a cached shell in front of a moved
		// deployment), with the tab's memory: its chunks fail, the server still
		// says "moved" — and it stays put, however many fail.
		const again = world({
			memory: {
				getItem: (key) => first.kept.get(key) ?? null,
				setItem: (key, value) => void first.kept.set(key, value)
			}
		});
		for (let chunk = 0; chunk < 5; chunk++) await again.fail();
		expect(again.reload).not.toHaveBeenCalled();
		// A NEWER build's document, later, may recover in its turn.
		const newer = world({
			version: 'v2',
			memory: {
				getItem: (key) => first.kept.get(key) ?? null,
				setItem: (key, value) => void first.kept.set(key, value)
			}
		});
		await newer.fail();
		expect(newer.reload).toHaveBeenCalledOnce();
		expect(first.kept.get(RECOVERY_KEY)).toBe('v2');
	});

	it('no memory, no automatic reload: storage refused, or refusing the write', async () => {
		const none = world({ memory: null });
		await none.fail();
		expect(none.reload).not.toHaveBeenCalled();

		const readOnly = world({
			memory: {
				getItem: () => null,
				setItem: () => {
					throw new Error('QuotaExceededError');
				}
			}
		});
		await readOnly.fail();
		expect(readOnly.reload).not.toHaveBeenCalled();

		const throwing = world({
			memory: {
				getItem: () => {
					throw new Error('SecurityError');
				},
				setItem: () => {}
			}
		});
		await throwing.fail();
		expect(throwing.reload).not.toHaveBeenCalled();
	});

	it('a sheet is open: the reload waits — the page is not pulled from under a request', async () => {
		let open = true;
		const w = world({ busy: () => open });
		await w.fail();
		expect(w.reload).not.toHaveBeenCalled();
		expect(w.kept.size).toBe(0);
		// Closed: the next failure recovers.
		open = false;
		await w.fail();
		expect(w.reload).toHaveBeenCalledOnce();
	});

	it('an announced unload that did not happen is over once the person is using the page', async () => {
		const w = world();
		// A download, a `mailto:` link, a leave-prompt answered "stay".
		w.fire('beforeunload');
		await w.fail();
		expect(w.reload).not.toHaveBeenCalled();
		w.fire('pointerdown');
		await w.fail();
		expect(w.reload).toHaveBeenCalledOnce();

		const typed = world();
		typed.fire('beforeunload');
		typed.fire('keydown');
		await typed.fail();
		expect(typed.reload).toHaveBeenCalledOnce();

		// And a page brought back from the back/forward cache is here again.
		const back = world();
		back.fire('pagehide');
		back.fire('pageshow');
		await back.fail();
		expect(back.reload).toHaveBeenCalledOnce();
	});

	it('a version check that cannot be made is "not moved": never a reload on a guess', async () => {
		const w = world({
			moved: vi.fn(async () => {
				throw new Error('NetworkError');
			})
		});
		await w.fail();
		expect(w.reload).not.toHaveBeenCalled();
	});

	it('leaving while the server is being asked: its answer comes too late to act on', async () => {
		let answer: (moved: boolean) => void = () => {};
		const w = world({ moved: () => new Promise<boolean>((resolve) => (answer = resolve)) });
		w.fire('vite:preloadError');
		w.fire('beforeunload');
		answer(true);
		for (let turn = 0; turn < 5; turn++) await Promise.resolve();
		expect(w.reload).not.toHaveBeenCalled();
	});

	it('stopped, it listens to nothing', async () => {
		const w = world();
		w.stop();
		await w.fail();
		expect(w.reload).not.toHaveBeenCalled();
		expect(w.moved).not.toHaveBeenCalled();
	});
});
