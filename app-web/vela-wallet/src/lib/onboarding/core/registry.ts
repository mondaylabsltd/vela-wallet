/**
 * The public-key registry, over HTTP.
 *
 * Ported from `src/services/public-key-registry.ts` and
 * `src/services/registry-publish.ts` in the Expo client, narrowed to the six
 * operations the onboarding machines actually ask for.
 *
 * Nothing here decides anything. The one judgement it makes is the `network`
 * bit on a failure — whether the request reached the server at all — because
 * that is the single fact only a shell can know, and the core needs it to tell
 * "the service said no" from "the service was not there".
 */

import { loadCore, registryResolveKeyStep, registryResolveUnitStep } from '$lib/core/client';
import { PUBLIC_RPCS } from '$lib/services/rpc-pool-endpoints';
import type { RegistryProof } from '../generated/RegistryProof';
import type { RegistryUnitMember } from '../generated/RegistryUnitMember';

/** The v2 registry. Overridable so a self-hosted stack is a setting, not a fork. */
export const DEFAULT_REGISTRY_URL = 'https://p256-index-v2.getvela.app';

/** The health identities this endpoint accepts — the legacy index and the v2
 *  registry, so a wallet can point at either during the migration. */
const SERVICE_IDENTITIES = ['webauthn-p256-publickey-registry', 'webauthn-p256-publickey-index'];

const READ_TIMEOUT_MS = 15_000;
const WRITE_TIMEOUT_MS = 30_000;
const POLL_TIMEOUT_MS = 120_000;
const POLL_INTERVAL_MS = 2_000;

/**
 * A request that never reached the server, as opposed to one the server
 * refused. The core branches on this: an unreachable index is a transient
 * condition the person can fix by pointing somewhere else, while a 4xx is an
 * answer.
 */
export class RegistryError extends Error {
	readonly network: boolean;
	constructor(message: string, network: boolean) {
		super(message);
		this.name = 'RegistryError';
		this.network = network;
	}
}

export type ChallengeValue = { challenge: string; challengeBase64url: string };

export type GroupChallenge = {
	contentHash: string;
	groupChallenge: ChallengeValue;
	members: (ChallengeValue & { publicKey: string })[];
};

export type KeyStatus = { registered: boolean; unitIds: number[] };

export type UnitDetail = { metadataHex: string; members: RegistryUnitMember[] };

/** A vela wallet's founding set is capped at 7 keys; a larger group is not
 *  ours and must never be reconstructed into an account. */
const MAX_UNIT_MEMBERS = 7;

let baseUrl = DEFAULT_REGISTRY_URL;

export function setRegistryUrl(url: string): void {
	baseUrl =
		url
			.trim()
			.replace(/[\r\n]/g, '')
			.replace(/\/$/, '') || DEFAULT_REGISTRY_URL;
}

export function registryUrl(): string {
	return baseUrl;
}

async function request<T>(
	path: string,
	init: RequestInit,
	timeoutMs: number,
	label: string
): Promise<T> {
	const controller = new AbortController();
	const timer = setTimeout(() => controller.abort(), timeoutMs);
	let response: Response;
	try {
		response = await fetch(`${baseUrl}${path}`, { ...init, signal: controller.signal });
	} catch (error) {
		// Transport failure or abort: the request never arrived.
		throw new RegistryError(`${label} failed: ${describe(error)}`, true);
	} finally {
		clearTimeout(timer);
	}
	if (!response.ok) {
		// The server answered — a refusal is an answer.
		throw new RegistryError(`${label} failed: ${response.status}`, false);
	}
	return (await response.json()) as T;
}

function postJson<T>(path: string, body: unknown, timeoutMs: number, label: string): Promise<T> {
	return request<T>(
		path,
		{ method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) },
		timeoutMs,
		label
	);
}

/** MEMBER-mode challenge: one founding passkey confirming AT CREATION. Binds
 *  only (groupPublicKey, own attestation), so it exists before the rest of the
 *  set does — which is what makes the interleaved create→confirm flow work. */
