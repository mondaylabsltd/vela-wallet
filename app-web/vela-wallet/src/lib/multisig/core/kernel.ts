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
	| { kind: 'changeThreshold'; threshold: string };

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
	approvals: PasskeyApproval[];
	executor: string;
}

/** A founding key as the account record stores it. */
export interface FoundingKey {
	credentialId: string;
	publicKeyHex: string;
}

function call<T>(method: string, args: Record<string, unknown>): T {
	return JSON.parse(wasm.multisigCall(method, JSON.stringify(args))) as T;
}

export const multisigGenesis = (owners: string[], threshold: number, saltNonce: string) =>
	call<MultisigGenesis>('genesis', { owners, threshold, saltNonce });

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
