/**
 * How an execution stands before its passkey, in the screens' vocabulary: the
 * proposals machine looks (`previews`, an `execute:*` action `stopped`) and
 * decides; these are the shapes the sheet and the home word it in, and the
 * errors a stopped execution is said through. Nothing here reads or decides.
 */
import type { MultisigAmount } from '../generated/MultisigAmount';
import type { QuotedInBandFee } from '$lib/services/safe-transaction';
import type { GenesisRefusedError } from './refusal';
import type { Call } from './kernel';

/** Why an execution would fail if it were sent now. */
export type ExecutionBlock =
	/** The multisig holds less of a coin than the transaction spends. */
	/**
	 * r12-1: `have` / `need` as the core says them (`MultisigAmount`): in the
	 * token's decimals and symbol, or "decimals unknown" with base units —
	 * never scaled here.
	 */
	| { kind: 'short'; have: MultisigAmount; need: MultisigAmount }
	/** It reverts when it runs; the reason the chain or relay gave, if any. */
	| { kind: 'reverts'; reason: string | null };

export type ExecutionPreview =
	/** §3.5: it would deploy the multisig from a genesis the core does not find Verified there. */
	| { kind: 'refused'; error: GenesisRefusedError }
	| { kind: 'fails'; why: ExecutionBlock }
	/** The fee could not be priced now (the relay or the chain did not answer). */
	| { kind: 'unpriced' }
	/**
	 * Round 5: the look did not answer in time (a read that hangs): whether it
	 * would run is not known. Approving does not need it; executing looks
	 * again before the passkey.
	 */
	| { kind: 'unchecked' }
	/**
	 * r5reg-21: its number is used on the chain now — another member executed
	 * it (or another transaction at its number ran) while it was open. Never
	 * a failure: what ran is read and said, not "it would fail".
	 */
	| { kind: 'spent' }
	| {
			kind: 'priced';
			/** The calls priced — exactly the ones signed. */
			calls: Call[];
			/** The fee shown, and signed verbatim, in the chain's coin. */
			fee: QuotedInBandFee;
			/** The executor's coin on the chain; `null` when it could not be read. */
			balance: bigint | null;
			/** The executor holds the fee (or its balance could not be read). */
			canPay: boolean;
			/** The members whose given approvals it carries ({@link PlannedExecution}). */
			carries: string[];
			/** The executor approves by sending it (`msg.sender`). */
			executorApproves: boolean;
			setup: {
				multisig: boolean;
				/** Members whose accounts it activates (deploys) for their approvals. */
				members: string[];
				/** The executor's own account is deployed by this, its first operation there. */
				executor: boolean;
				/** About how much of the fee is that one-time setup; `null` when unmeasured. */
				amount: bigint | null;
			};
	  };

/**
 * The executor cannot pay for the execution: its fee (deploying the multisig
 * and the owners it needs, then `execTransaction`) is more than the account
 * holds. Said before the passkey is asked — a relay may accept an operation
 * that cannot pay, and it would never land. `need` is the fee and what the
 * calls send; `have` the account's balance, both in the chain's coin.
 */
export class ExecutorCannotPayError extends Error {
	constructor(
		readonly need: bigint,
		readonly have: bigint
	) {
		super('the executor cannot pay the network fee');
		this.name = 'ExecutorCannotPayError';
	}
}

/**
 * r14-9: the core quoted the execution again at the press and its fee is not
 * the one the sheet said — nothing was signed. `amount` is the new fee (the
 * sheet's fee line shows it now); the next press signs exactly it.
 */
export class ExecutionFeeMovedError extends Error {
	constructor(
		readonly amount: bigint,
		readonly executor: string
	) {
		super('the network fee moved since the press: nothing was signed');
		this.name = 'ExecutionFeeMovedError';
	}
}

/** It would fail if sent now (see {@link ExecutionBlock}): nothing was signed. */
export class ExecutionWouldFailError extends Error {
	constructor(readonly why: ExecutionBlock) {
		super('the execution would fail');
		this.name = 'ExecutionWouldFailError';
	}
}

/** The fee could not be priced now: nothing was signed, and a retry may price it. */
export class ExecutionUnpricedError extends Error {
	constructor() {
		super('the network fee could not be worked out');
		this.name = 'ExecutionUnpricedError';
	}
}

/** Whether it would run could not be checked in time: nothing was signed, and a retry may check it. */
export class ExecutionUncheckedError extends Error {
	constructor() {
		super('whether the execution would run could not be checked');
		this.name = 'ExecutionUncheckedError';
	}
}

/** r5reg-21: its number is used on the chain now — nothing to execute; what ran is read. */
export class ExecutionSpentError extends Error {
	constructor() {
		super('its number is used on the chain');
		this.name = 'ExecutionSpentError';
	}
}
