/**
 * Entering and leaving the parallel space (spec 026 T221).
 *
 * The safety property is the round trip: a person's real wallet cache must
 * come back exactly as it was, and re-entering must never overwrite the backup
 * with fixture data. The signer swap is the other half — after entry, every
 * passkey ceremony is served by the fixed keys, and after exit, none is.
 */
import '$lib/i18n/wasm-init.server';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const kv = new Map<string, string>();
vi.mock('$lib/services/storage', () => ({
	getItem: vi.fn(async (key: string) => kv.get(key) ?? null),
	setItem: vi.fn(async (key: string, value: string) => void kv.set(key, value)),
	removeItem: vi.fn(async (key: string) => void kv.delete(key))
}));

const local = new Map<string, string>();
vi.stubGlobal('localStorage', {
	getItem: (key: string) => local.get(key) ?? null,
	setItem: (key: string, value: string) => void local.set(key, value),
	removeItem: (key: string) => void local.delete(key),
	clear: () => local.clear()
});

import { hasPasskeyOverride, signWithAny } from '$lib/onboarding/core/passkey';
import { verifySafeWebAuthn } from '$lib/core/kernels';
import { listGrants, revokeAll } from '$lib/dapp/connections';
import { followActiveAccount } from '$lib/dapp/follow';
import { setGrant } from '$lib/dapp/grants';
import { CHAIN_PREFIX, PERM_PREFIX, SPACE_SITES_KEY } from '$lib/dapp/keys';
import { restoreSites } from '$lib/dapp/space-sites';
import { parallelFlagSet } from './parallel-flag.svelte';
import {
	applyParallelSpaceOnBoot,
	enterParallelSpace,
	exitParallelSpace,
	fixtureStoredAccounts,
	uninstallFixtureSigner
} from './parallel-space';

const REAL_ACCOUNTS = JSON.stringify([
	{
		id: 'real-cred',
		name: 'My Wallet',
		address: '0x14fb1f4e2b9c7a5d8e3f6a1b4c7d9e2f5a8b1d1e',
		public_key_hex: '04' + 'ab'.repeat(64),
		created_at_iso: '2026-01-01T00:00:00.000Z',
		keys: []
	}
]);

beforeEach(() => {
	kv.clear();
	local.clear();
	uninstallFixtureSigner();
	delete (globalThis as { chrome?: unknown }).chrome;
});

describe('the fixture wallets', () => {
	it('are three single-key Safes plus the golden multi-key one', () => {
		const accounts = fixtureStoredAccounts();
		expect(accounts).toHaveLength(4);
		expect(accounts.slice(0, 3).every((a) => a.keys.length === 1)).toBe(true);
		expect(accounts[3].keys).toHaveLength(3);
		expect(accounts[3].address).toBe('0x88cCA0EeDbF2C4426110bbFc998F048689266894');
		// keys[0] mirrors the scalar fields — the stored shape's invariant.
		for (const account of accounts) {
			expect(account.keys[0].credential_id).toBe(account.id);
			expect(account.keys[0].public_key_hex).toBe(account.public_key_hex);
		}
	});
});

describe('the round trip', () => {
	it('swaps the wallet in, and gives the real one back untouched', async () => {
		local.set('vela.accounts', REAL_ACCOUNTS);
		local.set('vela.activeAccountIndex', '2');

		await enterParallelSpace();
		expect(parallelFlagSet()).toBe(true);
		expect(local.get('vela.accounts')).not.toBe(REAL_ACCOUNTS);
		expect(local.get('vela.activeAccountIndex')).toBe('0');
		expect(JSON.parse(local.get('vela.accounts')!)).toHaveLength(4);

		await exitParallelSpace();
		expect(parallelFlagSet()).toBe(false);
		expect(local.get('vela.accounts')).toBe(REAL_ACCOUNTS);
		expect(local.get('vela.activeAccountIndex')).toBe('2');
		expect(local.has('vela.parallelSpace.realWalletBackup')).toBe(false);
	});

	it('re-entering does not overwrite the backup with fixture data', async () => {
		local.set('vela.accounts', REAL_ACCOUNTS);
		await enterParallelSpace();
		await enterParallelSpace();
		await exitParallelSpace();
		expect(local.get('vela.accounts')).toBe(REAL_ACCOUNTS);
	});

	it('a visitor with no wallet gets no wallet back', async () => {
		await enterParallelSpace();
		expect(JSON.parse(local.get('vela.accounts')!)).toHaveLength(4);
		await exitParallelSpace();
		expect(local.has('vela.accounts')).toBe(false);
		expect(local.has('vela.activeAccountIndex')).toBe(false);
	});

	it('seeds one fixture contact and removes exactly that one', async () => {
		kv.set('vela.contacts', JSON.stringify([{ address: '0xmine', name: 'Mine', kind: 'eoa' }]));
		await enterParallelSpace();
		const seeded = JSON.parse(kv.get('vela.contacts')!) as { address: string }[];
		expect(seeded).toHaveLength(2);
		await enterParallelSpace(); // idempotent
		expect(JSON.parse(kv.get('vela.contacts')!)).toHaveLength(2);
		await exitParallelSpace();
		const left = JSON.parse(kv.get('vela.contacts')!) as { address: string }[];
		expect(left).toEqual([{ address: '0xmine', name: 'Mine', kind: 'eoa' }]);
	});
});

