/**
 * The core's multisig rules (`vela_core::multisig`), through its one door
 * (`multisigCall`). Every hash, signature layout and verdict is the core's;
 * this file only names the JSON. Callers run after `loadCore()`.
 */
import * as wasm from '../../../../../../rust/pkg-web/vela_core.js';
import type { SimVerdict } from '$lib/core/generated/SimVerdict';

/** A Safe's creation parameters, as the Safe Genesis Registry stores them. */
export interface Genesis {
	factory: string;
	singleton: string;
	saltNonce: string;
	initializer: string;
	/**
	 * The record's name (registry v4): a label, not part of the address, not
	 * unique. The registry requires one, 1 to 64 bytes; whoever writes the
	 * record first chooses it, and the Safe itself may change it (`rename`).
	 * A genesis read from chain history has none until one is given
	 * ({@link namedGenesis}).
	 */
	name?: string;
}

export interface MultisigGenesis {
	address: string;
	genesis: Genesis;
	owners: string[];
	threshold: number;
}

/** A Safe transaction as `execTransaction` takes it; quantities are hex. */
export interface SafeTx {
	to: string;
	value: string;
	data: string;
	operation: number;
	safeTxGas: string;
	baseGas: string;
	gasPrice: string;
	gasToken: string;
	refundReceiver: string;
	nonce: string;
}

/** One owner's approval of a safeTxHash, by one of its passkeys. */
export interface PasskeyApproval {
	owner: string;
	ownerGenesis: Genesis;
	signer: string;
	authenticatorData: string;
	clientDataFields: string;
	r: string;
	s: string;
}

/**
 * An owner that is a plain account (MetaMask, Rabby, …): its EIP-712
 * signature of the SafeTx, `r ‖ s ‖ v`.
 */
export interface EcdsaApproval {
	owner: string;
	signature: string;
}

/**
 * An owner that approved on chain (`approveHash(safeTxHash)`, read back as
 * `approvedHashes(owner, hash) != 0`): it sits in the signatures as the
 * executor does (`v = 1`). Read from the chain, never sent to the mailbox.
 */
export interface OnChainApproval {
	owner: string;
	onChain: true;
}

/** One owner's approval, any kind (told apart by their fields). */
export type Approval = PasskeyApproval | EcdsaApproval | OnChainApproval;

export const isEcdsaApproval = (a: Approval): a is EcdsaApproval => 'signature' in a;
export const isPasskeyApproval = (a: Approval): a is PasskeyApproval => 'ownerGenesis' in a;
export const isOnChainApproval = (a: Approval): a is OnChainApproval => 'onChain' in a;

export interface Call {
	to: string;
	value: string;
	data: string;
}

export interface Transfer {
	/** Absent or empty: the chain's native coin. */
	token?: string | null;
	to: string;
	/** Base units, decimal or hex. */
	amount: string;
}

/** What a SafeTx does, as the core reads it. Amounts are base units, decimal. */
export type Action =
	| { kind: 'transfer'; token: string | null; to: string; amount: string }
	/** Several calls in ONE transaction (MultiSendCallOnly), every one a CALL. */
	| { kind: 'batch'; calls: BatchCall[] }
	| { kind: 'cancel' }
	| { kind: 'addOwner'; owner: string; threshold: string }
	| { kind: 'removeOwner'; owner: string; threshold: string }
	| { kind: 'swapOwner'; oldOwner: string; newOwner: string }
	| { kind: 'changeThreshold'; threshold: string }
	/** The multisig renames its own record in the genesis registry (a label; nothing else changes). */
	| { kind: 'rename'; name: string }
	/** A dApp's call to another contract; `value` in base units, decimal. */
	| { kind: 'call'; to: string; value: string; data: string };

/** The kinds of action that change a multisig's own members or threshold. */
export const MEMBER_CHANGES = ['addOwner', 'removeOwner', 'swapOwner', 'changeThreshold'] as const;

/**
 * One call of a batch: as packed (`value` base units, decimal; `data` hex)
 * and as the core reads it — a transfer, a call to another contract, or a
 * member change (a call into the multisig itself).
 */
