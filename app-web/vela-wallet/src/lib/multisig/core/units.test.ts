import { describe, expect, it } from 'vitest';
import { formatUnits, parseUnits, plainUnits } from './units';

describe('amounts', () => {
	it("reads what a person types, in the token's units", () => {
		expect(parseUnits('1.5', 18)).toBe(1_500_000_000_000_000_000n);
		expect(parseUnits('0.000001', 6)).toBe(1n);
		expect(parseUnits('1,000.25', 6)).toBe(1_000_250_000n);
		expect(parseUnits('.5', 2)).toBe(50n);
	});

	it('refuses what is not an amount, or finer than the token', () => {
		expect(parseUnits('', 18)).toBeNull();
		expect(parseUnits('.', 18)).toBeNull();
		expect(parseUnits('1.2.3', 18)).toBeNull();
		expect(parseUnits('-1', 18)).toBeNull();
		expect(parseUnits('0.0000001', 6)).toBeNull();
		expect(parseUnits('1e18', 18)).toBeNull();
	});

	it('shows amounts grouped, with at most six decimals', () => {
		expect(formatUnits(1_500_000_000_000_000_000n, 18)).toBe('1.5');
		expect(formatUnits('1234567000000', 6)).toBe('1,234,567');
		expect(formatUnits(123_456_789n, 8)).toBe('1.234567');
	});

	it('shows a fee without trailing zeros, and a tiny amount as more than nothing', () => {
		// 0.0210000123: six decimals are "021000" — shown as 0.021.
		expect(formatUnits(21_000_012_300_000_000n, 18)).toBe('0.021');
		expect(formatUnits(1_000_000_000_000_000_000n, 18)).toBe('1');
		expect(formatUnits(1n, 18)).toBe('<0.000001');
		expect(formatUnits(0n, 18)).toBe('0');
		expect(formatUnits(1_000_000_000_000_000_001n, 18)).toBe('1');
	});

	it('writes records as plain decimals, every digit kept', () => {
		expect(plainUnits('1234567000000', 6)).toBe('1234567');
		expect(plainUnits('250000000000000000', 18)).toBe('0.25');
		expect(plainUnits('123456789', 8)).toBe('1.23456789');
	});
});
