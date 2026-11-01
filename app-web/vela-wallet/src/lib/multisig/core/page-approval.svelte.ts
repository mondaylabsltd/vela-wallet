/**
 * A multisig member's approval on the member's trusted signing page
 * (2026-10-29, the owner's Bybit lesson: a signer must be able to review and
 * sign on a surface independent of the wallet app's code).
 *
 * When the member's account reviews and signs on a page
 * (`multisigApprovalRoute` — the venue the person chose), the approval is not
 * the wallet's passkey prompt. The wallet:
 *
 * 1. checks the page as every Vela app does (spec 076/102 R6: the core picks
 *    the version — `trustedSignerPageTarget` — this fetches exactly that URL
 *    as a browser navigates to it, hashes the bytes, and the core rules —
 *    `trustedSignerPageAdmit`). A page that does not match is not opened, and
 *    the person reads why (the core's integrity line);
 * 2. shows the hand-off card; Open opens the checked version in a window of
 *    its own (a tap: a browser opens a window only for one), and once the page
 *    says it is ready, hands it the request over `postMessage` — the raw
 *    SafeTx, the multisig, the chain, the member, and the core's safeTxHash
 *    for the page to compare (`trustedSignerApprovalRequest`);
 * 3. accepts the page's answer only as the core accepts it
 *    (`trustedSignerApprovalAccept`): a user-verified assertion over exactly
 *    the approval bytes the core computed, by one of the member's keys, that
 *    verifies the way the chain will. Nothing the page says about what it
 *    signed is read.
 *
 * The page decodes the SafeTx itself, from its fields: what the person
 * approves there does not depend on this file being honest about it.
 *
 * Messages come only from the window this opened, from the page's own origin
 * (`event.source`, `event.origin` — the browser's facts), and go only there.
 */
import type { Account } from '$lib/core/generated/Account';
import type { KeyLabel } from '$lib/core/generated/KeyLabel';
import { trustedSignerApprovalAccept, trustedSignerApprovalRequest } from '$lib/core/kernels';
import { chainName, nativeSymbol } from '$lib/services/networks';
import {
	browserDeps,
	handOff,
	PageHandoffError,
	type PageHandoffDeps,
	type PageHandoffFailure
} from '$lib/signing/page-handoff.svelte';
import type { PasskeyApproval, SafeTx } from './kernel';

// The hand-off machinery is every signature's (`$lib/signing/page-handoff`,
// 2026-10-30); an approval is one kind of it. The names it had here stay.
export {
	browserDeps,
	checkPage,
	pageHandoff,
	type PageHandoff
} from '$lib/signing/page-handoff.svelte';
export { PageHandoffError as PageApprovalError };
export type PageApprovalFailure = PageHandoffFailure;
export type PageApprovalDeps = PageHandoffDeps;

export interface PageApprovalInput {
	/** The member's record as stored. */
	member: Account;
	/** The page the member reviews and signs on (its base address). */
	page: string;
	chainId: number;
	multisig: string;
	/** The multisig's name on this device: the page marks it "named by the app". */
	multisigName?: string | null;
	/** r9-10: the one review asks for "… anyway" (`PasskeyPurpose.anyway`). */
	anyway?: boolean;
	/**
	 * r10 page: the multisig's members and the approvals it needs now
	 * (`PasskeyPurpose.owners` / `.threshold`) — a member change reads
	 * before → after on the page, the "before" marked as the app's word.
	 */
	owners?: string[];
	threshold?: number | null;
	tx: SafeTx;
	/** The safeTxHash the core computed: what the approval is FOR. */
	safeTxHash: string;
	/** The member's founding keys. */
	keys: { credentialId: string; publicKeyHex: string }[];
	/** The member's key route (the core's `SigningPlan.key`), when the record names one. */
	keyRoute?: unknown;
	/** Which key confirms it, for the card's key row (`SigningPlan.key_label`). */
	keyLabel?: KeyLabel | null;
	/** The withdrawn proposal: nothing more is asked. */
	signal?: AbortSignal;
}

/**
 * The member approves `safeTxHash` on its page: the check, the card, the page,
 * the answer — and back, the approval the core accepted.
 */
/**
 * What a page answered, for this page's life, by member, chain and
 * safeTxHash: the same bytes approved twice are one approval, and the person
 * is not asked again.
 */
// Not reactive state: nothing draws it, it is only looked up.
// eslint-disable-next-line svelte/prefer-svelte-reactivity
const answered = new Map<string, PasskeyApproval>();
const answerKey = (input: Pick<PageApprovalInput, 'member' | 'chainId' | 'safeTxHash'>) =>
	`${input.member.address.toLowerCase()}:${input.chainId}:${input.safeTxHash.toLowerCase()}`;

export async function approveOnPage(
	input: PageApprovalInput,
	deps: PageApprovalDeps = browserDeps
): Promise<PasskeyApproval> {
	const kept = answered.get(answerKey(input));
	if (kept) return kept;
	const approval = await handOffApproval(input, deps);
	answered.set(answerKey(input), approval);
	return approval;
}

/** Forget what the pages answered (tests). */
export function forgetPageAnswers(): void {
	answered.clear();
}

function handOffApproval(
	input: PageApprovalInput,
	deps: PageApprovalDeps
): Promise<PasskeyApproval> {
	const request = trustedSignerApprovalRequest({
		chainId: input.chainId,
		chainName: chainName(input.chainId) || null,
		nativeSymbol: nativeSymbol(input.chainId) || null,
		multisig: input.multisig,
		multisigName: input.multisigName ?? null,
		tx: input.tx,
		member: input.member.address,
		memberName: input.member.name,
		credentialIdsHex: input.keys.map((k) => k.credentialId),
		keyRoute: input.keyRoute,
		anyway: input.anyway === true,
		owners: input.owners ?? [],
		threshold: input.threshold ?? null
	});
	return handOff<PasskeyApproval>(
		{
			kind: 'approval',
			page: input.page,
			signer: input.member.name,
			keyLabel: input.keyLabel ?? null,
			request,
			accept: (result) =>
				trustedSignerApprovalAccept<PasskeyApproval>({
					result,
					chainId: input.chainId,
					member: input.member.address,
					safeTxHash: input.safeTxHash,
					keys: input.keys
				}),
			signal: input.signal
		},
		deps
	);
}