export interface BatchCall {
	to: string;
	value: string;
	data: string;
	action: Exclude<Action, { kind: 'batch' } | { kind: 'cancel' }>;
}

/** The transfers a transaction makes (one, or a batch's), as the activity records them. */
export function transfersOf(action: Action | null): Extract<Action, { kind: 'transfer' }>[] {
	if (action?.kind === 'transfer') return [action];
	if (action?.kind !== 'batch') return [];
	return action.calls.flatMap((c) => (c.action.kind === 'transfer' ? [c.action] : []));
}

export interface Review {
	safeTxHash: string;
	nonce: string;
	/** `null` when Vela does not sign this transaction; `refusal` says why. */
	action: Action | null;
	refusal: string | null;
}

export interface ExecutionPlan {
	multisig: string;
	deployMultisig: Genesis | null;
	deployOwners: Genesis[];
	tx: SafeTx;
	approvals: Approval[];
	/** The executor when it is an owner (it approves as `msg.sender`); `null` for any other sender. */
	executor: string | null;
	/**
	 * The code of what `deployMultisig` depends on, read on the chain just now
	 * ({@link counterfactualReads}): the core deploys it only from a genesis
	 * Verified there.
	 */
	deployFacts?: ChainFacts | null;
}

/** One Safe in a Safe{Wallet} export, across the chains it is listed on. */
export interface ExportedSafe {
	address: string;
	/** The person's own name for it (the export's address book): a note on this device. */
	name: string | null;
	chains: {
		chainId: number;
		deployed: boolean;
		owners: string[];
		threshold: number;
		/** An undeployed Safe's creation parameters (the service's `safeWalletProps`). */
		props: unknown | null;
		version: string | null;
	}[];
}

/** A founding key as the account record stores it. */
export interface FoundingKey {
	credentialId: string;
	publicKeyHex: string;
}

function call<T>(method: string, args: Record<string, unknown>): T {
	return JSON.parse(wasm.multisigCall(method, JSON.stringify(args))) as T;
}

export const multisigGenesis = (
	owners: string[],
	threshold: number,
	name: string,
	saltNonce: string
) => call<MultisigGenesis>('genesis', { owners, threshold, name, saltNonce });

/** Whether a multisig can be created with this name (the registry's rule). */
export const validName = (name: string) => call<boolean>('validName', { name });

/** Whether `address` can be a member (not the zero address, not Safe's `0x…01` sentinel). */
export const validOwner = (address: string) => call<boolean>('validOwner', { address });

export const genesisAddress = (genesis: Genesis) => call<string>('genesisAddress', { genesis });

export const genesisMembers = (genesis: Genesis) =>
	call<{ owners: string[]; threshold: number }>('genesisMembers', { genesis });

export const velaAccountGenesis = (keys: FoundingKey[]) =>
	call<{ address: string; genesis: Genesis }>('velaAccountGenesis', { keys });

export const deployCall = (genesis: Genesis) => call<Call>('deployCall', { genesis });

export const safeTxHash = (chainId: number, safe: string, tx: SafeTx) =>
	call<string>('safeTxHash', { chainId, safe, tx });

export const approvalChallenge = (chainId: number, owner: string, safeTxHash: string) =>
	call<string>('approvalChallenge', { chainId, owner, safeTxHash });

export const sponsorChallenge = (multisig: string) =>
	call<string>('sponsorChallenge', { multisig });

export const approvalFromAssertion = (
	keys: FoundingKey[],
	assertion: {
		credentialId: string;
		authenticatorData: string;
		clientDataJSON: string;
		signature: string;
	}
) => call<PasskeyApproval>('approvalFromAssertion', { keys, ...assertion });

export const verifyApproval = (chainId: number, safeTxHash: string, approval: PasskeyApproval) =>
	call<boolean>('verifyApproval', { chainId, safeTxHash, approval });

export const executionCalls = (plan: ExecutionPlan) => call<Call[]>('executionCalls', { plan });

