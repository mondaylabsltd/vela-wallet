/**
 * One network's verdict on who can act for a multisig (§3.5), as findings —
 * the same rule wherever it is asked: the re-check of the multisigs on this
 * device (`recheck.ts`) and the connect page, which acts for an outside
 * owner on a multisig this device may not hold (`external/outside`).
 */
import { counterfactualOn, deployedOn } from './authority';
import type { Genesis } from './kernel';
import type { Finding } from './refusal';

/**
 * What the chain says about `safe` on `chainId` now, as findings: `[]` —
 * only its members act for it (or its genesis is Verified there); `null` —
 * the chain did not answer, nothing can be said. Deployed there: the chain's
 * state (`deployedOn`); not deployed: the core's counterfactual rule, which
 * needs `genesis` (none: nothing can be said).
 */
export async function judgeOn(
	chainId: number,
	safe: string,
	genesis: Genesis | null,
	deployed: boolean
): Promise<Finding[] | null> {
	if (deployed) {
		const check = await deployedOn(chainId, safe).catch(() => null);
		if (!check || check.unread) return null;
		return check.risks.map((risk) => ({ from: 'chain' as const, chainId, risk }));
	}
	if (!genesis) return null;
	const authority = await counterfactualOn(chainId, safe, genesis).catch(() => null);
	if (!authority || authority.kind === 'unknown') return null;
	if (authority.kind === 'refused') {
		return authority.reasons.map((reason) => ({ from: 'genesis' as const, chainId, reason }));
	}
	return [];
}
