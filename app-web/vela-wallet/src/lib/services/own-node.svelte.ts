/**
 * The RPC pool's word on the person's own nodes (owner decision 2026-10-28),
 * reactive for the screens that say it: a network's row in Settings, the
 * balance breakdown, the one-time notice on the home.
 *
 * Every list is the core's (`RpcPoolView`), copied here as it arrives — no
 * rule is made in this file. `fallback`: the person's node is not answering
 * and public nodes serve the chain for now. `ownOnly`: "Only use my node" is
 * on and binds. `unreached`: of those, the ones that cannot be read now.
 */
export const ownNodes = $state<{ fallback: number[]; ownOnly: number[]; unreached: number[] }>({
	fallback: [],
	ownOnly: [],
	unreached: []
});

const same = (a: readonly number[], b: readonly number[]): boolean =>
	a.length === b.length && a.every((value, i) => value === b[i]);

/** The pool's latest lists; a list that did not change is not reassigned. */
export function noteOwnNodes(fallback: number[], ownOnly: number[], unreached: number[]): void {
	if (!same(ownNodes.fallback, fallback)) ownNodes.fallback = fallback;
	if (!same(ownNodes.ownOnly, ownOnly)) ownNodes.ownOnly = ownOnly;
	if (!same(ownNodes.unreached, unreached)) ownNodes.unreached = unreached;
}
