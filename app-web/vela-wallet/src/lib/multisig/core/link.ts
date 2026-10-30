/**
 * The link that asks another owner to approve: `…/wallet#ms=<chainId>.<multisig>.<safeTxHash>`.
 *
 * It names the transaction, nothing more. Whoever opens it reads the
 * transaction from the mailbox, re-hashes it and checks it is the one named —
 * and adds the multisig by its address first if this device does not have it.
 * Kept after `#`, so it never reaches a server; any chat app or a QR code can
 * carry it.
 */
import { SITE_ORIGIN } from '$lib/site';
// The same EIP-55 as the core's, without the wasm: a link is read before it loads.
import { eip55 } from '../external/eip55';

/**
 * The origin a link made here is for: this page's own when it is a web page
 * anyone can open, else the wallet's public web address — the browser
 * extension's `chrome-extension://…` opens nowhere but in that extension, so
 * a link to another member (or to the connect page) names the web wallet.
 */
export function shareOrigin(origin: string): string {
	return /^https?:\/\//.test(origin) ? origin : SITE_ORIGIN;
}

export interface ProposalLink {
	chainId: number;
	multisig: string;
	safeTxHash: string;
}

const PREFIX = 'ms=';

export function proposalLink(origin: string, locale: string, link: ProposalLink): string {
	return `${origin}/${locale}/wallet#${PREFIX}${link.chainId}.${link.multisig}.${link.safeTxHash}`;
}

/**
 * A hash as the link meant it: `#msa%3D…` / `#ms%3D…` (an app that encoded
 * the `=`, r4b-17) reads as `#msa=…` / `#ms=…`, and spaces a chat app or a
 * mail client put around it are dropped. Anything else as given — an
 * invite's member labels stay encoded until they are read one by one.
 */
function linkBody(hash: string): string {
	const body = (hash.startsWith('#') ? hash.slice(1) : hash).trim();
	if (!/^msa?%3d/i.test(body)) return body;
	try {
		return decodeURIComponent(body);
	} catch {
		return body;
	}
}

/** The link in a location hash, or `null`. */
export function parseProposalLink(hash: string): ProposalLink | null {
	const body = linkBody(hash);
	if (!body.startsWith(PREFIX)) return null;
	const [chain, multisig, safeTxHash, ...rest] = body.slice(PREFIX.length).split('.');
	if (rest.length > 0) return null;
	if (!/^[1-9]\d{0,15}$/.test(chain ?? '')) return null;
	const chainId = Number(chain);
	if (!Number.isSafeInteger(chainId)) return null;
	if (!/^0x[0-9a-fA-F]{40}$/.test(multisig ?? '')) return null;
	if (!/^0x[0-9a-fA-F]{64}$/.test(safeTxHash ?? '')) return null;
	return { chainId, multisig, safeTxHash };
}

/**
 * ms-22: a hash that is meant as one of these links (`#ms=` / `#msa=`) but
 * does not read as one — cut short, or damaged on the way. Said, never
 * ignored.
 */
export function brokenLink(hash: string): 'proposal' | 'add' | null {
	const body = linkBody(hash);
	if (body.startsWith(PREFIX) && !parseProposalLink(hash)) return 'proposal';
	if (body.startsWith(ADD_PREFIX) && !parseAddLink(hash)) return 'add';
	return null;
}

/**
 * r4b-17: the hash to carry to the wallet when a multisig link opened another
 * page (`/en#msa=…`, the site's root): the link itself, or `''`.
 */
export function carriedLinkHash(hash: string): string {
	const body = linkBody(hash);
	return isMultisigLink(body) ? `#${body}` : '';
}

/** Whether `hash` is meant as one of these links at all, readable or not. */
export function isMultisigLink(hash: string): boolean {
	const body = linkBody(hash);
	return body.startsWith(PREFIX) || body.startsWith(ADD_PREFIX);
}

/** What a damaged link held, for the add sheet to show beside what is wrong with it. */
export function brokenLinkText(hash: string): string {
	const body = linkBody(hash).replace(/^msa?=/, '');
	try {
		return decodeURIComponent(body);
	} catch {
		return body;
	}
}

const ADD_PREFIX = 'msa=';
/** Founding members in the genesis's order, no addresses (links from before r6). */
const MEMBERS = 'm=';
/** r4D-5: every member the inviter knows, each by its address. */
const ADDRESSED = 'a=';

/**
 * What the inviter knows of a member: a label and its kind — a Vela account
 * (a passkey), an outside wallet or a smart contract account — and, in a
 * link made since r6, its
 * address. Labels only: who may act is the multisig's genesis and each
 * chain's owners, never this.
 */
export interface InvitedMember {
	/** Its address (r4D-5, r5reg-24); absent in a link from before they rode along. */
	address?: string;
	name?: string;
	kind?: 'passkey' | 'wallet' | 'contract';
}

/** An invite (r4D-4, r4D-5): the multisig, the networks it is used on, what the inviter knows of its members. */
export interface AddLink {
	multisig: string;
	/** Its networks, the one it was first used on first; empty in a link from before they rode along. */
	chainIds: number[];
	/**
	 * Each member the inviter knows, by address — or, in a link from before
	 * r6, one per founding member in the genesis's order (no address); `null`
	 * when the link carries none (or none that reads — they are labels,
	 * dropped, never guessed at).
	 */
	members: InvitedMember[] | null;
}

/**
 * Whether `address` (`0x` + 40 hex) is written as an address is: all one
 * case, or its EIP-55 checksum (r5reg-22). A link whose letters were
 * changed on the way is refused, never read as another address quietly.
 */