export const reviewTx = (chainId: number, multisig: string, tx: SafeTx) =>
	call<Review>('review', { chainId, multisig, tx });

export const transfersTx = (transfers: Transfer[], nonce: string) =>
	call<SafeTx>('transfersTx', { transfers, nonce });

/** A site's call, as `eth_sendTransaction` or one of `wallet_sendCalls`' calls carries it. */
export interface CallArg {
	to: string;
	value: string;
	data: string;
}

/** A site's calls as ONE transaction: one is a plain CALL, several one MultiSendCallOnly batch. */
export const callsTx = (calls: CallArg[], nonce: string) =>
	call<SafeTx>('callsTx', { calls, nonce });

export const cancelTx = (multisig: string, nonce: string) =>
	call<SafeTx>('cancelTx', { multisig, nonce });

/** The same address, whatever its case. */
export function sameAddress(a: string | null | undefined, b: string | null | undefined): boolean {
	return !!a && !!b && a.toLowerCase() === b.toLowerCase();
}

/**
 * A plain account's approval, checked as the Safe checks it; `v` comes back
 * as 27/28 (typed data), or 31/32 (the `eth_sign` form).
 */
export const verifyEcdsaApproval = (safeTxHash: string, approval: EcdsaApproval) =>
	call<EcdsaApproval>('verifyEcdsaApproval', { safeTxHash, approval });

/**
 * How a plain account signed: EIP-712 typed data (`v` 27/28), or Safe's
 * `eth_sign` form (`v` 31/32: `personal_sign` over the safeTxHash, for a
 * wallet that cannot sign typed data). Throws for anything else.
 */
export const ecdsaForm = (signature: string) =>
	call<'typedData' | 'ethSign'>('ecdsaForm', { signature });

/**
 * The hashes a hardware wallet shows for the SafeTx's typed data — domain
 * hash, message hash, and the safeTxHash they make — and what `personal_sign`
 * signs in the `eth_sign` form, for the owner to match on the device.
 */
export interface SafeTxHashes {
	domainHash: string;
	messageHash: string;
	safeTxHash: string;
	ethSignHash: string;
}

export const safeTxHashes = (chainId: number, safe: string, tx: SafeTx) =>
	call<SafeTxHashes>('safeTxHashes', { chainId, safe, tx });

/** The SafeTx as `eth_signTypedData_v4` takes it (its EIP-712 hash is the safeTxHash). */
export const safeTxTypedData = (chainId: number, safe: string, tx: SafeTx) =>
	call<Record<string, unknown>>('safeTxTypedData', { chainId, safe, tx });

/** The signatures `execTransaction` takes, sorted by owner; no executor when a wallet sends it. */
export const signatures = (approvals: Approval[], executor: string | null) =>
	call<string>('signatures', { approvals, executor });

export const execTransactionData = (tx: SafeTx, signatures: string) =>
	call<string>('execTransactionData', { tx, signatures });

/** An undeployed Safe{Wallet} Safe's genesis from its props; throws unless it derives `address`. */
export const safeWalletPropsGenesis = (props: unknown, address: string) =>
	call<Genesis>('safeWalletPropsGenesis', { props, address });

/** A Safe{Wallet} export, merged by address. Throws on a file that is not one. */
export const parseSafeExport = (json: string) => call<ExportedSafe[]>('parseSafeExport', { json });

/**
 * The Safe Genesis Registry (v4, `keccak256("safe-genesis-registry/v4")`
 * through the deterministic deployer): the same address on every chain it is
 * on. The core's `GENESIS_REGISTRY`, which a test pins against it.
 */
export const GENESIS_REGISTRY = '0xF412388bF908E510066d3Be5c704bd9A327f945d';

/**
 * The registry call that records `genesis` on any chain, with its name — its
 * own, else the one the service would give it ({@link defaultName}); anyone
 * may send it.
 */
export const recordCall = (genesis: Genesis) => call<Call>('recordCall', { genesis });

