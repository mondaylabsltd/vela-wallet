/**
 * r7: a member's kind, declared then verified — "Vela account" or "Other
 * wallet", asked where a member is added (Create, Safe{Wallet} import,
 * Settings → Add member). What the wallet can tell (`signers.evidence`)
 * shows where it can, and the person is asked where it can't. What is kept
 * is a label that steers what each member is offered (it travels in invites
 * as p/w/c), never authority, and evidence beats it: the index or a passkey
 * approval make it a Vela account; code at its address with no index
 * record, a smart contract account.
 *
 * "Other wallet" from evidence is only a hint (no code anywhere, no index
 * record — but a Vela account not yet set up anywhere reads the same): it is
 * shown and said, and kept only if the person picks it. Left unsaid, the
 * member is offered both ways to approve.
 */
import type { KindEvidence } from './signers.svelte';
import type { MemberInfo } from './store.svelte';

/** A Vela account, an outside wallet, or a smart contract account — as kept with a member. */
export type MemberKind = NonNullable<MemberInfo['kind']>;

export interface KindChoice {
	/** What is kept (and drawn picked); `null`: nothing — the member stays unsure. */
	kind: MemberKind | null;
	/** What the wallet suggests without keeping it: "Other wallet", drawn as a hint. */
	suggested: 'wallet' | null;
	/** Evidence (or a kept contract) says what it is: shown, not asked. */
	locked: boolean;
	/**
	 * The line under it: `known` (a Vela account, found), `contract` (a smart
	 * contract account: approves from its own app, which Vela can't collect
	 * yet), `likely` (the "Other wallet" hint), `ask` (it can't tell), or
	 * `null` (the person's own word).
	 */
	note: 'known' | 'contract' | 'likely' | 'ask' | null;
}

/**
 * What a member's kind choice shows, from what the wallet can tell and what
 * the person said (here, or on this device before — `said`).
 */
export function kindChoice(evidence: KindEvidence, said: MemberKind | undefined): KindChoice {
	if (evidence === 'vela') return { kind: 'passkey', suggested: null, locked: true, note: 'known' };
	if (evidence === 'contract' || (said === 'contract' && evidence === null))
		return { kind: 'contract', suggested: null, locked: true, note: 'contract' };
	if (said && said !== 'contract')
		return { kind: said, suggested: null, locked: false, note: null };
	if (evidence === 'wallet')
		return { kind: null, suggested: 'wallet', locked: false, note: 'likely' };
	return { kind: null, suggested: null, locked: false, note: 'ask' };
}
