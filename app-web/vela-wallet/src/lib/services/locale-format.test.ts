/**
 * Every preset, pinned by example (spec 028 T435 — research D47).
 *
 * The rule these exist to defend: the product's own presets, never the
 * platform's. `Intl` is allowed to SUGGEST a preset once, when a person has
 * chosen `auto`; it is never allowed to format a figure, because then the same
 * balance reads two ways on two machines and a person cannot tell a thousands
 * separator from a decimal point in a language they are guessing at.
 *
 * The examples below are also what the pickers show as labels, so a row that
 * lies about what it will do fails right here.
 */
import { beforeEach, describe, expect, it } from 'vitest';
import { preferences } from './preferences.svelte';
import {
	dateFormatOptions,
	formatCompact,
	formatDate,
	formatNumber,
	formatTime,
	formatTokenAmount,
	groupDigits,
	numberFormatOptions,
	numberSeparators,
	parseLocaleNumber,
	timeFormatOptions
} from './locale-format';

const SAMPLE = new Date(2026, 5, 13, 13, 45);

beforeEach(() => {
	preferences.resetForTests();
});

describe('number presets', () => {
	it('each one groups and points the way it says it does', () => {
		const n = 1234567.89;
		const opts = { minimumFractionDigits: 2, maximumFractionDigits: 2 } as const;
		expect(formatNumber(n, { ...opts, key: 'comma_dot' })).toBe('1,234,567.89');
		expect(formatNumber(n, { ...opts, key: 'dot_comma' })).toBe('1.234.567,89');
		expect(formatNumber(n, { ...opts, key: 'space_comma' })).toBe('1 234 567,89');
		// Indian grouping is 2-3, not 3-3: the last three digits, then pairs.
		expect(formatNumber(n, { ...opts, key: 'indian' })).toBe('12,34,567.89');
	});

	it('the chosen preset is what an unqualified call uses', () => {
		preferences.setNumberFormat('dot_comma');
		expect(formatNumber(1234.5, { minimumFractionDigits: 2 })).toBe('1.234,50');
		expect(numberSeparators()).toEqual({ group: '.', decimal: ',' });
	});

	it('groups a DIGIT STRING without ever making it a number', () => {
		// A uint256 balance is longer than a double can hold exactly. Rounding
		// one here would be a wrong figure on a screen someone signs from.
		const huge = '123456789012345678901234567890';
		expect(groupDigits(huge, 'comma_dot')).toBe('123,456,789,012,345,678,901,234,567,890');
		expect(groupDigits(huge, 'indian')).toBe('1,23,45,67,89,01,23,45,67,89,01,23,45,67,890');
	});

	it('reads back what it wrote, and what a foreign keyboard wrote', () => {
		expect(parseLocaleNumber('1.234.567,89', 'dot_comma')).toBe('1234567.89');
		expect(parseLocaleNumber('1 234 567,89', 'space_comma')).toBe('1234567.89');
		// A plain ASCII amount still parses under a comma preset's cousin.
		expect(parseLocaleNumber('1234.56', 'comma_dot')).toBe('1234.56');
		// Arabic-Indic digits map to ASCII, or the field would refuse a number a
		// person can plainly see they typed.
		expect(parseLocaleNumber('١٢٣٤,٥٦', 'dot_comma')).toBe('1234.56');
	});

	it('a compact figure keeps the preset it abbreviates under', () => {
		expect(formatCompact(1234567.89, 'comma_dot')).toBe('1.23M');
		expect(formatCompact(1234567.89, 'dot_comma')).toBe('1,23M');
		expect(formatCompact(4.5e9, 'comma_dot')).toBe('4.5B');
		expect(formatCompact(820, 'comma_dot')).toBe('820');
	});

	it('token amounts keep dust visible instead of rounding it to zero', () => {
		preferences.setNumberFormat('comma_dot');
		expect(formatTokenAmount(1234.5)).toBe('1,234.50');
		// Rounded, not truncated — `toFixed`'s behaviour, kept from the port.
		expect(formatTokenAmount(1.23456789)).toBe('1.2346');
		expect(formatTokenAmount(0.000012)).toBe('0.000012');
		// The glanceable form caps at 4 decimals — but never at the price of
		// printing "0" beside a transfer that really moved something.
		expect(formatTokenAmount(0.000012, { compact: true })).toBe('0.000012');
		expect(formatTokenAmount(0)).toBe('0');
	});
});

describe('date and time presets', () => {
	it('each order and separator is what its name says', () => {
		expect(formatDate(SAMPLE, 'ymd_slash')).toBe('2026/06/13');
		expect(formatDate(SAMPLE, 'mdy_slash')).toBe('06/13/2026');
		expect(formatDate(SAMPLE, 'dmy_slash')).toBe('13/06/2026');
		expect(formatDate(SAMPLE, 'dmy_dot')).toBe('13.06.2026');
		expect(formatDate(SAMPLE, 'iso')).toBe('2026-06-13');
	});

	it('12- and 24-hour clocks, including the midnight and noon edges', () => {
		expect(formatTime(SAMPLE, 'h24')).toBe('13:45');
		expect(formatTime(SAMPLE, 'h12')).toBe('1:45 PM');
		expect(formatTime(new Date(2026, 5, 13, 0, 5), 'h12')).toBe('12:05 AM');
		expect(formatTime(new Date(2026, 5, 13, 12, 5), 'h12')).toBe('12:05 PM');
	});

	it('the chosen preset is what an unqualified call uses', () => {
		preferences.setDateFormat('iso');
		preferences.setTimeFormat('h12');
		expect(formatDate(SAMPLE)).toBe('2026-06-13');
		expect(formatTime(SAMPLE)).toBe('1:45 PM');
	});
});

describe('the pickers', () => {
	it('offer every preset, with the sample that tells them apart', () => {
		// 2026-06-13 13:45 is the one date/time where every order and both
		// clocks produce a different string; a sample like 01/01 would not.
		expect(dateFormatOptions().map((o) => o.example)).toEqual([
			expect.any(String), // `auto`, whatever this machine reports
			'2026/06/13',
			'06/13/2026',
			'13/06/2026',
			'13.06.2026',
			'2026-06-13'
		]);
		expect(
			timeFormatOptions()
				.map((o) => o.example)
				.slice(1)
		).toEqual(['13:45', '1:45 PM']);
		expect(
			numberFormatOptions()
				.map((o) => o.example)
				.slice(1)
		).toEqual(['1,234,567.89', '1.234.567,89', '1 234 567,89', '12,34,567.89']);
	});

	it('a label is the format DOING its job, so a row cannot lie', () => {
		for (const option of numberFormatOptions().slice(1)) {
			expect(formatNumber(1234567.89, { minimumFractionDigits: 2, key: option.key })).toBe(
				option.example
			);
		}
	});

	it('names the notes a screen has to translate, and nothing else', () => {
		expect(numberFormatOptions().map((o) => o.noteKey)).toEqual([
			'system',
			undefined,
			undefined,
			undefined,
			'indian'
		]);
		expect(timeFormatOptions().map((o) => o.noteKey)).toEqual(['system', 'h24', 'h12']);
	});
});