/** Whether the registry takes `name` (1 to 64 bytes); {@link validName} is the wallet's narrower rule. */
export const recordableName = (name: string) => call<boolean>('recordableName', { name });

/** The service's name for a Safe recorded without one ("Quiet Harbor"), from its address. */
export const defaultName = (safe: string) => call<string>('defaultName', { safe });

/**
 * `genesis` as the registry and its service take it: with a name (its own,
 * else {@link defaultName}). The service ignores a genesis sent without one.
 */
export const namedGenesis = (genesis: Genesis) => call<Genesis>('namedGenesis', { genesis });

// -- changing the members (the core's `member_change_tx`) --------------------------

/** A change the person asks for; built per network against that network's members. */
export type MemberChangeRequest =
	/** No threshold: keep the network's. */
	| { kind: 'add'; owner: string; threshold?: number }
	/** No threshold: keep the network's (refused when too few would be left for it). */
	| { kind: 'remove'; owner: string; threshold?: number }
	/** The way back from a lost passkey: the member's new account in its place. */
	| { kind: 'replace'; oldOwner: string; newOwner: string }
	| { kind: 'threshold'; threshold: number };

/** One network's members now: owners as `getOwners()` lists them, and whether a member change waits there. */
export interface MemberState {
	owners: string[];
	threshold: number;
	changeWaiting: boolean;
}

export interface MemberSet {
	owners: string[];
	threshold: number;
}

/** Why a change cannot be made on a network (the core's `MemberRefusal`). */
export type MemberRefusal =
	| { kind: 'notAnAddress'; address: string }
	| { kind: 'noOne'; address: string }
	| { kind: 'theMultisig' }
	| { kind: 'alreadyMember'; owner: string }
	| { kind: 'notMember'; owner: string }
	| { kind: 'lastMember' }
	| { kind: 'thresholdTooHigh'; threshold: number; owners: number }
	| { kind: 'thresholdZero' }
	| { kind: 'belowThreshold'; owners: number; threshold: number }
	| { kind: 'unchanged' }
	| { kind: 'tooMany'; max: number }
	| { kind: 'changeWaiting' };

export type MemberVerdict =
	| { kind: 'ready'; tx: SafeTx; before: MemberSet; after: MemberSet }
	| { kind: 'refused'; refusal: MemberRefusal };

/**
 * `change` as the multisig's own transaction at `nonce` on a network whose
 * members are `state` — `addOwnerWithThreshold`, `removeOwner` /
 * `swapOwner` with the previous owner from that network's list, or
 * `changeThreshold` — with the members before and after; or the core's
 * reason it cannot be made there.
 */
export const memberChangeTx = (
	multisig: string,
	state: MemberState,
	change: MemberChangeRequest,
	nonce: string
) => call<MemberVerdict>('memberChangeTx', { multisig, state, change, nonce });

/** The multisig's transaction that renames its record in the registry, at `nonce`. */
export const renameTx = (name: string, nonce: string) => call<SafeTx>('renameTx', { name, nonce });

/** `genesisOf(safe)` calldata, for an `eth_call` to {@link GENESIS_REGISTRY}. */
export const genesisOfCall = (safe: string) => call<string>('genesisOfCall', { safe });

/** What one chain's registry holds for a Safe, as the core reads `genesisOf`. */
export interface RegistryRecord {
	/** The genesis, with the record's name on that chain. */
	genesis: Genesis;
	proxyCreationCodeHash: string;
	recorder: string;
	recordedAt: number;
	index: number;
	/** Block of the Safe's last rename there, 0 if it never renamed itself. */
	renamedAt: number;
	/** The name is the Safe's own (it renamed itself, or wrote the record). */
	nameBySafe: boolean;
}

/**
 * A `genesisOf(safe)` answer read by the core: `null` when the registry holds
 * nothing for `safe`. Throws on an answer that is not one, or whose genesis
 * does not make `safe` (the core re-derives it).
 */
export const readGenesisOf = (safe: string, ret: string) =>
	call<RegistryRecord | null>('readGenesisOf', { safe, ret });

