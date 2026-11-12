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
import { signingGateRule, type MultisigReviewOut, type SignRule } from './kernel';
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
	// The accounts machine's findings for it (its stored row's, or this
	// session's verdict there, `check_on`), on this network.
	const findings = (account.findings ?? []).filter((f) => f.chainId === chainId);
	return gateOf(findings, chainId, sign);
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

/**
 * Who can act for it on `chainId` could not be read: nothing is signed yet.
 * §39 (r22-2): `disagreed` — two independent nodes answered differently
 * (`ActionState::SigningDisagreed`), a real signal: nothing is signed, and
 * there is no "… anyway".
 */
export class SigningUncheckedError extends Error {
	constructor(
		readonly chainId: number,
		readonly disagreed = false
	) {
		super(
			disagreed
				? `two nodes disagree about who can act for this multisig on chain ${chainId}`
				: `who can act for this multisig on chain ${chainId} could not be checked`
		);
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
