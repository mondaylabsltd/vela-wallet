/**
 * The connect page's link (ms-14): `/connect#<invite>&for=<member>,<member>`.
 *
 * Everything it carries rides in the fragment — the PeerConnect invite (a
 * bearer credential until paired) and the members Vela asks for — so none of
 * it reaches a server's logs or a referrer. The members let the page say
 * which account is connecting and refuse, loudly, one that is not among
 * them; Vela checks every signature itself whatever the page says.
 *
 * A link from before the members rode along (`/connect#<invite>`) still
 * opens: the page then has no one to check against, and says nothing about it.
 */

const FOR = 'for=';
const ADDRESS = /^0x[0-9a-fA-F]{40}$/;
const INVITE = /^wss?:\/\/[^#]+#[A-Za-z0-9_-]{86}$/;

/** The page to open: `invite` for a new room (none for one paired before), and whom it is for. */
export function connectLink(page: string, invite: string | null, members: string[]): string {
	const parts: string[] = [];
	if (invite) parts.push(encodeURIComponent(invite));
	const valid = members.filter((a) => ADDRESS.test(a)).map((a) => a.toLowerCase());
	if (valid.length > 0) parts.push(`${FOR}${[...new Set(valid)].join(',')}`);
	return parts.length > 0 ? `${page}#${parts.join('&')}` : page;
}

export interface ConnectHash {
	/** The room's invite, or `null` (a page reopened, or none that reads as one). */
	invite: string | null;
	/** The members Vela asked for, lowercase; empty when the link named none. */
	members: string[];
}

/** What a connect page's fragment holds. Anything that does not read is dropped, never guessed. */
export function readConnectHash(hash: string): ConnectHash {
	const raw = hash.replace(/^#/, '');
	let invite: string | null = null;
	let members: string[] = [];
	for (const part of raw ? raw.split('&') : []) {
		if (part.startsWith(FOR)) {
			members = part
				.slice(FOR.length)
				.split(',')
				.filter((a) => ADDRESS.test(a))
				.map((a) => a.toLowerCase());
			continue;
		}
		try {
			const decoded = decodeURIComponent(part);
			if (INVITE.test(decoded)) invite = decoded;
		} catch {
			// not an invite
		}
	}
	return { invite, members: [...new Set(members)] };
}

/** Whether `account` is one of `members` — any account, when the link named none. */
export function isExpected(members: string[], account: string): boolean {
	return members.length === 0 || members.includes(account.toLowerCase());
}