export function memberChallenge(body: {
	rpId: string;
	groupPublicKey: string;
	publicKey: string;
	attestation?: string;
}): Promise<ChallengeValue> {
	return postJson('/api/challenge', body, READ_TIMEOUT_MS, 'Challenge');
}

/** GROUP-mode challenge: closing the group at publish. */
export function groupChallenge(body: {
	rpId: string;
	metadata?: string;
	groupPublicKey: string;
	members: { publicKey: string; attestation?: string }[];
}): Promise<GroupChallenge> {
	return postJson('/api/challenge', body, READ_TIMEOUT_MS, 'Challenge');
}

/**
 * Register the closed group. `members` is the REGISTRY's camelCase shape, not
 * the core's snake_case wire type — `publish.ts` translates, and this signature
 * refuses the core type so the translation cannot be skipped.
 */
export function registerGroup(body: {
	rpId: string;
	metadata?: string;
	groupPublicKey: string;
	groupProof: RegistryProof;
	members: {
		publicKey: string;
		attestation?: string;
		credentialId?: string;
		authenticatorAttachment?: string;
		transports?: string;
		proof: RegistryProof;
	}[];
}): Promise<{ id?: string; status: string; contentHash?: string }> {
	return postJson('/api/register', body, WRITE_TIMEOUT_MS, 'Register');
}

type TaskStatus = { id: string; status: 'pending' | 'done' | 'failed'; error?: string | null };

/** Poll until terminal. A transient read failure is retried until the budget
 *  runs out — the task is already accepted, so giving up on one bad read would
 *  report a failure that did not happen. */
export async function awaitTask(id: string): Promise<void> {
	const deadline = Date.now() + POLL_TIMEOUT_MS;
	let lastError: unknown = null;
	while (Date.now() < deadline) {
		try {
			const task = await request<TaskStatus>(
				`/api/task/${encodeURIComponent(id)}`,
				{},
				READ_TIMEOUT_MS,
				'Task status'
			);
			if (task.status === 'done') return;
			if (task.status === 'failed') {
				throw new RegistryError(`Register failed: ${task.error ?? 'unknown'}`, false);
			}
		} catch (error) {
			if (error instanceof RegistryError && !error.network) throw error;
			lastError = error;
		}
		await sleep(POLL_INTERVAL_MS);
	}
	throw new RegistryError(
		`Register timed out after ${POLL_TIMEOUT_MS / 1000}s${lastError ? `: ${describe(lastError)}` : ''}`,
		true
	);
}

type KeyProfile = {
	entry: { publicKey: string } | null;
	groups?: { total: number; unitIds: number[] };
};

type UnitMember = {
	publicKey: string;
	credentialId: string;
	authenticatorAttachment?: string;
	transports?: string;
};

type UnitResponse = {
	unit: { metadata: string };
	members?: { total: number; items: UnitMember[] };
};

// ---------------------------------------------------------------------------
// The three layers — transport for `vela_core::registry_resolve` (064)
// ---------------------------------------------------------------------------
//
// The index for speed, the chain for truth, Ethereum for survival. WHICH is
// asked, in what order, and whether an index answer is believed — it is not:
// its `contentHash` is recomputed and compared with the chain's — are the
// core's, written once for four shells. This file performs the requests.
// Until 064 it carried its own copy of the ladder, which believed any index
// that answered.
//
// Unit ids are per DEPLOYMENT (unit 10 on Gnosis is unit 0 on Ethereum), so a
// key's units are asked of whoever listed them: `unitSource` is the opaque
// token the core hands back with a listing.

/** Who listed the last key's units — the core's token, handed back verbatim. */
let unitSource = 'index';

type ResolveRequest =
	| { type: 'eth_call'; id: string; chain_id: number; to: string; data: string }
	| { type: 'index_get'; id: string; path: string };
