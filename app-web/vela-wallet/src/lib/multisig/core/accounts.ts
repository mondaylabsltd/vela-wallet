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
import { hasCode } from './chain';
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
	let answer;
	try {
		answer = await fetchGenesis(address);
		// A multisig created moments ago: its record may still be on its way
		// to the registry. Ask again for a little while before giving up.
		for (let wait = 0; !answer.found && wait < 4; wait += 1) {
			await new Promise((r) => setTimeout(r, 5000));
			answer = await fetchGenesis(address);
		}
	} catch {
		return { ok: false, reason: 'unavailable' };
	}
	if (!answer.found) {
		const deployedOn = await firstDeployed(address, chainIds);
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

async function firstDeployed(address: string, chainIds: number[]): Promise<number | null> {
	const found = await deployedChains(address, chainIds);
	return found[0] ?? null;
}

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
	| { ok: false; reason: 'notFound' | 'unavailable' | 'unsupported' };

/**
 * Import one Safe from a Safe{Wallet} export. Its genesis is, in order: the
 * one the genesis service holds; one derived here from Safe{Wallet}'s own
 * creation parameters (an undeployed Safe); or one the service resolves from
 * a chain it is deployed on, and records. Whichever it is, the core must
 * derive the Safe's address from it. The address book's name is kept as this
 * device's label. A Safe already here is kept, and given the label if it has
 * none.
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
	let serviceDown = false;
	try {
		const known = await fetchGenesis(safe.address);
		if (known.found && known.genesis) {
			genesis = known.genesis;
			recorded = !known.pending;
		}
	} catch {
		serviceDown = true;
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
	if (!genesis && !serviceDown) {
		for (const c of safe.chains.filter((c) => c.deployed)) {
			try {
				const imported = await importGenesis({ chainId: c.chainId, safe: safe.address });
				if (imported.status === 'queued') await waitForTask(imported.taskId);
				if (imported.status === 'error') serviceDown = true;
				if (imported.status === 'error' || imported.status === 'refused') continue;
				const known = await fetchGenesis(safe.address);
				if (known.found && known.genesis) {
					genesis = known.genesis;
					recorded = !known.pending;
					serviceDown = false;
					break;
				}
			} catch {
				// the next chain may be one the service reads
				serviceDown = true;
			}
		}
	}
	if (!genesis) return { ok: false, reason: serviceDown ? 'unavailable' : 'notFound' };
	try {
		if (!sameAddress(genesisAddress(genesis), safe.address))
			return { ok: false, reason: 'notFound' };
	} catch {
		return { ok: false, reason: 'unsupported' };
	}
	const account = multisig.add(safe.address, genesis, recorded, chainIds);
	if (safe.name) multisig.setLabel(account.address, safe.name);
	if (recorded) multisig.markShared(account.address);
	return { ok: true, account: multisig.find(account.address) ?? account };
}
