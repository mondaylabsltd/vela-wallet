/**
 * The proposals machine's word on an action (`MultisigActionState`) and on an
 * execution's look (its `previews` entry), read back as the typed errors and
 * the preview the screens have always said them by — so every sheet keeps
 * its corpus words exactly: a passkey closed, timed out or failing its check,
 * a page that did not sign, the mailbox down or refusing, a network read-only
 * or not checked, an execution that would fail, cannot be priced or paid.
 *
 * Nothing is decided here: the machine decided; the shell's own failures
 * come as its `ShellFailure` detail JSON (`live.ts`), read with
 * {@link readShellFailure}. A detail the shell did not write is the core's
 * own reason — said as "something went wrong", never raw.
 */
import { PasskeyError } from '$lib/onboarding/core/passkey';
import type { FailureKind } from '$lib/onboarding/generated/FailureKind';
import {
	PageDeclinedError,
	PageSignFailedError,
	VenueBlockedError
} from '$lib/signing/sign-challenge';
import { PageApprovalError } from '../core/page-approval.svelte';
import {
	riskDeclinedError,
	SigningBlockedError,
	SigningRefusedError,
	SigningUncheckedError
} from '../core/gate.svelte';
import {
	ExecutionSpentError,
	ExecutionUncheckedError,
	ExecutionUnpricedError,
	ExecutionWouldFailError,
	ExecutorCannotPayError,
	type ExecutionBlock,
	type ExecutionPreview
} from '../core/execution';
import { GenesisRefusedError, type Finding } from '../core/refusal';
import { PasskeyCheckError } from '../core/owners';
import {
	MailboxRefusedError,
	MailboxUnreadableError,
	NoOutsideSignerError
} from '../core/proposal-errors';
import type { MultisigActionState } from '../generated/MultisigActionState';
import { readShellFailure } from './failure';

/** A token's word for an amount (`tokenInfo`), when it is known. */
export interface TokenWords {
	symbol: string;
	decimals: number;
}

const big = (v: unknown): bigint => {
	try {
		return BigInt(String(v ?? '0'));
	} catch {
		return 0n;
	}
};

/** Why it would fail, as the sheet words it (`whyText`). */
export function blockOf(why: unknown, token: TokenWords | null = null): ExecutionBlock {
	const w = (why ?? {}) as { kind?: string; have?: unknown; need?: unknown; reason?: unknown };
	if (w.kind === 'short') {
		return {
			kind: 'short',
			symbol: token?.symbol ?? '',
			decimals: token?.decimals ?? 18,
			have: big(w.have),
			need: big(w.need)
		};
	}
	return { kind: 'reverts', reason: typeof w.reason === 'string' ? w.reason : null };
}

/** The token a `short` look is about (`null`: the chain's coin), to word its amounts. */
export function shortToken(value: unknown): string | null | undefined {
	const v = value as { kind?: string; why?: { kind?: string; token?: string | null } } | null;
	return v?.kind === 'fails' && v.why?.kind === 'short' ? (v.why.token ?? null) : undefined;
}

/** One executor's look (`previews[<chain>:<hash>:<executor>]`) as the sheet draws it. */
export function previewOf(
	value: unknown,
	chainId: number,
	token: TokenWords | null = null
): ExecutionPreview | null {
	const v = value as Record<string, unknown> | null | undefined;
	if (!v || typeof v.kind !== 'string') return null;
	switch (v.kind) {
		case 'refused':
			return {
				kind: 'refused',
				error: new GenesisRefusedError(chainId, v.authority as never)
			};
		case 'fails':
			return { kind: 'fails', why: blockOf(v.why, token) };
		case 'unpriced':
			return { kind: 'unpriced' };
		case 'unchecked':
			return { kind: 'unchecked' };
		case 'spent':
			return { kind: 'spent' };
		case 'priced': {
			const fee = (v.fee ?? {}) as Record<string, unknown>;
			const setup = (v.setup ?? {}) as Record<string, unknown>;
			return {
				kind: 'priced',
				calls: (v.calls ?? []) as never,
				fee: { ...fee, amount: big(fee.amount) } as never,
				balance: v.balance === null || v.balance === undefined ? null : big(v.balance),
				canPay: v.canPay !== false,
				carries: (v.carries ?? []) as string[],
				executorApproves: v.executorApproves === true,
				setup: {
					multisig: setup.multisig === true,
					members: (setup.members ?? []) as string[],
					executor: setup.executor === true,
					amount: setup.amount === null || setup.amount === undefined ? null : big(setup.amount)
				}
			} as ExecutionPreview;
		}
	}
	return null;
}

