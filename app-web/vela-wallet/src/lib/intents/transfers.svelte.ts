/**
 * Transfers through the intents (spec 103): a draft lands on the wallet's
 * own review — the screen a person reaches by filling the form — and its
 * operation is followed from there to the chain.
 *
 * Nothing about sending is decided here. Drafting is the send core's own
 * events, sent in the order the form's controls send them (`sendDesk`); where
 * an operation stands is READ — from the send while it is open, from the
 * passkey's ceremony, and from the chain tracker once it has a hash.
 */
import { goto } from '$app/navigation';
import { resolve } from '$app/paths';
import { agentSettings } from '$lib/agent/settings.svelte';
import type { IntentActor } from '$lib/core/generated/IntentActor';
import type { IntentProgress } from '$lib/core/generated/IntentProgress';
import { intentOperationProgress } from '$lib/core/client';
import type { IntentOperation } from '$lib/core/generated/IntentOperation';
import type { IntentOperationState } from '$lib/core/generated/IntentOperationState';
import type { IntentReason } from '$lib/core/generated/IntentReason';
import type { SendView } from '$lib/core/generated/SendView';
import { sendDesk, type SendDeskPort } from '$lib/flows/send-desk.svelte';
import { localeOfPath } from '$lib/i18n/locales';
import { onSignCeremony, type SignCeremonyEvent } from '$lib/onboarding/core/passkey';
import { networkId } from '$lib/services/networks';
import { session } from '$lib/session/core/session.svelte';
import { balance } from '$lib/wallet/core/balance.svelte';
import { txTrackerView } from '$lib/wallet/core/tracker-resident';

export type Draft = { chain_id: number; token: string | null; to: string; amount: string };
export type BatchDraft = {
	chain_id: number;
	token: string | null;
	recipients: { to: string; amount: string }[];
};

/** `.5` as the form writes it: `0.5`. */
const decimal = (amount: string) =>
	amount.trim().startsWith('.') ? `0${amount.trim()}` : amount.trim();

const FINAL: readonly IntentOperationState[] = ['confirmed', 'failed', 'cancelled'];

const same = (a: string, b: string) => a.trim().toLowerCase() === b.trim().toLowerCase();

const reason = (code: IntentReason['code'], detail: string): IntentReason => ({ code, detail });

type Tracked = IntentOperation & {
	/** The send it was drafted on (`SendDeskPort.session`). */
	session: number;
	/**
	 * How many passkey ceremonies had begun when confirm was pressed for it: a
	 * later one is its passkey. A count, not a clock — an earlier transfer's
	 * ceremony in the same millisecond is still an earlier one.
	 */
	confirmedAfter: number | null;
	/** The finished transfer whose receipt this draft closed (its Done). */
	closed: string | null;
	/** Who put it on the review: an agent's draft, or the person's own form. */
	draftedBy: IntentActor;
	/** When its review came on screen (epoch ms). */
	reviewedAt: number;
};

/**
 * How long a review an agent drafted is on screen before the person's
 * confirm counts. An agent can cancel a review and draft another without
 * asking (`cancel_transfer` is operate-level); without this, a tap meant for
 * the review the person was reading could land on its replacement — and the
 * passkey prompt after it shows no transaction.
 */
export const REVIEW_SETTLE_MS = 1_500;

/** How long each step of a draft may take before it is given up. */
const STEP_MS = { page: 8_000, open: 10_000, amount: 15_000, review: 20_000 } as const;

async function until<T>(read: () => T | null | undefined, ms: number): Promise<T | null> {
	const deadline = Date.now() + ms;
	for (;;) {
		const value = read();
		if (value !== null && value !== undefined) return value;
		if (Date.now() >= deadline) return null;
		await new Promise((done) => setTimeout(done, 50));
	}
}

/** A send's refusal or warning, by its own name. */
function named(value: { type: string } | null): string | null {
	return value === null ? null : value.type;
}