type ResolveAnswer = { id: string; outcome: 'ok' | 'not_found' | 'failed'; body: string | null };
type ResolveStep =
	| { type: 'ask'; requests: ResolveRequest[] }
	| {
			type: 'done';
			body: string | null;
			source: string;
			verified_by: 'gnosis' | 'ethereum' | 'none';
			index_discarded: boolean;
	  };

/** How many rounds a walk may take; it only stops a contract bug from spinning. */
const MAX_RESOLVE_ROUNDS = 16;

/** One `eth_call` on one chain, through its public RPCs in order. `null` = the
 *  chain did not answer (every endpoint failed, or an RPC error). A bare `0x`
 *  IS an answer ("no such contract here") and is passed on as one. */
async function ethCall(chainId: number, to: string, data: string): Promise<string | null> {
	for (const url of PUBLIC_RPCS[chainId] ?? []) {
		const controller = new AbortController();
		const timer = setTimeout(() => controller.abort(), READ_TIMEOUT_MS);
		try {
			const response = await fetch(url, {
				method: 'POST',
				headers: { 'content-type': 'application/json' },
				body: JSON.stringify({
					jsonrpc: '2.0',
					id: 1,
					method: 'eth_call',
					params: [{ to, data }, 'latest']
				}),
				signal: controller.signal
			});
			const body = (await response.json()) as { result?: unknown };
			if (typeof body.result === 'string') return body.result;
		} catch {
			/* next endpoint */
		} finally {
			clearTimeout(timer);
		}
	}
	return null;
}

/**
 * Drive one resolver walk to its end.
 *
 * When NOBODY answers, the error reported is the INDEX's own — reachable or
 * not — because that is the distinction the core's notices are written for;
 * the chains being silent too adds nothing a person can act on.
 */
async function resolve<T>(
	label: string,
	step: (answersJson: string) => string,
	/** Only a LISTING names who its unit ids belong to. A unit's own source may
	 *  be a chain the index's ids mean nothing on. */
	listing: boolean
): Promise<T> {
	await loadCore();
	const answers: ResolveAnswer[] = [];
	let indexFailure: RegistryError | null = null;

	const perform = async (request: ResolveRequest): Promise<ResolveAnswer> => {
		const failed: ResolveAnswer = { id: request.id, outcome: 'failed', body: null };
		if (request.type === 'eth_call') {
			const result = await ethCall(request.chain_id, request.to, request.data);
			return result === null ? failed : { id: request.id, outcome: 'ok', body: result };
		}
		const controller = new AbortController();
		const timer = setTimeout(() => controller.abort(), READ_TIMEOUT_MS);
		try {
			const response = await fetch(`${baseUrl}${request.path}`, { signal: controller.signal });
			if (!response.ok) {
				// The server answered — a refusal. Remembered, and the chain is asked.
				indexFailure = new RegistryError(`${label} failed: ${response.status}`, false);
				return response.status === 404 ? { ...failed, outcome: 'not_found' } : failed;
			}
			return { id: request.id, outcome: 'ok', body: await response.text() };
		} catch (error) {
			indexFailure = new RegistryError(`${label} failed: ${describe(error)}`, true);
			return failed;
		} finally {
			clearTimeout(timer);
		}
	};

	for (let round = 0; round < MAX_RESOLVE_ROUNDS; round++) {
		const next = JSON.parse(step(JSON.stringify(answers))) as ResolveStep;
		if (next.type === 'ask') {
			answers.push(...(await Promise.all(next.requests.map(perform))));
			continue;
		}
		if (next.body === null) break;
		if (next.index_discarded) {
			// Our own index described a founding set the chain does not hold. The
			// chain's is what is used; this line is for whoever runs the index.
			console.warn(`[registry] the index's answer did not match the chain — discarded`);
		}
		if (listing) unitSource = next.source;
		return JSON.parse(next.body) as T;
	}
	throw indexFailure ?? new RegistryError(`${label} failed: nobody answered`, true);
}

