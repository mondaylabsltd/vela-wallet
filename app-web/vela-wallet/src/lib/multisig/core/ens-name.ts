/**
 * An ENS name typed where a member's address goes (r4D-5): read on Ethereum
 * by the core's accounts machine (`resolve_ens` — the registry's resolver for
 * the name, then that resolver's `addr`, through this wallet's own RPC) and
 * shown as the address it gives, prominently, for the person to add. The
 * name is a label only: the member is the address, checked and checksummed
 * like a typed one, and nothing about who may act ever rests on the name.
 *
 * Plain ASCII names only (`alice.eth`, `team.alice.eth`): a name with other
 * characters needs ENSIP-15 normalisation, which the core does not do — it
 * says so rather than resolve a lookalike.
 */
import type { MultisigEnsAnswer } from '../generated/MultisigEnsAnswer';
import { multisig } from './store.svelte';

/** Whether `text` is meant as an ENS name (ends in `.eth`), readable here or not: which field it is. */
export function looksLikeEns(text: string): boolean {
	return /^[^\s/]+\.eth$/i.test(text.trim());
}

/**
 * `found` (its address, checksummed), `none` (no resolver, or no address
 * set), `unavailable` (Ethereum did not answer: nothing is said of the
 * name), `unsupported` (characters the core does not normalise).
 */
export type EnsAnswer = MultisigEnsAnswer;

export function resolveEns(typed: string): Promise<EnsAnswer> {
	// The machine drops the last answer as it is asked: the next one is this name's.
	return multisig.request({ type: 'resolve_ens', name: typed }, (view) => view.add.ens);
}
