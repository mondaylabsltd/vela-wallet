/**
 * The web's ONE shell for the core's multisig machines (`vela_core::app::multisig_*`):
 * every `MultisigOperation` any of the five machines asks — accounts,
 * proposals, members, the connect page and Vela's live session — is answered
 * here, by exactly one `MultisigShellResult`.
 *
 * This file draws nothing and decides nothing. It performs I/O and reports
 * what happened, in the vocabulary's own words: an HTTP answer as it came, a
 * JSON-RPC reply as the node gave it, a store key as it was. What to ask,
 * when, how often and what the answer means is the core's (the same core the
 * desktop, Android and iOS drive). The product's own pipelines — the passkey
 * and its signing page, the risk question, the send pipeline, simulation,
 * recipient facts, the relay's word on a user operation, the PeerConnect
 * transport, the injected wallet, and where a notice goes — come in as
 * {@link MultisigShellDeps}, so a test answers them from a simulated world and
 * the app from `live.ts`.
 *
 * The contract the shared effect loop holds every shell to: `execute` never
 * rejects for an expected failure. A dependency that throws is answered with
 * that operation's own failure ({@link operationFailure}).
 */
import { isProductionWriteRefused } from '$lib/services/production-guard';
import type { MultisigOperation } from '../generated/MultisigOperation';
import type { MultisigShellResult } from '../generated/MultisigShellResult';
import type { MultisigNotice } from '../generated/MultisigNotice';
import type { MultisigPasskeyPurpose } from '../generated/MultisigPasskeyPurpose';

/** One operation as the effect loop hands it over. */
export interface MultisigEffect {
	id: number;
	operation: MultisigOperation;
}

type Op<T extends MultisigOperation['type']> = Extract<MultisigOperation, { type: T }>;

/** A JSON-RPC reply through the wallet's own pool: its `result`, or the node's `error`. */
export interface RpcReply {
	result?: unknown;
	error?: { code?: number | null; message?: string; data?: unknown } | null;
}

/** The transport a PeerConnect room runs on (`peer.ts` binds the vendored client). */
export interface PeerTransport {
	/** Create (`invite` and `pairing` absent), join (`invite`) or reopen (`pairing`) a room. */
	open(input: {
		relay: string | null;
		invite: string | null;
		pairing: string | null;
	}): Promise<{ room: string; invite: string | null; pairing: string | null }>;
	send(room: string, message: unknown): Promise<void>;
	/** The next message, or the room's close (its reason). */
	receive(room: string, signal: AbortSignal): Promise<{ message: unknown } | { closed: string }>;
	close(room: string, cancel: boolean, end: boolean): Promise<void>;
}

/**
 * The product pipelines a multisig operation reaches. Every one may throw;
 * the handler answers a throw with the operation's failure.
 */
