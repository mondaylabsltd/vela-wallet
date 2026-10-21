/**
 * One way in for the person's UI and an AI agent (spec 103).
 *
 * Both ask for the same typed {@link IntentAction}; who asks is the only
 * difference, and it matters only to authorization. A call goes:
 *
 * 1. **check** — the core's shared rules: may this asker ask for this, and is
 *    the input well formed (`intentCheck`, the same on every shell);
 * 2. **examine** — what the core cannot know: does what it names exist, is it
 *    already so (`rules.ts`, against the residents' views);
 * 3. **approve** — an agent's manage-level request waits for the person, in
 *    the wallet (the approval is on show in `approvals`);
 * 4. **perform** — the same resident calls the UI makes (`perform.ts`);
 * 5. **observe** — answered `completed` only once the change is SEEN in the
 *    residents' views; a step that ran is not yet a change that held.
 *
 * The answer is an {@link IntentOutcome}: `completed`, or `rejected` with a
 * code an agent can act on (ClientMCP SPEC §9.2).
 */
import type { IntentAction } from '$lib/core/generated/IntentAction';
import type { IntentActor } from '$lib/core/generated/IntentActor';
import type { IntentCheck } from '$lib/core/generated/IntentCheck';
import type { IntentOperation } from '$lib/core/generated/IntentOperation';
import type { IntentReason } from '$lib/core/generated/IntentReason';
import type { IntentReasonCode } from '$lib/core/generated/IntentReasonCode';
import type { IntentRisk } from '$lib/core/generated/IntentRisk';
import type { IntentScreen } from '$lib/core/generated/IntentScreen';
import { baselineOf, examine, shows, verdict, type Verdict } from './rules';
import type { World } from './world';

export type IntentOutcome =
	| { status: 'completed' }
	/** It started, and goes on as `operation` (a transfer, followed by its state). */
	| {
			status: 'accepted';
			operation: IntentOperation;
			/** The finished transfer whose receipt drafting this one closed. */
			closed?: string;
	  }
	| { status: 'rejected'; reason: IntentReason };

/** An agent's request, waiting for the person. */
export type PendingApproval = {
	id: number;
	action: IntentAction;
	risk: IntentRisk;
	since: number;
};

export type Stage = 'approving' | 'performing' | 'observing';

/** One of the agent's calls, for the person to see what it did. */
export type ActivityEntry = {
	id: number;
	action: IntentAction['type'];
	stage: Stage | null;
	outcome: IntentOutcome | null;
};

export type IntentsDeps = {
	/** Resolves once the core's rules can be asked. */
	ready(): Promise<void>;
	/** The core's shared check, for this asker, with the grants in `world`. */
	check(actor: IntentActor, action: IntentAction, world: World): IntentCheck;
	/** The residents' views, now. */
	world(): World;
	/**
	 * The same calls the UI makes. Resolves with `null` (done), why it was
	 * refused (words, or a reason with a code), or the operation it started.
	 */
	perform(
		action: IntentAction,
		world: World,
		nowMs: number,
		actor: IntentActor
	): Promise<null | string | IntentReason | { operation: IntentOperation; closed?: string | null }>;
	now(): number;
	/**
	 * Open `screen` for the person, as `show` does: where an agent's change is
	 * seen, once its request is done.
	 */
	reveal?(screen: IntentScreen): Promise<void>;
	/** How long an agent's request may wait for the person. */
	approvalMs: number;
	/** How long a performed change may take to show. */
	observeMs: number;
	pollMs: number;
};

/** How many of the agent's recent calls are kept on show. */
export const ACTIVITY_KEPT = 20;

type Answer = 'approved' | 'denied' | 'stopped' | 'timed_out' | 'cancelled';

/** How an agent's call was given up before it ran: nothing was done. */
const CANCELLED =
	'the agent’s call was cancelled (its connection or the bridge gave up on it) before anything was done';

const rejected = (code: IntentReasonCode, detail: string): IntentOutcome => ({
	status: 'rejected',
	reason: { code, detail }
});

export class Intents {
	/** Agent requests waiting for the person, oldest first. */
	approvals = $state<PendingApproval[]>([]);
	/** The agent's recent calls, newest first. */
	activity = $state<ActivityEntry[]>([]);

	#deps: IntentsDeps;
	#next = 1;
	// Not reactive on purpose: what is on show is `approvals`; this only
	// holds the way to settle each.
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- see above
	#answers = new Map<number, (answer: Answer) => void>();
	/** Bumped by `stop()`: an agent call from an older epoch stops at its next step. */
	#epoch = 0;

	constructor(deps: IntentsDeps) {
		this.#deps = deps;
	}

