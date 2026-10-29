/**
 * Creating a multisig, and adding one by its address.
 *
 * Creating deploys nothing (docs/multisig-vault-design.md §4.1): the genesis
 * fixes the address on every chain, and the first execution on a chain
 * deploys it. What makes it reachable from another device is the genesis in
 * the Safe Genesis Registry, written by the service on the creator's passkey
 * proof. Until the service confirms that, the account keeps the proof and
 * asks again (`retryUnrecorded`) — the same outbox the passkey index has.
 *
 * Adding needs only the address: the genesis comes from the service and is
 * accepted only if the core re-derives the address from it — a different
 * genesis for the same address would take a 160-bit preimage, so the service
 * cannot hand over a wrong one. Deriving the address is not enough to know
 * who controls it, though (§3.5): where it is deployed, the chain says
 * (modules, guards, handler, singleton — shown, and added only on the
 * person's confirmation); where it is not, the core's counterfactual rule
 * must find its genesis Verified (no setup code, module, strange handler or
 * payment; Safe's own code on the chain), else it is not added at all.
 *
 * Registry v4 (2026-10-29): every record carries a name, and the multisig
 * itself may rename its record (a transaction of its own). What a record's
 * name is now is read from the registry on chain where this wallet has the
 * network (`checkRecords`), the service's word only where it does not. The
 * service reads only v4: a multisig recorded in an earlier registry (v1–v3,
 * testnets and local forks only) is one it no longer has, and is shown as not
 * shared — never silently as shared — until it is shared again.
 *
 * The registry is an INDEX, not a home (owner, 2026-10-29): a genesis is
 * recorded on the index chain (the service's home chain, Gnosis; Ethereum on
 * the person's own fee) whatever network the multisig is used on, and the
 * multisig is the same on every chain with the Safe v1.4.1 contracts. Which
 * networks it is used on is what says so — deployed there, proposals there,
 * a link naming one, the creator's pick (the core's `multisigNetworks`) —
 * never the chain its record is on.
 */
import { poolRpcCall } from '$lib/services/rpc-pool';
import { counterfactualOn, deployedOn, type Finding } from './authority';
import { hasCode } from './chain';
import { creationGenesis, hasTransactionService } from './safe-service';
import {
	GENESIS_REGISTRY,
	genesisAddress,
	genesisMembers,
	genesisOfCall,
	multisigGenesis,
	multisigNetworks,
	namedGenesis,
	readGenesisOf,
	safeWalletPropsGenesis,
	sameAddress,
	validName,
	type ExportedSafe,
	type Genesis
} from './kernel';
import { signOwnerProof, type LocalOwner } from './owners';
import {
	fetchGenesis,
	fetchProposals,
	fetchTask,
	importGenesis,
	servedChains,
	serviceTable,
	ServiceUnavailableError,
	type OwnerProof
} from './service';
import { networkForChainId } from '$lib/services/networks';
import { serviceOutage } from './service-state.svelte';
import { multisig, type MultisigAccount } from './store.svelte';

/**
 * The default service's home chain (Gnosis): the index chain its registry
 * records on. Only a fallback — the service in use names its own (`/api/health`'s `homeChainId`,
 * {@link recordHomeChain}); this is used when it does not say.
 */
export const REGISTRY_HOME_CHAIN_ID = 100;

/**
 * Where every record the service writes goes — the index chain, whatever
 * network the multisig is used on: the service's own home chain, as it says
 * (`homeChainId`); {@link REGISTRY_HOME_CHAIN_ID} only when it says nothing
 * (its table could not be read, or carries no home chain). A local service
 * homed on Arc Testnet records there.
 */
export async function recordHomeChain(): Promise<number> {
	const table = await serviceTable().catch(() => null);
	return table?.homeChainId ?? REGISTRY_HOME_CHAIN_ID;
}

const PROOFS_KEY = 'vela.multisigPendingProofs';