/** The genesis inside a creating transaction's calldata that derives `safe`, if any. */
export const genesisFromCalldata = (input: string, safe: string) =>
	call<Genesis | null>('genesisFromCalldata', { input, safe });

// -- where a multisig can live, and where it does ---------------------------------

/**
 * The Safe v1.4.1 contracts a new multisig's genesis names (factory,
 * singleton, fallback handler): a chain with code at all three can deploy it.
 */
export const multisigContracts = () =>
	call<{ name: string; address: string }[]>('multisigContracts', {});

/** Whether a chain holds those contracts, as the core reads it. */
export type SafeContracts =
	| { kind: 'present' }
	| { kind: 'missing'; names: string[] }
	/** The chain did not answer for some: nothing can be said either way. */
	| { kind: 'unknown' };

/**
 * The core's verdict on one chain from `eth_getCode` of each of
 * {@link multisigContracts}, in that order (`null`: the chain did not answer).
 */
export const safeContracts = (codes: (string | null)[]) =>
	call<SafeContracts>('safeContracts', { codes });

/** What says which networks a multisig is used on (the core's `NetworkEvidence`). */
export interface NetworkEvidence {
	/** Chains it has code on. */
	deployed?: number[];
	/** Chains the mailbox holds proposals of it for. */
	proposals?: number[];
	/** The network a link names, or the person picked. */
	named?: number | null;
	/** What this device kept: the creator's pick, and every network used here since. */
	kept?: number[];
	/**
	 * The networks this device has. Given, one it lacks is never a network
	 * of the multisig here ({@link missingNetworks} lists it instead).
	 */
	have?: number[] | null;
}

/**
 * The networks a multisig is used on, by the core's rule: deployed, then
 * proposals, then named, then kept — never the chain its record is on (the
 * registry is an index). Empty: nothing says; ask the person.
 */
export const multisigNetworks = (evidence: NetworkEvidence) =>
	call<number[]>('multisigNetworks', { evidence });

/**
 * The networks the mailbox's proposals (or a link) put the multisig on that
 * this device does not have (`have`): shown, offered to add, never read or
 * judged.
 */
export const missingNetworks = (evidence: NetworkEvidence) =>
	call<number[]>('missingNetworks', { evidence });

/** How a network's members now differ from the original ones (the core's `MemberDrift`). */
export interface MemberDrift {
	same: boolean;
	added: string[];
	removed: string[];
	thresholdWas: number;
	thresholdIs: number;
}

export const memberDrift = (
	original: { owners: string[]; threshold: number },
	current: { owners: string[]; threshold: number }
) =>
	call<MemberDrift>('memberDrift', {
		original: original.owners,
		originalThreshold: original.threshold,
		current: current.owners,
		currentThreshold: current.threshold
	});

// -- what a transaction changes about the members; an execution's facts ----------

/** "M of N": approvals needed, of how many members. */
export interface Rule {
	threshold: number;
	owners: number;
}

/** One member change, read against the members it meets (the core's `MemberChange`). */
export interface MemberChange {
	/** Its call's number in a batch (from 1); `null` for a transaction that is the change alone. */
	call: number | null;
	kind: 'add' | 'remove' | 'replace' | 'threshold';
	added: string | null;
	removed: string | null;
	before: Rule;
	after: Rule;
	/** Safe accepts it against the members it meets; `false`: executing it would revert. */
	fits: boolean;
}

export interface MemberChanges {
	changes: MemberChange[];
	before: Rule;
	after: Rule;
	fits: boolean;
}

/**
 * The member changes `action` makes to a multisig whose members on that
 * network are `owners` with `threshold` — in order, each against what the
 * ones before it left — or `null` when it makes none (`multisig_reading`).
 */
export const readMemberChanges = (
	action: Action,
	members: { owners: string[]; threshold: number }
) =>
	call<MemberChanges | null>('memberChanges', {
		action,
		owners: members.owners,
		threshold: members.threshold
	});

/** One log of a transaction receipt, as the node gave it. */
export interface ReceiptLog {
	address: string;
	topics: string[];
	data: string;
}

