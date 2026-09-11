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

export const DEFAULT_SERVICE_ENDPOINTS = {
	ethereumDataURL: 'https://ethereum-data.awesometools.dev',
	passkeyIndexURL: 'https://p256-index-v2.getvela.app',
	bundlerServiceURL: 'https://vela-relay.getvela.app',
	// Vela's self-hosted Frankfurter instance (github.com/mondaylabsltd/vela-currency):
	// FOSS, no key, ~160 currencies incl. VND. base=USD is required (default base is EUR).
	fiatRatesURL: 'https://vela-currency.getvela.app/v2/rates?base=USD',
	// Spec 038 #E4: the AAGUID directory is meant to be OUR node; until it
	// exists the default is the one the core's catalog names, and the field
	// is what moves the lookup the day it does — no build in between.
	aaguidDirectoryURL: 'https://aaguid-explorer.awesometools.dev'
} as const;

export function getEthereumDataURL(): string {
	return loadServiceEndpoints().ethereumDataURL || DEFAULT_SERVICE_ENDPOINTS.ethereumDataURL;
}

export function getBundlerServiceURL(): string {
	return loadServiceEndpoints().bundlerServiceURL || DEFAULT_SERVICE_ENDPOINTS.bundlerServiceURL;
}

export function getFiatRatesURL(): string {
	return loadServiceEndpoints().fiatRatesURL || DEFAULT_SERVICE_ENDPOINTS.fiatRatesURL;
}

export function getPasskeyIndexURL(): string {
	return loadServiceEndpoints().passkeyIndexURL || DEFAULT_SERVICE_ENDPOINTS.passkeyIndexURL;
}

export function getAaguidDirectoryURL(): string {
	return (
		loadServiceEndpoints().aaguidDirectoryURL?.trim() || DEFAULT_SERVICE_ENDPOINTS.aaguidDirectoryURL
	);
}
