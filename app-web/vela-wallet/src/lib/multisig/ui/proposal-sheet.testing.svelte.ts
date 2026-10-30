/**
 * Tests: the proposals machine as a sheet sees it — the parts of its view
 * an action moves (`actions`, `executions`, `outbox`) as state, so a sheet
 * redraws when an event it sent settles, as it does over the real machine.
 * The rest of the view a test reads from its own seams.
 */
import type { MultisigActionState } from '../generated/MultisigActionState';

export const machineSeam = $state({
	actions: {} as Record<string, MultisigActionState>,
	executions: {} as Record<string, unknown>,
	outbox: {} as Record<string, { why: string; retryAt: number | null }>,
	/** Bumped when the machine reads again (Retry): what it says may have moved. */
	reads: 0
});

/** Every event a sheet sent, in order (plain: a sheet sends from its effects). */
export const sentLog: { type: string; [key: string]: unknown }[] = [];

export function resetMachineSeam(): void {
	machineSeam.actions = {};
	machineSeam.executions = {};
	machineSeam.outbox = {};
	machineSeam.reads = 0;
	sentLog.length = 0;
}

/**
 * What an old typed error was, as the machine reports it (`ActionState`):
 * the failure a test sets up, said back through the real reading
 * (`shell/proposal-failure.ts`).
 */
export function stateOf(error: unknown): MultisigActionState {
	if (error instanceof DOMException) {
		return error.message === 'The person did not sign for the risk'
			? { kind: 'riskDeclined' }
			: { kind: 'cancelled' };
	}
	const e = error as {
		name?: string;
		constructor?: { name?: string };
		kind?: string;
		timedOut?: boolean;
		chainId?: number;
		findings?: unknown[];
		message?: string;
		why?: unknown;
		need?: bigint;
		have?: bigint;
	};
	const name = e?.name === 'Error' ? (e.constructor?.name ?? 'Error') : (e?.name ?? '');
	switch (name) {
		case 'PasskeyError':
			return {
				kind: 'failed',
				detail: JSON.stringify({ kind: 'passkey', failure: e.kind, timedOut: !!e.timedOut })
			};
		case 'SigningRefusedError':
			return { kind: 'signingRefused', findings: e.findings ?? [] };
		case 'SigningUncheckedError':
			return { kind: 'signingUnchecked' };
		case 'SigningBlockedError':
			return { kind: 'signingBlocked' };
		case 'MailboxUnreadableError':
			return { kind: 'mailboxUnreadable' };
		case 'ExecutorCannotPayError':
			return {
				kind: 'stopped',
				preview: {
					kind: 'priced',
					calls: [],
					fee: { amount: String(e.need ?? 1n), recipient: '0xfe' },
					balance: String(e.have ?? 0n),
					canPay: false,
					carries: [],
					executorApproves: false,
					setup: { multisig: false, members: [], executor: false, amount: null }
				}
			};
		case 'ExecutionWouldFailError':
			return { kind: 'stopped', preview: { kind: 'fails', why: e.why } };
		case 'ExecutionUnpricedError':
			return { kind: 'stopped', preview: { kind: 'unpriced' } };
		case 'ExecutionUncheckedError':
			return { kind: 'stopped', preview: { kind: 'unchecked' } };
		default:
			return { kind: 'failed', detail: e?.message ?? String(error) };
	}
}

/** A look as the machine keeps it (`previews`): amounts as decimal text. */
export function machineLook(look: unknown): unknown {
	if (!look) return undefined;
	const l = look as { kind: string; error?: { authority?: unknown } };
	if (l.kind === 'refused') return { kind: 'refused', authority: l.error?.authority ?? null };
	return JSON.parse(
		JSON.stringify(look, (_k, v) => (typeof v === 'bigint' ? v.toString() : v))
	) as unknown;
}

const known = (ms: number | undefined) => (typeof ms === 'number' && ms > 0 ? ms : null);
const same = (a: string | null | undefined, b: string | null | undefined) =>
	!!a && !!b && a.toLowerCase() === b.toLowerCase();

/**
 * An executed proposal's story as the machine tells it (`timelines`; Rust:
 * `created_each_approval_in_order_then_executed` and its neighbours in
 * app_multisig_presence.rs): created, each approval in order with whether
 * it counted, the executor's own step, then executed.
 */
export function machineTimeline(
	item: {
		proposer: string;
		createdMs?: number;
		approvals: { owner: string; counts: boolean; createdMs?: number }[];
	},
	txHash: string,
	frozen: {
		approvers: string[];
		executor: string | null;
		executorApproved: boolean;
		atMs: number | null;
	} | null
): unknown[] {
	const steps: unknown[] = [{ kind: 'created', who: item.proposer, at_ms: known(item.createdMs) }];
	const given = [...item.approvals]
		.filter((a) => a.counts || frozen?.approvers.some((o) => same(o, a.owner)))
		.sort((a, b) => (known(a.createdMs) ?? Infinity) - (known(b.createdMs) ?? Infinity));
	const approved = (who: string, at: number | null, counted: boolean | null, by: boolean) => ({
		kind: 'approved',
		who,
		at_ms: at,
		counted,
		by_executing: by,
		unsent: false,
		on_chain: false
	});
	for (const a of given) {
		const counted = frozen ? frozen.approvers.some((o) => same(o, a.owner)) : null;
		steps.push(approved(a.owner, known(a.createdMs), counted, false));
	}
	for (const owner of frozen?.approvers ?? []) {
		if (given.some((a) => same(a.owner, owner))) continue;
		const executor = !!frozen?.executor && same(frozen.executor, owner);
		steps.push(
			approved(
				owner,
				executor ? (frozen?.atMs ?? null) : null,
				true,
				executor && !!frozen?.executorApproved
			)
		);
	}
	steps.push({
		kind: 'executed',
		who: frozen?.executor ?? null,
		tx_hash: txHash,
		at_ms: frozen?.atMs ?? null
	});
	return steps;
}
