/**
 * Round 6: after a send failed as the network's, the confirm opens by itself
 * once the network answers again — asked by bounded reads, at once when the
 * browser is online again, one read at a time, and told once.
 */
import { describe, expect, it } from 'vitest';
import {
	RECOVERY_EVERY_MS,
	RECOVERY_READ_MS,
	watchChainRecovery,
	type RecoveryDeps
} from './chain-recovery';

/** A clock and a network the test runs by hand. */
function harness() {
	let now = 0;
	const timers = new Map<number, { at: number; run: () => void }>();
	let id = 0;
	const reads: { chainId: number; answer: (ok: boolean) => void }[] = [];
	const online = new Set<() => void>();
	const deps: RecoveryDeps = {
		read: (chainId) =>
			new Promise<void>((resolve, reject) => {
				reads.push({ chainId, answer: (ok) => (ok ? resolve() : reject(new Error('down'))) });
			}),
		setTimeout: (run, ms) => {
			timers.set(++id, { at: now + ms, run });
			return id;
		},
		clearTimeout: (handle) => void timers.delete(handle as number),
		onOnline: (run) => {
			online.add(run);
			return () => online.delete(run);
		}
	};
	const flush = () => new Promise((r) => setTimeout(r, 0));
	return {
		deps,
		reads,
		online,
		timers,
		async advance(ms: number) {
			now += ms;
			for (const [key, timer] of [...timers].sort((a, b) => a[1].at - b[1].at)) {
				if (timer.at <= now && timers.has(key)) {
					timers.delete(key);
					timer.run();
				}
			}
			await flush();
		},
		flush
	};
}

describe('the network answering again (round 6)', () => {
	it('asks the form network every few seconds until it answers, then says so once', async () => {
		const h = harness();
		const told: number[] = [];
		watchChainRecovery(5042002, (chainId) => told.push(chainId), h.deps);
		expect(h.reads).toHaveLength(0);
		await h.advance(RECOVERY_EVERY_MS);
		expect(h.reads.map((r) => r.chainId)).toEqual([5042002]);
		h.reads[0].answer(false);
		await h.flush();
		expect(told).toEqual([]);
		await h.advance(RECOVERY_EVERY_MS);
		expect(h.reads).toHaveLength(2);
		h.reads[1].answer(true);
		await h.flush();
		expect(told).toEqual([5042002]);
		// Done: nothing more is asked, and "online" asks nothing.
		await h.advance(RECOVERY_EVERY_MS * 3);
		expect(h.reads).toHaveLength(2);
		expect(h.online.size).toBe(0);
	});

	it('a read that does not answer in time is no answer, and the next one goes', async () => {
		const h = harness();
		const told: number[] = [];
		watchChainRecovery(1, (chainId) => told.push(chainId), h.deps);
		await h.advance(RECOVERY_EVERY_MS);
		expect(h.reads).toHaveLength(1);
		await h.advance(RECOVERY_READ_MS);
		await h.advance(RECOVERY_EVERY_MS);
		expect(h.reads).toHaveLength(2);
		// The hung read answering late counts for nothing; the live one decides.
		h.reads[0].answer(true);
		await h.flush();
		expect(told).toEqual([]);
		h.reads[1].answer(true);
		await h.flush();
		expect(told).toEqual([1]);
	});

	it('asks at once when the browser is online again — and online alone opens nothing', async () => {
		const h = harness();
		const told: number[] = [];
		watchChainRecovery(1, (chainId) => told.push(chainId), h.deps);
		for (const run of h.online) run();
		await h.flush();
		expect(h.reads).toHaveLength(1);
		expect(told).toEqual([]);
		h.reads[0].answer(true);
		await h.flush();
		expect(told).toEqual([1]);
	});

	it('stopped (the line went another way): nothing more is asked or told', async () => {
		const h = harness();
		const told: number[] = [];
		const stop = watchChainRecovery(1, (chainId) => told.push(chainId), h.deps);
		await h.advance(RECOVERY_EVERY_MS);
		stop();
		h.reads[0].answer(true);
		await h.flush();
		await h.advance(RECOVERY_EVERY_MS * 2);
		expect(told).toEqual([]);
		expect(h.reads).toHaveLength(1);
		expect(h.timers.size).toBe(0);
	});
});
