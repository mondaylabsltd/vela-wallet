/**
 * The app's own pipelines behind the multisig shell ({@link MultisigShellDeps}):
 * the RPC pool, `localStorage`, the passkey (in Vela, or on the member's
 * trusted signing page), the high-risk question, the send and fee
 * pipelines, the signing sheet's simulation, recipient facts, the relay's
 * word on a user operation, the injected wallet.
 *
 * Each answers in the vocabulary and never decides: a failure is reported
 * as what it was ({@link ShellFailure}, carried as the `detail` the core
 * hands back in its views), so the sheet can say it in the corpus' words.
 */
import { loadAccounts } from '$lib/onboarding/core/storage';
import { PasskeyError } from '$lib/onboarding/core/passkey';
import { fromHex, multisigApprovalRoute, signingPlan, verifySafeWebAuthn } from '$lib/core/kernels';
import { findAccountByAddress } from '$lib/services/accounts';
import { pollUserOpStatus } from '$lib/services/tx-reconciler';
import { resolveRecipientRisk } from '$lib/services/recipient-risk';
import { rpcSimulateReply } from '$lib/services/sim/sim-engine-rpc';
import { PoolFailedError, poolRpcCall } from '$lib/services/rpc-pool';
import {
	keySetOf,
	sendBatchCalls,
	UserOpNotSentError,
	type QuotedInBandFee,
	type SignFn
} from '$lib/services/safe-transaction';
import {
	PageDeclinedError,
	PageSignFailedError,
	signChallenge,
	VenueBlockedError
} from '$lib/signing/sign-challenge';
import { requestFirstPartyTransaction } from '$lib/backup/ethereum-backup';
import { signRequest } from '$lib/signing/core/sign-resident.svelte';
import { riskConfirmation } from '../core/gate.svelte';
import { approveOnPage, PageApprovalError } from '../core/page-approval.svelte';
import { quoteExecutionFee } from '../core/execution-fee';
import type { MultisigShellResult } from '../generated/MultisigShellResult';
import type { MultisigOperation } from '../generated/MultisigOperation';
import { passkeyRequest, type MultisigShellDeps, type PeerTransport } from './handler';
import { shellFailure, type ShellFailure } from './failure';

export { readShellFailure, failureText, shellFailure, type ShellFailure } from './failure';

type Op<T extends MultisigOperation['type']> = Extract<MultisigOperation, { type: T }>;

/** A dismissal the browser reports as its own (nothing said by the wallet). */
function browserCancel(error: unknown): boolean {
	return (
		error instanceof DOMException &&
		(error.name === 'NotAllowedError' || error.name === 'AbortError')
	);
}

/** What a thrown signing error was, in the vocabulary. */
function signingFailure(error: unknown): MultisigShellResult {
	if (error instanceof PasskeyError && error.kind === 'cancelled' && !error.timedOut) {
		return { type: 'passkey_cancelled' };
	}
	if (browserCancel(error)) return { type: 'passkey_cancelled' };
	return { type: 'passkey_failed', detail: shellFailure(failureOf(error)) };
}

function failureOf(error: unknown): ShellFailure {
	const failure = classified(error);
	return error instanceof Error && failure.message === undefined
		? { ...failure, message: error.message }
		: failure;
}

function classified(error: unknown): ShellFailure {
	if (error instanceof VenueBlockedError) return { kind: 'venue_blocked', block: error.block };
	if (error instanceof PageDeclinedError) return { kind: 'page_declined' };
	if (error instanceof PageSignFailedError) return { kind: 'page_sign_failed', page: error.page };
	if (error instanceof PageApprovalError) {
		return {
			kind: 'page_approval',
			failure: error.failure,
			code: error.code ?? null,
			line: error.line ?? null
		};
	}
	if (error instanceof PasskeyError) {
		return { kind: 'passkey', failure: error.kind, timedOut: error.timedOut };
	}
	if (error instanceof UserOpNotSentError) return { kind: 'not_sent' };
	return { kind: 'unknown', message: error instanceof Error ? error.message : String(error) };
}

