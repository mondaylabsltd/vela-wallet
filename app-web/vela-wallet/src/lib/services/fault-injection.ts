/**
 * Dev/e2e fault injection — WEB (specs 025 and 026; the `vela.*` console the
 * fault-resilience playbook mandates: every failure-state UX must be reachable
 * without breaking real infrastructure).
 *
 * Ported and extended from src/services/dev/fault-injection.ts @ f9bcb278.
 * The Expo harness covers the READ path; 026 adds the relay arms its own
 * TEST-OUTLINE names as the gap ("bundler underfunded … not covered by
 * `vela.*` — verify by hand or extend fault-injection.ts").
 *
 * Every check is a cheap set lookup, and unset faults never trigger. The
 * console is installed only behind the dev gate; the deployed Worker never
 * runs it.
 */

type FaultState = {
	failRpcChains: Set<number>;
	rateLimitRpcChains: Set<number>;
	rpcLatencyMs: number;
	nullPriceChains: Set<number>;
	fundingForceChains: Set<number>;
	gasQuoteZeroChains: Set<number>;
	failRelayChains: Set<number>;
	emptyTreasuryChains: Set<number>;
	rejectSubmitChains: Set<number>;
	silentReceiptChains: Set<number>;
	faultPoolChains: Set<number>;
	forgetPublicKey: boolean;
};

const state: FaultState = {
	failRpcChains: new Set(),
	rateLimitRpcChains: new Set(),
	rpcLatencyMs: 0,
	nullPriceChains: new Set(),
	fundingForceChains: new Set(),
	gasQuoteZeroChains: new Set(),
	failRelayChains: new Set(),
	emptyTreasuryChains: new Set(),
	rejectSubmitChains: new Set(),
	silentReceiptChains: new Set(),
	faultPoolChains: new Set(),
	forgetPublicKey: false
};

export function rpcShouldFail(chainId: number): boolean {
	return state.failRpcChains.has(chainId);
}

export function rpcShouldRateLimit(chainId: number): boolean {
	return state.rateLimitRpcChains.has(chainId);
}

export function rpcLatencyMs(): number {
	return state.rpcLatencyMs;
}

export function priceShouldNull(chainId: number): boolean {
	return state.nullPriceChains.has(chainId);
}

/** Force the gas-account check to report "deposit needed" (the in-sheet funding UX). */
export function fundingShouldForce(chainId: number): boolean {
	return state.fundingForceChains.has(chainId);
}

/** Force the bundler gas quote to 0x0 — the fee must fall back, never read "~0". */
export function gasQuoteShouldZero(chainId: number): boolean {
	return state.gasQuoteZeroChains.has(chainId);
}

/** The relay is unreachable: every bundler call fails as a transport error. */
export function relayShouldFail(chainId: number): boolean {
	return state.failRelayChains.has(chainId);
}

/** The relay's treasury for this chain reports itself empty (bootstrap needed). */
export function treasuryShouldBeEmpty(chainId: number): boolean {
	return state.emptyTreasuryChains.has(chainId);
}

/** The relay accepts nothing: `eth_sendUserOperation` is refused. */
export function submitShouldReject(chainId: number): boolean {
	return state.rejectSubmitChains.has(chainId);
}

/**
 * The op is accepted but its receipt never arrives — the failure the tracker's
 * polling budget exists for ("still pending" vs "unreachable"), and the one a
 * person is most likely to meet on a busy chain.
 */
export function receiptShouldStaySilent(chainId: number): boolean {
	return state.silentReceiptChains.has(chainId);
}

/**
 * The request pool fails INSIDE the app for this chain — its core faulted, or
 * it never booted — so a read never leaves the device (issue 483). The fee
 * says that as Vela's own fault (`FeeFailure::Internal`), never "can't reach
 * the chain"; without this switch that state is reachable only by breaking
 * the app itself.
 */
export function poolShouldFault(chainId: number): boolean {
	return state.faultPoolChains.has(chainId);
}

/**
 * The fee is asked with no passkey public key to build an undeployed account's
 * initCode — the one fee failure only a TAP can clear (`missing_public_key`,
 * PR 2 note 1: "Tap to retry" on the row, "Tap it to retry" under the
 * confirm). A passkey wallet always has its key, so without this switch that
 * state is unreachable on the web.
 */
