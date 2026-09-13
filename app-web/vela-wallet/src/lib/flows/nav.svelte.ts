/**
 * Flow navigation (spec 021 SC-002).
 *
 * The wallet is one route, and these four journeys are pushed screens inside
 * it — the same shape the three native shells use, and the reason the web does
 * not grow fifteen prerendered routes for screens that are still fixtures.
 *
 * A stack, not a current-screen field. The mocks stack: Receive opens a
 * network list and a network opens its QR; Send runs picker → form → confirm →
 * receipt. Back has to unwind one level, which a single field cannot express.
 *
 * The browser's Back button drives the same stack through `history.pushState`,
 * so on the web the platform gesture and the on-screen chevron do the same
 * thing. Without that, Back would leave the wallet entirely from four screens
 * deep, which on a phone is the single most common way out of a flow.
 */
import type { DesktopFlowStateId, FlowStateId } from './model';

/** Where a flow can be entered from the wallet home. */
export type FlowEntry =
	| 'receive'
	/** R3 / DR3L straight away: the code for one held token (spec 028 Phase 9, RULING 3). */
	| 'receive-token'
	| 'send'
	| 'scan'
	| 'activity'
	| 'assets'
	| 'add-token'
	| 'token-detail'
	| 'tx-detail';

/**
 * The stack an entry opens, deepest last.
 *
 * `add-token` opens two: the assets screen and the sheet over it. That is what
 * makes the back chevron in the T3 and DT3L mocks mean something — it goes to
 * the list you were adding to, not out of the flow entirely.
 */
const MOBILE_ENTRIES: Record<FlowEntry, FlowStateId[]> = {
	receive: ['r1'],
	'receive-token': ['r1', 'r3'],
	send: ['sd1'],
	scan: ['s1'],
	activity: ['a1'],
	assets: ['t1'],
	'add-token': ['t1', 't3'],
	'token-detail': ['t1', 't2'],
	'tx-detail': ['a1', 'a2']
};

const DESKTOP_ENTRIES: Record<FlowEntry, DesktopFlowStateId[]> = {
	receive: ['dr1'],
	'receive-token': ['dr1', 'dr3'],
	send: ['dsd1'],
	// The desktop scanner is a centred modal rather than a panel; the host
	// watches for this id and draws it over the window instead.
	scan: ['ds1'],
	activity: ['da1'],
	assets: ['dt1'],
	'add-token': ['dt1', 'dt3'],
	// No desktop token-detail panel exists: spec 015's asset-detail panel
	// already fills that column, and forking a second one would be the
	// duplicate SC-003 forbids.
	'token-detail': ['dt1'],
	'tx-detail': ['da1', 'da2']
};

/** Pushes a step deeper within a flow that is already open. */
const MOBILE_STEPS: Record<string, FlowStateId> = {
	'receive-qr': 'r2',
	'tx-detail': 'a2',
	'token-detail': 't2',
	'add-token': 't3',
	'send-form': 'sd2',
	'send-confirm': 'sd3',
	'send-receipt': 'sd4b',
	'contact-pick': 'sd2e',
	'fee-token': 'sd2f',
	'batch-import': 'sd2c',
	'send-multi': 'sd1b',
	'add-recipient': 'sd2b',
	scan: 's1'
};

const DESKTOP_STEPS: Record<string, DesktopFlowStateId> = {
	'receive-qr': 'dr2',
	'tx-detail': 'da2',
	'add-token': 'dt3',
	'send-form': 'dsd2',
	'send-confirm': 'dsd3',
	'send-receipt': 'dsd4',
	'contact-pick': 'dsd2e',
	'fee-token': 'dsd2f',
	'batch-import': 'dsd2c',
	scan: 'ds1'
};

export class FlowNav {
	/** Deepest state last. Empty means the wallet home is showing. */
	mobile = $state<FlowStateId[]>([]);
	desktop = $state<DesktopFlowStateId[]>([]);

	/** The state on top, or `undefined` when the home is showing. */
	get mobileTop(): FlowStateId | undefined {
		return this.mobile.at(-1);
	}

	get desktopTop(): DesktopFlowStateId | undefined {
		return this.desktop.at(-1);
	}

	get open(): boolean {
		return this.mobile.length > 0 || this.desktop.length > 0;
	}

	/** Open a flow from the wallet home. */
	enter(entry: FlowEntry): void {
		this.mobile = [...MOBILE_ENTRIES[entry]];
		this.desktop = [...DESKTOP_ENTRIES[entry]];
		this.#mark();
	}

	/**
	 * Step deeper. Unknown steps are ignored rather than throwing: the screens
	 * emit navigation intents generously (`done`, `chains`, …) and a flow that
	 * has nowhere to put one should do nothing, not crash a wallet.
	 */
	push(step: string): void {
		const next = MOBILE_STEPS[step];
		const nextDesktop = DESKTOP_STEPS[step];
		if (next === undefined && nextDesktop === undefined) {
			if (step === 'done') this.close();
			return;
		}
		if (next !== undefined && this.mobileTop !== next) this.mobile = [...this.mobile, next];
		if (nextDesktop !== undefined && this.desktopTop !== nextDesktop) {
			this.desktop = [...this.desktop, nextDesktop];
		}
		this.#mark();
	}

	/** One level up. At the root this leaves the flow and shows the wallet. */
	back(): void {
		this.mobile = this.mobile.slice(0, -1);
		this.desktop = this.desktop.slice(0, -1);
	}

	close(): void {
		this.mobile = [];
		this.desktop = [];
	}

	/**
	 * Push a history entry so the browser's Back unwinds one level.
	 *
	 * `pushState` and not a route change: the URL stays the wallet's, because
	 * these screens are fixture states and a shareable link to one would
	 * promise a page that does not exist yet. What is wanted here is only the
	 * back STACK, which pushState gives without inventing URLs.
	 */
	#mark(): void {
		if (typeof history === 'undefined') return;
		history.pushState({ velaFlow: this.mobile.length }, '');
	}
}