function bytes(hexText: string): Uint8Array {
	const clean = hexText.startsWith('0x') ? hexText.slice(2) : hexText;
	const out = new Uint8Array(clean.length / 2);
	for (let i = 0; i < out.length; i += 1) out[i] = parseInt(clean.slice(i * 2, i * 2 + 2), 16);
	return out;
}

const hex0x = (h: string) => (h.startsWith('0x') ? h : `0x${h}`);

function keysOf(address: string) {
	const stored = findAccountByAddress(address);
	return stored ? keySetOf(stored).keys : [];
}

/** `owner` signs `challenge`: in Vela, or on its trusted page for an approval routed there. */
async function passkey(op: Op<'passkey'>, signal: AbortSignal): Promise<MultisigShellResult> {
	const record = loadAccounts().find((a) => a.address.toLowerCase() === op.owner.toLowerCase());
	if (!record) {
		return {
			type: 'passkey_failed',
			detail: shellFailure({ kind: 'unknown', message: 'not an account on this device' })
		};
	}
	const keys = keysOf(record.address);
	try {
		if (op.purpose.kind === 'approve') {
			const route = multisigApprovalRoute(record);
			if (route?.type === 'blocked') throw new VenueBlockedError(route.block);
			if (route?.type === 'page') {
				const plan = signingPlan(record);
				const approval = await approveOnPage({
					member: record,
					page: route.url,
					chainId: op.chain_id,
					multisig: op.purpose.multisig,
					multisigName: op.purpose.name,
					anyway: op.purpose.anyway,
					tx: op.purpose.tx as never,
					safeTxHash: op.purpose.safe_tx_hash,
					keys,
					keyRoute: plan?.key ?? undefined,
					keyLabel: plan?.key_label ?? null,
					signal
				});
				return { type: 'page_approved', approval };
			}
		}
		const request = passkeyRequest(op.purpose);
		const assertion = await signChallenge(bytes(op.challenge), {
			account: record.address,
			keys,
			credentials: keys.map((k) => ({ id: k.credentialId })),
			request: { ...request, origin: '', chainId: op.chain_id }
		});
		return {
			type: 'asserted',
			credential_id: assertion.credentialId,
			authenticator_data: hex0x(assertion.authenticatorDataHex),
			client_data_json: hex0x(assertion.clientDataJSONHex),
			signature: hex0x(assertion.signatureHex)
		};
	} catch (error) {
		return signingFailure(error);
	}
}

/** How long before the one automatic retry of a send the relay never took. */
export const RELAY_RETRY_MS = 1_500;

/** r4a-23: out of reach before anything was signed — asked once more. */
function unreachable(error: unknown): boolean {
	if (error instanceof PoolFailedError) return !error.rateLimited && !error.internal;
	return error instanceof UserOpNotSentError && error.rejection === null;
}

/** The fee quote as the core reads it: amounts decimal text. */
function quoteJson(quote: Awaited<ReturnType<typeof quoteExecutionFee>>): unknown {
	return JSON.parse(
		JSON.stringify(quote, (_key, value) => (typeof value === 'bigint' ? value.toString() : value))
	);
}

/** The fee the person saw, back as the send pipeline signs it. */
function feeOf(fee: unknown): QuotedInBandFee | undefined {
	if (!fee || typeof fee !== 'object') return undefined;
	const f = fee as { amount?: unknown; recipient?: unknown; tier?: unknown; token?: unknown };
	if (typeof f.recipient !== 'string' || f.amount === undefined || f.amount === null) {
		return undefined;
	}
	return {
		amount: BigInt(String(f.amount)),
		recipient: f.recipient,
		...(typeof f.tier === 'string' ? { tier: f.tier } : {})
	} as QuotedInBandFee;
}

/**
 * What the executor's trusted signing page is shown beside the operation
 * (round 7, r7-1): the page reads what runs from the operation's bytes; this
 * says what they do not carry — the transaction (the number it runs at; the
 * core adds its safeTxHash there), the multisig, its name and how many
 * approvals it needs — each drawn as the app's word, the hash checked
 * against the bytes.
 */
