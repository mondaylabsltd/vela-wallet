/**
 * What a page knows of a multisig link before any wasm (Welcome loads none):
 *
 * - r4b-9: a link opened on a device with no account yet is kept — only a
 *   multisig link — in the record the core takes once the wallet is there
 *   (`take_kept_link`; Rust: `a_link_kept_through_sign_in_opens_once`, and
 *   the day's limit in multisig_link_vectors.rs);
 * - r4b-17: a link opened on another page is carried to the wallet, read as
 *   chat apps and mail clients carry it.
 */
import '$lib/i18n/wasm-init.server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import * as wasm from '../../../../../../rust/pkg-web/vela_core.js';
import { KEPT_LINK_KEY, carriedLinkHash, isMultisigLink, keepLink } from './kept-link';

const store = new Map<string, string>();
vi.stubGlobal('localStorage', {
	getItem: (k: string) => store.get(k) ?? null,
	setItem: (k: string, v: string) => void store.set(k, v),
	removeItem: (k: string) => void store.delete(k)
});

const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const HASH = '0xed2132b0c8f16940fb0fa92a8fbaed407bed1ed264f61bd9c38e84d56ed0d8aa';
const INVITE = `#msa=5042002:${SAFE}&m=pAlice,wBob`;

/** What the core takes from the record (`takeKeptLink`). */
const taken = (now: number) =>
	JSON.parse(
		wasm.multisigLinkCall('takeKeptLink', JSON.stringify({ stored: store.get(KEPT_LINK_KEY), now }))
	) as string | null;

beforeEach(() => store.clear());

describe('a link kept through onboarding (r4b-9)', () => {
	it('keeps an invite in the record the core opens once the account exists', () => {
		expect(keepLink(INVITE, 1_000)).toBe(true);
		expect(taken(2_000)).toBe(INVITE);
	});

	it('keeps a proposal link, and a damaged one (said when it is opened)', () => {
		expect(keepLink(`#ms=100.${SAFE}.0x12`, 1_000)).toBe(true);
		expect(taken(2_000)).toBe(`#ms=100.${SAFE}.0x12`);
	});

	it('keeps nothing that is no multisig link', () => {
		expect(keepLink('')).toBe(false);
		expect(keepLink('#flow=receive')).toBe(false);
		expect(store.has(KEPT_LINK_KEY)).toBe(false);
	});
});

describe('links as chat apps and mail clients carry them (r4b-17)', () => {
	it('a link opened on another page (`/en#msa=…`) is carried to the wallet', () => {
		expect(carriedLinkHash(`#msa=${SAFE}`)).toBe(`#msa=${SAFE}`);
		expect(carriedLinkHash(`#ms%3D100.${SAFE}.${HASH}`)).toBe(`#ms=100.${SAFE}.${HASH}`);
		expect(carriedLinkHash(`# msa=${SAFE} `)).toBe(`#msa=${SAFE}`);
		expect(carriedLinkHash('#intro')).toBe('');
		expect(carriedLinkHash('')).toBe('');
	});

	it('meant as a link, readable or not, as the core reads it', () => {
		for (const hash of [
			`#ms=100.${SAFE}.${HASH}`,
			`#ms=100.${SAFE}.${HASH}.extra`,
			`#msa%3D${SAFE.slice(0, 30)}`,
			'#intro',
			''
		]) {
			const core = JSON.parse(
				wasm.multisigLinkCall('isMultisigLink', JSON.stringify({ hash }))
			) as boolean;
			expect(isMultisigLink(hash), hash).toBe(core);
		}
	});
});