class Transfers {
	/** Every operation drafted or confirmed this visit, newest first. */
	operations = $state<Tracked[]>([]);
	#next = 1;
	#ceremony: { event: SignCeremonyEvent; seq: number } | null = null;
	#ceremonies = 0;
	#listening = false;
	/**
	 * The clock, and whether the agent was stopped or switched off since its
	 * draft began (what it was drafting is then given up). The tests' own.
	 */
	clock = { now: () => Date.now(), halted: () => !agentSettings.enabled };

	/** The operations as an agent reads them, each brought up to date. */
	list(): IntentOperation[] {
		this.sync();
		return this.operations.map((operation) => this.#plain(operation));
	}

	get(id: string): IntentOperation | null {
		this.sync();
		const found = this.operations.find((operation) => operation.id === id);
		return found ? this.#plain(found) : null;
	}

	/**
	 * Draft a transfer into the review. Resolves with the operation, or with
	 * why it could not be drafted — in which case nothing is left open.
	 */
	prepare(draft: Draft, by: IntentActor = 'agent'): Promise<IntentOperation | IntentReason> {
		return this.#draft(by, draft.chain_id, draft.token, draft.to, (port) => {
			const amount = decimal(draft.amount);
			port.dispatch(amount === 'max' ? { type: 'tap_max' } : { type: 'set_amount', amount });
			return (view) => {
				if (view.amount_warning) return `amount: ${named(view.amount_warning)}`;
				return view.can_continue ? 'ok' : null;
			};
		});
	}

	/**
	 * Draft one token to several recipients — the form's several-recipients
	 * mode, its rows set as a whole as the importer sets them.
	 */
	prepareBatch(
		draft: BatchDraft,
		by: IntentActor = 'agent'
	): Promise<IntentOperation | IntentReason> {
		return this.#draft(by, draft.chain_id, draft.token, null, (port) => {
			port.dispatch({
				type: 'seed_split_recipients',
				recipients: draft.recipients.map((row) => ({
					id: '',
					address: row.to.trim(),
					amount: decimal(row.amount),
					name: null
				}))
			});
			return (view) => {
				if (view.recipients.length !== draft.recipients.length)
					return `recipients: the form took ${view.recipients.length} of ${draft.recipients.length}`;
				const issue = view.split_row_issues[0];
				if (issue)
					return `recipient ${issue.ordinal}: address ${issue.address}, amount ${issue.amount}`;
				if (view.split_over_balance) return 'amount: the total is more than the balance';
				return view.can_continue ? 'ok' : null;
			};
		});
	}

	/**
	 * Open the send on the token, let `fill` put the draft on the form (and
	 * say when the form is ready), then Continue to the review — the order
	 * the form's own controls go in.
	 */
	async #draft(
		by: IntentActor,
		chainId: number,
		contract: string | null,
		recipient: string | null,
		fill: (port: SendDeskPort) => (view: SendView) => string | null
	): Promise<IntentOperation | IntentReason> {
		const stopped = () => by === 'agent' && this.clock.halted();
		const halted = reason(
			'stopped',
			'the agent was stopped or switched off while drafting: nothing was left on the review'
		);
		if (stopped()) return halted;
		this.#listen();
		const port = await this.#desk();
		if (!port) return reason('not_ready', 'the wallet page did not open');
		const open = port.view();
		let closed: string | null = null;
		if (open?.stage === 'receipt') {
			// The send core's last step: the transfer has gone out, and the receipt
			// only tells how it went. Its own Done gives way to the next draft;
			// the transfer is followed on the chain from here (`#fromChain`).
			this.sync();
			closed = this.operations.find((entry) => entry.session === port.session())?.id ?? null;
			port.dispatch({ type: 'done' });
			port.close();
		} else if (open !== null) {
			const review = open.stage === 'confirm' ? this.current() : null;
			return reason(
				'busy',
				review
					? `transfer ${review.id} is on its review (${review.state}): submit or cancel it first`
					: 'the person is filling in a send in the wallet: wait for them to finish or close it'
			);
		}

		// "Not held" is only true of balances that have been read: right after
		// the page opens, they may not have been yet.
		const account = session.view.address;
		await until(() => (balance.settledFor(account) ? true : null), STEP_MS.open);
		const token = balance.view.tokens.find(
			(held) =>
				!held.spam &&
				held.chain_id === chainId &&
				(contract === null
					? held.token_address === null
					: held.token_address !== null && same(held.token_address, contract))
		);
		if (!token) return reason('not_found', 'the wallet holds no such token on that chain');
		if (stopped()) return halted;

		port.open({
			preselected_symbol: token.symbol,
			preselected_network: networkId(token.chain_id),
			...(recipient !== null && { prefilled_recipient: recipient })
		});
		const opened = await until(() => {
			if (stopped()) return 'stopped';
			const view = port.view();
			if (!view || view.loading) return null;
			return view.selected_token && view.stage === 'enter_details' ? view : 'elsewhere';
		}, STEP_MS.open);
		if (opened === null || typeof opened === 'string') {
			port.close();
			if (stopped()) return halted;
			return reason('not_found', 'the send did not offer that token');
		}

		const settled = fill(port);
		const ready = await until(() => {
			if (stopped()) return 'stopped';
			const view = port.view();
			return view ? settled(view) : 'closed';
		}, STEP_MS.amount);
		if (ready !== 'ok') {
			port.close();
			if (stopped()) return halted;
			return reason('refused', ready ?? 'the form could not be checked in time');
		}

		if (stopped()) {
			port.close();
			return halted;
		}
		port.dispatch({ type: 'continue' });
		const reviewed = await until(() => {
			if (stopped()) return 'stopped';
			const alert = port.alert();
			if (alert) return `alert: ${named(alert)}`;
			const view = port.view();
			if (!view) return 'closed';
			return view.stage === 'confirm' ? 'ok' : null;
		}, STEP_MS.review);
		if (reviewed !== 'ok') {
			port.close();
			if (stopped()) return halted;
			return reason('refused', reviewed ?? 'the review did not open in time');
		}
		const drafted = this.#adopt(port, token.chain_id);
		drafted.closed = closed;
		drafted.draftedBy = by;
		return this.#plain(drafted);
	}

	/** Who drafted `id` onto its review. */
	draftedBy(id: string): IntentActor | null {
		return this.operations.find((entry) => entry.id === id)?.draftedBy ?? null;
	}

	/** The finished transfer whose receipt drafting `id` closed, if any. */
	closedFor(id: string): string | null {
		return this.operations.find((entry) => entry.id === id)?.closed ?? null;
	}

	/**
	 * The open send's operation, adopted if it has none yet — a person who
	 * filled the form themselves confirms through the same intent.
	 */
	current(): IntentOperation | null {
		const port = sendDesk.port;
		const view = port?.view();
		if (!port || !view || view.stage !== 'confirm') return null;
		this.#listen();
		return this.#plain(this.#adopt(port, view.selected_token?.chain_id ?? null));
	}

	/**
	 * Confirm the operation's review. The person's confirm presses it, and
	 * their passkey signs. An agent's only asks them (owner, 2026-10-21): the
	 * passkey prompt shows no transaction, so it is never raised over a review
	 * the person did not choose to confirm — their own tap on the review is
	 * the only way to it.
	 */
	submit(id: string, by: IntentActor = 'person'): IntentOperation | IntentReason {
		const port = sendDesk.port;
		const operation = this.operations.find((entry) => entry.id === id);
		if (!port || !operation || port.session() !== operation.session || port.view() === null)
			return reason('not_found', 'that transfer is no longer on the review screen');
		this.sync();
		const view = port.view();
		// One network, one nonce: the core holds the confirm while the account's
		// previous transfer there is still going through, and opens it itself.
		const waiting = view?.previous_pending;
		if (waiting)
			return reason(
				'not_ready',
				`the previous transfer on this network (${waiting.user_op_hash}) is still going through: the confirm opens once it is final`
			);
		if (by === 'agent') {
			// The review is the screen in front of the person; the bar says the
			// agent asks them to confirm it. Nothing is pressed.
			if (operation.state === 'awaiting_confirmation')
				operation.reason = 'waiting for the person to confirm it on the review';
			return this.#plain(operation);
		}
		if (!view?.can_confirm) return reason('refused', 'the review cannot be confirmed yet');
		if (
			operation.draftedBy === 'agent' &&
			this.clock.now() - operation.reviewedAt < REVIEW_SETTLE_MS
		)
			return reason(
				'not_ready',
				"the agent's review has only just opened: look it over, then confirm again"
			);
		operation.reason = null;
		operation.confirmedAfter = this.#ceremonies;
		port.dispatch({ type: 'slide_confirm' });
		this.sync();
		return this.#plain(operation);
	}

	/** Leave the operation's review without sending — a failed one's too. */
	cancel(id: string): IntentReason | null {
		const port = sendDesk.port;
		const operation = this.operations.find((entry) => entry.id === id);
		if (!port || !operation || port.session() !== operation.session || port.view() === null)
			return reason('not_found', 'that transfer is no longer on the review screen');
		port.close();
		// A review that failed before anything was sent is left like any other,
		// and keeps why it failed. One still waiting says it was left: what it
		// was waiting for is over.
		if (operation.state === 'failed' && operation.user_op_hash === null)
			operation.state = 'cancelled';
		else operation.reason = null;
		this.sync();
		return null;
	}

	/**
	 * Bring every operation up to date with what the machines say now: its
	 * send while open, else the chain tracker, and the passkey's ceremony —
	 * read by the core (`intent_rules::operation_progress`), the same for
	 * every shell. Before the core has loaded nothing has moved to read.
	 */
	sync(): void {
		const port = sendDesk.port;
		const tracker = JSON.stringify(txTrackerView().entries);
		const ceremony = JSON.stringify(this.#ceremony);
		for (const operation of this.operations) {
			if (FINAL.includes(operation.state)) continue;
			const open = port !== null && port.session() === operation.session ? port.view() : null;
			let next: IntentProgress;
			try {
				next = JSON.parse(
					intentOperationProgress(
						JSON.stringify({
							operation: this.#plain(operation),
							confirmed_after: operation.confirmedAfter
						}),
						JSON.stringify(open),
						ceremony,
						tracker
					)
				) as IntentProgress;
			} catch {
				return;
			}
			Object.assign(operation, {
				state: next.operation.state,
				user_op_hash: next.operation.user_op_hash,
				tx_hash: next.operation.tx_hash,
				reason: next.operation.reason
			});
		}
	}

	#adopt(port: SendDeskPort, chainId: number | null): Tracked {
		const session = port.session();
		// One that has ended — left without sending, say — is not this review's:
		// the person went back, and what they review now is another transfer.
		const existing = this.operations.find(
			(operation) => operation.session === session && !FINAL.includes(operation.state)
		);
		if (existing) return existing;
		const created: Tracked = {
			id: `transfer-${this.#next++}`,
			state: 'awaiting_confirmation',
			chain_id: chainId,
			user_op_hash: null,
			tx_hash: null,
			reason: null,
			closed: null,
			draftedBy: 'person',
			reviewedAt: this.clock.now(),
			session,
			confirmedAfter: null
		};
		this.operations = [created, ...this.operations];
		return this.operations[0];
	}

	#plain(operation: Tracked): IntentOperation {
		const { id, state, chain_id, user_op_hash, tx_hash, reason } = operation;
		return { id, state, chain_id, user_op_hash, tx_hash, reason };
	}

	/** The wallet page's send, opening the page if it is not up. */
	async #desk(): Promise<SendDeskPort | null> {
		if (!sendDesk.port) {
			const locale = localeOfPath(location.pathname) ?? 'en';
			await goto(resolve('/[locale]/wallet', { locale }));
		}
		return until(() => sendDesk.port, STEP_MS.page);
	}

	#listen(): void {
		if (this.#listening) return;
		this.#listening = true;
		onSignCeremony((event) => {
			this.#ceremonies += 1;
			this.#ceremony = { event, seq: this.#ceremonies };
		});
	}
}

export const transfers = new Transfers();