export function executeRequest(purpose: Op<'send_calls'>['purpose']): {
	method: 'vela_multisigExecute';
	params: unknown;
} {
	if (purpose?.kind !== 'execute') return { method: 'vela_multisigExecute', params: {} };
	return {
		method: 'vela_multisigExecute',
		params: {
			tx: purpose.tx,
			multisig: purpose.multisig,
			name: purpose.name ?? null,
			threshold: purpose.threshold
		}
	};
}

/**
 * `from` sends `calls` as one user operation, by what it is for
 * (`purpose`). An execution: the multisig path's own — ONE signature, the
 * executor's own operation (r7-3), its page handed the operation and what it
 * runs; the coin checked before the passkey; one retry when the relay was out
 * of reach before anything was signed. The Ethereum backup's record call: the
 * signing sheet, as any first-party transaction.
 */
async function sendCalls(op: Op<'send_calls'>): Promise<MultisigShellResult> {
	// An older machine says no purpose: an execution is the one with a fee.
	const record = op.purpose
		? op.purpose.kind === 'record'
		: op.fee === null || op.fee === undefined;
	if (record) return firstParty(op);
	const stored = findAccountByAddress(op.from);
	if (!stored) {
		return {
			type: 'calls_failed',
			detail: shellFailure({ kind: 'unknown', message: 'the executor is not on this device' }),
			maybe_sent: false
		};
	}
	const walletSigner =
		stored.keys && stored.keys.length > 1 ? keySetOf(stored) : stored.publicKeyHex;
	const keys = keySetOf(stored).keys;
	const calls = (op.calls as { to: string; value: string; data: string }[]).map((c) => ({
		to: c.to,
		value: c.value,
		data: c.data
	}));
	let asked = false;
	const signer: SignFn = async (challenge, operation) => {
		asked = true;
		const assertion = await signChallenge(
			challenge,
			{
				account: op.from,
				keys,
				credentials: keys.map((k) => ({ id: k.credentialId })),
				request: { ...executeRequest(op.purpose), origin: '', chainId: op.chain_id }
			},
			operation
		);
		const compat = verifySafeWebAuthn(assertion);
		if (!compat.ok) throw new Error(`passkey check: ${compat.reason ?? 'unverifiable'}`);
		return {
			signature: fromHex(assertion.signatureHex),
			authenticatorData: fromHex(assertion.authenticatorDataHex),
			clientDataJSON: fromHex(assertion.clientDataJSONHex),
			credentialId: assertion.credentialId
		};
	};
	signer.beforeSign = async ({ token, amount, value }) => {
		if (token !== null) return;
		const answer = await poolRpcCall('eth_getBalance', [op.from, 'latest'], op.chain_id).catch(
			() => null
		);
		const raw = answer?.result;
		if (typeof raw !== 'string' || !/^0x[0-9a-fA-F]+$/.test(raw)) return;
		const have = BigInt(raw);
		if (have < amount + value) {
			throw Object.assign(new Error('cannot pay'), {
				shell: { kind: 'cannot_pay', need: String(amount + value), have: String(have) }
			});
		}
	};
	const send = () =>
		sendBatchCalls(
			op.from,
			calls,
			op.chain_id,
			walletSigner,
			signer,
			undefined,
			null,
			feeOf(op.fee)
		);
	try {
		let result;
		try {
			result = await send();
		} catch (error) {
			if (asked || !unreachable(error)) throw error;
			await new Promise((r) => setTimeout(r, RELAY_RETRY_MS));
			result = await send();
		}
		// A lost answer after it may have left: followed by its hash, never sent again.
		if (result.maybeSent) {
			return { type: 'calls_failed', detail: result.userOpHash, maybe_sent: true };
		}
		return { type: 'calls_sent', user_op_hash: result.userOpHash, tx_hash: null };
	} catch (error) {
		const shell = (error as { shell?: ShellFailure }).shell;
		if (shell) return { type: 'calls_failed', detail: shellFailure(shell), maybe_sent: false };
		const failed = signingFailure(error);
		if (failed.type === 'passkey_cancelled') return { type: 'calls_cancelled' };
		if (error instanceof PoolFailedError && error.maybeDelivered) {
			return {
				type: 'calls_failed',
				detail: shellFailure({ kind: 'unknown', message: error.message }),
				maybe_sent: false
			};
		}
		return {
			type: 'calls_failed',
			detail: failed.type === 'passkey_failed' ? failed.detail : shellFailure(failureOf(error)),
			maybe_sent: false
		};
	}
}