function randomSeed(): string {
	const bytes = new Uint8Array(32);
	crypto.getRandomValues(bytes);
	return `0x${Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('')}`;
}

function readProofs(): Record<string, OwnerProof> {
	try {
		return JSON.parse(localStorage.getItem(PROOFS_KEY) ?? '{}') as Record<string, OwnerProof>;
	} catch {
		return {};
	}
}

function writeProofs(proofs: Record<string, OwnerProof>): void {
	try {
		localStorage.setItem(PROOFS_KEY, JSON.stringify(proofs));
	} catch {
		// best effort
	}
}

/** What the person chose; the core checks it (no duplicates, a threshold that can be met). */
export interface CreateChoice {
	owners: string[];
	threshold: number;
	/**
	 * The network it is first used on, kept on this device as its first
	 * network. Its record goes to the index chain whatever this is.
	 */
	chainId: number;
	/** The owner on this device whose passkey asks the service to record it. */
	creator: LocalOwner;
	/** Its name, written with its record: a label, not part of the address. */
	name: string;
}

/** The address a choice gives, for the review screen (the salt is fixed per draft). */
export function draftGenesis(
	choice: Pick<CreateChoice, 'owners' | 'threshold' | 'name'>,
	saltNonce: string
) {
	return multisigGenesis(choice.owners, choice.threshold, choice.name, saltNonce);
}

/** 32 random bytes: the multisig's salt nonce. */
export function newSaltNonce(): string {
	return randomSeed();
}

/**
 * Record `genesis` through the service, on the index chain; `true` once the
 * registry holds it (or will). Never the network the multisig is used on: the
 * service reads its own chains only, and the record is one index entry
 * wherever the multisig lives. Sent named (`namedGenesis`): v4 records every
 * Safe with a name, and a genesis without one is the service's to name.
 * Without `proof` the service pays only for a Safe deployed where it reads;
 * anything else comes back as a call for someone to send, and is not
 * recorded here.
 */
async function record(
	address: string,
	genesis: Genesis,
	proof: OwnerProof | undefined
): Promise<boolean> {
	const answer = await importGenesis({
		chainId: await recordHomeChain(),
		safe: address,
		genesis: namedGenesis(genesis),
		ownerProof: proof
	});
	return answer.status === 'recorded' || answer.status === 'queued';
}

/** Multisigs asked about without a proof this session: once each, not every minute. */
const askedWithoutProof = new Set<string>();

/**
 * Create the multisig: one passkey prompt (the creator's proof), then the
 * service records it. The account exists — and is shown — even if the
 * service is unreachable right now; the record is retried.
 */
export async function createMultisig(
	choice: CreateChoice,
	saltNonce: string
): Promise<MultisigAccount> {
	const draft = multisigGenesis(choice.owners, choice.threshold, choice.name, saltNonce);
	const proof = await signOwnerProof(choice.creator.account, draft.address, choice.chainId);
	let recorded = false;
	try {
		recorded = await record(draft.address, draft.genesis, proof);
	} catch (error) {
		if (!(error instanceof ServiceUnavailableError)) throw error;
	}
	if (!recorded) writeProofs({ ...readProofs(), [draft.address.toLowerCase()]: proof });
	const account = multisig.add(draft.address, draft.genesis, recorded, [choice.chainId]);
	// Whatever the record's fate, the service may already hand it out.
	if (!recorded && (await fetchGenesis(draft.address).catch(() => ({ found: false }))).found) {
		multisig.markShared(draft.address);
	}
	multisig.activate(account.address);
	return multisig.find(account.address) ?? account;
}

