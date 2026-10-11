/**
 * A connected site follows the wallet's active account (spec 027 T350's rule,
 * performed at last).
 *
 * `dapp_permissions` has said since 027 what an account switch tells a site:
 * the grant is re-pinned to the new address and the page hears
 * `accountsChanged`. The extension never ran that rule — there was no live
 * document to push into — so a person who switched wallets in Vela watched
 * every dApp stay on the old one. Now the wallet asks the core once per grant
 * (`planAccountSwitch`), writes what it authored, and the worker announces the
 * change to every tab of that origin.
 *
 * Every grant, not only the sites with a tab open: a grant is what
 * `eth_accounts` answers from on the NEXT load too, and a site opened tomorrow
 * should see the account the wallet is on, not the one it was on last week.
 */
import { loadCore } from '$lib/core/client';
import { checksumAddress } from '$lib/core/kernels';
import type { Account } from '$lib/core/generated/Account';
import { parallelFlagSet } from '$lib/dev/parallel-flag.svelte';
import { listGrants } from './connections';
import { getGrant, revokeGrant, setGrant } from './grants';
import { planAccountSwitch } from './core/dperm-connect';
import { toWireGrant } from './core/dperm-types';
import { publishExtSnapshot, type SnapshotFacts } from './core/ext-cache';
import { sitesAreSetAside } from './space-sites';

export interface FollowFacts {
	/** The account the wallet switched TO. */
	activeAddress: string;
	/** Every wallet address; `null` while storage is still being read. */
	addresses: string[] | null;
	nowMs?: number;
	/**
	 * Whether those are the parallel space's accounts rather than the
	 * person's. Left out, it is read from the space's stored flag — the one
	 * fact every document of this origin shares, a side panel that was open
	 * when the space was entered in a tab included. The core is told so
	 * unless the person's sites are known to be set aside (see below).
	 */
	space?: boolean;
}

/** What was written, for the caller (and the tests) to see. */
export interface FollowOutcome {
	repinned: string[];
	removed: string[];
}

export async function followActiveAccount(facts: FollowFacts): Promise<FollowOutcome> {
	const outcome: FollowOutcome = { repinned: [], removed: [] };
	const grants = await listGrants();
	if (grants.length === 0) return outcome;
	await loadCore();
	const nowMs = facts.nowMs ?? Date.now();
	// The parallel space swaps the account list for its fixtures. Asked
	// without this, the core takes every grant of the person's for one whose
	// account left the wallet and removes it — entering the space cut every
	// site off. Told, it leaves alone whatever is not for one of the space's
	// accounts.
	//
	// Told only while the person's grants can be among the ones asked about:
	// when they could not be set aside (`space-sites.ts`). Once they are,
	// every grant in the store is the SPACE's own and follows the space's
	// accounts by the ordinary rule — re-pinned on a switch, dropped when its
	// account leaves the space's list (issue 315's walk, which runs inside the
	// space). Told there too, the core would take a grant of an account that
	// left the space's list for the person's, and keep it.
	const inSpace = facts.space ?? parallelFlagSet();
	const space = inSpace && !(await sitesAreSetAside());

	for (const stored of grants) {
		if (stored.address.toLowerCase() === facts.activeAddress.toLowerCase()) continue;
		const plan = planAccountSwitch({
			origin: stored.origin,
			storedGrant: toWireGrant(stored)!,
			currentAddresses: facts.addresses,
			activeAddress: facts.activeAddress,
			nowMs,
			space
		});
		switch (plan.kind) {
			case 'repin':
				await setGrant({
					origin: plan.grant.origin,
					address: plan.grant.address,
					chainId: plan.grant.chain_id,
					grantedAt: plan.grant.granted_at_ms
				});
				outcome.repinned.push(stored.origin);
				break;
			case 'remove':
				await revokeGrant(stored.origin);
				outcome.removed.push(stored.origin);
				break;
			case 'none':
				break;
		}
	}
	return outcome;
}

/** What the follow reads of the session — `SessionView`, narrowed. */
export interface FollowedSession {
	loading: boolean;
	has_wallet: boolean;
	address: string;
	active_index: number;
	accounts: readonly { account: Account }[];
}