export function checksumHolds(address: string): boolean {
	const hex = address.slice(2);
	if (hex === hex.toLowerCase() || hex === hex.toUpperCase()) return true;
	return eip55(address) === address;
}

/** The longest member label a link carries, in characters (the core's name rule bounds it again). */
export const MEMBER_NAME_MAX = 64;
/** r7: `c` — a smart contract account; `p` and `w` read as they always did. */
const KIND_CODE = { passkey: 'p', wallet: 'w', contract: 'c' } as const;

function memberCode(member: InvitedMember): string {
	const kind = member.kind ? KIND_CODE[member.kind] : '-';
	const name = (member.name ?? '').trim().slice(0, MEMBER_NAME_MAX);
	const at = member.address ? member.address.slice(2).toLowerCase() : '';
	return at + kind + encodeURIComponent(name);
}

/** `m=` (by the genesis's order) or, `addressed`, `a=` (each by its address). */
function readMembers(raw: string, addressed: boolean): InvitedMember[] | null {
	const out: InvitedMember[] = [];
	for (const whole of raw.split(',')) {
		let part = whole;
		const member: InvitedMember = {};
		if (addressed) {
			const hex = part.slice(0, 40);
			if (!/^[0-9a-f]{40}$/.test(hex)) return null;
			member.address = eip55(`0x${hex}`);
			part = part.slice(40);
		}
		const kind = part[0];
		if (kind !== 'p' && kind !== 'w' && kind !== 'c' && kind !== '-') return null;
		let name: string;
		try {
			name = decodeURIComponent(part.slice(1)).trim();
		} catch {
			return null;
		}
		if (name.length > MEMBER_NAME_MAX) return null;
		if (name) member.name = name;
		if (kind === 'p') member.kind = 'passkey';
		if (kind === 'w') member.kind = 'wallet';
		if (kind === 'c') member.kind = 'contract';
		out.push(member);
	}
	return out;
}

/**
 * The link that adds a multisig (r4D-4):
 * `…/wallet#msa=<chainId>,<chainId>:<multisig>&a=<member>,<member>`. The
 * networks it is used on ride with it — Safe{Wallet}'s links name theirs —
 * so whoever opens it is never asked to guess one; and, when the inviter
 * knows them, each member's address (40 hex, lowercase), its kind — `p` a
 * Vela account, `w` an outside wallet, `c` a smart contract account (r7), `-`
 * not known — and its label,
 * URI-encoded: the founding members and those added since (r4D-5), and the
 * one who opens it is told which is them (r5reg-24). A member given without
 * an address is written the old way (`m=`, in the genesis's order). All in
 * the fragment: none of it reaches a server's logs or a referrer.
 */
export function addLink(
	origin: string,
	locale: string,
	multisig: string,
	extra: { chainIds?: number[]; members?: InvitedMember[] } = {}
): string {
	const chains = [...new Set(extra.chainIds ?? [])].filter(
		(id) => Number.isSafeInteger(id) && id > 0
	);
	let body = `${ADD_PREFIX}${chains.length > 0 ? `${chains.join(',')}:` : ''}${multisig}`;
	const members = extra.members ?? [];
	const addressed = members.length > 0 && members.every((m) => !!m.address);
	if (addressed) {
		body += `&${ADDRESSED}${members.map(memberCode).join(',')}`;
	} else if (members.some((m) => m.name?.trim() || m.kind)) {
		body += `&${MEMBERS}${members.map((m) => memberCode({ ...m, address: undefined })).join(',')}`;
	}
	return `${origin}/${locale}/wallet#${body}`;
}

/**
 * Why an invite does not read, or `null` when it does (or is no invite):
 * `checksum` — its address's letters were changed on the way (r5reg-22);
 * `damaged` — part of it is missing or wrong.
 */
export function addLinkFault(hash: string): 'checksum' | 'damaged' | null {
	const body = linkBody(hash);
	if (!body.startsWith(ADD_PREFIX) || parseAddLink(hash)) return null;
	const head = body.slice(ADD_PREFIX.length).split('&')[0];
	const address = head.slice(head.lastIndexOf(':') + 1);
	return /^0x[0-9a-fA-F]{40}$/.test(address) && !checksumHolds(address) ? 'checksum' : 'damaged';
}

/**
 * The invite a location hash holds, or `null`. A link from before the
 * networks rode along (`#msa=<multisig>`) still reads, naming none: the
 * person is asked then. Members that do not read are dropped; the rest of
 * the link still opens. An address whose letter case is not its checksum
 * does not read (r5reg-22): a link changed on the way is never opened.
 */
export function parseAddLink(hash: string): AddLink | null {
	const body = linkBody(hash);
	if (!body.startsWith(ADD_PREFIX)) return null;
	const [head, ...rest] = body.slice(ADD_PREFIX.length).split('&');
	const colon = head.lastIndexOf(':');
	const address = colon === -1 ? head : head.slice(colon + 1);
	if (!/^0x[0-9a-fA-F]{40}$/.test(address) || !checksumHolds(address)) return null;
	let chainIds: number[] = [];
	if (colon !== -1) {
		const ids = head.slice(0, colon).split(',');
		if (!ids.every((id) => /^[1-9]\d{0,15}$/.test(id))) return null;
		chainIds = [...new Set(ids.map(Number))];
		if (!chainIds.every(Number.isSafeInteger)) return null;
	}
	const addressed = rest.find((part) => part.startsWith(ADDRESSED));
	const listed = rest.find((part) => part.startsWith(MEMBERS));
	const members = addressed
		? readMembers(addressed.slice(ADDRESSED.length), true)
		: listed
			? readMembers(listed.slice(MEMBERS.length), false)
			: null;
	return { multisig: address, chainIds, members };
}