/** Ask again for every multisig the registry has not confirmed. Quiet on failure. */
export async function retryUnrecorded(): Promise<void> {
	// The service is away: asked again on its schedule, not every minute.
	if (serviceOutage.blocked()) return;
	const proofs = readProofs();
	for (const account of multisig.accounts.filter((a) => !a.recorded)) {
		try {
			const known = await fetchGenesis(account.address, { background: true });
			if (known.found && !known.pending) {
				multisig.markRecorded(account.address);
				continue;
			}
			if (known.found) multisig.markShared(account.address);
			const key = account.address.toLowerCase();
			const proof = proofs[key];
			// No proof kept (one recorded in an older registry, or imported):
			// asked once a session — the service pays for a Safe deployed
			// somewhere; anything else waits for "Share it again".
			if (!proof && askedWithoutProof.has(key)) continue;
			if (!proof) askedWithoutProof.add(key);
			if (await record(account.address, account.genesis, proof)) {
				// Queued: confirmed by a later `fetchGenesis`.
				multisig.markShared(account.address);
			}
		} catch {
			// the next launch asks again
		}
	}
	const left = readProofs();
	for (const a of multisig.accounts) if (a.recorded) delete left[a.address.toLowerCase()];
	writeProofs(left);
}

/**
 * Share a multisig the service does not have again (one recorded in an
 * earlier registry, or whose first record never landed): `owner`'s passkey
 * proves an owner asks, as on creation (one prompt), and the service records
 * it on its home chain. Resolves `true` once the service holds it.
 */
export async function shareAgain(account: MultisigAccount, owner: LocalOwner): Promise<boolean> {
	// Nothing awaited before the passkey: its prompt needs the person's tap.
	const proof = await signOwnerProof(
		owner.account,
		account.address,
		account.chainIds[0] ?? REGISTRY_HOME_CHAIN_ID
	);
	let recorded = false;
	try {
		recorded = await record(account.address, account.genesis, proof);
	} catch (error) {
		if (!(error instanceof ServiceUnavailableError)) throw error;
	}
	if (!recorded) {
		// Kept and asked again with the record loop, as a new multisig's is.
		writeProofs({ ...readProofs(), [account.address.toLowerCase()]: proof });
	}
	// Not paid for now (its sponsor's budget spent), the service still hands
	// out what an owner proved — shared, with the record to follow.
	const shared =
		recorded || (await fetchGenesis(account.address).catch(() => ({ found: false }))).found;
	if (shared) multisig.markShared(account.address);
	return shared;
}

/**
 * The name `account`'s record holds now — read from the registry on
 * `chainId` with this wallet's own RPC, the core re-deriving the address from
 * what comes back. `undefined` when that chain cannot be read here (the
 * network is not this wallet's, or it did not answer): then nothing is said.
 */
async function recordedNameOnChain(
	address: string,
	chainId: number
): Promise<string | null | undefined> {
	if (networkForChainId(chainId) === null) return undefined;
	try {
		const read = await poolRpcCall(
			'eth_call',
			[{ to: GENESIS_REGISTRY, data: genesisOfCall(address) }, 'latest'],
			chainId
		);
		if (typeof read.result !== 'string') return undefined;
		const record = readGenesisOf(address, read.result);
		return record === null ? null : (record.genesis.name ?? null);
	} catch {
		return undefined;
	}
}

/**
 * Look at `address`'s record again now (it was just renamed on this device):
 * its name is read from the chain at once.
 */
export async function recheckRecord(address: string): Promise<void> {
	checked.delete(address.toLowerCase());
	await checkRecords();
}

/** How long a record the service queued may take to land before its absence means anything. */
const LANDING_MS = 10 * 60_000;

/** Multisigs whose record was looked at this session. */
const checked = new Set<string>();

/**
 * Look at the record of every multisig this device calls shared, once a
 * session: its name now (the multisig may have renamed itself — read from
 * the chain where this wallet can, the service's word only where it cannot),
 * and whether the service has it at all. A service that answers it holds no
 * record of one (an earlier registry's: v1–v3 are no longer read) makes it
 * not shared again, said on screen and asked again, never left looking
 * shared. Quiet on failure: the next session looks again.
 */