export interface MultisigShellDeps {
	fetch: typeof fetch;
	/**
	 * One call through the RPC pool. Throws when no node answered. `person`:
	 * the person's own ask (the core's `rpc.person`, r17-10) — always sent.
	 */
	rpc(chainId: number, method: string, params: unknown, person?: boolean): Promise<RpcReply>;
	/**
	 * R22 (review 1-2): one CORROBORATED read through the pool — two
	 * independent nodes, or the person's own alone — as the core's envelope
	 * (`rpcCorroboratedEnvelope`, a JSON object): `{result | error, witness?,
	 * ownNode?}`. Throws when no node answered (its words the `detail`).
	 */
	rpcCorroborated(
		chainId: number,
		method: string,
		params: unknown,
		person?: boolean
	): Promise<unknown>;
	/** `localStorage`, or `null` where there is none (it answers `store_failed`). */
	storage(): Pick<Storage, 'getItem' | 'setItem' | 'removeItem'> | null;
	now(): number;
	random(bytes: number): Uint8Array;
	/** The member's passkey, in Vela or on its trusted page: answered in the vocabulary. */
	passkey(op: Op<'passkey'>, signal: AbortSignal): Promise<MultisigShellResult>;
	/** The high-risk question (`RiskConfirmSheet`): `false` when nobody can be asked. */
	askRisk(op: Op<'ask_risk'>): Promise<boolean>;
	/** The send pipeline: answered `calls_sent` / `calls_failed` / `calls_cancelled`. */
	sendCalls(op: Op<'send_calls'>, signal: AbortSignal): Promise<MultisigShellResult>;
	/** The fee pipeline: answered `quoted` / `quote_failed`. */
	quoteCalls(op: Op<'quote_calls'>): Promise<MultisigShellResult>;
	/**
	 * The signing sheet's simulation envelope (`rpcSimulateReply`), or `null`.
	 * R22: always corroborated — the core's envelope with the witness's answer
	 * (or `ownNode`), as a JSON string.
	 */
	simulate(op: Op<'simulate'>): Promise<string | null>;
	/** What the send flow knows of a recipient (`resolveRecipientRisk`). */
	recipientFacts(
		op: Op<'recipient_risk'>
	): Promise<{ isContract: boolean | null; firstTime: boolean | null }>;
	/** The relay's word on a user operation (`pollUserOpStatus`), or `null`. */
	userOpStatus(
		op: Op<'user_op_status'>
	): Promise<{ status: string | null; txHash: string | null } | null>;
	peer: PeerTransport;
	/** One EIP-1193 request to the injected wallet; throws `{ code, message }` on a refusal. */
	wallet(method: string, params: unknown): Promise<unknown>;
	/** Route a notice (another machine, the activity, a screen). */
	notice(notice: MultisigNotice): void | Promise<void>;
}

/** An HTTP request's `Retry-After`, as the core reads it (the header's raw text). */
function retryAfter(response: Response): string | null {
	return response.headers.get('retry-after');
}

function errorText(error: unknown): string {
	const text = error instanceof Error ? `${error.name}: ${error.message}` : String(error);
	// r14-5: a pool that gave up because its nodes answered only "slow down"
	// (`PoolFailedError.rateLimited`) says so first, in the core's word
	// (`multisig_shell::RATE_LIMITED_DETAIL`): the core then names the
	// network busy, not out of reach.
	const limited =
		typeof error === 'object' &&
		error !== null &&
		(error as { rateLimited?: unknown }).rateLimited === true;
	return limited ? `rate-limited: ${text}` : text;
}

/** The wallet's refusal as EIP-1193 gives it: `{ code, message }`. */
function walletRefusal(error: unknown): Extract<MultisigShellResult, { type: 'wallet_refused' }> {
	const e = (error ?? {}) as { code?: unknown; message?: unknown };
	const code = typeof e.code === 'number' && Number.isFinite(e.code) ? BigInt(e.code) : null;
	return {
		type: 'wallet_refused',
		code,
		message: typeof e.message === 'string' ? e.message : String(error)
	};
}

function hex(bytes: Uint8Array): string {
	let out = '0x';
	for (const b of bytes) out += b.toString(16).padStart(2, '0');
	return out;
}

/** Resolves after `ms`, or at once when `signal` aborts. */
function sleep(ms: number, signal: AbortSignal): Promise<void> {
	return new Promise((resolve) => {
		if (signal.aborted) return resolve();
		const timer = setTimeout(done, ms);
		function done() {
			clearTimeout(timer);
			signal.removeEventListener('abort', done);
			resolve();
		}
		signal.addEventListener('abort', done, { once: true });
	});
}

/**
 * `promise`, or `null` past `ms` (the web's `bounded`): a read that never
 * answers never holds the machine.
 */
async function within<T>(promise: Promise<T>, ms: number | null): Promise<T | null> {
	if (ms === null) return promise;
	let timer: ReturnType<typeof setTimeout> | undefined;
	const late = new Promise<null>((resolve) => {
		timer = setTimeout(() => resolve(null), ms);
	});
	try {
		return await Promise.race([promise, late]);
	} finally {
		clearTimeout(timer);
	}
}

/**
 * The failure each operation answers when its dependency threw — the result
 * the machine waiting on it reads as "it did not happen", never a hang.
 */
