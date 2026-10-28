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
 * cannot hand over a wrong one.
 */
import { poolRpcCall } from '$lib/services/rpc-pool';
import { hasCode } from './chain';
import { creationGenesis, hasTransactionService } from './safe-service';
import {
	genesisAddress,
	multisigGenesis,
	safeWalletPropsGenesis,
	sameAddress,
	type ExportedSafe,
	type Genesis
} from './kernel';
import { signOwnerProof, type LocalOwner } from './owners';
import {
	fetchGenesis,
	fetchTask,
	importGenesis,
	ServiceUnavailableError,
	type OwnerProof
} from './service';
import { multisig, type MultisigAccount } from './store.svelte';

/** The service's home chain: where the registry it reads first lives. */
export const REGISTRY_HOME_CHAIN_ID = 100;

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
	/** The network it is first used on. */
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

/** Record `genesis` through the service; `true` once the registry holds it (or will). */
async function record(
	address: string,
	genesis: Genesis,
	proof: OwnerProof,
	chainId: number
): Promise<boolean> {
	let answer = await importGenesis({ chainId, safe: address, genesis, ownerProof: proof });
	// A chain the service does not serve: its home chain reads the same factory.
	if (answer.status === 'error' && chainId !== REGISTRY_HOME_CHAIN_ID) {
		answer = await importGenesis({
			chainId: REGISTRY_HOME_CHAIN_ID,
			safe: address,
			genesis,
			ownerProof: proof
		});
	}
	return answer.status === 'recorded' || answer.status === 'queued';
}

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
		recorded = await record(draft.address, draft.genesis, proof, choice.chainId);
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
	const proofs = readProofs();
	for (const account of multisig.accounts.filter((a) => !a.recorded)) {
		try {
			const known = await fetchGenesis(account.address);
			if (known.found && !known.pending) {
				multisig.markRecorded(account.address);
				continue;
			}
			if (known.found) multisig.markShared(account.address);
			const proof = proofs[account.address.toLowerCase()];
			if (
				proof &&
				(await record(
					account.address,
					account.genesis,
					proof,
					account.chainIds[0] ?? REGISTRY_HOME_CHAIN_ID
				))
			) {
				// Queued: confirmed by a later `fetchGenesis`.
			}
		} catch {
			// the next launch asks again
		}
	}
	const left = readProofs();
	for (const a of multisig.accounts) if (a.recorded) delete left[a.address.toLowerCase()];
	writeProofs(left);
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

export type AddOutcome =
	| { ok: true; account: MultisigAccount }
	| { ok: false; reason: 'notFound' | 'unavailable' | 'unsupported' | 'notMultisig' };

/**
 * Add a multisig by address. When the registry does not hold it yet but it
 * is deployed on one of `chainIds`, the service resolves its genesis from that
 * chain and records it (its creator paid for a deployment, so the service
 * pays), and the address is looked up again.
 */
export async function addMultisig(address: string, chainIds: number[]): Promise<AddOutcome> {
	const existing = multisig.find(address);
	if (existing) {
		multisig.activate(existing.address);
		return { ok: true, account: existing };
	}
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
		const deployedOn = (await onChain).deployed[0] ?? null;
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
	const account = multisig.add(
		address,
		genesis,
		!answer.pending,
		used.length > 0 ? used : chainIds.slice(0, 1)
	);
	// It came from the service: the service can hand it out.
	multisig.markShared(account.address);
	multisig.activate(account.address);
	return { ok: true, account };
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

async function waitForTask(id: string): Promise<void> {
	for (let i = 0; i < 30; i += 1) {
		await new Promise((r) => setTimeout(r, 2000));
		const task = await fetchTask(id).catch(() => null);
		if (task && (task.status === 'done' || task.status === 'failed')) return;
	}
}

export type ImportOutcome =
	| { ok: true; account: MultisigAccount }
	| { ok: false; reason: 'notFound' | 'unavailable' | 'unsupported' | 'notDeployed' };

/**
 * Import one Safe from a Safe{Wallet} export. Its genesis is, in order: the
 * one the genesis service holds; one derived here from Safe{Wallet}'s own
 * creation parameters (an undeployed Safe); or the creation Safe's
 * transaction service has on record (a deployed one) — read from here, so
 * importing never depends on the genesis service, which is then asked to
 * record it (best effort). Whichever it is, the core must derive the Safe's
 * address from it. The address book's name is kept as this device's label.
 * A Safe already here is kept, and given the label if it has none.
 */
export async function importExported(safe: ExportedSafe): Promise<ImportOutcome> {
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
	const account = multisig.add(safe.address, genesis, recorded, chainIds);
	if (safe.name) multisig.setLabel(account.address, safe.name);
	if (recorded) multisig.markShared(account.address);
	else {
		// The genesis service, if it is there, records it for other devices.
		const deployedOn = safe.chains.find((c) => c.deployed)?.chainId;
		if (deployedOn !== undefined) {
			void importGenesis({ chainId: deployedOn, safe: safe.address }).catch(() => {});
		}
	}
	return { ok: true, account: multisig.find(account.address) ?? account };
}