export async function checkRecords(): Promise<void> {
	if (serviceOutage.blocked()) return;
	for (const account of multisig.accounts.filter((a) => a.recorded || a.shared)) {
		const key = account.address.toLowerCase();
		if (checked.has(key)) continue;
		checked.add(key);
		try {
			const known = await fetchGenesis(account.address, { background: true });
			if (!known.found) {
				// A record queued moments ago may not have landed yet.
				if (Date.now() - account.addedMs > LANDING_MS) multisig.markUnshared(account.address);
				else checked.delete(key);
				continue;
			}
			if (known.pending || !known.genesis) continue;
			if (!sameAddress(genesisAddress(known.genesis), account.address)) continue;
			const from = known.readFrom ?? known.recordedOn?.[0];
			const onChain =
				from === undefined ? undefined : await recordedNameOnChain(account.address, from);
			const name = onChain === undefined ? known.genesis.name : onChain;
			if (name && name !== account.genesis.name && validName(name)) {
				multisig.setRecordedName(account.address, name);
			}
		} catch {
			checked.delete(key);
		}
	}
}

/** How often a record that has not landed is asked about again while a page is open. */
export const RECORD_RETRY_MS = 60_000;

let retrying: Promise<void> | null = null;

/**
 * Keep asking about every multisig whose record has not landed — now, when
 * the browser comes back online, when the page is shown again, and every
 * {@link RECORD_RETRY_MS} — not only when one page mounts: a record whose
 * first attempt met a service that was down would otherwise wait for a
 * reload nobody knows to make. One retry at a time. Returns the stop.
 */
export function followUnrecorded(retry: () => Promise<void> = retryUnrecorded): () => void {
	const run = () => {
		if (retrying || !multisig.accounts.some((a) => !a.recorded)) return;
		retrying = retry().finally(() => (retrying = null));
	};
	const shown = () => {
		if (document.visibilityState === 'visible') run();
	};
	run();
	const timer = setInterval(run, RECORD_RETRY_MS);
	window.addEventListener('online', run);
	window.addEventListener('focus', run);
	document.addEventListener('visibilitychange', shown);
	return () => {
		clearInterval(timer);
		window.removeEventListener('online', run);
		window.removeEventListener('focus', run);
		document.removeEventListener('visibilitychange', shown);
	};
}

/**
 * Why a multisig is not added as it stands (§3.5): `refused` — not deployed
 * on any of its networks, and its genesis is one the core will not deploy
 * or sign for (its findings say why); `unverified` — the chain did not
 * answer for the Safe contracts it needs (`chainId`), so who would control
 * it cannot be said yet; `highRisk` — deployed, and something besides its
 * members can act for it there: added only on the person's explicit
 * confirmation (`acceptRisks`), and the findings stay shown.
 */
export type AuthorityOutcome =
	| { ok: false; reason: 'refused'; findings: Finding[] }
	| { ok: false; reason: 'unverified'; chainId: number }
	| { ok: false; reason: 'highRisk'; findings: Finding[]; account: MultisigAccount };

export type AddOutcome =
	| { ok: true; account: MultisigAccount }
	| {
			ok: false;
			/**
			 * `chooseNetwork`: nothing says which network it is used on — not
			 * deployed on any, and its record names none this wallet has (a
			 * record still on its way names none at all). The person is asked;
			 * it is not guessed.
			 */
			reason: 'notFound' | 'unavailable' | 'unsupported' | 'notMultisig' | 'chooseNetwork';
	  }
	| AuthorityOutcome;

/**
 * Add a multisig by address, and show it. When the registry does not hold it
 * yet but it is deployed on one of `chainIds`, the service resolves its
 * genesis from that chain and records it (its creator paid for a
 * deployment, so the service pays), and the address is looked up again.
 * A high-risk one is added only with `acceptRisks` — the person's tap on
 * the warning, read again now — and keeps its findings.
 */
