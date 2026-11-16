import { beforeEach, describe, expect, it } from 'vitest';
import { CONNECT_OPS_KEY, CONNECT_OP_TTL_MS, batchIdOf, findOp, rememberOp } from './ops';

class MemoryStorage {
	private items = new Map<string, string>();
	get length() {
		return this.items.size;
	}
	key(i: number) {
		return [...this.items.keys()][i] ?? null;
	}
	getItem(k: string) {
		return this.items.get(k) ?? null;
	}
	setItem(k: string, v: string) {
		this.items.set(k, v);
	}
	removeItem(k: string) {
		this.items.delete(k);
	}
	clear() {
		this.items.clear();
	}
}

const ID = '0x' + 'ab'.repeat(32);
const SITE = 'https://sub3.dev';

beforeEach(() => {
	(globalThis as { localStorage?: unknown }).localStorage = new MemoryStorage();
});

describe('the batches a site was answered with', () => {
	it('finds a batch for the site that sent it, whatever the id’s case', () => {
		rememberOp(ID.toUpperCase().replace('0X', '0x'), 5042002, SITE, 1_000);
		expect(findOp(ID, SITE, 2_000)).toEqual({ chainId: 5042002, origin: SITE, at: 1_000 });
	});

	it('never tells another site about it', () => {
		rememberOp(ID, 5042002, SITE, 1_000);
		expect(findOp(ID, 'https://evil.example', 2_000)).toBeNull();
	});

	it('forgets it after a day', () => {
		rememberOp(ID, 1, SITE, 1_000);
		expect(findOp(ID, SITE, 1_000 + CONNECT_OP_TTL_MS)).toBeNull();
	});

	it('survives a storage it cannot read', () => {
		localStorage.setItem(CONNECT_OPS_KEY, '{not json');
		expect(findOp(ID, SITE)).toBeNull();
		rememberOp(ID, 1, SITE, 5);
		expect(findOp(ID, SITE, 6)?.chainId).toBe(1);
	});
});

describe('batchIdOf', () => {
	it('reads both shapes EIP-5792 answers with', () => {
		expect(batchIdOf(ID)).toBe(ID);
		expect(batchIdOf({ id: ID })).toBe(ID);
		expect(batchIdOf({ hash: ID })).toBeNull();
		expect(batchIdOf(null)).toBeNull();
	});
});
