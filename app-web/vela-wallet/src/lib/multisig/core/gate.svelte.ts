/**
 * Whether this device signs for a multisig on a network (§3.5, §3.6), asked
 * right before every passkey — approve, propose (a transfer, a rename, a
 * member change, a dApp's calls, a cancellation), execute, or an outside
 * wallet's approval. The rule is the core's (`multisig_review::signing_gate`);
 * this file reads what it needs and acts on its answer:
 *
 * - its genesis refused there (`refused`): nothing is signed, ever — the
 *   network is read-only for it;
 * - the transaction itself is one Vela does not sign (`blocked`): it cannot
 *   be read in full, or executing it changes who controls the multisig in a
 *   way it does not declare (the state-diff guard) — no override;
 * - something besides its members can act for it there (`risk`): the person
 *   is asked, naming what was found, every time — one confirmation per
 *   signature, never remembered;
 * - neither: signed as always.
 *
 * Read from the chain right before each signature, never this session's
 * verdict (`checkNow`): who can act for it there, its members now, and the
 * execution's state diff. A chain that does not answer signs nothing yet
 * (`SigningUncheckedError`).
 */
import type { Finding } from './refusal';
import { checkNow, findingsOn } from './recheck';
import { readMembers } from './chain';
import { signingGateRule, type MultisigReviewOut, type SafeTx, type SignRule } from './kernel';
import { gatherReview, recentReview, reviewOf } from './proposal-review.svelte';
import type { MultisigAccount } from './store.svelte';

export type SigningGate =
	| { kind: 'open' }
	/** Its genesis refused on this network: read-only here. */
	| { kind: 'refused'; chainId: number; findings: Finding[] }
	/** Something besides its members can act for it here: each signature asks. */
	| { kind: 'risk'; chainId: number; findings: Finding[] }
	/** This transaction is not one Vela signs: no button, no override. */
	| { kind: 'blocked'; chainId: number };

/** The core's gate over `findings` on `chainId` and, for a transaction, its review's `sign`. */
export function gateOf(findings: Finding[], chainId: number, sign?: SignRule | null): SigningGate {
	const gate = signingGateRule({ chainId, findings, sign: sign ?? null });
	switch (gate.kind) {
		case 'refused':
			return { kind: 'refused', chainId, findings: gate.findings as Finding[] };
		case 'risk':
			return { kind: 'risk', chainId, findings: gate.findings as Finding[] };
		case 'blocked':
			return { kind: 'blocked', chainId };
		default:
			return { kind: 'open' };
	}
}

/** The gate on `chainId` as this device knows it now (the screens draw it). */
export function signingGate(
	account: Pick<MultisigAccount, 'address' | 'findings'>,
	chainId: number,
	sign?: SignRule | null
): SigningGate {
	return gateOf(findingsOn(account, chainId), chainId, sign);
}

/** Its genesis is refused on `chainId`: Vela signs nothing for it there. */
export class SigningRefusedError extends Error {
	constructor(
		readonly chainId: number,
		readonly findings: Finding[]
	) {
		super(`Vela does not sign for this multisig on chain ${chainId}`);
		this.name = 'SigningRefusedError';
	}
}

/** Who can act for it on `chainId` could not be read: nothing is signed yet. */
export class SigningUncheckedError extends Error {
	constructor(readonly chainId: number) {
		super(`who can act for this multisig on chain ${chainId} could not be checked`);
		this.name = 'SigningUncheckedError';
	}
}

/**
 * The transaction is not one Vela signs, read right before the signature:
 * its execution changes who controls the multisig without declaring it, or
 * it cannot be read in full. `review` says what was found.
 */
export class SigningBlockedError extends Error {
	constructor(
		readonly chainId: number,
		readonly review: MultisigReviewOut
	) {
		super(`Vela does not sign this transaction on chain ${chainId}`);
		this.name = 'SigningBlockedError';
	}
}

export interface RiskRequest {
	chainId: number;
	safe: string;
	findings: Finding[];
	answer(signs: boolean): void;
}