/** What an executed transaction was then, from its own receipt (the core's `ExecutionFacts`). */
export interface ExecutionFacts {
	executor: string;
	threshold: number;
	nonce: string;
	/** The members whose approvals counted, in the order Safe checked them. */
	approvers: string[];
	/** The executor approved by sending it. */
	executorApproved: boolean;
}

/**
 * The frozen facts of `safeTxHash`'s execution, read from the receipt logs of
 * the transaction that executed it; `null` when they do not say (a Safe that
 * is not an L2 one emits no `SafeMultiSigTransaction`).
 */
export const executionFacts = (
	chainId: number,
	safe: string,
	safeTxHash: string,
	logs: ReceiptLog[]
) => call<ExecutionFacts | null>('executionFacts', { chainId, safe, safeTxHash, logs });

// -- the multisig service's health answer (`app::genesis_service`) ---------------

/** A chain the service says it serves (its own name for it, if any). */
export interface ServiceChain {
	chainId: number;
	name: string | null;
	/** The registry is there and the service reads it (records can be looked up there). */
	registry: boolean;
}

/**
 * What a `GET /api/health` answer says the service is. `ok: false`: its own
 * 503 — there and answering, but not all of its job can be done now.
 */
export type ServiceAnswer =
	| {
			kind: 'genesis';
			chains: ServiceChain[] | null;
			homeChainId: number | null;
			ok: boolean;
	  }
	| { kind: 'other' }
	| { kind: 'unreachable' };

/** Whether a health answer (its status and body text) is a multisig service's. */
export const serviceAnswer = (status: number, body: string) =>
	call<ServiceAnswer>('serviceAnswer', { status, body });

// -- who can act for a Safe: its chain, or the counterfactual rule (§3.5) ----------

/** `eth_getCode` answers on one chain, by address (`null`: the chain did not answer). */
export interface ChainFacts {
	codes: Record<string, string | null>;
}

/** Why a genesis is not the Safe it claims, or cannot be stood behind on a chain (the core's). */
export type Refusal =
	| { kind: 'wrongAddress' }
	| { kind: 'notSetup' }
	| { kind: 'badMembers' }
	| { kind: 'unknownFactory'; address: string }
	| { kind: 'unknownSingleton'; address: string }
	/** Its setup enables a module: it can move funds without the members. */
	| { kind: 'module'; address: string }
	/** Its setup runs code there (a delegatecall) Vela cannot read in full. */
	| { kind: 'setupCode'; address: string }
	| { kind: 'handler'; address: string }
	/** Its setup pays `amount` (base units) of `token` (zero: the coin) to `receiver` on deployment. */
	| { kind: 'payment'; token: string; amount: string; receiver: string }
	| { kind: 'missingCode'; name: string; address: string }
	| { kind: 'wrongCode'; name: string; address: string };

/** Who can act for a Safe not deployed on a chain yet, once deployed there (the core's verdict). */
export type Authority =
	| { kind: 'verified'; owners: string[]; threshold: number }
	| { kind: 'refused'; reasons: Refusal[] }
	/** Nothing wrong found, but the chain did not answer for these: not verified yet. */
	| { kind: 'unknown'; unread: string[] };

/** The contracts whose code the counterfactual verdict needs read on the target chain. */
export const counterfactualReads = (genesis: Genesis, safe: string) =>
	call<string[]>('counterfactualReads', { genesis, safe });

/** The core's counterfactual verdict for `safe` from `genesis`, with the target chain's code. */
export const counterfactualAuthority = (genesis: Genesis, safe: string, facts: ChainFacts) =>
	call<Authority>('counterfactualAuthority', { genesis, safe, facts });

/** What can act for a deployed Safe besides its members, as its chain shows (the core's). */
export type Risk =
	| { kind: 'module'; address: string }
	| { kind: 'guard'; address: string }
	| { kind: 'handler'; address: string }
	| { kind: 'singleton'; address: string };