/** What stopped a look before its passkey, as the typed error it was. */
export function stopError(preview: ExecutionPreview): Error | null {
	switch (preview.kind) {
		case 'refused':
			return preview.error;
		case 'fails':
			return new ExecutionWouldFailError(preview.why);
		case 'unpriced':
			return new ExecutionUnpricedError();
		case 'unchecked':
			return new ExecutionUncheckedError();
		case 'spent':
			return new ExecutionSpentError();
		case 'priced':
			return preview.canPay || preview.balance === null
				? null
				: new ExecutorCannotPayError(preview.fee.amount, preview.balance);
	}
}

const PAGE_FAILURE: Record<string, string> = {
	unanswered: 'silent',
	mismatch: 'unverified',
	not_opened: 'not_opened',
	refused: 'declined'
};

/** A `failed` action's detail: the shell's own failure, else the core's reason. */
export function detailError(detail: string): Error {
	const shell = readShellFailure(detail);
	if (!shell) {
		// The core's own words for a passkey whose signature did not check out.
		if (/did not check out|does not check out/.test(detail)) return new PasskeyCheckError(detail);
		return new Error(detail);
	}
	switch (shell.kind) {
		case 'venue_blocked':
			return new VenueBlockedError(shell.block as never);
		case 'page_declined':
			return new PageDeclinedError();
		case 'page_sign_failed':
			return new PageSignFailedError(PAGE_FAILURE[shell.page] ?? 'declined');
		case 'page_approval':
			return new PageApprovalError(
				shell.failure as never,
				(shell.line ?? null) as never,
				shell.code ?? ''
			);
		case 'passkey':
			return new PasskeyError(shell.failure as FailureKind, shell.failure, shell.timedOut);
		case 'passkey_check':
			return new PasskeyCheckError(shell.detail);
		case 'not_sent':
			// The relay provably never took it (the send pipeline's `UserOpNotSentError`).
			return Object.assign(new Error('the relay never took it'), { name: 'UserOpNotSentError' });
		case 'cannot_pay':
			return new ExecutorCannotPayError(big(shell.need), big(shell.have));
		case 'unknown':
			return new Error(shell.message);
	}
}

/**
 * The core refused to build it: a member change's refusal (the core's JSON:
 * `memberRefusal` reads it), else its reason.
 */
function refusedError(reason: string): Error {
	return Object.assign(new Error(reason), { refusal: memberRefusal(reason) });
}

/** A member change's refusal the core wrote as JSON in a `refused` reason, else `null`. */
export function memberRefusal(reason: string): { kind: string } | null {
	if (!reason.startsWith('{')) return null;
	try {
		const refusal = JSON.parse(reason) as { kind?: unknown };
		return typeof refusal.kind === 'string' ? (refusal as { kind: string }) : null;
	} catch {
		return null;
	}
}

/**
 * Where one action stands, as the error the screens word: `null` while it
 * works or once it is done. `chainId`: the network it was on.
 */
export function actionError(
	state: MultisigActionState | null | undefined,
	chainId: number,
	token: TokenWords | null = null
): unknown {
	if (!state) return null;
	switch (state.kind) {
		case 'working':
		case 'done':
		case 'outsideAsked':
			return null;
		case 'cancelled':
			return new DOMException('the passkey prompt was closed', 'AbortError');
		case 'riskDeclined':
			return riskDeclinedError();
		case 'mailboxUnreadable':
			return new MailboxUnreadableError();
		case 'noOutsideSigner':
		case 'noMemberHere':
			return new NoOutsideSignerError();
		case 'signingRefused':
			return new SigningRefusedError(chainId, state.findings as Finding[]);
		case 'signingBlocked':
			return new SigningBlockedError(chainId, null as never);
		case 'signingUnchecked':
			return new SigningUncheckedError(chainId);
		case 'refused':
			return refusedError(state.reason);
		case 'mailboxRefused':
			return new MailboxRefusedError(state.refusal);
		case 'stopped': {
			const preview = previewOf(state.preview, chainId, token);
			return (preview && stopError(preview)) ?? new ExecutionUnpricedError();
		}
		case 'failed':
			return detailError(state.detail);
	}
}

/** Whether the action is still under way. */
export const working = (state: MultisigActionState | null | undefined): boolean =>
	state?.kind === 'working';