export function publicKeyShouldVanish(): boolean {
	return state.forgetPublicKey;
}

/** Every verb, in one object: the console publishes it, the seam below plants into it. */
const api = {
	failRpc: (chainId: number) => state.failRpcChains.add(chainId),
	rateLimitRpc: (chainId: number) => state.rateLimitRpcChains.add(chainId),
	slowRpc: (ms: number) => (state.rpcLatencyMs = ms),
	nullPrice: (chainId: number) => state.nullPriceChains.add(chainId),
	forceFunding: (chainId: number) => state.fundingForceChains.add(chainId),
	zeroGasQuote: (chainId: number) => state.gasQuoteZeroChains.add(chainId),
	failRelay: (chainId: number) => state.failRelayChains.add(chainId),
	emptyTreasury: (chainId: number) => state.emptyTreasuryChains.add(chainId),
	rejectSubmit: (chainId: number) => state.rejectSubmitChains.add(chainId),
	silentReceipt: (chainId: number) => state.silentReceiptChains.add(chainId),
	faultPool: (chainId: number) => state.faultPoolChains.add(chainId),
	forgetPublicKey: () => (state.forgetPublicKey = true),
	clearFaults: () => {
		state.failRpcChains.clear();
		state.rateLimitRpcChains.clear();
		state.nullPriceChains.clear();
		state.fundingForceChains.clear();
		state.gasQuoteZeroChains.clear();
		state.failRelayChains.clear();
		state.emptyTreasuryChains.clear();
		state.rejectSubmitChains.clear();
		state.silentReceiptChains.clear();
		state.faultPoolChains.clear();
		state.forgetPublicKey = false;
		state.rpcLatencyMs = 0;
	},
	faults: () => ({
		failRpc: [...state.failRpcChains],
		rateLimitRpc: [...state.rateLimitRpcChains],
		nullPrice: [...state.nullPriceChains],
		forceFunding: [...state.fundingForceChains],
		zeroGasQuote: [...state.gasQuoteZeroChains],
		failRelay: [...state.failRelayChains],
		emptyTreasury: [...state.emptyTreasuryChains],
		rejectSubmit: [...state.rejectSubmitChains],
		silentReceipt: [...state.silentReceiptChains],
		faultPool: [...state.faultPoolChains],
		forgetPublicKey: state.forgetPublicKey,
		slowRpc: state.rpcLatencyMs
	})
};

/**
 * Install `window.vela.*` (dev builds and the e2e worker only — callers gate;
 * the deployed production Worker never runs this).
 */
export function installFaultConsole(): void {
	if (typeof window === 'undefined') return;
	const g = window as unknown as { vela?: Record<string, unknown> };
	g.vela = Object.assign(g.vela ?? {}, api);
}

/**
 * Faults an automation planted BEFORE the app booted.
 *
 * A Playwright `addInitScript` can set `window.__VELA_FAULT_INIT__` to a list
 * of `[verb, arg]` pairs. They are applied where the call below sits — at this
 * module's load, which every product module that can fault already imports —
 * so the fault is armed before the first read rather than whenever the gated
 * console happens to arrive. That timing is the whole point: the console is a
 * DYNAMIC import, and a fault applied after the tracker's first poll proves
 * nothing.
 */
function applyInitFaults(): void {
	if (typeof window === 'undefined') return;
	const planted = (window as unknown as { __VELA_FAULT_INIT__?: [string, unknown][] })
		.__VELA_FAULT_INIT__;
	if (!Array.isArray(planted)) return;
	const verbs = api as unknown as Record<string, unknown>;
	for (const [verb, arg] of planted) {
		const fn = verbs[verb];
		if (typeof fn === 'function') (fn as (value: unknown) => void)(arg);
	}
}

// At MODULE LOAD — earlier than any component effect, and earlier than the
// gated console — so the very first read already runs under the fault and no
// test has to play refresh-timing games. A no-op unless the global is set,
// i.e. never in real use.
applyInitFaults();