describe('the signer swap', () => {
	it('installs the fixed keys on entry and takes them away on exit', async () => {
		expect(hasPasskeyOverride()).toBe(false);
		await enterParallelSpace();
		expect(hasPasskeyOverride()).toBe(true);

		// The ceremony a send would run, answered by a fixture key.
		const assertion = await signWithAny('0x' + '11'.repeat(32), [
			{ id: fixtureStoredAccounts()[0].id }
		]);
		expect(verifySafeWebAuthn(assertion)).toEqual({ ok: true });

		await exitParallelSpace();
		expect(hasPasskeyOverride()).toBe(false);
	});

	it('re-arms on boot only when the flag is set', async () => {
		await applyParallelSpaceOnBoot();
		expect(hasPasskeyOverride()).toBe(false);

		local.set('vela.parallelSpace', '1');
		await applyParallelSpaceOnBoot();
		expect(hasPasskeyOverride()).toBe(true);
	});
});

/**
 * Device walk of 2026-10-11, item 5: a walk through the parallel space cut
 * every site of the person's off. The space swaps the account list, the
 * extension's grants are pinned to accounts, and the follow asked the core
 * about each of them with the fixtures as "the wallet's accounts".
 *
 * The extension's storage area here is a store that changes only by its own
 * three operations and keeps what it is given as JSON text, the way the real
 * one serialises — so "exactly as before" is a comparison of bytes.
 */