/** One call through the signing sheet (`requestFirstPartyTransaction`). */
async function firstParty(op: Op<'send_calls'>): Promise<MultisigShellResult> {
	const call = (op.calls as { to: string; value: string; data: string }[])[0];
	if (!call) return { type: 'calls_failed', detail: 'nothing to send', maybe_sent: false };
	await signRequest.boot();
	const settled = await requestFirstPartyTransaction(op.from, {
		chain_id: op.chain_id,
		to: call.to,
		value: BigInt(call.value || '0').toString(),
		data: call.data
	});
	if (settled.kind === 'submitted') {
		return { type: 'calls_sent', user_op_hash: settled.hash, tx_hash: settled.hash };
	}
	if (settled.code === 4001) return { type: 'calls_cancelled' };
	return { type: 'calls_failed', detail: settled.message, maybe_sent: false };
}

async function quoteCalls(op: Op<'quote_calls'>): Promise<MultisigShellResult> {
	try {
		const quote = await quoteExecutionFee(
			op.chain_id,
			op.from,
			op.calls as { to: string; value: string; data: string }[]
		);
		return { type: 'quoted', quote: quoteJson(quote) };
	} catch (error) {
		return { type: 'quote_failed', detail: error instanceof Error ? error.message : String(error) };
	}
}

/** The injected wallet the connect page acts through, when one was picked. */
export interface InjectedWallet {
	request(args: { method: string; params?: unknown }): Promise<unknown>;
}

/**
 * The app's dependencies. `peer` and `wallet` belong to the screen that
 * opened them (a session's rooms, the connect page's wallet); `notice` to the
 * host that routes between machines.
 */
export function liveDeps(input: {
	peer: PeerTransport;
	wallet?: () => InjectedWallet | null;
	notice: MultisigShellDeps['notice'];
}): MultisigShellDeps {
	return {
		fetch: (url, init) => fetch(url, { ...init, cache: 'no-store' }),
		rpc: (chainId, method, params) =>
			poolRpcCall(method, (Array.isArray(params) ? params : [params]) as unknown[], chainId),
		storage: () => {
			try {
				return typeof localStorage === 'undefined' ? null : localStorage;
			} catch {
				return null;
			}
		},
		now: () => Date.now(),
		random: (n) => crypto.getRandomValues(new Uint8Array(n)),
		passkey,
		askRisk: (op) =>
			riskConfirmation.ask({ chainId: op.chain_id, safe: op.safe, findings: op.findings as never }),
		sendCalls,
		quoteCalls,
		simulate: (op) => rpcSimulateReply(op.from, op.calls as never, op.chain_id).catch(() => null),
		recipientFacts: async (op) => {
			const risk = await resolveRecipientRisk(op.chain_id, op.address);
			return { isContract: risk.isContract, firstTime: risk.firstInteraction };
		},
		userOpStatus: async (op) => {
			const answer = await pollUserOpStatus(op.user_op_hash, op.chain_id);
			return answer ? { status: answer.status, txHash: answer.txHash ?? null } : null;
		},
		peer: input.peer,
		wallet: async (method, params) => {
			const wallet = input.wallet?.() ?? null;
			if (!wallet) throw { code: 4900, message: 'no wallet connected' };
			return wallet.request({ method, params: params as never });
		},
		notice: input.notice
	};
}
