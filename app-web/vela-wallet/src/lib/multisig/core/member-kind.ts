/**
 * r7: a member's kind, declared then verified — "Vela account" or "Other
 * wallet", asked where a member is added (Create, Safe{Wallet} import,
 * Settings → Add member). The wallet pre-fills it where it can tell
 * (`signers.evidence`) and asks where it can't. What is kept is a label that
 * steers what each member is offered (it travels in invites as p/w), never
 * authority, and evidence beats it: the index, code at the address on one of
 * the multisig's networks, or a passkey approval make it a Vela account.
 */
import type { KindEvidence } from './signers.svelte';
import type { MemberInfo } from './store.svelte';

/** A Vela account (its key a passkey) or an outside wallet, as kept with a member. */
export type MemberKind = NonNullable<MemberInfo['kind']>;

export interface KindChoice {
	/** What the choice shows picked; `null`: nothing — the person is asked. */
	kind: MemberKind | null;
	/** Evidence says a Vela account: shown, not asked. */
	locked: boolean;
	/**
	 * The line under it: `known` (evidence), `likely` (pre-filled "Other
	 * wallet", for the person to confirm), `ask` (it can't tell), or `null`
	 * (the person's own word).
	 */
	note: 'known' | 'likely' | 'ask' | null;
}

/**
 * What a member's kind choice shows, from what the wallet can tell and what
 * the person said (here, or on this device before — `said`).
 */
export function kindChoice(evidence: KindEvidence, said: MemberKind | undefined): KindChoice {
	if (evidence === 'vela') return { kind: 'passkey', locked: true, note: 'known' };
	if (said) return { kind: said, locked: false, note: null };
	if (evidence === 'wallet') return { kind: 'wallet', locked: false, note: 'likely' };
	return { kind: null, locked: false, note: 'ask' };
}
