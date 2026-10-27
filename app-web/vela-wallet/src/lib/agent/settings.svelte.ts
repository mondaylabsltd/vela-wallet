/**
 * "AI agent" — whether this wallet answers a remote AI agent through a
 * ClientMCP bridge, and which bridge. Off unless the person turns it on.
 *
 * localStorage, like the analytics switch (`analytics/consent.svelte.ts`) and
 * for its reasons: read before anything else on the page needs it. Erase This
 * Device sweeps the `vela.` namespace, so the token goes with everything else.
 */
import { browser } from '$app/environment';

export const AGENT_KEY = 'vela.agent';

export type AgentConfig = {
	enabled: boolean;
	/** The bridge's client endpoint, e.g. `wss://bridge.example.com/connect`. */
	url: string;
	/** A ClientMCP client token for this wallet. */
	token: string;
	/** Spec 106: whose the token is, when clientmcp.dev issued it — not a secret. */
	linked: AgentLinked | null;
	/**
	 * Spec 106: the MCP server address clientmcp.dev gave for the person's AI
	 * agent, its token in the path — a credential, kept as the client token is.
	 * Empty unless `linked`.
	 */
	agentUrl: string;
	/** Spec 107 E8: the wallets the agent may use; none unless the person says so. */
	wallets: string[];
};

export type AgentLinked = { site: string; login: string; clientId: string };

const OFF: AgentConfig = {
	enabled: false,
	url: '',
	token: '',
	linked: null,
	agentUrl: '',
	wallets: []
};

function linkedOf(raw: unknown): AgentLinked | null {
	const l = raw as Partial<AgentLinked> | null | undefined;
	return typeof l?.site === 'string' &&
		typeof l.login === 'string' &&
		typeof l.clientId === 'string'
		? { site: l.site, login: l.login, clientId: l.clientId }
		: null;
}

function stored(): AgentConfig {
	if (!browser) return OFF;
	try {
		const raw = JSON.parse(
			localStorage.getItem(AGENT_KEY) ?? 'null'
		) as Partial<AgentConfig> | null;
		return {
			enabled: raw?.enabled === true,
			url: typeof raw?.url === 'string' ? raw.url : '',
			token: typeof raw?.token === 'string' ? raw.token : '',
			linked: linkedOf(raw?.linked),
			agentUrl: typeof raw?.agentUrl === 'string' ? raw.agentUrl : '',
			wallets: Array.isArray(raw?.wallets)
				? raw.wallets.filter((w): w is string => typeof w === 'string')
				: []
		};
	} catch {
		return OFF;
	}
}

/**
 * Why a bridge URL cannot be used, or `null`. Encrypted (`wss:`) everywhere;
 * plain `ws:` only to this machine — the same exception the service endpoints
 * make for a developer's own server (`services/endpoint-admission.ts`).
 */
export function bridgeUrlProblem(url: string): 'invalid' | 'insecure' | null {
	let parsed: URL;
	try {
		// eslint-disable-next-line svelte/prefer-svelte-reactivity -- a parse, read once and dropped
		parsed = new URL(url.trim());
	} catch {
		return 'invalid';
	}
	if (parsed.protocol === 'wss:') return null;
	if (parsed.protocol === 'ws:' && ['localhost', '127.0.0.1', '[::1]'].includes(parsed.hostname)) {
		return null;
	}
	return parsed.protocol === 'ws:' ? 'insecure' : 'invalid';
}

/**
 * The MCP endpoint an agent connects to, derived from the bridge's client
 * endpoint (`wss://host/connect` → `https://host/mcp`), or `null`.
 */
export function mcpEndpoint(url: string): string | null {
	if (bridgeUrlProblem(url) !== null) return null;
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- a parse, read once and dropped
	const parsed = new URL(url.trim());
	parsed.protocol = parsed.protocol === 'wss:' ? 'https:' : 'http:';
	parsed.pathname = parsed.pathname.replace(/\/(connect)?\/?$/, '') + '/mcp';
	parsed.search = '';
	return parsed.toString();
}

class AgentSettings {
	enabled = $state(false);
	url = $state('');
	token = $state('');
	linked = $state<AgentLinked | null>(null);
	agentUrl = $state('');
	wallets = $state<string[]>([]);

	/** Read the stored choice (idempotent). */
	boot(): void {
		const config = stored();
		this.enabled = config.enabled;
		this.url = config.url;
		this.token = config.token;
		this.linked = config.linked;
		this.agentUrl = config.agentUrl;
		this.wallets = config.wallets;
	}

	/** Whether the bridge can be dialled with what is entered. */
	get ready(): boolean {
		return this.enabled && bridgeUrlProblem(this.url) === null && this.token.trim() !== '';
	}

	set(patch: Partial<AgentConfig>): void {
		if (patch.enabled !== undefined) this.enabled = patch.enabled;
		if (patch.url !== undefined) this.url = patch.url;
		if (patch.token !== undefined) this.token = patch.token;
		// A token from anywhere else is no longer clientmcp.dev's.
		if (patch.linked !== undefined) this.linked = patch.linked;
		else if (patch.token !== undefined) this.linked = null;
		if (patch.agentUrl !== undefined) this.agentUrl = patch.agentUrl;
		if (patch.wallets !== undefined) this.wallets = patch.wallets.map((w) => w.trim());
		// The agent's address is clientmcp.dev's, and goes with the link.
		if (this.linked === null) this.agentUrl = '';
		if (!browser) return;
		try {
			const config: AgentConfig = {
				enabled: this.enabled,
				url: this.url,
				token: this.token,
				linked: this.linked,
				agentUrl: this.agentUrl,
				wallets: this.wallets
			};
			localStorage.setItem(AGENT_KEY, JSON.stringify(config));
		} catch {
			/* Blocked storage: the choice holds for this visit. */
		}
	}
}

export const agentSettings = new AgentSettings();