export function operationFailure(op: MultisigOperation, error: unknown): MultisigShellResult {
	const detail = errorText(error);
	switch (op.type) {
		case 'http':
			// r18-8: a refusal by this test build is its own kind, here too.
			return isProductionWriteRefused(error)
				? { type: 'http_unreachable', detail, refused: true }
				: { type: 'http_unreachable', detail };
		case 'rpc':
			return { type: 'rpc_unanswered', detail };
		case 'load_store':
		case 'write_store':
			return { type: 'store_failed', detail };
		case 'wait':
			return { type: 'waited' };
		case 'now':
			return { type: 'now', now_ms: Date.now() };
		case 'random':
			return { type: 'random', hex: '0x' };
		case 'passkey':
			return { type: 'passkey_failed', detail };
		case 'ask_risk':
			return { type: 'risk_answered', signs: false };
		case 'send_calls':
			// A throw nothing classified: the pipeline's own errors are answered
			// by `sendCalls` itself; this one never reached the relay's answer.
			return { type: 'calls_failed', detail, maybe_sent: false };
		case 'quote_calls':
			return { type: 'quote_failed', detail };
		case 'simulate':
			return { type: 'simulated', reply: null };
		case 'recipient_risk':
			return { type: 'recipient_facts', is_contract: null, first_time: null };
		case 'user_op_status':
			return { type: 'user_op_status', status: null, tx_hash: null };
		case 'peer_open':
		case 'peer_receive':
			return { type: 'peer_closed', reason: detail };
		case 'peer_send':
		case 'peer_close':
		case 'notice':
			return { type: 'ack' };
		case 'wallet_request':
			return walletRefusal(error);
	}
}

/**
 * What a passkey is for, as the request a trusted page and the extension are
 * shown. An owner proof's params are the core's (§16: the genesis, the
 * member, the networks — `trusted_signer::owner_proof::params`), handed over
 * verbatim; the page reads everything it shows from them.
 */
export function passkeyRequest(purpose: MultisigPasskeyPurpose): {
	method: string;
	params: unknown;
} {
	switch (purpose.kind) {
		case 'approve':
			return { method: 'vela_multisigApprove', params: { safeTxHash: purpose.safe_tx_hash } };
		case 'record':
			return { method: 'vela_multisigCreate', params: purpose.params ?? null };
	}
}