/** The reads a deployed Safe's check takes: `getModulesPaginated` call data and storage slots. */
export interface DeployedReads {
	modulesCall: string;
	guardSlot: string;
	moduleGuardSlot: string;
	handlerSlot: string;
	singletonSlot: string;
}

/** What was read on a deployed Safe's chain (`null`: the chain did not answer). */
export interface DeployedFacts {
	modules: string | null;
	guard: string | null;
	moduleGuard: string | null;
	handler: string | null;
	singleton: string | null;
	codes: Record<string, string | null>;
}

export interface DeployedCheck {
	risks: Risk[];
	/** Read `eth_getCode` of these and ask again with them in `codes`. */
	needCode: string[];
	/** The chain did not answer for something: nothing can be said yet. */
	unread: boolean;
}

export const deployedReads = () => call<DeployedReads>('deployedReads', {});

export const deployedCheck = (facts: DeployedFacts) =>
	call<DeployedCheck>('deployedCheck', { facts });

// -- an execution in flight (`multisig_watch`) -----------------------------------

/** What a lookup of the transaction that executed a safeTxHash found. */
export type ExecutedIn = { kind: 'found'; txHash: string } | { kind: 'none' } | { kind: 'unknown' };

/** What the relay says of this device's own operation. */
export type OurOp =
	| { kind: 'landed'; txHash: string }
	| { kind: 'pending' }
	| { kind: 'refused' }
	| { kind: 'unknown' };

/** What the receipt of this device's landed transaction shows (the core's `OurReceipt`). */
export type OurReceipt = 'executed' | 'notExecuted' | 'unread';

/** Where an execution in flight stands (the core's `ExecutionVerdict`). */
export type ExecutionVerdict =
	| { kind: 'executed'; txHash: string }
	| { kind: 'executedByOther'; txHash: string; paid: boolean }
	| { kind: 'replaced'; paid: boolean }
	| { kind: 'failed'; txHash: string }
	| { kind: 'notSent' }
	| { kind: 'unknown' }
	| { kind: 'waiting' }
	| { kind: 'timedOut' };

/** The core's rule: what the gathered facts say of an execution in flight. */
export const executionVerdict = (facts: {
	nonceUsed: boolean | null;
	executedIn: ExecutedIn;
	ourOp: OurOp;
	receipt?: OurReceipt;
	elapsedMs: number;
	limitMs: number;
}) => call<ExecutionVerdict>('executionVerdict', { facts });

// -- the one review, its reads, the gate (`multisig_review`, §3.6) ------------------

/** One inner call, as the signing sheet simulates calls (`value` a hex quantity). */
export interface SimCall {
	to: string;
	value: string;
	data: string;
}

/** A JSON-RPC request to send as it is. */
export interface RpcRequest {
	method: string;
	params: unknown[];
}

/** What a review starts from: the Safe on its chain, its members there now. */
export interface ReviewReadsInput {
	chainId: number;
	safe: string;
	tx: SafeTx;
	owners: string[];
	threshold: number;
	deployed: boolean;
	genesis?: Genesis | null;
	/** `eth_getCode` of what an earlier answer named in `needCode`. */
	codes?: Record<string, string | null>;
	/** Tokens whose balances the probe reads (the transaction's own are added). */
	tokens?: string[];
}

/** The reads a review needs (the core's `Reads`). */
export interface ReviewReads {
	needCode: string[];
	simCalls: SimCall[];
	trace: RpcRequest | null;
	probe: RpcRequest | null;
	unavailable: string | null;
	recipients: string[];
	tokens: string[];
}

/** One change to what decides who controls the multisig (`CriticalWrite`). */
export type CriticalWrite =
	| { kind: 'singleton'; before: string; after: string }
	| { kind: 'module'; address: string; enabled: boolean }
	| { kind: 'guard'; before: string; after: string }
	| { kind: 'moduleGuard'; before: string; after: string }
	| { kind: 'handler'; before: string; after: string }
	| { kind: 'owners'; added: string[]; removed: string[] }
	| { kind: 'ownerEntry'; address: string }
	| { kind: 'threshold'; before: string; after: string }
	| { kind: 'ownerCount'; before: string; after: string }
	| { kind: 'nonce' }
	| { kind: 'storage'; slot: string };

