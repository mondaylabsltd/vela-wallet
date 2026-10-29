/**
 * r4b-1, r4b-7: an execution sent from this device, followed to its end —
 * the facts gathered here, the verdict the core's (`executionVerdict`, the
 * real core): the race's loser told within one round, an op the relay
 * refused said as not sent, the watch always bounded, and what is in flight
 * kept across a reload.
 */
import { readFileSync } from 'node:fs';
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { initSync } from '../../../../../../rust/pkg-web/vela_core.js';
import { WASM_URL } from '../../../../../../rust/pkg-web/vela_core_wasm_url.js';

const storage = new Map<string, string>();
vi.stubGlobal('localStorage', {
	getItem: (k: string) => storage.get(k) ?? null,
	setItem: (k: string, v: string) => void storage.set(k, v),
	removeItem: (k: string) => void storage.delete(k)
});

const { EXECUTION_LIMIT_MS, executions, judgeExecution, watchExecution } =
	await import('./execution-watch.svelte');

const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const HASH = `0x${'ab'.repeat(32)}`;
const THEIRS = `0x${'77'.repeat(32)}`;
const OURS = `0x${'88'.repeat(32)}`;

const flight = (over: Record<string, unknown> = {}) => ({
	chainId: 5042002,
	multisig: SAFE,
	safeTxHash: HASH,
	nonce: '4',
	userOpHash: '0x01',
	by: '0x00000000000000000000000000000000000000aa',
	maybeSent: false,
	startedMs: 1_000,
	createdMs: 500,
	...over
});

/** The chain and the relay, as each test sets them. */
function ports(over: Partial<Record<string, unknown>> = {}) {
	return {
		status: async () => ({ status: 'submitted' as const }),
		nonce: async () => 4n,
		lookup: async () => null,
		carries: async () => false,
		now: () => 3_000,
		...over
	} as never;
}

// The real core is compiled once for this file.
vi.setConfig({ testTimeout: 30_000 });
beforeAll(() => {
	initSync({ module: readFileSync(`../../assets/wasm${WASM_URL}`) });
});

beforeEach(() => {
	storage.clear();
	executions.resetForTests();
});

describe('what an execution in flight came to', () => {
	it('another member executed it first: said within one round, nothing paid here', async () => {
		const verdict = await judgeExecution(
			flight(),
			ports({ nonce: async () => 5n, lookup: async () => THEIRS })
		);
		expect(verdict).toEqual({ kind: 'executedByOther', txHash: THEIRS, paid: false });
	});

	it('its execution carried this device’s own operation: executed here', async () => {
		const verdict = await judgeExecution(
			flight(),
			ports({ nonce: async () => 5n, lookup: async () => OURS, carries: async () => true })
		);
		expect(verdict).toEqual({ kind: 'executed', txHash: OURS });
	});

	it('the relay says it landed, and the chain agrees: executed here', async () => {
		const verdict = await judgeExecution(
			flight(),
			ports({
				status: async () => ({ status: 'included', txHash: OURS }),
				nonce: async () => 5n,
				lookup: async () => OURS
			})
		);
		expect(verdict).toEqual({ kind: 'executed', txHash: OURS });
	});

	it('its number used by another transaction: replaced', async () => {
		const verdict = await judgeExecution(flight(), ports({ nonce: async () => 5n }));
		expect(verdict).toEqual({ kind: 'replaced', paid: false });
	});

	it('its number used and the history unreadable: unknown, never a guess', async () => {
		const verdict = await judgeExecution(
			flight(),
			ports({ nonce: async () => 5n, lookup: async () => undefined })
		);
		expect(verdict).toEqual({ kind: 'unknown' });
	});

	it('a lost reply the relay never heard of, for long enough: not sent', async () => {
		const verdict = await judgeExecution(
			flight({ maybeSent: true }),
			ports({ status: async () => ({ status: 'not_found' }), now: () => 20_000 })
		);
		expect(verdict).toEqual({ kind: 'notSent' });
	});
});

describe('the watch', () => {
	it('is bounded: past its time it says so, never "Executing…" without end', async () => {
		let now = 1_000;
		const verdict = await watchExecution(flight(), {
			signal: new AbortController().signal,
			pauseMs: 0,
			ports: ports({
				now: () => (now += EXECUTION_LIMIT_MS / 4)
			})
		});
		expect(verdict).toEqual({ kind: 'timedOut' });
	});

	it('stops when the sheet goes', async () => {
		const stop = new AbortController();
		const watching = watchExecution(flight(), {
			signal: stop.signal,
			pauseMs: 10_000,
			ports: ports()
		});
		stop.abort();
		expect(await watching).toEqual({ kind: 'waiting' });
	});
});

describe('what is in flight is kept (r4b-7)', () => {
	it('across a reload, until its end is known — and not past its time', async () => {
		executions.start(flight({ startedMs: Date.now() }));
		// A reload: a new page reads what was kept.
		vi.resetModules();
		const again = (await import('./execution-watch.svelte')).executions;
		expect(again.get(5042002, SAFE.toLowerCase(), HASH)?.userOpHash).toBe('0x01');
		again.end(5042002, SAFE, HASH);
		expect(again.get(5042002, SAFE, HASH)).toBeNull();
		// One far past its watch may no longer land: not held against it.
		again.start(flight({ startedMs: Date.now() - EXECUTION_LIMIT_MS - 120_000 }));
		expect(again.get(5042002, SAFE, HASH)).toBeNull();
	});
});