export async function addMultisig(
	address: string,
	chainIds: number[],
	network?: number,
	options: { acceptRisks?: boolean } = {}
): Promise<AddOutcome> {
	const existing = multisig.find(address);
	if (existing) {
		multisig.activate(existing.address);
		return { ok: true, account: existing };
	}
	const found = await findMultisig(address, chainIds, network);
	let account: MultisigAccount;
	if (found.ok) account = keepMultisig(found.account);
	else if (found.reason === 'highRisk' && options.acceptRisks) {
		account = keepMultisig(found.account, found.findings);
	} else return found;
	multisig.activate(account.address);
	return { ok: true, account };
}

/**
 * Who can act for `address` on its networks (§3.5): where it is deployed,
 * what else can act for it there; where it is deployed nowhere, whether its
 * genesis is Verified on each network it is used on. `null`: nothing found.
 */
async function vetAuthority(
	address: string,
	genesis: Genesis,
	deployed: number[],
	used: number[]
): Promise<
	| { ok: false; reason: 'refused'; findings: Finding[] }
	| { ok: false; reason: 'unverified'; chainId: number }
	| { ok: false; reason: 'unavailable' }
	| { ok: true; findings: Finding[] }
> {
	const findings: Finding[] = [];
	for (const chainId of deployed) {
		const check = await deployedOn(chainId, address);
		if (check.unread) return { ok: false, reason: 'unavailable' };
		findings.push(...check.risks.map((risk) => ({ from: 'chain' as const, chainId, risk })));
	}
	// Deployed somewhere: the chain is its authority there; the executor
	// still deploys it nowhere else unless its genesis is Verified there.
	if (deployed.length > 0) return { ok: true, findings };
	for (const chainId of used) {
		const authority = await counterfactualOn(chainId, address, genesis);
		if (authority.kind === 'refused') {
			return {
				ok: false,
				reason: 'refused',
				findings: authority.reasons.map((reason) => ({
					from: 'genesis' as const,
					chainId,
					reason
				}))
			};
		}
		if (authority.kind === 'unknown') return { ok: false, reason: 'unverified', chainId };
	}
	return { ok: true, findings };
}

/**
 * The multisig at `address`, read and checked as {@link addMultisig} does —
 * but kept nowhere (ms-21): an account to read a link's transaction from on
 * a device that has none of its members, which only the person's tap adds.
 */
