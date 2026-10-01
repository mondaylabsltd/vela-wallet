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
import { listGrants } from './connections';
import { getGrant, revokeGrant, setGrant } from './grants';
import { planAccountSwitch } from './core/dperm-connect';
import { toWireGrant } from './core/dperm-types';

export interface FollowFacts {
	/** The account the wallet switched TO. */
	activeAddress: string;
	/** Every wallet address; `null` while storage is still being read. */
	addresses: string[] | null;
	nowMs?: number;
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

	for (const stored of grants) {
		if (stored.address.toLowerCase() === facts.activeAddress.toLowerCase()) continue;
		const plan = planAccountSwitch({
			origin: stored.origin,
			storedGrant: toWireGrant(stored)!,
			currentAddresses: facts.addresses,
			activeAddress: facts.activeAddress,
			nowMs
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
	address: string;
	accounts: readonly { account: { address: string } }[];
}

/**
 * The account a connected site should be on, followed from EVERY screen (spec
 * 082 RJ20, G58).
 *
 * The FIRST address the session settles on is a boot, not a switch: only a
 * change from one known address to another asks the core to re-pin the
 * grants (`followActiveAccount`), and the worker announces each re-pinned
 * grant to the site's tabs as `accountsChanged`.
 *
 * Before 082 round 2 this lived in the wallet page — an effect over a
 * component-local `followedAddress`. A switch made in Settings (切换账户) never
 * reached a site, because the wallet page was not mounted; and coming back to
 * the wallet remounted it with `null`, so the change read as a boot and was
 * dropped for good. The root layout now calls `note()` on every session view,
 * and the address it last saw lives here, in the module, for the document's
 * whole life.
 */
export class SessionFollower {
	#followed: string | null = null;

	/**
	 * Called with each session view. Returns the re-pin it started, or `null`
	 * when the view is not a switch (still loading, the boot, the same account).
	 */
	note(view: FollowedSession, nowMs?: number): Promise<FollowOutcome> | null {
		if (view.loading || !view.address) return null;
		const previous = this.#followed;
		this.#followed = view.address;
		if (previous === null || previous.toLowerCase() === view.address.toLowerCase()) return null;
		return followActiveAccount({
			activeAddress: view.address,
			addresses: view.accounts.map((row) => row.account.address),
			nowMs
		});
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
