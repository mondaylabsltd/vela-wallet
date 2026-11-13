/**
 * A stand-in for the session machine's host (`OutsideSessionHost`), for
 * `ExternalSignSheet.svelte.test.ts`: its view is whatever a test sets, and
 * what the sheet asks of it is recorded (the machine itself is pinned by the
 * Rust suite, `tests/app_multisig_outside.rs`).
 */
import type { OutsideSessionEvent } from '../generated/OutsideSessionEvent';
import type { OutsideSessionView } from '../generated/OutsideSessionView';

export function sessionView(over: Partial<OutsideSessionView> = {}): OutsideSessionView {
	return {
		open: true,
		resumed: false,
		invite: 'wss://relay#invite',
		members: [],
		tx: null,
		phase: 'waiting',
		since: Date.now(),
		away: false,
		relay: { kind: 'up' },
		accounts: [],
		chain: null,
		wallet: null,
		pairedAccount: null,
		failure: null,
		outcome: null,
		...over
	};
}

export class FakeSession {
	view = $state<OutsideSessionView>(sessionView());
	events: OutsideSessionEvent[] = [];
	retried = 0;
	disposed = false;
	readonly #on: (session: FakeSession, event: OutsideSessionEvent) => void;

	constructor(
		view: OutsideSessionView,
		on: (session: FakeSession, event: OutsideSessionEvent) => void
	) {
		this.view = view;
		this.#on = on;
	}

	get link(): string {
		return this.view.invite
			? `https://w/en/connect#${encodeURIComponent(this.view.invite)}&for=x`
			: 'https://w/en/connect#for=x';
	}

	set(patch: Partial<OutsideSessionView>): void {
		this.view = { ...this.view, ...patch };
	}

	dispatch(event: OutsideSessionEvent): void {
		this.events.push(event);
		this.#on(this, event);
	}

	retryRelay(): void {
		this.retried += 1;
	}

	dispose(): void {
		this.disposed = true;
	}
}