export async function findMultisig(
	address: string,
	chainIds: number[],
	network?: number
): Promise<AddOutcome> {
	// What the address is on chain, read beside the registry (ms-16): the
	// wait for a record on its way is only for an address with no code and no
	// transactions anywhere — a multisig created moments ago.
	const onChain = Promise.all([
		deployedChains(address, chainIds),
		sentFrom(address, chainIds)
	]).then(([deployed, sent]) => ({ deployed, sent }));
	let answer;
	try {
		answer = await fetchGenesis(address);
		if (!answer.found) {
			const { deployed, sent } = await onChain;
			// Code that is no Safe (a token, any contract), or an account that
			// sends its own transactions: no multisig, said at once.
			if (deployed.length > 0 && !(await isSafe(deployed[0], address))) {
				return { ok: false, reason: 'notMultisig' };
			}
			if (deployed.length === 0 && sent) return { ok: false, reason: 'notMultisig' };
		}
		// A multisig created moments ago: its record may still be on its way
		// to the registry. Ask again for a little while before giving up.
		for (
			let wait = 0;
			!answer.found && wait < 4 && (await onChain).deployed.length === 0;
			wait += 1
		) {
			await new Promise((r) => setTimeout(r, 5000));
			answer = await fetchGenesis(address);
		}
	} catch {
		return { ok: false, reason: 'unavailable' };
	}
	if (!answer.found) {
		// The service resolves a deployed Safe from a chain it reads: one of
		// those it is deployed on, if any; else it is asked of the first.
		const deployed = (await onChain).deployed;
		const served = await servedChains().catch(() => null);
		const deployedOn = deployed.find((id) => served?.has(id)) ?? deployed[0] ?? null;
		if (deployedOn === null) return { ok: false, reason: 'notFound' };
		try {
			const imported = await importGenesis({ chainId: deployedOn, safe: address });
			if (imported.status === 'queued') await waitForTask(imported.taskId);
			if (imported.status === 'refused') return { ok: false, reason: 'notMultisig' };
			answer = await fetchGenesis(address);
		} catch {
			return { ok: false, reason: 'unavailable' };
		}
		if (!answer.found || !answer.genesis) return { ok: false, reason: 'notFound' };
	}
	const genesis = answer.genesis;
	if (!genesis) return { ok: false, reason: 'notFound' };
	let derived: string;
	try {
		derived = genesisAddress(genesis);
	} catch {
		return { ok: false, reason: 'unsupported' };
	}
	if (!sameAddress(derived, address)) return { ok: false, reason: 'notFound' };
	const used = await deployedChains(address, chainIds);
	// The mailbox is asked network by network only when nothing else says:
	// one read per network the wallet has would spend the service's per-client
	// budget the person's next read needs.
	const proposed =
		used.length === 0 && network === undefined ? await proposalChains(address, chainIds) : [];
	// Where it is used, from what says so (the core's rule): the chains it is
	// deployed on, those the mailbox holds proposals of it for, then the
	// network the person (or a link) names. Never the chain its record is on
	// — the registry is an index — nor the first network of the space shown.
	const where = multisigNetworks({ deployed: used, proposals: proposed, named: network ?? null });
	if (where.length === 0) return { ok: false, reason: 'chooseNetwork' };
	// The service's genesis decides nothing by itself (§3.5): the chain where
	// it is deployed, the core's counterfactual rule where it is not.
	const vetted = await vetAuthority(address, genesis, used, where);
	if (!vetted.ok) return vetted;
	const { owners, threshold } = genesisMembers(genesis);
	const account: MultisigAccount = {
		address,
		genesis,
		owners,
		threshold,
		recorded: !answer.pending,
		// It came from the service: the service can hand it out.
		shared: true,
		chainIds: where,
		addedMs: Date.now()
	};
	if (vetted.findings.length > 0) {
		return { ok: false, reason: 'highRisk', findings: vetted.findings, account };
	}
	return { ok: true, account };
}

/**
 * Keep a multisig {@link findMultisig} read on this device — in the account
 * list, not shown: which account is shown stays the person's to pick. A
 * high-risk one keeps what was found, to be shown wherever it is.
 */
export function keepMultisig(found: MultisigAccount, findings: Finding[] = []): MultisigAccount {
	const account = multisig.add(found.address, found.genesis, found.recorded, found.chainIds);
	// It came from the service: the service can hand it out.
	multisig.markShared(account.address);
	if (findings.length > 0) multisig.setFindings(account.address, findings);
	return multisig.find(account.address) ?? account;
}

/** `address` has sent a transaction of its own on one of `chainIds`: an account, no undeployed Safe. */
async function sentFrom(address: string, chainIds: number[]): Promise<boolean> {
	const counts = await Promise.all(
		chainIds.map((id) =>
			poolRpcCall('eth_getTransactionCount', [address, 'latest'], id)
				.then((r) => (typeof r.result === 'string' ? BigInt(r.result) : 0n))
				.catch(() => 0n)
		)
	);
	return counts.some((n) => n > 0n);
}

/** The contract at `address` on `chainId` answers as a Safe (`getThreshold` is a number ≥ 1). */
async function isSafe(chainId: number, address: string): Promise<boolean> {
	try {
		const read = await poolRpcCall(
			'eth_call',
			[{ to: address, data: GET_THRESHOLD_CALL }, 'latest'],
			chainId
		);
		// A node's error is no answer; a revert or nothing back is no Safe.
		if (read.error && !/revert/i.test(String(read.error.message ?? ''))) return true;
		return typeof read.result === 'string' && read.result.length === 66 && BigInt(read.result) > 0n;
	} catch {
		// The chain did not say: not called a non-multisig on no evidence.
		return true;
	}
}

