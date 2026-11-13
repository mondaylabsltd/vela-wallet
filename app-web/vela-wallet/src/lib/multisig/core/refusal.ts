/**
 * What the screens are told about who can act for a multisig (§3.5) — the
 * core's verdicts (`authority.ts` gathers them), in the shapes the sheets
 * draw. Nothing here runs the core.
 */
import type { Authority, Refusal, Risk } from './kernel';

/**
 * One thing found about a multisig on a chain: a refusal of its genesis
 * (it is not deployed there), or what else can act for it where it is.
 */
export type Finding =
	| { from: 'genesis'; chainId: number; reason: Refusal }
	| { from: 'chain'; chainId: number; risk: Risk };

/**
 * The executor will not deploy the multisig on `chainId`: the core refused
 * its genesis there, or could not verify it (the chain did not answer).
 * Thrown before any passkey is asked.
 */
export class GenesisRefusedError extends Error {
	constructor(
		readonly chainId: number,
		readonly authority: Exclude<Authority, { kind: 'verified' }>
	) {
		super(
			authority.kind === 'refused'
				? `Vela does not deploy this multisig on chain ${chainId}: ${authority.reasons.map((r) => r.kind).join(', ')}`
				: `the Safe contracts on chain ${chainId} could not be checked`
		);
		this.name = 'GenesisRefusedError';
	}
}
