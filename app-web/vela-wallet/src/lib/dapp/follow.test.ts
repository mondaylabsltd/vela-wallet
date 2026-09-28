/**
 * A connected site follows the wallet's active account — the CORE's rule,
 * driven end to end (spec 027 T350, performed at last).
 *
 * `planAccountSwitch` seeds a throwaway `dapp_permissions` the way the popup
 * seeds it and asks what an account switch tells one granted origin. These
 * pin the three answers that matter: a re-pin to the new address for a site
 * whose account is still in the wallet, a removal for a site whose account
 * left, and silence when nothing changed — and that `followActiveAccount`
 * writes exactly those into the extension's storage.
 */
import '$lib/i18n/wasm-init.server';
import { afterEach, describe, expect, it } from 'vitest';
import { planAccountSwitch } from './core/dperm-connect';
import { toWireGrant } from './core/dperm-types';
import { followActiveAccount, normalizeGrantSpelling } from './follow';
import { checksumAddress } from '$lib/core/kernels';
import { PERM_PREFIX } from './keys';
import type { DAppGrant } from './grants';

const ALICE = `0x${'a1'.repeat(20)}`;
const BOB = `0x${'b2'.repeat(20)}`;
const CAROL = `0x${'c3'.repeat(20)}`;
const NOW = 1_800_000_000_000;

const grantFor = (origin: string, address: string, chainId = 100): DAppGrant => ({
	origin,
	address,
	chainId,
	grantedAt: NOW - 1000
});

describe('what the core says an account switch tells a site', () => {
	it('re-pins a connected site to the new address, keeping the chain it connected on', () => {
		const plan = planAccountSwitch({
			origin: 'https://app.example',
			storedGrant: toWireGrant(grantFor('https://app.example', ALICE, 8453))!,
			currentAddresses: [ALICE, BOB],
			activeAddress: BOB,
			nowMs: NOW
		});
		expect(plan.kind).toBe('repin');
		if (plan.kind !== 'repin') return;
		expect(plan.grant.origin).toBe('https://app.example');
		// The core spells the re-pinned address EIP-55 (spec 082 RG10).
		expect(plan.grant.address).toBe(checksumAddress(BOB));
		expect(plan.grant.chain_id).toBe(8453);
		expect(plan.grant.granted_at_ms).toBe(NOW);
	});

	it('removes the grant of a site whose account left the wallet', () => {
		// ALICE is gone; the wallet now holds BOB and CAROL. `should_drop_grant`
		// fires on the read, and no re-pin follows for a site no longer connected.
		const plan = planAccountSwitch({
			origin: 'https://app.example',
			storedGrant: toWireGrant(grantFor('https://app.example', ALICE))!,
			currentAddresses: [BOB, CAROL],
			activeAddress: CAROL,
			nowMs: NOW
		});
		expect(plan.kind).toBe('remove');
	});

	it('says nothing on a cold read — never logs a site out on "not known yet"', () => {
		// Invariant ②: an empty/unknown address set must not be read as "gone".
		// The grant survives and the site still resolves to ALICE; but nothing
		// can be re-pinned to an address the wallet has not confirmed either.
		const plan = planAccountSwitch({
			origin: 'https://app.example',
			storedGrant: toWireGrant(grantFor('https://app.example', ALICE))!,
			currentAddresses: null,
			activeAddress: BOB,
			nowMs: NOW
		});
		expect(plan.kind).not.toBe('remove');
	});
});

describe('followActiveAccount writes what the core authored', () => {
	const store = new Map<string, unknown>();
	const local = {
		get: async (keys: null | string | string[]) => {
			if (keys === null) return Object.fromEntries(store);
			const list = Array.isArray(keys) ? keys : [keys];
			return Object.fromEntries(list.filter((k) => store.has(k)).map((k) => [k, store.get(k)]));
		},
		set: async (items: Record<string, unknown>) => {
			for (const [k, v] of Object.entries(items)) store.set(k, v);
		},
		remove: async (keys: string | string[]) => {
			for (const k of Array.isArray(keys) ? keys : [keys]) store.delete(k);
		}
	};

	afterEach(() => {
		store.clear();
		delete (globalThis as { chrome?: unknown }).chrome;
	});

	it('re-pins every granted site, removes the orphaned one, leaves the current one alone', async () => {
		(globalThis as { chrome?: unknown }).chrome = { storage: { local } };
		store.set(PERM_PREFIX + 'https://a.example', grantFor('https://a.example', ALICE));
		store.set(PERM_PREFIX + 'https://b.example', grantFor('https://b.example', CAROL, 1));
		store.set(PERM_PREFIX + 'https://c.example', grantFor('https://c.example', BOB));

		const outcome = await followActiveAccount({
			activeAddress: BOB,
			addresses: [ALICE, BOB],
			nowMs: NOW
		});

		expect(outcome.repinned).toEqual(['https://a.example']);
		expect(outcome.removed).toEqual(['https://b.example']);
		expect(store.get(PERM_PREFIX + 'https://a.example')).toEqual({
			origin: 'https://a.example',
			address: checksumAddress(BOB),
			chainId: 100,
			grantedAt: NOW
		});
		expect(store.has(PERM_PREFIX + 'https://b.example')).toBe(false);
		// Already on BOB: untouched, so the worker announces nothing to it.
		expect(store.get(PERM_PREFIX + 'https://c.example')).toEqual(
			grantFor('https://c.example', BOB)
		);
	});

	it('is a no-op off the extension', async () => {
		const outcome = await followActiveAccount({ activeAddress: BOB, addresses: [BOB] });
		expect(outcome).toEqual({ repinned: [], removed: [] });
	});

	/** Spec 082 RG10 (L-D6): one spelling of an address toward every site. */
	it('rewrites lower-case grants to the core’s EIP-55 spelling, once', async () => {
		(globalThis as { chrome?: unknown }).chrome = { storage: { local } };
		const SPELLED = checksumAddress(ALICE);
		store.set(PERM_PREFIX + 'https://a.example', grantFor('https://a.example', ALICE));
		store.set(PERM_PREFIX + 'https://b.example', grantFor('https://b.example', SPELLED));

		expect(await normalizeGrantSpelling()).toEqual(['https://a.example']);
		expect(store.get(PERM_PREFIX + 'https://a.example')).toEqual({
			...grantFor('https://a.example', ALICE),
			address: SPELLED
		});
		// Idempotent: the second boot finds nothing to do.
		expect(await normalizeGrantSpelling()).toEqual([]);
		expect(store.get(PERM_PREFIX + 'https://b.example')).toEqual(
			grantFor('https://b.example', SPELLED)
		);
	});

	it('normalises nothing off the extension', async () => {
		expect(await normalizeGrantSpelling()).toEqual([]);
	});
});