/** A Safe's `getThreshold()`. */
const GET_THRESHOLD_CALL = '0xe75235b8';

async function deployedChains(address: string, chainIds: number[]): Promise<number[]> {
	const results = await Promise.all(
		chainIds.map(async (id) => ((await hasCode(id, address).catch(() => false)) ? id : null))
	);
	return results.filter((id): id is number => id !== null);
}

/** How many mailbox reads {@link proposalChains} has out at once. */
const MAILBOX_READS_AT_ONCE = 3;

/**
 * Which of `chainIds` the mailbox holds proposals of `address` for. The
 * mailbox takes proposals on any chain and has no list across them, so this
 * is asked per network, a few at a time; one that cannot be read says
 * nothing (it is not a "no").
 */
async function proposalChains(address: string, chainIds: number[]): Promise<number[]> {
	const found: number[] = [];
	for (let i = 0; i < chainIds.length; i += MAILBOX_READS_AT_ONCE) {
		const batch = chainIds.slice(i, i + MAILBOX_READS_AT_ONCE);
		const results = await Promise.all(
			batch.map(async (id) => {
				const answer = await fetchProposals(id, address, { background: true }).catch(() => null);
				return answer && answer.proposals.length > 0 ? id : null;
			})
		);
		found.push(...results.filter((id): id is number => id !== null));
	}
	return found;
}

/** Multisigs whose networks were looked for this session. */
const discovered = new Set<string>();

/**
 * Once a session, look on every network of `chainIds` for a deployment this
 * device did not see (another member deployed it there): such a network
 * becomes one of its networks here (the core's rule, over what was kept).
 * Read with the wallet's own RPC only — never one mailbox read per network
 * per page, which spent the service's per-client budget the person's own
 * reads need; a network another member proposed on arrives with its link.
 * Quiet on failure; a network that cannot be read adds nothing.
 */
export async function discoverChains(account: MultisigAccount, chainIds: number[]): Promise<void> {
	const key = account.address.toLowerCase();
	if (discovered.has(key)) return;
	discovered.add(key);
	const unseen = chainIds.filter((id) => !account.chainIds.includes(id));
	if (unseen.length === 0) return;
	const deployed = await deployedChains(account.address, unseen);
	const where = multisigNetworks({ deployed, kept: account.chainIds });
	for (const id of where) {
		if (!account.chainIds.includes(id)) multisig.useChain(account.address, id);
	}
}

async function waitForTask(id: string): Promise<void> {
	for (let i = 0; i < 30; i += 1) {
		await new Promise((r) => setTimeout(r, 2000));
		const task = await fetchTask(id).catch(() => null);
		if (task && (task.status === 'done' || task.status === 'failed')) return;
	}
}

export type ImportOutcome =
	| { ok: true; account: MultisigAccount }
	| { ok: false; reason: 'notFound' | 'unavailable' | 'unsupported' | 'notDeployed' }
	| { ok: false; reason: 'refused' | 'highRisk'; findings: Finding[] }
	| { ok: false; reason: 'unverified'; chainId: number };

/**
 * Import one Safe from a Safe{Wallet} export. Its genesis is, in order: the
 * one the genesis service holds; one derived here from Safe{Wallet}'s own
 * creation parameters (an undeployed Safe); or the creation Safe's
 * transaction service has on record (a deployed one) — read from here, so
 * importing never depends on the genesis service, which is then asked to
 * record it (best effort). Whichever it is, the core must derive the Safe's
 * address from it. The address book's name is kept as this device's label.
 * A Safe already here is kept, and given the label if it has none. Who
 * controls it is vetted as for an address added (§3.5): a refused genesis
 * is not imported; a high-risk Safe only with `acceptRisks`.
 */
