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
import { SessionFollower, followActiveAccount, normalizeGrantSpelling } from './follow';
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

	it('never brings back a grant that changed while it ran', async () => {
		// The rewrite reads every grant, then writes them one by one. A site
		// the person disconnects — or one re-pinned to another account — in
		// between must stay as it now is: writing the spelling of what was read
		// would reconnect it, or point it back at the old account.
		const racing = {
			...local,
			set: async (items: Record<string, unknown>) => {
				await local.set(items);
				if (PERM_PREFIX + 'https://a.example' in items) {
					store.delete(PERM_PREFIX + 'https://b.example');
					store.set(PERM_PREFIX + 'https://c.example', grantFor('https://c.example', BOB));
				}
			}
		};
		(globalThis as { chrome?: unknown }).chrome = { storage: { local: racing } };
		store.set(PERM_PREFIX + 'https://a.example', grantFor('https://a.example', ALICE));
		store.set(PERM_PREFIX + 'https://b.example', grantFor('https://b.example', ALICE));
		store.set(PERM_PREFIX + 'https://c.example', grantFor('https://c.example', ALICE));

		expect(await normalizeGrantSpelling()).toEqual(['https://a.example']);
		expect(store.has(PERM_PREFIX + 'https://b.example')).toBe(false);
		expect(store.get(PERM_PREFIX + 'https://c.example')).toEqual(
			grantFor('https://c.example', BOB)
		);
	});

	/**
	 * Spec 082 G58: the device pass switched to Parallel Two in Settings →
	 * 切换账户 and the site kept `eth_accounts` = One. The follow now runs
	 * from the root layout on every route, with the last address kept in the
	 * module — so the switch is seen wherever it is made, and a screen that
	 * mounts again is not a boot.
	 */
	it('follows a switch made on any screen, once, in the core’s spelling', async () => {
		(globalThis as { chrome?: unknown }).chrome = { storage: { local } };
		store.set(PERM_PREFIX + 'https://a.example', grantFor('https://a.example', ALICE));
		const follower = new SessionFollower();
		const view = (address: string, loading = false) => ({
			loading,
			address,
			accounts: [ALICE, BOB].map((a) => ({ account: { address: a } }))
		});

		// Still loading, then the boot: nothing is re-pinned.
		expect(follower.note(view('', true))).toBeNull();
		expect(follower.note(view(ALICE))).toBeNull();
		expect(store.get(PERM_PREFIX + 'https://a.example')).toEqual(
			grantFor('https://a.example', ALICE)
		);

		// The switch, made in Settings: the site is re-pinned to BOB, EIP-55.
		const outcome = await follower.note(view(BOB), NOW);
		expect(outcome).toEqual({ repinned: ['https://a.example'], removed: [] });
		expect(store.get(PERM_PREFIX + 'https://a.example')).toMatchObject({
			address: checksumAddress(BOB)
		});

		// The wallet screen mounting again sees the same account: not a boot, not a switch.
		expect(follower.note(view(BOB))).toBeNull();
		expect(follower.note(view(checksumAddress(BOB)))).toBeNull();
	});

	it('normalises nothing off the extension', async () => {
		expect(await normalizeGrantSpelling()).toEqual([]);
	});
});