function readKeyProfile(publicKey: string): Promise<KeyProfile> {
	return resolve<KeyProfile>(
		'Query',
		(answers) => registryResolveKeyStep(publicKey, answers),
		true
	);
}

function readUnit(unitId: number): Promise<UnitResponse> {
	// The token is read NOW: the listing that set it is the one these ids belong to.
	const source = unitSource;
	return resolve<UnitResponse>(
		'Query',
		(answers) => registryResolveUnitStep(unitId, source, answers),
		false
	);
}

/** Test seam: forget which source listed the last key's units. */
export function _resetUnitSource(): void {
	unitSource = 'index';
}

/** `/api/query?publicKey=` — is this key registered, and which groups does it
 *  found? */
export async function queryByPublicKey(publicKey: string): Promise<KeyStatus> {
	const profile = await readKeyProfile(publicKey);
	const unitIds = profile.groups?.unitIds ?? [];
	// The core speaks u32 unit ids because the wire is JSON. An id past 2^32
	// would truncate into a DIFFERENT group, so this fails the query instead of
	// quietly fetching the wrong founding set.
	if (unitIds.some((id) => !Number.isInteger(id) || id < 0 || id >= 2 ** 32)) {
		throw new RegistryError(
			`Query failed: unit id out of u32 range in ${JSON.stringify(unitIds)}`,
			false
		);
	}
	return { registered: profile.entry !== null, unitIds };
}

/**
 * `/api/query?unitId=` — the group's frozen metadata and ALL its founding
 * members in ascending order, which IS the canonical founding order the Safe
 * address derivation pins.
 *
 * Both guards refuse rather than degrade: a group larger than a wallet's cap is
 * not ours, and a partial page would rebuild the address from a SUBSET of the
 * founding set — a different, wrong, fundable address.
 */
export async function queryUnit(unitId: number): Promise<UnitDetail> {
	const detail = await readUnit(unitId);
	const total = detail.members?.total ?? 0;
	const items = detail.members?.items ?? [];
	if (total > MAX_UNIT_MEMBERS) {
		throw new RegistryError(
			`Query failed: unit ${unitId} has ${total} members (cap ${MAX_UNIT_MEMBERS})`,
			false
		);
	}
	if (items.length !== total) {
		throw new RegistryError(
			`Query failed: unit ${unitId} page holds ${items.length} of ${total} members`,
			false
		);
	}
	return {
		metadataHex: detail.unit.metadata,
		members: items.map((member) => ({
			credential_id: member.credentialId,
			public_key_hex: member.publicKey,
			authenticator_attachment: member.authenticatorAttachment ?? '',
			transports: member.transports ?? ''
		}))
	};
}

/** One health probe. Never throws: the core asks a yes/no question. */
export async function probeHealth(): Promise<boolean> {
	try {
		const health = await request<{ service?: string; status?: string }>(
			`/api/health?_t=${Date.now()}`,
			{},
			READ_TIMEOUT_MS,
			'Health'
		);
		return SERVICE_IDENTITIES.includes(health.service ?? '') && health.status === 'ok';
	} catch {
		return false;
	}
}

/** The v1 index's display name for a credential — the only place a v1-era
 *  wallet's name survives. Best-effort and read-only; a lost name degrades the
 *  label, never the flow. */
export async function legacyName(credentialId: string): Promise<string | null> {
	try {
		const record = await request<{ name?: string }>(
			`/api/query?credentialId=${encodeURIComponent(credentialId)}`,
			{},
			READ_TIMEOUT_MS,
			'Legacy name'
		);
		return record.name?.trim() || null;
	} catch {
		return null;
	}
}

function sleep(ms: number): Promise<void> {
	return new Promise((resolve) => setTimeout(resolve, ms));
}

function describe(error: unknown): string {
	if (error instanceof Error) return error.message;
	return String(error);
}
