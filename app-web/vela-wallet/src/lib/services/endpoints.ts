/**
 * Service-endpoint resolution — WEB (spec 024).
 *
 * The defaults are the Expo table verbatim (src/models/types.ts
 * DEFAULT_SERVICE_ENDPOINTS @ e78afdfa); overrides come from the stored
 * `vela.serviceEndpoints` record, read through its one storage home
 * (research D3a). Read-through rather than cached: every caller here is an
 * async executor operation, so the Expo in-memory cache has nothing to feed.
 */

import { loadServiceEndpoints } from '$lib/onboarding/core/storage';
import { allowedOrNull } from './url-policy';

export const DEFAULT_SERVICE_ENDPOINTS = {
	ethereumDataURL: 'https://ethereum-data.getvela.app',
	passkeyIndexURL: 'https://p256-index-v2.getvela.app',
	bundlerServiceURL: 'https://vela-relay-cf.getvela.app',
	// Vela's self-hosted Frankfurter instance (github.com/mondaylabsltd/vela-currency):
	// FOSS, no key, ~160 currencies incl. VND. base=USD is required (default base is EUR).
	fiatRatesURL: 'https://vela-currency.getvela.app/v2/rates?base=USD',
	// Spec 038 #E4: the AAGUID directory is meant to be OUR node; until it
	// exists the default is the one the core's catalog names, and the field
	// is what moves the lookup the day it does — no build in between.
	aaguidDirectoryURL: 'https://aaguid-explorer.awesometools.dev',
	// The multisig service (Safe Genesis Registry): records multisigs and
	// carries their proposals. The core's `DEFAULT_GENESIS_SERVICE_URL`.
	genesisServiceURL: 'https://safe-genesis.getvela.app'
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
