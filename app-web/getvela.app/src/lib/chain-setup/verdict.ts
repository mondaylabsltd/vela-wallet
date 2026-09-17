/**
 * The verdict, and the plan that follows from it. Pure — every rule about what
 * a chain needs lives here, testable without a network.
 *
 * Three answers, in this order of precedence:
 *
 * - **blocked** — no P-256 at `0x100`. Nothing deployable changes this; the
 *   verifier address is part of every Vela address, so only the chain can fix
 *   it. Deployment steps are not offered, because offering them would imply
 *   they lead somewhere.
 * - **needs-setup** — P-256 is there, some contracts are not. The plan lists
 *   only what is missing, in dependency order, each step saying who can do it.
 * - **ready** — everything the wallet requires is present.
 */
import {
	ARACHNID_DEPLOYER_EOA,
	ARACHNID_FUNDING_WEI,
	MULTICALL3_DEPLOYER_EOA,
	MULTICALL3_FUNDING_WEI,
	estimatedCreate2Gas
} from './deployers';
import type { ContractStatus, P256Status } from './rpc';
import type { Factory, RequiredContract } from './required-contracts';

export type Verdict = 'ready' | 'needs-setup' | 'blocked';

export type Step =
	/** Fund a one-time sender, then broadcast the pre-signed bytes. Anyone can. */
	| {
			kind: 'fund-and-broadcast';
			contract: RequiredContract;
			deployer: `0x${string}`;
			fundingWei: bigint;
			rawTx: 'arachnid' | 'multicall3';
	  }
	/** Only Safe can do this one; the step is a request and a re-check. */
	| { kind: 'external'; contract: RequiredContract }
	/** CREATE2 through a factory, from any funded account. */
	| {
			kind: 'create2';
			contract: RequiredContract;
			factory: Factory;
			/** The factory it needs is itself missing — this step waits on that one. */
			blockedBy: RequiredContract | null;
	  };

export interface Plan {
	verdict: Verdict;
	p256: P256Status;
	present: ContractStatus[];
	missing: ContractStatus[];
	/** Addresses occupied by the wrong code — a chain problem, not a deployment step. */
	mismatched: ContractStatus[];
	/** Reads that failed; the verdict is provisional until these answer. */
	unknown: ContractStatus[];
	steps: Step[];
	/** The subset of `steps` this page can run itself (no external party). */
	runnable: Step[];
}

export function plan(contracts: ContractStatus[], p256: P256Status): Plan {
	const present = contracts.filter((c) => c.deployed);
	const missing = contracts.filter((c) => !c.deployed && !c.unknown);
	const mismatched = contracts.filter((c) => c.mismatch);
	const unknown = contracts.filter((c) => c.unknown);

	const isMissing = (key: string) => missing.some((c) => c.contract.key === key);
	const byKey = (key: string) => contracts.find((c) => c.contract.key === key)?.contract ?? null;

	const steps: Step[] = missing.map(({ contract }): Step => {
		switch (contract.method) {
			case 'presigned':
				return contract.key === 'arachnidProxy'
					? {
							kind: 'fund-and-broadcast',
							contract,
							deployer: ARACHNID_DEPLOYER_EOA,
							fundingWei: ARACHNID_FUNDING_WEI,
							rawTx: 'arachnid'
						}
					: {
							kind: 'fund-and-broadcast',
							contract,
							deployer: MULTICALL3_DEPLOYER_EOA,
							fundingWei: MULTICALL3_FUNDING_WEI,
							rawTx: 'multicall3'
						};
			case 'external':
				return { kind: 'external', contract };
			case 'create2': {
				const factory = contract.factory!;
				const factoryKey = factory === 'arachnid' ? 'arachnidProxy' : 'safeSingletonFactory';
				return {
					kind: 'create2',
					contract,
					factory,
					blockedBy: isMissing(factoryKey) ? byKey(factoryKey) : null
				};
			}
		}
	});

	const verdict: Verdict =
		p256 === 'missing'
			? 'blocked'
			: missing.length === 0 && unknown.length === 0
				? 'ready'
				: 'needs-setup';

	return {
		verdict,
		p256,
		present,
		missing,
		mismatched,
		unknown,
		// A blocked chain gets no plan: nothing on it leads to a working wallet.
		steps: verdict === 'blocked' ? [] : steps,
		runnable: verdict === 'blocked' ? [] : steps.filter((s) => s.kind !== 'external')
	};
}

/**
 * What the throwaway key needs for the CREATE2 steps still to run, at today's
 * gas price, with a quarter on top for a base fee that moves while it works.
 * The two keyless deployments are funded separately and exactly, so they are
 * not in this number.
 */
export function estimatedFundingWei(p: Plan, gasPriceWei: bigint): bigint {
	let gas = 0n;
	for (const s of p.steps) if (s.kind === 'create2') gas += estimatedCreate2Gas(s.contract.key);
	return (gas * gasPriceWei * 125n) / 100n;
}
