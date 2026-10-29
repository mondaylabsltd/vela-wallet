/**
 * The connect page's link (ms-14):
 * `/connect#<invite>&for=<member>,<member>&ms=<chainId>.<multisig>.<safeTxHash>`.
 *
 * Everything it carries rides in the fragment — the PeerConnect invite (a
 * bearer credential until paired), the members Vela asks for, and the
 * transaction — so none of it reaches a server's logs or a referrer. The
 * members let the page say which account is connecting and refuse, loudly,
 * one that is not among them; Vela checks every signature itself whatever
 * the page says.
 *
 * r4C-2: the transaction (`ms=`, the proposal link's own shape) lets the page
 * approve it on its own — read from the mailbox, re-hashed and reviewed by the
 * core, signed in the wallet, checked, and handed to the mailbox — with no
 * Vela on the other end. A link with only `ms=` and `for=` (no invite) is the
 * approve link a proposal shows for an outside member.
 *
 * An older link (`/connect#<invite>`, or with `for=`) still opens: the page
 * then has no transaction to read, and waits for Vela as before.
 */
import type { ProposalLink } from '../core/link';

const FOR = 'for=';
const TX = 'ms=';
const ADDRESS = /^0x[0-9a-fA-F]{40}$/;
const INVITE = /^wss?:\/\/[^#]+#[A-Za-z0-9_-]{86}$/;

/**
 * The page to open: `invite` for a new room (none for one paired before, or
 * an approve link), whom it is for, and the transaction (`tx`) when the
 * mailbox holds it.
 */
export function connectLink(
	page: string,
	invite: string | null,
	members: string[],
	tx: ProposalLink | null = null
): string {
	const parts: string[] = [];
	if (invite) parts.push(encodeURIComponent(invite));
	const valid = members.filter((a) => ADDRESS.test(a)).map((a) => a.toLowerCase());
	if (valid.length > 0) parts.push(`${FOR}${[...new Set(valid)].join(',')}`);
	if (tx && readTx(`${tx.chainId}.${tx.multisig}.${tx.safeTxHash}`)) {
		parts.push(`${TX}${tx.chainId}.${tx.multisig.toLowerCase()}.${tx.safeTxHash.toLowerCase()}`);
	}
	return parts.length > 0 ? `${page}#${parts.join('&')}` : page;
}

export interface ConnectHash {
	/** The room's invite, or `null` (a page reopened, or none that reads as one). */
	invite: string | null;
	/** The members Vela asked for, lowercase; empty when the link named none. */
	members: string[];
	/** The transaction to approve from the page itself; `null` for an older link. */
	tx: ProposalLink | null;
}

function readTx(body: string): ProposalLink | null {
	const [chain, multisig, safeTxHash] = body.split('.');
	if (!/^[1-9]\d{0,15}$/.test(chain ?? '')) return null;
	const chainId = Number(chain);
	if (!Number.isSafeInteger(chainId)) return null;
	if (!ADDRESS.test(multisig ?? '')) return null;
	if (!/^0x[0-9a-fA-F]{64}$/.test(safeTxHash ?? '')) return null;
	return { chainId, multisig: multisig.toLowerCase(), safeTxHash: safeTxHash.toLowerCase() };
}

/** What a connect page's fragment holds. Anything that does not read is dropped, never guessed. */
export function readConnectHash(hash: string): ConnectHash {
	const raw = hash.replace(/^#/, '');
	let invite: string | null = null;
	let members: string[] = [];
	let tx: ProposalLink | null = null;
	for (const part of raw ? raw.split('&') : []) {
		if (part.startsWith(FOR)) {
			members = part
				.slice(FOR.length)
				.split(',')
				.filter((a) => ADDRESS.test(a))
				.map((a) => a.toLowerCase());
			continue;
		}
		if (part.startsWith(TX)) {
			tx = readTx(part.slice(TX.length));
			continue;
		}
		try {
			const decoded = decodeURIComponent(part);
			if (INVITE.test(decoded)) invite = decoded;
		} catch {
			// not an invite
		}
	}
	return { invite, members: [...new Set(members)], tx };
}

/** Whether `account` is one of `members` — any account, when the link named none. */
export function isExpected(members: string[], account: string): boolean {
	return members.length === 0 || members.includes(account.toLowerCase());
}