export type StateVerdict =
	| { kind: 'unchanged' }
	| { kind: 'declared' }
	| { kind: 'undeclared'; writes: CriticalWrite[] }
	| { kind: 'reverts' }
	| { kind: 'unavailable'; reason: string };

/** The state-diff guard's reading: from a trace, the probe, or nothing. */
export interface StateGuard {
	source: 'trace' | 'probe' | 'none';
	verdict: StateVerdict;
}

export interface RecipientFacts {
	address: string;
	isContract: boolean | null;
	firstTime: boolean | null;
}

/**
 * Whose an address the person knows is (r4a-18): a contact, a member of the
 * multisig, or one of this device's own accounts — the closest last.
 */
export type KnownKind = 'contact' | 'member' | 'own';

/** An address the person knows (a contact, an account, a member). */
export interface KnownAddress {
	address: string;
	label: string;
	/** Absent: a contact. */
	kind?: KnownKind;
}

export interface ReviewRecipient {
	call: number | null;
	role: 'recipient' | 'contract' | 'member';
	address: string;
	isContract: boolean | null;
	firstTime: boolean | null;
	lookalike: KnownAddress | null;
	/** The address the person knows it is, exactly (r4a-18): the closest of them. */
	known: KnownAddress | null;
	/** The core says "first time sending here" of it — never of one's own, a member, a new member. */
	firstTimeCue: boolean;
	/** The core says money goes to a contract — never to one's own smart account, nor a member. */
	contractCue: boolean;
}

export type ReviewConcern =
	| { kind: 'notSigned'; why: string }
	| { kind: 'stateChange' }
	| { kind: 'stateUnread'; reason: string }
	| { kind: 'reverts' }
	| { kind: 'unsimulated' }
	| { kind: 'lookalike'; address: string; of: string; label: string }
	| { kind: 'firstTime'; address: string }
	| { kind: 'toContract'; address: string }
	| { kind: 'blind' };

export type ReviewTone = 'ok' | 'caution' | 'danger' | 'refused';

/** Whether a signature may be given: `anyway` — the signing sheet's "Sign anyway" friction. */
export interface SignRule {
	allowed: boolean;
	anyway: boolean;
}

/** What the shell read for `multisigReview`. */
export interface ReviewInput extends ReviewReadsInput {
	simReply?: string | null;
	trace?: unknown;
	probe?: unknown;
	recipientFacts?: RecipientFacts[];
	known?: KnownAddress[];
	blind?: boolean;
	origin?: string | null;
}

/** The one review (the core's `MultisigReview`). */
export interface MultisigReviewOut {
	review: Review;
	memberChanges: MemberChanges | null;
	hashes: SafeTxHashes;
	fields: { name: string; value: string }[];
	typedData: unknown;
	recipients: ReviewRecipient[];
	state: StateGuard;
	/** The core's `SimVerdict` (snake_case, as the signing sheet's). */
	simulation: SimVerdict;
	simulationSource: 'simulate' | 'probe' | 'none';
	concerns: ReviewConcern[];
	tone: ReviewTone;
	sign: SignRule;
	/** What the agent reads as the request on screen: bound to this proposal. */
	adviceRequest: Record<string, unknown>;
}

export const reviewReads = (input: ReviewReadsInput) => call<ReviewReads>('reviewReads', { input });

export const multisigReview = (input: ReviewInput) =>
	call<MultisigReviewOut>('multisigReview', { input });

/** Whether this device signs, right now (the core's `Gate`). */
export type CoreGate =
	| { kind: 'open' }
	| { kind: 'unchecked' }
	| { kind: 'refused'; findings: unknown[] }
	| { kind: 'blocked' }
	| { kind: 'risk'; findings: unknown[] };

export const signingGateRule = (input: {
	chainId: number;
	findings: unknown[] | null;
	sign?: SignRule | null;
}) => call<CoreGate>('signingGate', { input });
