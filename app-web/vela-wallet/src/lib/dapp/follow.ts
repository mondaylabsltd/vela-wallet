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
import { listGrants } from './connections';
import { revokeGrant, setGrant } from './grants';
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
