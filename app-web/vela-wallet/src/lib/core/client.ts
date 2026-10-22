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
	// The 25 machines, exactly as `vela-core-wasm` registers them
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
	dappOriginLock,
	dpermSettleOnClose,
	DisplayCurrencyCore,
	fiatSymbol,
	ExtCacheCore,
	FeePolicyCore,
	FeeSpeedCore,
	FeeTierPrefCore,
	groupPublicKeyFromSeed,
	i18nPluralSuffix,
	identiconNormalizeSeed,
	identiconSvgCircular,
	keccak256,
	LoginCore,
	minGasPriceWei,
	ManageTokensCore,
	maskedAmount,
	NetworkAdminCore,
	passkeyDirectoryEntry,
	passkeyDirectoryUrl,
	passkeyFallbackIconDataUri,
	passkeyProviderIconDataUri,
	PaymentRequestCore,
	peggedNativeUsd,
	intentAgentState,
	intentApprovalDetail,
	intentBaseline,
	intentCatalog,
	intentCheck,
	intentExamine,
	intentOperationProgress,
	intentReview,
	intentShows,
	intentVerdict,
	intentWorldNeeds,
	prefsMigrations,
	prefsRead,
	publicRpcUrls,
	ReceiveWatchCore,
	registryBackupStep,
	registryChainKeyPlan,
	registryChainKeyStatus,
	registryChainUnit,
	registryChainUnitPlan,
	registryNameStep,
	registryResolveKeyStep,
	registryResolveUnitStep,
	RpcPoolCore,
	SendCore,
	SessionCore,
	SignRequestCore,
	storageIsCacheKey,
	storageIsErasableKey,
	storageIsOurs,
	storageItemOfKey,
	storageItems,
	storageRecordsIn,
	toHex,
	TokenTrustCore,
	TxTrackerCore,
	walletKeysStep
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
	FeeSpeedCore,
	FeeTierPrefCore,
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
// A hidden amount as balance privacy draws it — the mask, and the unit kept:
// "•••• xDAI" (`privacy::masked_amount`). One rule for all four apps; the web
// kept a mirror of it (`maskedFigure`) until the core exported its own.
// Synchronous, and callable once `loadCore()` has resolved — every caller is
// a view-model builder over a core view, which cannot exist before that.
export { maskedAmount };
// The symbol a fiat figure is written with (`l10n::fiat_symbol`): one rule for
// all four apps, where the web kept a table of its own that wrote the Hong
// Kong dollar with the US dollar's "$" and the yen and the yuan with one "¥".
// `language` is the page's locale; a currency with no glyph of its own comes
// back as its code. Synchronous, callable once `loadCore()` has resolved.
export { fiatSymbol };
export { identiconNormalizeSeed, identiconSvgCircular };
// Which plural form a count takes in a language (CLDR, issue 409): flow copy
// ships every form, and the screen asks this rather than `count === 1`.
export { i18nPluralSuffix };
export { passkeyFallbackIconDataUri, passkeyProviderIconDataUri };
export { passkeyDirectoryEntry, passkeyDirectoryUrl };
export { registryBackupStep, registryNameStep, walletKeysStep };
// The curated public RPCs behind each built-in network's default — ONE list
// for all four apps (`network_admin::PUBLIC_RPCS`), where each shell used to
// keep its own copy and they drifted (the web's still named endpoints that
// had been answering HTTP 502 for every call).
export { publicRpcUrls };
// How a pending request is settled when the request window goes away: 4900,
// never 4001 (spec 070 T063). The window asks rather than restating it.
export { dpermSettleOnClose };
// The mark a site's address wears (`dapp_browser::origin_lock`): `secure` for
// https — the only padlock — `local` for http on this device or the person's
// own network, `insecure` for public http. One rule for all four apps; a
// padlock used to be drawn from "safe to use the wallet from", which is true
// of `http://127.0.0.1:8137` and says nothing about encryption.
export { dappOriginLock };
export { registryChainKeyPlan, registryChainKeyStatus, registryChainUnit, registryChainUnitPlan };
export { registryResolveKeyStep, registryResolveUnitStep };
// Settings' shared rules (spec 072): how every shell's preference spellings
// read, and what each stored key IS — its storage row, whether "clear all
// caches" takes it, whether an erase does. JSON in and out, like the machines.
export { prefsMigrations, prefsRead };
// The wallet's intents (spec 103): the catalog, and the check every request
// passes — who may ask, and whether the input is well formed. JSON in and out.
export { intentCatalog, intentCheck };
// Their judgements over the wallet's state (spec 104): examine, verdict, a
// transfer's progress, and what an agent reads. JSON in and out.
export {
	intentAgentState,
	intentApprovalDetail,
	intentBaseline,
	intentExamine,
	intentOperationProgress,
	intentReview,
	intentShows,
	intentVerdict,
	intentWorldNeeds
};
export {
	storageIsCacheKey,
	storageIsErasableKey,
	storageIsOurs,
	storageItemOfKey,
	storageItems,
	storageRecordsIn
};

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
