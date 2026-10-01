/**
 * The worker's log (spec 082 RB14, contract §15, FR-018/019).
 *
 * The ring is evidence on the device (quickstart EX-LOG), so it must hold the
 * right lines — and it must never hold what a log may not: request params,
 * results, signatures, addresses, or a URL's path and query.
 */
import { describe, expect, it } from 'vitest';
import {
	SW_COUNTS_KEY,
	SW_LOG_CAP,
	SW_LOG_KEY,
	appendLine,
	counterKey,
	createSwLog,
	formatLine,
	hostOf
} from '../../../extension/lib/swlog.js';

const ADDRESS = '0x7687C0bC1dD2B9d7e9a5b1b4e1B0cBd8e0C3D141';
const NOW = Date.UTC(2026, 8, 29, 1, 2, 3);

function memoryStorage() {
	const data: Record<string, unknown> = {};
	return {
		data,
		async get(keys: string[]) {
			const out: Record<string, unknown> = {};
			for (const key of keys) if (key in data) out[key] = data[key];
			return out;
		},
		async set(items: Record<string, unknown>) {
			Object.assign(data, items);
		}
	};
}

describe('one line', () => {
	it('is `[vela-sw] <iso> <event> k=v`', () => {
		expect(formatLine('req.settled', { cause: 'page_left', tab: 7 }, NOW)).toBe(
			'[vela-sw] 2026-09-29T01:02:03.000Z req.settled cause=page_left tab=7'
		);
	});

	it('writes only the fixed events', () => {
		expect(formatLine('req.params', { method: 'personal_sign' }, NOW)).toBeNull();
	});

	it('drops an address, a hash or a signature wherever it is passed', () => {
		const line = formatLine(
			'req.answered',
			{ result: ADDRESS, hash: `0x${'ab'.repeat(32)}`, sig: `0x${'cd'.repeat(65)}`, tab: 2 },
			NOW
		);
		expect(line).toBe('[vela-sw] 2026-09-29T01:02:03.000Z req.answered tab=2');
	});

	it('keeps an endpoint’s host and never its path, query or key', () => {
		const line = formatLine(
			'read.fail',
			{ host: 'https://rpc.example.org/v2/SECRETKEY?apikey=abc', kind: 'timeout' },
			NOW
		);
		expect(line).toContain('host=rpc.example.org');
		expect(line).not.toMatch(/SECRETKEY|apikey|\/v2/);
	});

	it('drops any other value that carries a path or free text', () => {
		const line = formatLine('req.arrived', { origin: 'https://a.example/path', note: 'a b' }, NOW);
		expect(line).toBe('[vela-sw] 2026-09-29T01:02:03.000Z req.arrived');
	});

	it('reads a host from a bare host too', () => {
		expect(hostOf('rpc.gnosischain.com')).toBe('rpc.gnosischain.com');
		expect(hostOf('not a host/at all')).toBeNull();
	});
});

describe('the ring and the counters', () => {
	it('keeps the last 200 lines', () => {
		let ring: string[] = [];
		for (let i = 0; i < SW_LOG_CAP + 25; i += 1) ring = appendLine(ring, `line ${i}`);
		expect(ring).toHaveLength(SW_LOG_CAP);
		expect(ring[0]).toBe('line 25');
		expect(ring.at(-1)).toBe(`line ${SW_LOG_CAP + 24}`);
	});

	it('counts `<event>.<cause>`', () => {
		expect(counterKey('req.settled', { cause: 'page_left' })).toBe('req.settled.page_left');
		expect(counterKey('read.fail', { kind: 'timeout' })).toBe('read.fail.timeout');
		expect(counterKey('sw.start', {})).toBe('sw.start');
	});

	it('writes the ring and the counters to storage, one line after another', async () => {
		const storage = memoryStorage();
		const lines: string[] = [];
		const swlog = createSwLog({
			storage,
			sink: { info: (line: string) => lines.push(line) },
			now: () => NOW
		});
		void swlog.log('req.settled', { cause: 'page_left' });
		void swlog.log('req.settled', { cause: 'page_left' });
		void swlog.log('req.claim', { phase: 'sign', live: true, account: ADDRESS });
		await swlog.flush();
		expect(lines).toHaveLength(3);
		expect(storage.data[SW_LOG_KEY]).toHaveLength(3);
		expect(storage.data[SW_COUNTS_KEY]).toEqual({ 'req.settled.page_left': 2, 'req.claim': 1 });
		expect(JSON.stringify(storage.data)).not.toContain(ADDRESS);
	});

	it('never throws into the worker when storage refuses', async () => {
		const swlog = createSwLog({
			storage: {
				get: async () => {
					throw new Error('denied');
				},
				set: async () => {}
			},
			sink: { info: () => {} },
			now: () => NOW
		});
		await expect(swlog.log('sw.start', { records: 0 })).resolves.toBeUndefined();
	});
});
