/**
 * Service-endpoint resolution — WEB (spec 024).
 *
 * The defaults are the Expo table verbatim (src/models/types.ts
 * DEFAULT_SERVICE_ENDPOINTS @ e78afdfa); overrides come from the stored
 * `vela.serviceEndpoints` record, read through its one storage home
 * (research D3a). Read-through rather than cached: every caller here is an
 * async executor operation, so the Expo in-memory cache has nothing to feed.
 */

import { SERVICE_DEFAULTS } from '$lib/core/generated/serviceDefaults';
import { loadServiceEndpoints } from '$lib/onboarding/core/storage';
import { allowedOrNull } from './url-policy';

/**
 * The defaults are the CORE's (`vela_core::service_defaults`, §28), generated
 * into `serviceDefaults.ts` and drift-gated — the web keeps no copy of its
 * own. Named as the stored record names them.
 */
export const DEFAULT_SERVICE_ENDPOINTS = {
	ethereumDataURL: SERVICE_DEFAULTS.ethereumDataUrl,
	passkeyIndexURL: SERVICE_DEFAULTS.passkeyIndexUrl,
	bundlerServiceURL: SERVICE_DEFAULTS.bundlerServiceUrl,
	// Vela's self-hosted Frankfurter instance (github.com/mondaylabsltd/vela-currency):
	// FOSS, no key, ~160 currencies incl. VND. base=USD is required (default base is EUR).
	fiatRatesURL: SERVICE_DEFAULTS.fiatRatesUrl,
	// Spec 038 #E4: the AAGUID directory is meant to be OUR node; until it
	// exists the default is the one the core's catalog names, and the field
	// is what moves the lookup the day it does — no build in between.
	aaguidDirectoryURL: SERVICE_DEFAULTS.aaguidDirectoryUrl,
	// The multisig service (Safe Genesis Registry): records multisigs and
	// carries their proposals.
	genesisServiceURL: SERVICE_DEFAULTS.genesisServiceUrl
} as const;

/**
 * The stored address for one service, when it is one the wallet may use
 * ({@link allowedOrNull}), else the default. `||` alone used whatever was
 * stored — `javascript:`, `ftp://`, a word — for every request after it.
 */
function stored(value: string | undefined, fallback: string): string {
	return allowedOrNull(value) ?? fallback;
}

export function getEthereumDataURL(): string {
	return stored(loadServiceEndpoints().ethereumDataURL, DEFAULT_SERVICE_ENDPOINTS.ethereumDataURL);
}

export function getBundlerServiceURL(): string {
	return stored(
		loadServiceEndpoints().bundlerServiceURL,
		DEFAULT_SERVICE_ENDPOINTS.bundlerServiceURL
	);
}

export function getFiatRatesURL(): string {
	return stored(loadServiceEndpoints().fiatRatesURL, DEFAULT_SERVICE_ENDPOINTS.fiatRatesURL);
}

export function getPasskeyIndexURL(): string {
	return stored(loadServiceEndpoints().passkeyIndexURL, DEFAULT_SERVICE_ENDPOINTS.passkeyIndexURL);
}

export function getAaguidDirectoryURL(): string {
	return stored(
		loadServiceEndpoints().aaguidDirectoryURL,
		DEFAULT_SERVICE_ENDPOINTS.aaguidDirectoryURL
	);
}

/**
 * The multisig service in use, no trailing slash (its paths are appended);
 * the default when the record holds none, or one the rule refuses.
 */
export function getGenesisServiceURL(): string {
	return stored(
		loadServiceEndpoints().genesisServiceURL,
		DEFAULT_SERVICE_ENDPOINTS.genesisServiceURL
	).replace(/\/+$/, '');
}