describe('the person’s connected sites', () => {
	const REAL = '0x14fb1f4e2b9c7a5d8e3f6a1b4c7d9e2f5a8b1d1e';
	const TESTDAPP = 'http://127.0.0.1:8137';
	const ORBS = 'https://arcorbs.game3.app';
	const NEW_SITE = 'https://new.example';

	class Area {
		readonly items = new Map<string, string>();
		/** Set to make every `set` fail, as a store that refuses writes does. */
		failSet = false;
		/** Writes (`set` + `remove`) performed, and how many more are allowed
		 *  before the store stops answering — a page that died part-way. */
		writes = 0;
		stopAfterWrites = Infinity;

		#write(): void {
			if (this.writes >= this.stopAfterWrites) throw new Error('page gone');
			this.writes++;
		}
		async get(keys: null | string | string[]): Promise<Record<string, unknown>> {
			const names = keys === null ? [...this.items.keys()] : Array.isArray(keys) ? keys : [keys];
			return Object.fromEntries(
				names.filter((k) => this.items.has(k)).map((k) => [k, JSON.parse(this.items.get(k)!)])
			);
		}
		async set(items: Record<string, unknown>): Promise<void> {
			if (this.failSet) throw new Error('storage denied');
			this.#write();
			for (const [k, v] of Object.entries(items)) this.items.set(k, JSON.stringify(v));
		}
		async remove(keys: string | string[]): Promise<void> {
			this.#write();
			for (const k of Array.isArray(keys) ? keys : [keys]) this.items.delete(k);
		}
		/** Every site entry, as bytes, in key order. */
		sites(): string {
			return JSON.stringify(
				[...this.items]
					.filter(([k]) => k.startsWith(PERM_PREFIX) || k.startsWith(CHAIN_PREFIX))
					.sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
			);
		}
	}

	/** The extension, with the person's wallet and two connected sites. */
	function extension(): Area {
		const area = new Area();
		(globalThis as { chrome?: unknown }).chrome = { storage: { local: area } };
		local.set('vela.accounts', REAL_ACCOUNTS);
		local.set('vela.activeAccountIndex', '0');
		const grant = (origin: string, chainId: number, grantedAt: number) =>
			JSON.stringify({ origin, address: REAL, chainId, grantedAt });
		area.items.set(PERM_PREFIX + TESTDAPP, grant(TESTDAPP, 100, 1_790_000_000_000));
		area.items.set(PERM_PREFIX + ORBS, grant(ORBS, 56, 1_790_000_111_000));
		area.items.set(CHAIN_PREFIX + ORBS, '8453');
		// Something that is neither: the set-aside must not take it with it.
		area.items.set('vela.ext.surface', JSON.stringify('panel'));
		return area;
	}

	/** What a visit does: connect the test dApp again, connect a new site,
	 *  and let each put itself on a network. */
	async function connectInside(area: Area): Promise<void> {
		const fixture = fixtureStoredAccounts()[0].address;
		await setGrant({
			origin: TESTDAPP,
			address: fixture,
			chainId: 1,
			grantedAt: 1_791_000_000_000
		});
		await setGrant({
			origin: NEW_SITE,
			address: fixture,
			chainId: 10,
			grantedAt: 1_791_000_000_001
		});
		await area.set({ [CHAIN_PREFIX + TESTDAPP]: 42161, [CHAIN_PREFIX + NEW_SITE]: 10 });
	}

	it('are set aside on the way in: the space starts with none connected', async () => {
		const area = extension();
		const before = area.sites();

		await enterParallelSpace();
		expect(area.sites()).toBe('[]');
		expect(await listGrants()).toEqual([]);
		expect(area.items.get('vela.ext.surface')).toBe(JSON.stringify('panel'));
		// Whole, in the one record no reader of the two prefixes sees.
		expect(
			JSON.stringify(
				Object.entries(JSON.parse(area.items.get(SPACE_SITES_KEY)!) as object)
					.map(([k, v]) => [k, JSON.stringify(v)])
					.sort(([a], [b]) => (a < b ? -1 : 1))
			)
		).toBe(before);
	});

	it('come back byte for byte, whatever was connected inside', async () => {
		const area = extension();
		const before = area.sites();

		await enterParallelSpace();
		await connectInside(area);
		// The space's own follow — on its first account, then a switch inside.
		const fixtures = fixtureStoredAccounts().map((a) => a.address);
		await followActiveAccount({ activeAddress: fixtures[0], addresses: fixtures });
		await followActiveAccount({ activeAddress: fixtures[1], addresses: fixtures });
		expect((await listGrants()).map((g) => g.origin).sort()).toEqual([TESTDAPP, NEW_SITE].sort());

		await exitParallelSpace();
		expect(area.sites()).toBe(before);
		expect(area.items.has(SPACE_SITES_KEY)).toBe(false);
		expect(area.items.get('vela.ext.surface')).toBe(JSON.stringify('panel'));
		expect(local.get('vela.accounts')).toBe(REAL_ACCOUNTS);
		// And the person's own follow, back in their wallet, changes nothing.
		expect(await followActiveAccount({ activeAddress: REAL, addresses: [REAL] })).toEqual({
			repinned: [],
			removed: []
		});
		expect(area.sites()).toBe(before);
	});

	/**
	 * With the person's sites set aside, every grant in the store is the
	 * space's own, and follows the space's accounts by the ordinary rule —
	 * the space is told to the core only to protect what could NOT be set
	 * aside. (`extension-follow-account.e2e.ts` signs out and into another
	 * wallet inside the space: the grant of the account that left is dropped
	 * and the site cut off, never handed the new account unasked — issue 315.)
	 */
	it('inside, a site connected to a space account that leaves the space is cut off, as anywhere', async () => {
		const area = extension();
		const before = area.sites();
		await enterParallelSpace();
		await connectInside(area);
		const fixtures = fixtureStoredAccounts().map((a) => a.address);

		// Signed in to Parallel Two alone: Parallel One left this device.
		const outcome = await followActiveAccount({
			activeAddress: fixtures[1],
			addresses: [fixtures[1]]
		});
		expect(outcome.removed.sort()).toEqual([TESTDAPP, NEW_SITE].sort());
		expect(outcome.repinned).toEqual([]);
		expect(await listGrants()).toEqual([]);

		// The person's own, set aside, were not part of that — and come back.
		await exitParallelSpace();
		expect(area.sites()).toBe(before);
	});

	it('a second entry does not replace what was set aside with the space’s own', async () => {
		const area = extension();
		const before = area.sites();

		await enterParallelSpace();
		const record = area.items.get(SPACE_SITES_KEY);
		await connectInside(area);
		await enterParallelSpace();
		expect(area.items.get(SPACE_SITES_KEY)).toBe(record);
		// What the space connected is still the space's, still in place.
		expect((await listGrants()).length).toBe(2);

		await exitParallelSpace();
		expect(area.sites()).toBe(before);
	});

	it('a visit by somebody with no connected site leaves none behind', async () => {
		const area = extension();
		for (const key of [...area.items.keys()]) {
			if (key !== 'vela.ext.surface') area.items.delete(key);
		}
		await enterParallelSpace();
		await connectInside(area);
		await exitParallelSpace();
		expect(area.sites()).toBe('[]');
		expect(area.items.has(SPACE_SITES_KEY)).toBe(false);
	});

	/**
	 * The set-aside is a write, and a write can fail. The person's grants then
	 * stay where the follow finds them, beside the fixture accounts — which is
	 * exactly the state that used to remove them. `space` is what stands
	 * between: the follow reads the space's flag and says so to the core.
	 */
	it('are not touched by the follow even when they could not be set aside', async () => {
		const area = extension();
		const before = area.sites();
		area.failSet = true;

		await enterParallelSpace();
		expect(area.items.has(SPACE_SITES_KEY)).toBe(false);
		expect(area.sites()).toBe(before);

		const fixtures = fixtureStoredAccounts().map((a) => a.address);
		for (const active of [fixtures[0], fixtures[3]]) {
			expect(await followActiveAccount({ activeAddress: active, addresses: fixtures })).toEqual({
				repinned: [],
				removed: []
			});
		}
		expect(area.sites()).toBe(before);

		// Leaving finds nothing set aside, so it drops nothing either.
		await exitParallelSpace();
		expect(area.sites()).toBe(before);

		// The proof that it is the flag doing this: the same follow, asked
		// with the fixtures as the wallet's accounts and no space, removes
		// both of the person's sites — what a walk through the space did.
		area.failSet = false;
		const asked = await followActiveAccount({ activeAddress: fixtures[0], addresses: fixtures });
		expect(asked.removed.sort()).toEqual([TESTDAPP, ORBS].sort());
	});

	it('a leaving cut short at any step is finished by leaving again, with the same result', async () => {
		// The three writes of the way out: the space's entries go; the
		// person's come back and the record is spent; the spent record goes.
		for (const writesDone of [0, 1, 2]) {
			local.clear();
			const area = extension();
			const before = area.sites();
			await enterParallelSpace();
			await connectInside(area);

			area.writes = 0;
			area.stopAfterWrites = writesDone;
			await restoreSites().catch(() => {});

			area.stopAfterWrites = Infinity;
			expect(await exitParallelSpace(), `cut after ${writesDone} write(s)`).toBe(true);
			expect(area.sites(), `cut after ${writesDone} write(s)`).toBe(before);
			expect(area.items.has(SPACE_SITES_KEY)).toBe(false);
		}
	});

	it('does not leave the space while the sites set aside cannot be given back', async () => {
		const area = extension();
		const before = area.sites();
		await enterParallelSpace();
		await connectInside(area);

		area.failSet = true;
		expect(await exitParallelSpace()).toBe(false);
		// Still in the space, wallet and signer as they were; the person's
		// sites still whole in their record.
		expect(parallelFlagSet()).toBe(true);
		expect(hasPasskeyOverride()).toBe(true);
		expect(JSON.parse(local.get('vela.accounts')!)).toHaveLength(4);
		expect(area.items.has(SPACE_SITES_KEY)).toBe(true);

		area.failSet = false;
		expect(await exitParallelSpace()).toBe(true);
		expect(area.sites()).toBe(before);
		expect(local.get('vela.accounts')).toBe(REAL_ACCOUNTS);
	});

	it('“Disconnect all” inside the space takes the grants set aside too', async () => {
		const area = extension();
		await enterParallelSpace();
		await connectInside(area);

		await revokeAll();
		expect(await listGrants()).toEqual([]);

		await exitParallelSpace();
		// No grant of anybody's is left; the network a site chose is not a
		// grant, and stays as "Disconnect all" leaves it outside a space.
		expect(area.sites()).toBe(JSON.stringify([[CHAIN_PREFIX + ORBS, '8453']]));
	});

	it('is nothing at all on the plain web page, which has no grants', async () => {
		local.set('vela.accounts', REAL_ACCOUNTS);
		await enterParallelSpace();
		await exitParallelSpace();
		expect(local.get('vela.accounts')).toBe(REAL_ACCOUNTS);
		expect(await listGrants()).toEqual([]);
	});
});
