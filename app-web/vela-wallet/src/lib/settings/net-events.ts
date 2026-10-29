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

import type { SettingsPageId } from './model';

export type SettingsNetEvent =
	// Networks list + per-network editor
	| { kind: 'select-network'; id: string }
	| { kind: 'delete-network'; id: string }
	// Spec 105: a network the person added, to the other space; the switch
	| { kind: 'move-network'; id: string }
	| { kind: 'switch-space' }
	| { kind: 'detail-field'; field: NetOverrideField; value: string }
	| { kind: 'detail-blur'; field: NetOverrideField }
	// Owner decision 2026-10-28: the network's "Only use my node" switch
	| { kind: 'only-mine'; on: boolean }
	// Add-network wizard
	| { kind: 'open-add' }
	/** set-5: the person left the wizard and chose to discard what they entered. */
	| { kind: 'discard-add' }
	| { kind: 'search'; query: string }
	| { kind: 'pick-suggestion'; chainId: number }
	| { kind: 'custom-rpc'; value: string }
	| { kind: 'confirm-add' }
	| { kind: 'recheck' }
	| { kind: 'add-space'; testnet: boolean }
	// Service endpoints
	| { kind: 'endpoints-open' }
	| { kind: 'endpoint'; field: NetEndpointField; value: string }
	| { kind: 'endpoint-blur'; field: NetEndpointField }
	| { kind: 'endpoints-reset' };

export type OnNetEvent = (event: SettingsNetEvent) => void;

/**
 * The pages whose OPENING is itself a core event: the machine probes what the
 * person is about to look at. One table for both layouts: the wide one used
 * to keep no list at all.
 */
export const OPENED_EVENT: Partial<Record<SettingsPageId, SettingsNetEvent>> = {
	endpoints: { kind: 'endpoints-open' },
	'add-network': { kind: 'open-add' }
};
