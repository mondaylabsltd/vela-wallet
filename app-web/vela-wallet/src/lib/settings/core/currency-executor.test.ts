/**
 * The web display_currency executor (spec 024 T038, rate arm live in 025
 * Phase 5): the stored key, the web's null device currency, the rate
 * forwarded from the waterfall (null stays null — never 1), the failure twin.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { CurrencyEffect } from './currency-executor';

const kv = new Map<string, string>();
vi.mock('$lib/services/storage', () => ({
	getItem: vi.fn(async (key: string) => kv.get(key) ?? null),
	setItem: vi.fn(async (key: string, value: string) => void kv.set(key, value)),
	removeItem: vi.fn(async (key: string) => void kv.delete(key))
}));

vi.mock('$lib/services/currency-rate', () => ({
	resolveRate: vi.fn(async (code: string) => (code === 'JPY' ? 155 : null))
}));

import { currencyOperationFailure, executeCurrencyOperation } from './currency-executor';
import { useWalletSyncChannel } from '$lib/wallet/core/wallet-sync';

const effect = (operation: CurrencyEffect['operation']): CurrencyEffect => ({ id: 1, operation });

beforeEach(() => kv.clear());

describe('display_currency executor', () => {
	it('round-trips the stored code under vela.displayCurrency', async () => {
		expect(await executeCurrencyOperation(effect({ type: 'read_stored_code' }))).toEqual({
			type: 'stored_code',
			code: null
		});
		await executeCurrencyOperation(effect({ type: 'write_stored_code', code: 'EUR' }));
		expect(kv.get('vela.displayCurrency')).toBe('EUR');
		expect(await executeCurrencyOperation(effect({ type: 'read_stored_code' }))).toEqual({
			type: 'stored_code',
			code: 'EUR'
		});
	});

	it('says a written choice to the other documents (a tab, the side panel)', async () => {
		const posted: unknown[] = [];
		useWalletSyncChannel({
			postMessage: (m) => posted.push(m),
			addEventListener: () => {},
			removeEventListener: () => {}
		});
		try {
			await executeCurrencyOperation(effect({ type: 'read_stored_code' }));
			expect(posted).toEqual([]);
			await executeCurrencyOperation(effect({ type: 'write_stored_code', code: 'EUR' }));
			expect(posted).toEqual([{ topic: 'currency' }]);
		} finally {
			useWalletSyncChannel(undefined);
		}
	});

	it('the web has no device currency — answered, never skipped', async () => {
		expect(await executeCurrencyOperation(effect({ type: 'read_device_currency' }))).toEqual({
			type: 'device_currency',
			code: null
		});
	});

	it('resolve_rate forwards the waterfall answer; an unpriceable code is null, NOT 1', async () => {
		expect(await executeCurrencyOperation(effect({ type: 'resolve_rate', code: 'JPY' }))).toEqual({
			type: 'rate_resolved',
			code: 'JPY',
			rate: 155
		});
		expect(await executeCurrencyOperation(effect({ type: 'resolve_rate', code: 'XXX' }))).toEqual({
			type: 'rate_resolved',
			code: 'XXX',
			rate: null
		});
	});

	it('the failure twin answers every operation', () => {
		expect(currencyOperationFailure(effect({ type: 'read_stored_code' }))).toEqual({
			type: 'stored_code',
			code: null
		});
		expect(currencyOperationFailure(effect({ type: 'write_stored_code', code: 'EUR' }))).toEqual({
			type: 'code_written'
		});
		expect(currencyOperationFailure(effect({ type: 'resolve_rate', code: 'KRW' }))).toEqual({
			type: 'rate_resolved',
			code: 'KRW',
			rate: null
		});
	});
});