/**
 * The worker's picture of the session, and every grant, kept on the account
 * the person is signed in to — from EVERY screen (spec 082 RJ20, G58) and on
 * every session the document settles on, its first included (spec 086, issue
 * 315).
 *
 * Two writes, in this order, each the core's:
 *
 *   1. the snapshot (`ext_cache`) — who is signed in, which is what the
 *      service worker answers `eth_accounts` from. It used to be published by
 *      the wallet page alone, so a switch made in Settings left the worker
 *      answering for the account the person had left;
 *   2. the grants (`followActiveAccount`) — each re-pinned to the signed-in
 *      account, or dropped when its own account left the device; the worker
 *      announces each change to the site's tabs. Second, so a page that hears
 *      `accountsChanged([new])` and asks `eth_accounts` reads the new account.
 *
 * The FIRST session a document settles on is followed too. It used to be "a
 * boot, not a switch" — but a person who signs out and signs in to another
 * wallet, in a fresh document, never switches in the follower's sight, and
 * every site stayed on the previous account (issue 315). The in-app browsers
 * have always done this (`dapp_browser`: "the active account changed —
 * initial load included"). Re-asking about a session already followed is a
 * no-op: the same session is noted once.
 *
 * Signed out, the snapshot goes and the grants stay: signing back in lines
 * them back up (`session.rs`), and the worker tells each site it is no longer
 * answered.
 */
export class SessionFollower {
	#followed: string | null = null;
	/** One follow at a time, in the order the sessions were seen. */
	#queue: Promise<unknown> = Promise.resolve();
	readonly #publish: (facts: SnapshotFacts) => Promise<unknown>;

	constructor(publish: (facts: SnapshotFacts) => Promise<unknown> = publishExtSnapshot) {
		this.#publish = publish;
	}

	/**
	 * Called with each session view. Returns the follow it started, or `null`
	 * when there is nothing to follow (still loading, or the session already
	 * followed).
	 */
	note(
		view: FollowedSession,
		options: { locale?: string; nowMs?: number } = {}
	): Promise<FollowOutcome> | null {
		if (view.loading) return null;
		// Read now: the view is live state, and the follow runs later.
		const address = view.address;
		const accounts = view.accounts.map((row) => row.account);
		const addresses = accounts.map((account) => account.address);
		// Read with the accounts it describes: whose list this is must not be
		// decided later, when the queued follow runs.
		const space = parallelFlagSet();
		const key = JSON.stringify([
			view.has_wallet,
			address.toLowerCase(),
			addresses.map((a) => a.toLowerCase()),
			space
		]);
		if (key === this.#followed) return null;
		this.#followed = key;
		const facts: SnapshotFacts = {
			isLoading: false,
			hasWallet: view.has_wallet,
			accounts,
			active: accounts[view.active_index] ?? null,
			theme: 'dark',
			locale: options.locale ?? 'en'
		};
		const run = this.#queue.then(async (): Promise<FollowOutcome> => {
			await this.#publish(facts);
			if (!address) return { repinned: [], removed: [] };
			return followActiveAccount({
				activeAddress: address,
				addresses,
				nowMs: options.nowMs,
				space
			});
		});
		this.#queue = run.catch(() => {});
		return run;
	}
}

/** The document's one follower (the root layout feeds it). */
export const sessionFollower = new SessionFollower();

/**
 * One spelling of an address toward every site (spec 082 RG10, L-D6).
 *
 * The core now writes a grant's address EIP-55 (its `write_grant`), and the
 * provider hands a page exactly what the worker reads from the grant. Grants
 * written before 082 are lower-case, so a site saw `0xabc…` on `eth_accounts`
 * and `0xAbC…` after an account switch — the same account in two spellings,
 * which some dApps read as two accounts. This rewrites the old ones once per
 * document, from the root layout — whichever screen the wallet boots on (G58:
 * a panel that opened on Settings left them lower-case). A rewrite changes
 * only the case, so the worker (which compares case-insensitively) announces
 * nothing to the site's tabs.
 *
 * Idempotent: a grant already in the core's spelling is left alone. Returns
 * the origins it rewrote.
 */
export async function normalizeGrantSpelling(): Promise<string[]> {
	const grants = await listGrants();
	if (grants.length === 0) return [];
	await loadCore();
	const rewritten: string[] = [];
	for (const listed of grants) {
		// Read again right before the write: the list is a moment old, and a
		// site disconnected or re-pinned since then must stay as it now is —
		// writing back the spelling of what was listed would reconnect it, or
		// point it at the account it was moved off.
		const grant = await getGrant(listed.origin);
		if (!grant || grant.address !== listed.address) continue;
		let spelled: string;
		try {
			spelled = checksumAddress(grant.address);
		} catch {
			continue; // not an address the core can spell — leave it for the core's own read
		}
		if (spelled === grant.address) continue;
		await setGrant({ ...grant, address: spelled });
		rewritten.push(grant.origin);
	}
	return rewritten;
}
