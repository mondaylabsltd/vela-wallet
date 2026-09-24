/**
 * The network surfaces' UI-event vocabulary (spec 024).
 *
 * One discriminated union threaded through the settings components as a
 * single optional callback, so the components stay pure pictures (absent
 * callback = gallery) and the ROUTE owns the translation into core events.
 * No variant here decides anything: each is "the person did this, here".
 */

import type { NetEndpointField } from '$lib/core/generated/NetEndpointField';
import type { NetOverrideField } from '$lib/core/generated/NetOverrideField';
import type { NetProviderId } from '$lib/core/generated/NetProviderId';
import type { SettingsPageId } from './model';

export type SettingsNetEvent =
	// Networks list + per-network editor
	| { kind: 'select-network'; id: string }
	| { kind: 'delete-network'; id: string }
	| { kind: 'detail-field'; field: NetOverrideField; value: string }
	| { kind: 'detail-blur'; field: NetOverrideField }
	// Add-network wizard
	| { kind: 'open-add' }
	| { kind: 'search'; query: string }
	| { kind: 'pick-suggestion'; chainId: number }
	| { kind: 'custom-rpc'; value: string }
	| { kind: 'confirm-add' }
	| { kind: 'recheck' }
	// Service endpoints
	| { kind: 'endpoints-open' }
	| { kind: 'endpoint'; field: NetEndpointField; value: string }
	| { kind: 'endpoint-blur'; field: NetEndpointField }
	| { kind: 'endpoints-reset' }
	// RPC providers
	| { kind: 'providers-open' }
	| { kind: 'provider-key'; provider: NetProviderId; value: string }
	| { kind: 'provider-blur'; provider: NetProviderId }
	| { kind: 'provider-test'; provider: NetProviderId };

export type OnNetEvent = (event: SettingsNetEvent) => void;

/**
 * The pages whose OPENING is itself a core event: the machine probes what the
 * person is about to look at, and the providers page seeds its drafts from
 * the saved keys — a field left without that seed saved an empty draft over
 * the key it showed (spec 072, P0). One table for both layouts: the wide one
 * used to keep no list at all.
 */
export const OPENED_EVENT: Partial<Record<SettingsPageId, SettingsNetEvent>> = {
	'rpc-providers': { kind: 'providers-open' },
	endpoints: { kind: 'endpoints-open' },
	'add-network': { kind: 'open-add' }
};
