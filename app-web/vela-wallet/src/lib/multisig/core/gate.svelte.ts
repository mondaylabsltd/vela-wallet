/**
 * Whether this device signs for a multisig on a network (§3.5), asked
 * before every passkey — approve, propose (a transfer, a rename, a dApp's
 * calls, a cancellation), execute, or an outside wallet's approval:
 *
 * - its genesis refused there (`refused`): nothing is signed, ever — the
 *   network is read-only for it;
 * - something besides its members can act for it there (`risk`): the person
 *   is asked, naming what was found, every time — one confirmation per
 *   signature, never remembered;
 * - neither: signed as always.
 *
 * What is known comes from the chain, checked this session (`recheck.ts`):
 * a network not checked yet is checked first, and one that does not answer
 * signs nothing yet (`SigningUncheckedError`).
 */
import type { Finding } from './refusal';
import { checkOn, findingsOn } from './recheck';
import type { MultisigAccount } from './store.svelte';

export type SigningGate =
	| { kind: 'open' }
	/** Its genesis refused on this network: read-only here. */
	| { kind: 'refused'; chainId: number; findings: Finding[] }
	/** Something besides its members can act for it here: each signature asks. */
	| { kind: 'risk'; chainId: number; findings: Finding[] };

/** The gate `findings` make on `chainId`. */
export function gateOf(findings: Finding[], chainId: number): SigningGate {
	const here = findings.filter((f) => f.chainId === chainId);
	const refused = here.filter((f) => f.from === 'genesis');
	if (refused.length > 0) return { kind: 'refused', chainId, findings: refused };
	const risks = here.filter((f) => f.from === 'chain');
	return risks.length > 0 ? { kind: 'risk', chainId, findings: risks } : { kind: 'open' };
}

/** The gate on `chainId` as this device knows it now (the screens draw it). */
export function signingGate(
	account: Pick<MultisigAccount, 'address' | 'findings'>,
	chainId: number
): SigningGate {
	return gateOf(findingsOn(account, chainId), chainId);
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
 * Before any signature for `account` on `chainId`: throws
 * `SigningRefusedError` (read-only there), `SigningUncheckedError` (not
 * known yet), or — the person declined the risk — an `AbortError`, read by
 * every caller as a cancel. Resolves when signing may go on.
 */
export async function authorizeSigning(account: MultisigAccount, chainId: number): Promise<void> {
	const found = await checkOn(account, chainId);
	if (found === null) throw new SigningUncheckedError(chainId);
	const gate = gateOf(found, chainId);
	if (gate.kind === 'open') return;
	if (gate.kind === 'refused') throw new SigningRefusedError(chainId, gate.findings);
	const signs = await riskConfirmation.ask({
		chainId,
		safe: account.address,
		findings: gate.findings
	});
	if (!signs) throw new DOMException(RISK_DECLINED, 'AbortError');
}

/** What the gate throws when the person chose "Don't sign" (an `AbortError`, so a cancel). */
const RISK_DECLINED = 'The person did not sign for the risk';

/**
 * The person declined to sign on a high-risk network: nothing was asked of
 * a passkey — a screen goes back to where it was, never "the passkey prompt
 * was closed".
 */
export function declinedRisk(error: unknown): boolean {
	return error instanceof DOMException && error.message === RISK_DECLINED;
}
