/**
 * r4b-9: a multisig link opened on a device with no account yet is kept
 * through onboarding and opened once — and only a multisig link, for a day.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { KEPT_LINK_MS, keepLink, takeKeptLink } from './kept-link';

const store = new Map<string, string>();
vi.stubGlobal('localStorage', {
	getItem: (k: string) => store.get(k) ?? null,
	setItem: (k: string, v: string) => void store.set(k, v),
	removeItem: (k: string) => void store.delete(k)
});

const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const INVITE = `#msa=5042002:${SAFE}&m=pAlice,wBob`;

beforeEach(() => store.clear());

describe('a link kept through onboarding (r4b-9)', () => {
	it('keeps an invite and opens it once, after the account exists', () => {
		expect(keepLink(INVITE, 1_000)).toBe(true);
		expect(takeKeptLink(2_000)).toBe(INVITE);
		// Taken: a second wallet visit opens nothing again.
		expect(takeKeptLink(3_000)).toBeNull();
	});

	it('keeps a proposal link, and a damaged one (said when it is opened)', () => {
		expect(keepLink(`#ms=100.${SAFE}.0x12`)).toBe(true);
		expect(takeKeptLink()).toBe(`#ms=100.${SAFE}.0x12`);
	});

	it('keeps nothing that is no multisig link', () => {
		expect(keepLink('')).toBe(false);
		expect(keepLink('#flow=receive')).toBe(false);
		expect(takeKeptLink()).toBeNull();
	});

	it('forgets a link that waited longer than a day', () => {
		keepLink(INVITE, 0);
		expect(takeKeptLink(KEPT_LINK_MS + 1)).toBeNull();
	});
});
