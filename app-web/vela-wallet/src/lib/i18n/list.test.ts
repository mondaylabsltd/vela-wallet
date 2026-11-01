import { describe, expect, it } from 'vitest';
import { listOf } from './list';

describe('listOf (r10-10)', () => {
	it('words a list in the given language', () => {
		expect(listOf(['#1', '#2', '#3'], 'en')).toBe('#1, #2, and #3');
		expect(listOf(['#1', '#2', '#3'], 'zh')).toBe('#1、#2和#3');
		expect(listOf(['#1', '#2', '#3'], 'zh')).not.toContain(', ');
	});

	it('says one item as it is and nothing as nothing', () => {
		expect(listOf(['#4'], 'zh')).toBe('#4');
		expect(listOf([], 'en')).toBe('');
	});
});
