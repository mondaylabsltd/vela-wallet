/**
 * vela-core, in the browser — loaded on demand, never on Welcome.
 *
 * This is the one place in this app where vela-core runs at RUNTIME (moved
 * here from `$lib/onboarding/core/wasm-client.ts`, spec 024, so the address
 * stops claiming the core belongs to onboarding). Everywhere else it runs at
 * BUILD time: `$lib/i18n/engine.server.ts` resolves the 15 locales while
 * prerendering and `$lib/wallet/identicon.server.ts` rasterizes, and
 * `e2e/welcome-ssr.e2e.ts` asserts the deployed Worker bundle contains no
 * wasm at all (Workers cannot compile wasm from bytes).
 *
 * Both facts stay true. The machines run live, in the browser, because that
 * is where the person is — so the module is fetched by the CLIENT, from our
 * own origin, and only once someone has committed to a flow that needs it.
 * The prerendered Welcome page still loads no wasm; measure it with the
 * network panel filtered to `.wasm` before believing otherwise.
 *
 * The artifact is ~3.6 MB and carries all 24 state machines (wasm is not
 * tree-shaken, so it arrives whole or not at all). That is the price of one
 * implementation of the rules — which is also why wiring another machine
 * costs zero additional bytes.
 */

import init, {
	// The 24 machines, exactly as `vela-core-wasm` registers them
	// (src/onboarding.rs + src/wallet_state.rs).
	ActivityFeedCore,
	bestNativeDexPrice,
	checksumAddress,
	chooseNativePrice,
	ApprovalGuardCore,
	BalanceDashboardCore,
	BatchImportCore,
	BrowserHistoryCore,
	buildGroupProof,
	buildMemberProof,
	ClearSigningCore,
	ContactsCore,
	CreateWalletCore,
	DappPermissionsCore,
	DappSessionCore,
	DisplayCurrencyCore,
	ExtCacheCore,
	FeePolicyCore,
	groupPublicKeyFromSeed,
	identiconNormalizeSeed,
	identiconSvgCircular,
	keccak256,
	LoginCore,
	minGasPriceWei,
	ManageTokensCore,
	NetworkAdminCore,
	passkeyDirectoryEntry,
	passkeyDirectoryUrl,
	passkeyFallbackIconDataUri,
	passkeyProviderIconDataUri,
	PaymentRequestCore,
	peggedNativeUsd,
	ReceiveWatchCore,
	registryNameStep,
	RpcPoolCore,
	SendCore,
	SessionCore,
	SignRequestCore,
	toHex,
	TokenTrustCore,
	TxTrackerCore
} from '../../../../../rust/pkg-web/vela_core.js';
import { WASM_URL } from '../../../../../rust/pkg-web/vela_core_wasm_url.js';

export {
	ActivityFeedCore,
	ApprovalGuardCore,
	BalanceDashboardCore,
	BatchImportCore,
	BrowserHistoryCore,
	ClearSigningCore,
	ContactsCore,
	CreateWalletCore,
	DappPermissionsCore,
	DappSessionCore,
	DisplayCurrencyCore,
	ExtCacheCore,
	FeePolicyCore,
	LoginCore,
	ManageTokensCore,
	NetworkAdminCore,
	PaymentRequestCore,
	ReceiveWatchCore,
	RpcPoolCore,
	SendCore,
	SessionCore,
	SignRequestCore,
	TokenTrustCore,
	TxTrackerCore
};
// Pure kernels callable once `loadCore()` has resolved. The comments they
// shipped with in the onboarding client still apply: proofs and the group
// key for the registry; identicon and passkey artwork resolved offline by
// construction.
export { buildGroupProof, buildMemberProof, groupPublicKeyFromSeed, toHex };
export { bestNativeDexPrice, checksumAddress, chooseNativePrice, keccak256 };
// Native-coin money rules the shells used to each keep a copy of (spec 060):
// the $1 peg for a coin that IS a dollar, and the chain gas floor below which
// Arc discards a transaction without saying so.
export { minGasPriceWei, peggedNativeUsd };
export { identiconNormalizeSeed, identiconSvgCircular };
export { passkeyFallbackIconDataUri, passkeyProviderIconDataUri };
export { passkeyDirectoryEntry, passkeyDirectoryUrl };
export { registryNameStep };

/**
 * The in-flight (or settled) initialization. Held as a promise rather than a
 * boolean so two screens starting at once share one fetch instead of racing
 * `init` twice — wasm-bindgen tolerates the second call, but not the second
 * download.
 */
let pending: Promise<void> | null = null;

/**
 * Fetch and instantiate the core. Idempotent, and safe to call from every
 * entry point that is about to construct one.
 *
 * Throws only if the module cannot be fetched or instantiated — a genuine
 * "this browser cannot run the wallet" fault, not a user error, so callers
 * surface it rather than converting it into a flow state.
 */
export function loadCore(): Promise<void> {
	if (pending) return pending;
	const started = init({ module_or_path: WASM_URL }).then(() => undefined);
	// A failed load must not be cached as "done": the next attempt should be
	// able to retry rather than construct a core over an uninitialized module
	// and fail with something unreadable.
	started.catch(() => {
		if (pending === started) pending = null;
	});
	pending = started;
	return started;
}

/** Test seam: forget the cached initialization. Not used by product code. */
export function resetCoreForTests(): void {
	pending = null;
}
