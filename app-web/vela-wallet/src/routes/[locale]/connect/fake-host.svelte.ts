/**
 * A stand-in for the connect page's machine host (`OutsidePageHost`): its
 * view is whatever a test sets, and what the page asks of it is recorded —
 * so `page.svelte.test.ts` pins what the page draws and says for each view
 * the core's page machine can give (the machine itself is pinned by the
 * Rust suite, `tests/app_multisig_outside.rs`).
 */
import type { OutsidePageEvent } from '$lib/multisig/generated/OutsidePageEvent';
import type { OutsidePageView } from '$lib/multisig/generated/OutsidePageView';
import type { RiskAsk } from '$lib/multisig/shell/outside-page.svelte';

export function emptyView(): OutsidePageView {
	return {
		link: null,
		load: { kind: 'idle' },
		tx: null,
		step: { kind: 'idle' },
		actions: null,
		fee: null,
		balance: null,
		accounts: [],
		walletChainId: 100,
		unsent: false,
		sessions: [],
		activity: [],
		request: null,
		connectPrompt: null,
		joinFailed: null,
		relay: { kind: 'up' },
		walletFailure: null,
		requestGenesis: null,
		requestEnough: false,
		requestReview: null,
		rows: []
	};
}

export class FakeHost {
	view = $state<OutsidePageView>(emptyView());
	risk = $state<RiskAsk | null>(null);
	events: OutsidePageEvent[] = [];
	walletUsed = false;
	retried = 0;
	disposed = false;
	readonly #on: (host: FakeHost, event: OutsidePageEvent) => void;

	constructor(on: (host: FakeHost, event: OutsidePageEvent) => void) {
		this.#on = on;
	}

	set(patch: Partial<OutsidePageView>): void {
		this.view = { ...this.view, ...patch };
	}

	dispatch(event: OutsidePageEvent): void {
		this.events.push(event);
		this.#on(this, event);
	}

	async useWallet(wallet: { request(a: { method: string }): Promise<unknown> }): Promise<void> {
		const accounts = (await wallet.request({ method: 'eth_accounts' })) as string[];
		this.set({ accounts: accounts.map((a) => a.toLowerCase()) });
		this.walletUsed = true;
	}

	retryRelay(): void {
		this.retried += 1;
	}

	dispose(): void {
		this.disposed = true;
	}
}
