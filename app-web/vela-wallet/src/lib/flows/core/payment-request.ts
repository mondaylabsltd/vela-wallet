/**
 * The `payment_request` machine, wired for the web (spec 025 Phase 4).
 *
 * Ported from the payment_request arms of
 * src/services/wallet-state-core/executors.ts + validate-pay.ts @ c13e89d4.
 * Two shapes of use, both the core's:
 *
 * - `validatePayQuery` — the `/pay` grammar, synchronously: `link_opened`
 *   requests no shell operations, so a core is constructed, dispatched once,
 *   read, freed. The strict parse lives in Rust
 *   (`payment_request.rs::validate_pay_query`).
 * - the session — the receive-request flow's acknowledgement gate: the
 *   per-account "I understand" flag under `vela.receiveWarned.<account>`,
 *   read/written through the KV. An unreadable flag SHOWS the gate.
 */

import { loadCore, PaymentRequestCore } from '$lib/core/client';
import type { EffectLoop } from '$lib/core/effect-loop';
import { createJsonWasmShell } from '$lib/core/json-shell';
import type { PayRefusal } from '$lib/core/generated/PayRefusal';
import type { PayRequest } from '$lib/core/generated/PayRequest';
import type { PaymentRequestEvent } from '$lib/core/generated/PaymentRequestEvent';
import type { PaymentRequestOperation } from '$lib/core/generated/PaymentRequestOperation';
import type { PaymentRequestShellResult } from '$lib/core/generated/PaymentRequestShellResult';
import type { PaymentRequestView } from '$lib/core/generated/PaymentRequestView';
import type { SessionOptions } from '$lib/core/types';
import { getItem, setItem } from '$lib/services/storage';

export type PaymentRequestEffect = { id: number; operation: PaymentRequestOperation };

/** The raw, untrusted `/pay` query — input to the synchronous validator. */
export interface RawPayQuery {
	to?: string;
	chain?: string;
	token?: string;
	amount?: string;
	sym?: string;
	dec?: string;
	net?: string;
}

/** Same per-account key the Expo Receive screen owns. */
const warnedStorageKey = (account: string) => `vela.receiveWarned.${account}`;

export async function executePaymentRequestOperation(
	effect: PaymentRequestEffect
): Promise<PaymentRequestShellResult> {
	const operation = effect.operation;
	switch (operation.type) {
		case 'read_ack': {
			const value = await getItem(warnedStorageKey(operation.account));
			return { type: 'ack_flag', acknowledged: value === '1' };
		}
		case 'write_ack':
			await setItem(warnedStorageKey(operation.account), '1');
			return { type: 'ack_written' };
		default: {
			const never: never = operation;
			throw new Error(`unhandled payment_request operation: ${JSON.stringify(never)}`);
		}
	}
}

export function paymentRequestOperationFailure(
	effect: PaymentRequestEffect
): PaymentRequestShellResult {
	const operation = effect.operation;
	switch (operation.type) {
		case 'read_ack':
			// An unreadable flag shows the gate — never skips the warning.
			return { type: 'ack_flag', acknowledged: false };
		case 'write_ack':
			return { type: 'ack_written' };
		default: {
			const never: never = operation;
			throw new Error(`unhandled payment_request operation: ${JSON.stringify(never)}`);
		}
	}
}

export type PaymentRequestSession = EffectLoop<PaymentRequestEvent>;

export function createPaymentRequestSession(
	options: SessionOptions<PaymentRequestView>
): PaymentRequestSession {
	return createJsonWasmShell<
		PaymentRequestView,
		PaymentRequestEvent,
		PaymentRequestEffect,
		PaymentRequestShellResult
	>(new PaymentRequestCore(), {
		onView: options.onView,
		execute: executePaymentRequestOperation,
		toFailure: paymentRequestOperationFailure,
		onError: options.onError
	});
}

/** The core's ruling on a `/pay` link: the request, or why it was refused. */
export type PayVerdict =
	{ valid: true; pay: PayRequest } | { valid: false; refusal: PayRefusal | null };

/**
 * The `/pay` grammar, judged by the core. A refusal carries the core's reason
 * (`pay_refusal`); the shell words it from the corpus. Loads the core if it
 * is not aboard yet.
 */
export async function validatePayQuery(query: RawPayQuery): Promise<PayVerdict> {
	await loadCore();
	const core = new PaymentRequestCore();
	try {
		const result = JSON.parse(
			core.dispatch(
				JSON.stringify({
					type: 'link_opened',
					to: query.to ?? null,
					chain: query.chain ?? null,
					token: query.token ?? null,
					amount: query.amount ?? null,
					sym: query.sym ?? null,
					dec: query.dec ?? null,
					net: query.net ?? null
				})
			)
		) as { view: PaymentRequestView };
		const { view } = result;
		return view.pay_valid === true && view.pay !== null
			? { valid: true, pay: view.pay }
			: { valid: false, refusal: view.pay_refusal };
	} finally {
		core.free();
	}
}
