/**
 * The core's multisig rules (`vela_core::multisig`), through its one door
 * (`multisigCall`). Every hash, signature layout and verdict is the core's;
 * this file only names the JSON. Callers run after `loadCore()`.
 */
import * as wasm from '../../../../../../rust/pkg-web/vela_core.js';

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

/** One owner's approval, either kind (told apart by their fields). */
export type Approval = PasskeyApproval | EcdsaApproval;

export const isEcdsaApproval = (a: Approval): a is EcdsaApproval => 'signature' in a;

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

/**
 * The calls a site's `eth_sendTransaction` / `wallet_sendCalls` asks a
 * multisig to make on `chainId` (`dapp_rpc::multisig_calls`), or the error the
 * site is answered with (another chain, a required capability, no calls, a
 * call with no `to`).
 */
export function dappMultisigCalls(
	method: string,
	params: unknown,
	chainId: number
): { calls: CallArg[] } | { error: { code: number; message: string } } {
	const answer = JSON.parse(
		wasm.dappRpcMultisigCalls(method, JSON.stringify(params ?? null), chainId)
	) as { result?: CallArg[]; error?: { code: number; message: string } };
	return answer.error ? { error: answer.error } : { calls: answer.result ?? [] };
}

/** `wallet_sendCalls`' answer for batch `id`, in the shape its request asked for (the core's rule). */
export function sendCallsResult(params: unknown, id: string): unknown {
	return JSON.parse(wasm.dappRpcSendCallsResult(JSON.stringify(params ?? null), id));
}

export const cancelTx = (multisig: string, nonce: string) =>
	call<SafeTx>('cancelTx', { multisig, nonce });

/** The same address, whatever its case. */
export function sameAddress(a: string | null | undefined, b: string | null | undefined): boolean {
	return !!a && !!b && a.toLowerCase() === b.toLowerCase();
}

/** A plain account's approval, checked as the Safe checks it; `v` comes back as 27/28. */
export const verifyEcdsaApproval = (safeTxHash: string, approval: EcdsaApproval) =>
	call<EcdsaApproval>('verifyEcdsaApproval', { safeTxHash, approval });

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
