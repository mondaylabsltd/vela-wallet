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
import {
	serviceAnswer,
	type Approval,
	type Genesis,
	type SafeTx,
	type ServiceAnswer
} from './kernel';
import { DEFAULT_GENESIS_SERVICE_URL, genesisService, serviceOutage } from './service-state.svelte';

export { DEFAULT_GENESIS_SERVICE_URL };

/** The service in use: the person's own (Settings → Service Endpoints), else the default. */
export function genesisServiceURL(): string {
	return genesisService.url;
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
/**
 * The longest wait a request over the service's per-client limit (`429`,
 * one-minute windows) waits out before asking once more: a person is waiting
 * on most calls — a longer wait is an answer of its own, not a pause.
 */
export const RETRY_AFTER_MAX_MS = 10_000;

/**
 * How long a `429` says to wait, in milliseconds: its `Retry-After` seconds,
 * else the seconds its body names ("too many requests; retry in 3 s") — a
 * page on another origin cannot read the header unless the service exposes
 * it. `null` when neither says.
 */
export function retryAfterMs(header: string | null, body = ''): number | null {
	const seconds = /^\d+$/.test(header?.trim() ?? '')
		? header!.trim()
		: (/retry in (\d+) ?s\b/.exec(body)?.[1] ?? null);
	return seconds === null ? null : Number(seconds) * 1000;
}

/**
 * `background`: a read nobody is waiting on (the 12 s refresh, a record
 * asked about again). While the service is away it asks nothing and fails at
 * once, so one outage is never a failed request per network per round.
 */
async function request<T>(
	path: string,
	init?: RequestInit,
	{ background = false }: { background?: boolean } = {}
): Promise<Answer<T>> {
	let last = 'no attempt';
	for (let attempt = 0; attempt < ATTEMPTS; attempt += 1) {
		if (attempt > 0) await new Promise((r) => setTimeout(r, 400 * 2 ** attempt));
		if (background && serviceOutage.blocked()) throw new ServiceUnavailableError(`away: ${last}`);
		const controller = new AbortController();
		const timer = setTimeout(() => controller.abort(), TIMEOUT_MS);
		try {
			let response: Response;
			try {
				response = await fetch(`${genesisServiceURL()}${path}`, {
					...init,
					signal: controller.signal,
					headers: init?.body ? { 'content-type': 'application/json' } : undefined
				});
			} catch (error) {
				// Nothing answered (refused, closed, offline, timed out): the
				// service is away, for every network at once.
				serviceOutage.failed();
				throw error;
			}
			// It answered — a chain's read failing behind it (a 502) is that
			// chain's, not the service being away.
			serviceOutage.answered();
			// A gateway's error page, or a cold start: worth one more try.
			if (response.status >= 500 && attempt < ATTEMPTS - 1) {
				last = `HTTP ${response.status}`;
				continue;
			}
			const text = await response.text();
			// Over the service's per-client limit: a short wait is waited out
			// once more; a long one is the answer (`429`), said as it is.
			const wait =
				response.status === 429 ? retryAfterMs(response.headers.get('retry-after'), text) : null;
			if (wait !== null && wait <= RETRY_AFTER_MAX_MS && attempt < ATTEMPTS - 1) {
				last = 'HTTP 429';
				await new Promise((r) => setTimeout(r, wait));
				continue;
			}
			const body = JSON.parse(text) as T;
			return { status: response.status, body };
		} catch (error) {
			last = error instanceof Error ? error.message : String(error);
		} finally {
			clearTimeout(timer);
		}
	}
	throw new ServiceUnavailableError(last);
}

const CHECK_TIMEOUT_MS = 8_000;

/**
 * What answers at `url` (`GET /api/health`, the service's cheapest read):
 * a multisig service and the chains it serves, something else, or nothing.
 * The core reads the answer. One try, no retries: a person is waiting.
 */
export async function checkService(url: string): Promise<ServiceAnswer> {
	const controller = new AbortController();
	const timer = setTimeout(() => controller.abort(), CHECK_TIMEOUT_MS);
	let response: Response;
	let text: string;
	try {
		response = await fetch(`${url}/api/health`, { signal: controller.signal, cache: 'no-store' });
		text = await response.text();
	} catch {
		return { kind: 'unreachable' };
	} finally {
		clearTimeout(timer);
	}
	return serviceAnswer(response.status, text);
}

/** Whether the service in use answers as itself now (the outage's one probe). */
export async function probeService(): Promise<boolean> {
	return (await checkService(genesisServiceURL())).kind === 'genesis';
}

/**
 * The networks a new multisig may be created on: those the service reads
 * (the first one is where its proposals are looked for; its record goes to
 * the service's home chain), all of them while that is not known.
 */
export function servedOnly<T extends { chainId: number }>(
	networks: T[],
	served: Set<number> | null
): T[] {
	return served === null ? networks : networks.filter((n) => served.has(n.chainId));
}

const SERVED_TTL_MS = 60_000;

/** What the service in use says of itself (`/api/health`), as far as it says. */
export interface ServiceTable {
	/** The chains it reads; `null`: it lists none. */
	chains: Set<number> | null;
	/**
	 * The chains its registry is on, where a record can be looked up (Gnosis
	 * and Ethereum; its home chain always); `null`: it lists none.
	 */
	registryChains: Set<number> | null;
	/** The chain its registry lives on — where it writes records; `null`: it does not say. */
	homeChainId: number | null;
}

let served: { url: string; at: number; table: Promise<ServiceTable | null> } | null = null;

/**
 * The service in use's own table, or `null` when it could not be read. Kept
 * a minute per service; one that could not be read is asked again next time.
 */
export function serviceTable(): Promise<ServiceTable | null> {
	const url = genesisServiceURL();
	if (served && served.url === url && Date.now() - served.at < SERVED_TTL_MS) return served.table;
	const table = checkService(url)
		.then((answer): ServiceTable | null =>
			answer.kind === 'genesis'
				? {
						chains: answer.chains === null ? null : new Set(answer.chains.map((c) => c.chainId)),
						registryChains:
							answer.chains === null
								? null
								: new Set(answer.chains.filter((c) => c.registry).map((c) => c.chainId)),
						homeChainId: answer.homeChainId
					}
				: null
		)
		.catch(() => null);
	const entry = { url, at: Date.now(), table };
	served = entry;
	void table.then((t) => {
		if (t === null && served === entry) served = null;
	});
	return table;
}

/**
 * The chains the service in use records on, by its own table, or `null` when
 * that is not known (it could not be read, or lists none) — nothing is then
 * narrowed.
 */
export async function servedChains(): Promise<Set<number> | null> {
	return (await serviceTable())?.chains ?? null;
}

// -- genesis ---------------------------------------------------------------------

/** What one registry holds for a Safe beyond its genesis: its own name, place and dates. */
export interface RegistryEntry {
	chainId: number;
	name: string | null;
	/** The name is the Safe's own (it renamed itself, or wrote the record). */
	nameBySafe: boolean;
	/** Block of its last rename on that chain; 0: never. */
	renamedAt: number;
	index: number;
	recorder: string;
	recordedAt: number;
}

/**
 * `GET /api/genesis/{safe}` (registry v4). The genesis is the same in every
 * registry; the name is each registry's own (`registries`), and `genesis.name`
 * is the one read from `readFrom` (the home chain's when it holds it). None
 * of it is trusted: the core re-derives the address from the genesis, and a
 * name is read from the chain itself where this wallet can.
 */
export interface GenesisAnswer {
	found: boolean;
	/** Proven but not recorded on chain yet (the service's mailbox holds it). */
	pending?: boolean;
	genesis?: Genesis;
	/** The registry chains that hold the record. */
	recordedOn?: number[];
	/** The chain the served record was read from. */
	readFrom?: number;
	verified?: boolean | null;
	nameBySafe?: boolean;
	renamedAt?: number;
	registries?: RegistryEntry[];
}

/** The genesis recorded for `safe`, from whichever chain holds it. */
export async function fetchGenesis(
	safe: string,
	options?: { background?: boolean }
): Promise<GenesisAnswer> {
	const { status, body } = await request<GenesisAnswer>(`/api/genesis/${safe}`, undefined, options);
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

/**
 * Ask the service to record a genesis (idempotent). Registry v4 records every
 * Safe with a name: a `genesis` carries its own (send it named,
 * `namedGenesis`); `name` is for one the service resolves itself, and without
 * either it names the record from the address. A name the registry would not
 * take is refused (`unrecordable`), never replaced.
 */
export async function importGenesis(req: {
	chainId: number;
	safe: string;
	genesis?: Genesis;
	ownerProof?: OwnerProof;
	name?: string;
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

export type StoredApproval = Approval & {
	createdMs: number;
};

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
export async function fetchProposals(
	chainId: number,
	safe: string,
	options?: { background?: boolean }
): Promise<ProposalsAnswer | null> {
	const { status, body } = await request<ProposalsAnswer>(
		`/api/proposals/${safe}?chainId=${chainId}`,
		undefined,
		options
	);
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
	approval: Approval;
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