/**
 * The confirmation a signature on a high-risk network asks — drawn by
 * `RiskConfirmSheet` (the multisig host mounts it, on top of any sheet or
 * the send flow). Where no host is up to ask, nothing is signed.
 */
class RiskConfirmation {
	/** Raw, not proxied: an answer clears the request it belongs to (identity, not a copy). */
	request = $state.raw<RiskRequest | null>(null);
	#hosts = 0;

	/** A host draws the requests while it is mounted; the returned function unmounts it. */
	attach(): () => void {
		this.#hosts += 1;
		return () => {
			this.#hosts -= 1;
			if (this.#hosts === 0) this.request?.answer(false);
		};
	}

	/** Whether the person signs, having read `findings` (`false`: declined, or nowhere to ask). */
	ask(input: Omit<RiskRequest, 'answer'>): Promise<boolean> {
		this.request?.answer(false);
		if (this.#hosts === 0) return Promise.resolve(false);
		return new Promise<boolean>((resolve) => {
			const request: RiskRequest = {
				...input,
				answer: (signs) => {
					if (this.request === request) this.request = null;
					resolve(signs);
				}
			};
			this.request = request;
		});
	}
}

export const riskConfirmation = new RiskConfirmation();

/**
 * The review's sign rule for `tx` as the chain stands now: its members read
 * again, the execution's state diff read again — the part of the review a
 * signature depends on. `null`: the chain did not answer (nothing is signed).
 */
async function signRuleNow(
	account: MultisigAccount,
	chainId: number,
	tx: SafeTx
): Promise<{ sign: SignRule; review: MultisigReviewOut } | null> {
	try {
		const members = await readMembers(chainId, account.address, account.genesis);
		const subject = { safe: account.address, chainId, tx, members, genesis: account.genesis };
		// r5reg-14: the review the sheet gathered moments ago, at these very
		// members and nonce, is the one signed from — not gathered twice.
		const input = recentReview(subject) ?? (await gatherReview(subject));
		const review = reviewOf(input, false);
		return { sign: review.sign, review };
	} catch {
		return null;
	}
}

/**
 * Before any signature for `account` on `chainId` over `tx`: throws
 * `SigningRefusedError` (read-only there), `SigningBlockedError` (Vela does
 * not sign this transaction), `SigningUncheckedError` (not known yet), or —
 * the person declined the risk — an `AbortError`, read by every caller as a
 * cancel. Resolves when signing may go on.
 */
export async function authorizeSigning(
	account: MultisigAccount,
	chainId: number,
	tx: SafeTx
): Promise<void> {
	const [found, rule] = await Promise.all([
		checkNow(account, chainId),
		signRuleNow(account, chainId, tx)
	]);
	if (found === null || rule === null) throw new SigningUncheckedError(chainId);
	const gate = signingGateRule({ chainId, findings: found, sign: rule.sign });
	if (gate.kind === 'open') return;
	if (gate.kind === 'unchecked') throw new SigningUncheckedError(chainId);
	if (gate.kind === 'refused') throw new SigningRefusedError(chainId, gate.findings as Finding[]);
	if (gate.kind === 'blocked') throw new SigningBlockedError(chainId, rule.review);
	const signs = await riskConfirmation.ask({
		chainId,
		safe: account.address,
		findings: gate.findings as Finding[]
	});
	if (!signs) throw riskDeclinedError();
}

/** What the gate throws when the person chose "Don't sign" (an `AbortError`, so a cancel). */
const RISK_DECLINED = 'The person did not sign for the risk';

/** The person's "Don't sign" on the risk, as the gate says it ({@link declinedRisk} reads it). */
export function riskDeclinedError(): DOMException {
	return new DOMException(RISK_DECLINED, 'AbortError');
}

/**
 * The person declined to sign on a high-risk network: nothing was asked of
 * a passkey — a screen goes back to where it was, never "the passkey prompt
 * was closed".
 */
export function declinedRisk(error: unknown): boolean {
	return error instanceof DOMException && error.message === RISK_DECLINED;
}
