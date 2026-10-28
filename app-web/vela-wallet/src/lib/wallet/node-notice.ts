/**
 * The one-time notice that a node of the person's own is not answering
 * (owner decision 2026-10-28): "Your node for Gnosis isn't answering; using
 * public nodes for now", with the way to that network's RPC.
 *
 * Which chains fall back is the core's (`RpcPoolView.fallback_chains`). What
 * is decided here is only the notice's manners: once per chain per browser
 * session — a tab reloaded does not say it again — and taken back when the
 * chain's own node answers again.
 */

const SESSION_KEY = 'vela.nodeFallbackNoticed';
const TOPIC_PREFIX = 'node-fallback:';

/** The toast topic of a chain's notice — how it is taken back. */
export function nodeNoticeTopic(chainId: number): string {
	return `${TOPIC_PREFIX}${chainId}`;
}

/** The first chain falling back that has not been noticed this session. */
export function nodeFallbackToNotice(
	fallback: readonly number[],
	noticed: ReadonlySet<number>
): number | null {
	return fallback.find((chainId) => !noticed.has(chainId)) ?? null;
}

/** Is `topic` a node notice for a chain that is no longer falling back? */
export function staleNodeNotice(topic: string | undefined, fallback: readonly number[]): boolean {
	if (topic === undefined || !topic.startsWith(TOPIC_PREFIX)) return false;
	return !fallback.includes(Number(topic.slice(TOPIC_PREFIX.length)));
}

/** The chains noticed this session (none when session storage is unreadable). */
export function readNoticed(): Set<number> {
	try {
		const raw = sessionStorage.getItem(SESSION_KEY);
		const parsed: unknown = raw === null ? [] : JSON.parse(raw);
		return new Set(Array.isArray(parsed) ? parsed.filter((v) => Number.isInteger(v)) : []);
	} catch {
		return new Set();
	}
}

export function writeNoticed(noticed: ReadonlySet<number>): void {
	try {
		sessionStorage.setItem(SESSION_KEY, JSON.stringify([...noticed]));
	} catch {
		// best-effort: a notice said twice beats one never said
	}
}
