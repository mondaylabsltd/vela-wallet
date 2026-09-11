/**
 * Names for the authenticator models the compiled catalog cannot name.
 *
 * The catalog carries software passkey providers; hardware keys live in the
 * FIDO metadata service, hundreds of models deep, which is what the directory
 * service answers for. It is OUR service and stores nothing (founder,
 * 2026-08-26), which is what makes asking it acceptable at all — and the
 * catalog still answers first, instantly and offline, so this only ever runs
 * for a key nothing on the device could name.
 *
 * The core owns the contract: which AAGUIDs are worth asking about, and what
 * counts as an answer (the body must be about the question, and an icon path
 * must be the service's own shape before anything fetches it). This module owns
 * only the transport and the memory.
 */
import { browser } from '$app/environment';
import { NET_TIMEOUTS, fetchWithTimeout } from '$lib/services/net';
import { passkeyDirectoryEntry, passkeyDirectoryUrl } from './wasm-client';

export interface DirectoryEntry {
	name: string;
	iconUrl?: string;
}

/** `null` = asked, no answer. `undefined` = not asked. */
type Slot = DirectoryEntry | null | undefined;

/** One request per AAGUID per session, whatever the answer. */
// eslint-disable-next-line svelte/prefer-svelte-reactivity -- dedup guard, not render state; `entries` is the reactive channel
const inFlight = new Set<string>();

const entries = $state<Record<string, DirectoryEntry | null>>({});

/**
 * What the directory says about `aaguid`, asking it the first time.
 *
 * Reading this in a component subscribes the component to the answer, so the
 * row re-renders when it lands. A failure is remembered as "no answer" rather
 * than retried on every redraw: the key list is not worth a retry storm, and
 * the row is already showing something honest.
 */
export function directoryEntry(aaguid: string, dark: boolean): Slot {
	if (!browser || !aaguid) return undefined;
	const key = `${aaguid.toLowerCase()}|${dark}`;
	const known = entries[key];
	if (known !== undefined) return known;
	if (!inFlight.has(key)) {
		inFlight.add(key);
		void lookup(aaguid, dark, key);
	}
	return undefined;
}

async function lookup(aaguid: string, dark: boolean, key: string): Promise<void> {
	const url = passkeyDirectoryUrl(aaguid);
	if (url === undefined) {
		entries[key] = null;
		return;
	}
	try {
		// On the app's timeout table (spec 038): the one bare fetch left.
		const response = await fetchWithTimeout(
			url,
			{ headers: { Accept: 'application/json' } },
			{ timeoutMs: NET_TIMEOUTS.ethereumData }
		);
		entries[key] = response.ok
			? ((passkeyDirectoryEntry(aaguid, await response.text(), dark) as DirectoryEntry) ?? null)
			: null;
	} catch {
		// Offline, blocked, or the service is down. The row keeps the honest
		// thing it already shows; nothing here is load-bearing.
		entries[key] = null;
	}
}

/**
 * What to call this key's holder: the compiled catalog's name, then the
 * directory's, and `undefined` when neither knows — the caller then says what
 * it always said about the METHOD.
 *
 * Same subscription rule as `directoryEntry`: read it in a component and the
 * component re-renders when the answer lands.
 */
export function providerLabel(
	provider_name: string,
	aaguid: string,
	dark: boolean
): string | undefined {
	if (provider_name) return provider_name;
	return directoryEntry(aaguid, dark)?.name;
}
