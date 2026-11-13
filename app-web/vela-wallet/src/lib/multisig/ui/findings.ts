/**
 * What the core found about who can act for a multisig (§3.5), in the
 * person's words: one line per reason or risk, each naming the address (and
 * the network, where it is the chain's) — said plainly, never summarised
 * away.
 */
import { fillTemplate } from '$lib/i18n/fill';
import { chainName } from '$lib/services/networks';
import type { Finding } from '../core/refusal';
import type { Refusal, Risk } from '../core/kernel';
import type { MultisigMessages } from '../messages';

type Copy = MultisigMessages['authority'];

const RISK_KEY: Record<Risk['kind'], keyof Copy> = {
	module: 'riskModule',
	guard: 'riskGuard',
	handler: 'riskHandler',
	singleton: 'riskSingleton'
};

/** One refusal of a genesis, as its line reads (`chainId`: where it was judged). */
export function refusalText(m: MultisigMessages, reason: Refusal, chainId: number): string {
	const network = chainName(chainId);
	switch (reason.kind) {
		case 'payment':
			return fillTemplate(m.authority.payment, { address: reason.receiver });
		case 'wrongAddress':
		case 'notSetup':
		case 'badMembers':
			return m.authority[reason.kind];
		default:
			return fillTemplate(m.authority[reason.kind], { address: reason.address, network });
	}
}

/** One finding, as its line reads. */
export function findingText(m: MultisigMessages, finding: Finding): string {
	if (finding.from === 'genesis') return refusalText(m, finding.reason, finding.chainId);
	return fillTemplate(m.authority[RISK_KEY[finding.risk.kind]], {
		address: finding.risk.address,
		network: chainName(finding.chainId)
	});
}

/** Every finding's line, each once. */
export function findingLines(m: MultisigMessages, findings: Finding[]): string[] {
	return [...new Set(findings.map((f) => findingText(m, f)))];
}
