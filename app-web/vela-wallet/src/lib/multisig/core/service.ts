/**
 * The Safe Genesis Registry service (`safe-genesis-registry`, genesis-cf):
 * where a multisig's genesis is recorded and read back by address, and the
 * proposal mailbox owners share.
 *
 * Nothing it answers is trusted: a genesis is re-derived to its address by
 * the core before it is used, and every proposal and approval is re-hashed
 * and re-verified (`kernel.ts`). The service is what lets another device find
 * them; it is never what decides they are right.
 *
 * Every call here is idempotent on the service's side (a record is
 * write-once, an approval merges), so a call that failed in flight is simply
 * made again.
 */
import type { Genesis, PasskeyApproval, SafeTx } from './kernel';

export const DEFAULT_GENESIS_SERVICE_URL = 'https://safe-genesis.getvela.app';
/** A developer's or a test's own service; unset means the default. */
export const GENESIS_SERVICE_KEY = 'vela.genesisServiceURL';

export function genesisServiceURL(): string {
	try {
		const own = localStorage.getItem(GENESIS_SERVICE_KEY);
		if (own && /^https?:\/\//.test(own)) return own.replace(/\/+$/, '');
	} catch {
		// storage unavailable: the default
	}
	return DEFAULT_GENESIS_SERVICE_URL;
}

/** The service could not be reached, or answered something unreadable. */
export class ServiceUnavailableError extends Error {
	constructor(readonly detail: string) {
		super(`genesis service unavailable: ${detail}`);
		this.name = 'ServiceUnavailableError';
	}
}

interface Answer<T> {
	status: number;
	body: T;
}

const TIMEOUT_MS = 15_000;
const ATTEMPTS = 3;

async function request<T>(path: string, init?: RequestInit): Promise<Answer<T>> {
	let last = 'no attempt';
	for (let attempt = 0; attempt < ATTEMPTS; attempt += 1) {
		if (attempt > 0) await new Promise((r) => setTimeout(r, 400 * 2 ** attempt));
		const controller = new AbortController();
		const timer = setTimeout(() => controller.abort(), TIMEOUT_MS);
		try {
			const response = await fetch(`${genesisServiceURL()}${path}`, {
				...init,
				signal: controller.signal,
				headers: init?.body ? { 'content-type': 'application/json' } : undefined
			});
			// A gateway's error page, or a cold start: worth one more try.
			if (response.status >= 500 && attempt < ATTEMPTS - 1) {
				last = `HTTP ${response.status}`;
				continue;
			}
			const body = (await response.json()) as T;
			return { status: response.status, body };
		} catch (error) {
			last = error instanceof Error ? error.message : String(error);
		} finally {
			clearTimeout(timer);
		}
	}
	throw new ServiceUnavailableError(last);
}

// -- genesis ---------------------------------------------------------------------

export interface GenesisAnswer {
	found: boolean;
	genesis?: Genesis;
	recordedOn?: number[];
	verified?: boolean | null;
}

/** The genesis recorded for `safe`, from whichever chain holds it. */
export async function fetchGenesis(safe: string): Promise<GenesisAnswer> {
	const { status, body } = await request<GenesisAnswer>(`/api/genesis/${safe}`);
	if (status === 404) return { found: false };
	if (status !== 200) throw new ServiceUnavailableError(`HTTP ${status}`);
	return body;
}

/** An owner's passkey asking for a multisig deployed nowhere yet to be recorded. */
export interface OwnerProof {
	owner: string;
	ownerGenesis: Genesis;
	authenticatorData: string;
	/** The exact text the authenticator signed. */
	clientDataJSON: string;
	signature: string;
}

export type ImportAnswer =
	| { status: 'recorded'; recordedOn: number[] }
	| { status: 'queued'; taskId: string }
	| { status: 'resolved'; reason: string; recordCall: { to: string; data: string } }
	| { status: 'refused'; refusal: unknown }
	| { status: 'error'; error: string };

/** Ask the service to record a genesis (idempotent). */
export async function importGenesis(req: {
	chainId: number;
	safe: string;
	genesis?: Genesis;
	ownerProof?: OwnerProof;
}): Promise<ImportAnswer> {
	const { status, body } = await request<Record<string, unknown>>('/api/import', {
		method: 'POST',
		body: JSON.stringify(req)
	});
	if (status === 422) return { status: 'refused', refusal: body.refusal };
	if (status >= 400) return { status: 'error', error: String(body.error ?? `HTTP ${status}`) };
	return body as unknown as ImportAnswer;
}

export async function fetchTask(id: string): Promise<{ status: string; error?: string } | null> {
	const { status, body } = await request<{ status: string; error?: string }>(`/api/task/${id}`);
	return status === 200 ? body : null;
}

// -- the proposal mailbox -------------------------------------------------------

export interface Members {
	deployed: boolean;
	owners: string[];
	threshold: string;
	nonce: string;
}

export interface StoredApproval extends PasskeyApproval {
	createdMs: number;
}

export interface Proposal {
	chainId: number;
	safe: string;
	safeTxHash: string;
	safeTx: SafeTx;
	proposer: string;
	approvals: StoredApproval[];
	createdMs: number;
	updatedMs: number;
}

export interface ProposalsAnswer {
	members: Members;
	proposals: Proposal[];
}

/**
 * A multisig's proposals on a chain, with who may approve them there.
 * `null`: the service knows nothing of this multisig on that chain.
 */
export async function fetchProposals(chainId: number, safe: string): Promise<ProposalsAnswer | null> {
	const { status, body } = await request<ProposalsAnswer>(`/api/proposals/${safe}?chainId=${chainId}`);
	if (status === 404) return null;
	if (status !== 200) throw new ServiceUnavailableError(`HTTP ${status}`);
	return body;
}

export type SubmitAnswer =
	| { ok: true; status: 'created' | 'approved' | 'unchanged'; proposal: Proposal }
	| { ok: false; refusal: unknown };

/** Put a transaction and one owner's approval of it in the mailbox (idempotent). */
export async function submitApproval(req: {
	chainId: number;
	safe: string;
	safeTx: SafeTx;
	approval: PasskeyApproval;
	/** The multisig's genesis: lets the mailbox take a proposal before the registry holds the record. */
	genesis?: Genesis;
}): Promise<SubmitAnswer> {
	const { status, body } = await request<Record<string, unknown>>('/api/proposals', {
		method: 'POST',
		body: JSON.stringify(req)
	});
	if (status === 200 || status === 201) {
		return {
			ok: true,
			status: body.status as 'created' | 'approved' | 'unchanged',
			proposal: body.proposal as Proposal
		};
	}
	if (status === 422) return { ok: false, refusal: body.refusal };
	throw new ServiceUnavailableError(String(body.error ?? `HTTP ${status}`));
}
