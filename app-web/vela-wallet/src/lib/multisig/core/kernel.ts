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
	 * A name written with the record (registry v2 `recordNamed`): a label,
	 * not part of the address, never changed, not unique.
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
	| { kind: 'batch'; transfers: Extract<Action, { kind: 'transfer' }>[] }
	| { kind: 'cancel' }
	| { kind: 'addOwner'; owner: string; threshold: string }
	| { kind: 'removeOwner'; owner: string; threshold: string }
	| { kind: 'swapOwner'; oldOwner: string; newOwner: string }
	| { kind: 'changeThreshold'; threshold: string }
	/** A dApp's call to another contract; `value` in base units, decimal. */
	| { kind: 'call'; to: string; value: string; data: string };

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

/** The genesis registry (v2): the same address on every chain. */
export const GENESIS_REGISTRY = '0x70b35C0FC8885019ff938542CD4E431725874802';

/** The registry call that records `genesis` (with its name) on any chain; anyone may send it. */
export const recordCall = (genesis: Genesis) => call<Call>('recordCall', { genesis });

/** The genesis inside a creating transaction's calldata that derives `safe`, if any. */
export const genesisFromCalldata = (input: string, safe: string) =>
	call<Genesis | null>('genesisFromCalldata', { input, safe });
