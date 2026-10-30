/**
 * The multisig links: the one that asks another owner to approve
 * (`…/wallet#ms=<chainId>.<multisig>.<safeTxHash>`) and the invite that adds
 * a multisig (`…/wallet#msa=…`). Kept after `#`, so they never reach a
 * server; any chat app or a QR code can carry them.
 *
 * The codec is the core's (`multisig_link`, `multisigLinkCall`): what a link
 * says, what a damaged one held, how an invite carries its networks and its
 * members, the checksum refusal — and the accounts machine reads every link
 * opened here as one flow (`open_link`). What a page must know BEFORE any
 * wasm (whether a hash is meant as one, the hash to carry, keeping one for
 * later) is `kept-link.ts`'s: Welcome loads no core.
 */
import * as wasm from '../../../../../../rust/pkg-web/vela_core.js';
import type { AddLink } from '../generated/AddLink';
import type { InvitedMember } from '../generated/InvitedMember';
import type { ProposalLink } from '../generated/ProposalLink';

export type { AddLink, InvitedMember, ProposalLink };
export { carriedLinkHash, isMultisigLink } from './kept-link';

/** The longest member label a link carries, in characters (the core's name rule bounds it again). */
export const MEMBER_NAME_MAX = 64;

function linkCall<T>(method: string, args: Record<string, unknown>): T {
	return JSON.parse(wasm.multisigLinkCall(method, JSON.stringify(args))) as T;
}

/**
 * The origin a link made here is for: this page's own when it is a web page
 * anyone can open, else the wallet's public web address — the browser
 * extension's `chrome-extension://…` opens nowhere but in that extension.
 */
export function shareOrigin(origin: string): string {
	return linkCall<string>('shareOrigin', { origin });
}

/** The link that asks another owner to approve `link`'s transaction. */
export function proposalLink(origin: string, locale: string, link: ProposalLink): string {
	return linkCall<string>('proposalLink', { origin, locale, link });
}

/**
 * The link that adds `multisig` (r4D-4): the networks it is used on ride with
 * it, and each member the inviter knows (r4D-5) — all in the fragment.
 */
export function addLink(
	origin: string,
	locale: string,
	multisig: string,
	extra: { chainIds?: number[]; members?: InvitedMember[] } = {}
): string {
	return linkCall<string>('addLink', { origin, locale, multisig, extra });
}

/**
 * r4C-13: the connect page's link for members whose wallets are elsewhere —
 * `members` (any, when none is named) and the transaction they approve
 * (`tx`), and the live session's invite when there is one.
 */
export function connectLink(
	page: string,
	invite: string | null,
	members: string[],
	tx: ProposalLink | null = null
): string {
	return linkCall<string>('connectLink', { page, invite, members, tx });
}