/** The one executor every multisig machine's effects go through. */
export function multisigShell(deps: MultisigShellDeps) {
	async function execute(op: MultisigOperation, signal: AbortSignal): Promise<MultisigShellResult> {
		switch (op.type) {
			case 'http': {
				const controller = new AbortController();
				const stop = () => controller.abort();
				signal.addEventListener('abort', stop, { once: true });
				const timer = setTimeout(stop, op.timeout_ms);
				try {
					const response = await deps.fetch(op.url, {
						method: op.method,
						headers: op.body === null ? undefined : { 'content-type': 'application/json' },
						body: op.body ?? undefined,
						signal: controller.signal
					});
					const body = await response.text();
					return {
						type: 'http_answered',
						status: response.status,
						retry_after: retryAfter(response),
						body
					};
				} catch (error) {
					// r18-8 (§28, §32): this test build refused the write before it
					// left — its own kind (asked once, never an outage).
					return isProductionWriteRefused(error)
						? { type: 'http_unreachable', detail: errorText(error), refused: true }
						: { type: 'http_unreachable', detail: errorText(error) };
				} finally {
					clearTimeout(timer);
					signal.removeEventListener('abort', stop);
				}
			}
			case 'rpc': {
				if (op.corroborate === true) {
					// R22: a guard's read — both nodes' answers in the core's
					// envelope, compared by the core. No node answered (or not in
					// time): unanswered, with the pool's words (a rate limit says so).
					let envelope: unknown;
					try {
						envelope = await within(
							deps.rpcCorroborated(op.chain_id, op.method, op.params, op.person ?? false),
							op.bound_ms
						);
					} catch (error) {
						return { type: 'rpc_unanswered', detail: errorText(error) };
					}
					if (envelope === null)
						return { type: 'rpc_unanswered', detail: `no answer in ${op.bound_ms} ms` };
					return { type: 'rpc_corroborated', envelope };
				}
				let reply: RpcReply | null;
				try {
					reply = await within(
						deps.rpc(op.chain_id, op.method, op.params, op.person ?? false),
						op.bound_ms
					);
				} catch (error) {
					return { type: 'rpc_unanswered', detail: errorText(error) };
				}
				if (reply === null)
					return { type: 'rpc_unanswered', detail: `no answer in ${op.bound_ms} ms` };
				if (reply.error) {
					const code = reply.error.code;
					// r8-4: the node's `data` is kept — a revert's payload is where the
					// state-diff probe answers (simulateAndRevert always reverts).
					const data = reply.error.data;
					return {
						type: 'rpc_error',
						code: typeof code === 'number' && Number.isFinite(code) ? BigInt(code) : null,
						message: reply.error.message ?? '',
						...(data === undefined || data === null ? {} : { data })
					};
				}
				return { type: 'rpc_result', result: reply.result ?? null };
			}
			case 'load_store': {
				const store = deps.storage();
				if (!store) return { type: 'store_failed', detail: 'no storage' };
				const entries: Record<string, string | null> = {};
				for (const key of op.keys) entries[key] = store.getItem(key);
				return { type: 'store_loaded', entries };
			}
			case 'write_store': {
				const store = deps.storage();
				if (!store) return { type: 'store_failed', detail: 'no storage' };
				if (op.value === null) store.removeItem(op.key);
				else store.setItem(op.key, op.value);
				return { type: 'store_written' };
			}
			case 'wait':
				await sleep(op.ms, signal);
				return { type: 'waited' };
			case 'now':
				return { type: 'now', now_ms: deps.now() };
			case 'random':
				return { type: 'random', hex: hex(deps.random(op.bytes)) };
			case 'passkey':
				return deps.passkey(op, signal);
			case 'ask_risk':
				return { type: 'risk_answered', signs: await deps.askRisk(op) };
			case 'send_calls':
				return deps.sendCalls(op, signal);
			case 'quote_calls':
				return deps.quoteCalls(op);
			case 'simulate':
				return { type: 'simulated', reply: await deps.simulate(op) };
			case 'recipient_risk': {
				const facts = await deps.recipientFacts(op);
				return {
					type: 'recipient_facts',
					is_contract: facts.isContract,
					first_time: facts.firstTime
				};
			}
			case 'user_op_status': {
				const answer = await deps.userOpStatus(op);
				return {
					type: 'user_op_status',
					status: answer?.status ?? null,
					tx_hash: answer?.txHash ?? null
				};
			}
			case 'peer_open': {
				const opened = await deps.peer.open({
					relay: op.relay,
					invite: op.invite,
					pairing: op.pairing
				});
				return { type: 'peer_opened', ...opened };
			}
			case 'peer_send':
				await deps.peer.send(op.room, op.message);
				return { type: 'ack' };
			case 'peer_receive': {
				const next = await deps.peer.receive(op.room, signal);
				return 'closed' in next
					? { type: 'peer_closed', reason: next.closed }
					: { type: 'peer_message', message: next.message };
			}
			case 'peer_close':
				await deps.peer.close(op.room, op.cancel, op.end);
				return { type: 'ack' };
			case 'wallet_request':
				try {
					return { type: 'wallet_answered', result: await deps.wallet(op.method, op.params) };
				} catch (error) {
					return walletRefusal(error);
				}
			case 'notice':
				await deps.notice(op.notice);
				return { type: 'ack' };
		}
	}

	return {
		/** The effect loop's `execute`. */
		execute: (effect: MultisigEffect, signal: AbortSignal) => execute(effect.operation, signal),
		/** The effect loop's `toFailure`. */
		toFailure: (effect: MultisigEffect, error: unknown) => operationFailure(effect.operation, error)
	};
}

export type MultisigShell = ReturnType<typeof multisigShell>;