export async function importExported(
	safe: ExportedSafe,
	options: { acceptRisks?: boolean } = {}
): Promise<ImportOutcome> {
	const chainIds = safe.chains.map((c) => c.chainId);
	const existing = multisig.find(safe.address);
	if (existing) {
		for (const chainId of chainIds) multisig.useChain(existing.address, chainId);
		if (!existing.label && safe.name) multisig.setLabel(existing.address, safe.name);
		return { ok: true, account: multisig.find(existing.address) ?? existing };
	}
	let genesis: Genesis | null = null;
	let recorded = false;
	let unreachable = 0;
	let asked = 0;
	try {
		asked += 1;
		const known = await fetchGenesis(safe.address);
		if (known.found && known.genesis) {
			genesis = known.genesis;
			recorded = !known.pending;
		}
	} catch {
		unreachable += 1;
	}
	if (!genesis) {
		for (const c of safe.chains) {
			if (c.deployed || !c.props) continue;
			try {
				genesis = safeWalletPropsGenesis(c.props, safe.address);
				break;
			} catch {
				// another chain's entry may hold it
			}
		}
	}
	let unsupported = false;
	if (!genesis) {
		for (const c of safe.chains.filter((c) => c.deployed && hasTransactionService(c.chainId))) {
			try {
				asked += 1;
				const found = await creationGenesis(c.chainId, safe.address);
				if (found && 'genesis' in found) {
					genesis = found.genesis;
					break;
				}
				if (found) unsupported = true;
			} catch {
				unreachable += 1;
			}
		}
	}
	if (!genesis) {
		if (unsupported) return { ok: false, reason: 'unsupported' };
		if (asked > 0 && unreachable === asked) return { ok: false, reason: 'unavailable' };
		// Listed as added, yet deployed on none of its networks this wallet can read.
		const deployed = await Promise.all(
			safe.chains.map((c) => hasCode(c.chainId, safe.address).catch(() => null))
		);
		if (deployed.some((d) => d === false) && !deployed.some((d) => d === true)) {
			return { ok: false, reason: 'notDeployed' };
		}
		return { ok: false, reason: 'notFound' };
	}
	try {
		if (!sameAddress(genesisAddress(genesis), safe.address))
			return { ok: false, reason: 'notFound' };
	} catch {
		return { ok: false, reason: 'unsupported' };
	}
	// Who controls it, on the export's networks this wallet can read (§3.5):
	// the chain where it is deployed, the core's rule where it is not. None
	// readable: nothing can be said, and it is not imported unchecked.
	const readable = chainIds.filter((id) => networkForChainId(id) !== null);
	if (readable.length === 0) return { ok: false, reason: 'unverified', chainId: chainIds[0] ?? 0 };
	const live = await Promise.all(
		readable.map(async (id) => ((await hasCode(id, safe.address).catch(() => false)) ? id : null))
	);
	const vetted = await vetAuthority(
		safe.address,
		genesis,
		live.filter((id): id is number => id !== null),
		readable
	);
	if (!vetted.ok) return vetted;
	if (vetted.findings.length > 0 && !options.acceptRisks) {
		return { ok: false, reason: 'highRisk', findings: vetted.findings };
	}
	const account = multisig.add(safe.address, genesis, recorded, chainIds);
	if (vetted.findings.length > 0) multisig.setFindings(account.address, vetted.findings);
	if (safe.name) multisig.setLabel(account.address, safe.name);
	if (recorded) multisig.markShared(account.address);
	else {
		// The genesis service, if it is there, records it for other devices: a
		// deployment on a chain it reads is its evidence (it pays); on another
		// chain it is the genesis read here, sent to its index chain.
		const listedDeployed = safe.chains.filter((c) => c.deployed).map((c) => c.chainId);
		if (listedDeployed.length > 0) {
			const named = genesis;
			void (async () => {
				const served = await servedChains().catch(() => null);
				const read = listedDeployed.find((id) => served?.has(id));
				if (read === undefined) await record(safe.address, named, undefined);
				else await importGenesis({ chainId: read, safe: safe.address });
			})().catch(() => {});
		}
	}
	return { ok: true, account: multisig.find(account.address) ?? account };
}
