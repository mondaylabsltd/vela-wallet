/**
 * Spec 100: a page's add-network request is over (`network_admin`'s
 * `dapp_add_settled`). The executor hears it; the surface that owes the page
 * its answer (the extension's request window or side panel) listens here.
 * A module of its own so the executor and the resident import it without
 * importing each other.
 */

import type { DappAddOutcome } from '$lib/core/generated/DappAddOutcome';

export type DappAddSettledListener = (tab: string, id: string, outcome: DappAddOutcome) => void;

const listeners = new Set<DappAddSettledListener>();

/** Every listener hears every ending; the one that owes `id` answers it. */
export function dappAddSettled(tab: string, id: string, outcome: DappAddOutcome): void {
	for (const listener of [...listeners]) listener(tab, id, outcome);
}

/** Listen for add-network endings; returns the unsubscribe. */
export function onDappAddSettled(listener: DappAddSettledListener): () => void {
	listeners.add(listener);
	return () => {
		listeners.delete(listener);
	};
}