	/**
	 * Ask for an action. Never throws: every end is an outcome.
	 *
	 * `signal` is the caller giving up — an agent's call the bridge timed out
	 * or its connection lost. Before anything is done, that ends the call:
	 * an approval still waiting is withdrawn, and nothing is performed, so an
	 * agent told "timed out" never finds the change made after all. Once the
	 * step has begun it is not undone; the person sees how it ended.
	 */
	async invoke(
		actor: IntentActor,
		action: IntentAction,
		options: { signal?: AbortSignal } = {}
	): Promise<IntentOutcome> {
		const id = this.#next++;
		const epoch = this.#epoch;
		const agent = actor === 'agent';
		if (agent) this.#log({ id, action: action.type, stage: null, outcome: null });
		const outcome = await this.#run(id, actor, action, epoch, options.signal).catch(
			(error: unknown): IntentOutcome =>
				rejected('refused', error instanceof Error ? error.message : String(error))
		);
		if (agent) {
			this.#update(id, { stage: null, outcome });
			// What an agent did is seen where it is (owner, 2026-10-21: an
			// approved change that left the page as it was felt silent). A
			// `show` has opened its screen already.
			const done = outcome.status === 'completed' || outcome.status === 'accepted';
			const screen = done && action.type !== 'show' ? shows(action) : null;
			if (screen) await this.#deps.reveal?.(screen).catch(() => {});
		}
		return outcome;
	}

	/** The person answered an agent's request. */
	answer(id: number, approved: boolean): void {
		this.#answers.get(id)?.(approved ? 'approved' : 'denied');
	}

	/** The person stopped the agent: nothing of its still waits. */
	stop(): void {
		this.#epoch++;
		for (const settle of [...this.#answers.values()]) settle('stopped');
	}

	async #run(
		id: number,
		actor: IntentActor,
		action: IntentAction,
		epoch: number,
		signal: AbortSignal | undefined
	) {
		const deps = this.#deps;
		await deps.ready();
		const checked = deps.check(actor, action, deps.world());
		if (checked.decision.decision === 'deny') {
			return { status: 'rejected', reason: checked.decision.reason } satisfies IntentOutcome;
		}
		const first = examine(action, deps.world());
		if (first.kind === 'refused') return { status: 'rejected', reason: first.reason } as const;
		if (first.kind === 'already') return { status: 'completed' } as const;

		if (checked.decision.decision === 'approve' && checked.decision.by === 'wallet') {
			this.#update(id, { stage: 'approving' });
			const answer = await this.#ask(id, action, checked.risk ?? 'manage', signal);
			if (answer === 'cancelled') return rejected('stopped', CANCELLED);
			if (answer === 'denied') return rejected('denied', 'the person did not approve it');
			if (answer === 'timed_out')
				return rejected('approval_timed_out', 'nobody approved it in time');
			if (answer === 'stopped')
				return rejected('stopped', 'the person stopped the agent before approving');
			// An approval can take a while: the wallet may have changed under it.
			const again = examine(action, deps.world());
			if (again.kind === 'refused') return { status: 'rejected', reason: again.reason } as const;
			if (again.kind === 'already') return { status: 'completed' } as const;
		}
		if (actor === 'agent' && this.#epoch !== epoch)
			return rejected('stopped', 'the person stopped the agent');
		if (signal?.aborted) return rejected('stopped', CANCELLED);

		this.#update(id, { stage: 'performing' });
		const before = deps.world();
		const baseline = baselineOf(action, before);
		const done = await deps.perform(action, before, deps.now(), actor);
		if (typeof done === 'string') return rejected('refused', done);
		if (done !== null && 'operation' in done)
			return {
				status: 'accepted',
				operation: done.operation,
				...(done.closed ? { closed: done.closed } : {})
			} as const;
		if (done !== null) return { status: 'rejected', reason: done } as const;

		this.#update(id, { stage: 'observing' });
		// A sign-out ends on the session's own sheet: as long as an approval.
		const wait = action.type === 'sign_out' ? deps.approvalMs : deps.observeMs;
		const seen = await this.#observe(action, baseline, wait, actor === 'agent' ? epoch : null);
		if (seen.kind === 'held') return { status: 'completed' } as const;
		if (seen.kind === 'refused') return { status: 'rejected', reason: seen.reason } as const;
		if (actor === 'agent' && this.#epoch !== epoch)
			return rejected(
				'stopped',
				'the person stopped the agent; the step may already have taken effect'
			);
		return rejected('not_observed', 'the change was asked for, but did not show in time');
	}

	/** Watch the residents until the change holds, is refused, or time is up. */
	async #observe(
		action: IntentAction,
		baseline: ReturnType<typeof baselineOf>,
		ms: number,
		epoch: number | null
	): Promise<Verdict> {
		const deps = this.#deps;
		const deadline = deps.now() + ms;
		for (;;) {
			const seen = verdict(action, deps.world(), baseline);
			if (seen.kind !== 'not_yet') return seen;
			if (deps.now() >= deadline || (epoch !== null && this.#epoch !== epoch)) return seen;
			await new Promise((resolve) => setTimeout(resolve, deps.pollMs));
		}
	}

	#ask(
		id: number,
		action: IntentAction,
		risk: IntentRisk,
		signal: AbortSignal | undefined
	): Promise<Answer> {
		const deps = this.#deps;
		return new Promise<Answer>((resolve) => {
			if (signal?.aborted) return resolve('cancelled');
			const timer = setTimeout(() => settle('timed_out'), deps.approvalMs);
			const cancel = () => settle('cancelled');
			signal?.addEventListener('abort', cancel, { once: true });
			const settle = (answer: Answer) => {
				clearTimeout(timer);
				signal?.removeEventListener('abort', cancel);
				this.#answers.delete(id);
				this.approvals = this.approvals.filter((pending) => pending.id !== id);
				resolve(answer);
			};
			this.#answers.set(id, settle);
			this.approvals = [...this.approvals, { id, action, risk, since: deps.now() }];
		});
	}

	#log(entry: ActivityEntry): void {
		this.activity = [entry, ...this.activity].slice(0, ACTIVITY_KEPT);
	}

	#update(id: number, patch: Partial<ActivityEntry>): void {
		if (!this.activity.some((entry) => entry.id === id)) return;
		this.activity = this.activity.map((entry) =>
			entry.id === id ? { ...entry, ...patch } : entry
		);
	}
}
