/**
 * "AI agent" — whether this wallet answers a remote AI agent through a
 * ClientMCP bridge, and which bridge. Off unless the person turns it on.
 *
 * localStorage, like the analytics switch (`analytics/consent.svelte.ts`) and
 * for its reasons: web-only — the phone and desktop shells carry no bridge yet
 * — and read before anything else on the page needs it. Erase This Device
 * sweeps the `vela.` namespace, so the token goes with everything else.
 */
import { browser } from '$app/environment';

export const AGENT_KEY = 'vela.agent';

export type AgentConfig = {
	enabled: boolean;
	/** The bridge's client endpoint, e.g. `wss://bridge.example.com/connect`. */
	url: string;
	/** A ClientMCP client token for this wallet. */
	token: string;
	/** The agent may submit a transaction for the person's passkey (spec 103). Off unless granted. */
	submitTransactions: boolean;
};

const OFF: AgentConfig = { enabled: false, url: '', token: '', submitTransactions: false };

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
			submitTransactions: raw?.submitTransactions === true
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
	submitTransactions = $state(false);

	/** Read the stored choice (idempotent). */
	boot(): void {
		const config = stored();
		this.enabled = config.enabled;
		this.url = config.url;
		this.token = config.token;
		this.submitTransactions = config.submitTransactions;
	}

	/** Whether the bridge can be dialled with what is entered. */
	get ready(): boolean {
		return this.enabled && bridgeUrlProblem(this.url) === null && this.token.trim() !== '';
	}

	set(patch: Partial<AgentConfig>): void {
		if (patch.enabled !== undefined) this.enabled = patch.enabled;
		if (patch.url !== undefined) this.url = patch.url;
		if (patch.token !== undefined) this.token = patch.token;
		if (patch.submitTransactions !== undefined) this.submitTransactions = patch.submitTransactions;
		if (!browser) return;
		try {
			const config: AgentConfig = {
				enabled: this.enabled,
				url: this.url,
				token: this.token,
				submitTransactions: this.submitTransactions
			};
			localStorage.setItem(AGENT_KEY, JSON.stringify(config));
		} catch {
			/* Blocked storage: the choice holds for this visit. */
		}
	}
}

export const agentSettings = new AgentSettings();
