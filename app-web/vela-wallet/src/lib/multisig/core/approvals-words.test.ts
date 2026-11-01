/**
 * r9: approvals counted in the locale's plural form, and a refusal said by
 * the core's own reason (r9-9) — through the engine's real corpus words.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import { resolveMultisigMessages } from '$lib/i18n/engine.server';
import { refusedLine, rejectHint, requireApprovals } from './approvals-words';

describe('approvals, counted (r9)', () => {
	it('one approval is "1 approval", never "1 approvals"', () => {
		const m = resolveMultisigMessages('en');
		expect(requireApprovals(m, '1')).toBe('Require 1 approval');
		expect(requireApprovals(m, 3)).toBe('Require 3 approvals');
		expect(rejectHint(m, 1)).toContain('it needs 1 approval too');
		expect(rejectHint(m, 2)).toContain('it needs 2 approvals too');
	});

	it('Russian takes its own form for 2–4', () => {
		const m = resolveMultisigMessages('ru');
		expect(requireApprovals(m, 1)).toBe('Требовать 1 одобрение');
		expect(requireApprovals(m, 3)).toBe('Требовать 3 одобрения');
		expect(requireApprovals(m, 5)).toBe('Требовать 5 одобрений');
	});
});

describe('a refusal, by its own reason (r9-9)', () => {
	it('each key is its own line; the state guard keeps "who controls"', () => {
		const m = resolveMultisigMessages('en');
		expect(refusedLine(m, 'home.multisig.review.refusedWhy.delegatecall')).toContain(
			"runs another contract's code as the multisig"
		);
		expect(refusedLine(m, 'home.multisig.review.refusedWhy.tooDeep')).toContain('too deep to read');
		expect(refusedLine(m, 'home.multisig.review.blocked')).toContain('who controls the multisig');
		expect(refusedLine(m, null)).toBeNull();
		expect(refusedLine(m, 'home.multisig.review.somethingNew')).toBeNull();
	});
});
