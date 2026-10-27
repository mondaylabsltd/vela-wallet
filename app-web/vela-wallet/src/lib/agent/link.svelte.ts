/**
 * Spec 106 — connecting the AI agent with clientmcp.dev, on the web.
 *
 * The decisions are the core's (`vela_core::clientmcp_link`); this is the
 * browser's part: randomness, the trip to clientmcp.dev and back in the same
 * tab, and the one request that trades the code.
 *
 *   start()  — keep the pending sign-in in sessionStorage (this tab only) and
 *              go to clientmcp.dev.
 *   /clientmcp (a Worker route) — where clientmcp.dev sends the tab back; it
 *              hands the query to this settings page in the fragment, so the
 *              code never reaches a server log.
 *   finish() — check the return is this sign-in's, trade the code, and hand
 *              the bridge address and token to `apply` (the person's
 *              `configure_agent`).
 */
import { browser } from '$app/environment';
import {
	clientmcpInstallationId,
	clientmcpLinkAccept,
	clientmcpLinkBegin,
	clientmcpLinkExchange,
	clientmcpLinkReturned,
	loadCore
} from '$lib/core/client';
import { agentSettings } from './settings.svelte';

/** clientmcp.dev, unless a development build names another (staging). */
export const CLIENTMCP_SITE: string =
	(import.meta.env.VITE_CLIENTMCP_SITE as string | undefined) || 'https://clientmcp.dev';

/** The return address: this origin's `/clientmcp` (registered on clientmcp.dev). */
export const RETURN_PATH = '/clientmcp';
/** How the `/clientmcp` route hands the query over: `#clientmcp?code=…&state=…`. */
export const RETURN_FRAGMENT = '#clientmcp';

const PENDING_KEY = 'vela.agent.link';
const INSTALLATION_KEY = 'vela.agent.installation';

export type LinkProblem =
	| { kind: 'refused'; detail: string }
	| { kind: 'unavailable' | 'expired' | 'not_ours' | 'malformed' | 'network' };

export type Linked = {
	bridge_url: string;
	client_token: string;
	client_id: string;
	login: string;
	expires_at: number;
	/** The MCP server address for the person's AI agent (a credential). */
	agent_url?: string | null;
};

type Answer<T> = { ok: T } | { error: LinkProblem };

function random(bytes: number): Uint8Array {
	return crypto.getRandomValues(new Uint8Array(bytes));
}

/** This browser's installation ID: made once, kept with the agent's settings. */
function installation(): string {
	try {
		const kept = localStorage.getItem(INSTALLATION_KEY);
		if (kept) return kept;
		const made = clientmcpInstallationId(random(16));
		localStorage.setItem(INSTALLATION_KEY, made);
		return made;
	} catch {
		// Blocked storage: a new installation each time (it still works; signing
		// in again makes another client instead of replacing this one).
		return clientmcpInstallationId(random(16));
	}
}

class AgentLink {
	/** `going`: on the way to clientmcp.dev; `finishing`: back, trading the code. */
	phase = $state<'idle' | 'going' | 'finishing'>('idle');
	problem = $state<LinkProblem | null>(null);

	/**
	 * Off to clientmcp.dev. `back` is this settings page's path, so the return
	 * lands in the person's own language. `label` is what the site lists this
	 * browser as.
	 */
	async start(back: string, label: string): Promise<void> {
		this.problem = null;
		this.phase = 'going';
		try {
			await loadCore();
			const begun = JSON.parse(
				clientmcpLinkBegin(
					CLIENTMCP_SITE,
					location.origin + RETURN_PATH,
					label,
					installation(),
					random(64),
					Date.now()
				)
			) as Answer<{ authorize_url: string }>;
			if ('error' in begun) throw begun.error;
			sessionStorage.setItem(PENDING_KEY, JSON.stringify({ pending: begun.ok, back }));
			location.assign(begun.ok.authorize_url);
		} catch (e) {
			this.phase = 'idle';
			this.problem = isProblem(e) ? e : { kind: 'malformed' };
		}
	}

	/**
	 * Whether this page was opened by clientmcp.dev's return, and the path to
	 * finish it on if not this one (the sign-in started in another language).
	 */
	returned(): { here: boolean; elsewhere: string | null } {
		if (!browser || !location.hash.startsWith(RETURN_FRAGMENT)) {
			return { here: false, elsewhere: null };
		}
		const kept = this.kept();
		const elsewhere = kept && kept.back !== location.pathname ? kept.back + location.hash : null;
		return { here: true, elsewhere };
	}

	/**
	 * Trade the code clientmcp.dev returned with, `apply` the result (the
	 * person's `configure_agent`), then remember whose it is.
	 */
	async finish(apply: (linked: Linked) => Promise<void>): Promise<boolean> {
		const query = location.hash.slice(RETURN_FRAGMENT.length);
		// The code leaves the address bar (and the history) at once.
		history.replaceState(history.state, '', location.pathname + location.search);
		const kept = this.kept();
		try {
			sessionStorage.removeItem(PENDING_KEY);
		} catch {
			/* nothing kept */
		}
		this.problem = null;
		if (kept === null) {
			this.problem = { kind: 'not_ours' };
			return false;
		}
		this.phase = 'finishing';
		try {
			await loadCore();
			const pending = JSON.stringify(kept.pending);
			const code = JSON.parse(
				clientmcpLinkReturned(pending, location.origin + RETURN_PATH + query, Date.now())
			) as Answer<string>;
			if ('error' in code) throw code.error;
			const request = JSON.parse(clientmcpLinkExchange(pending, code.ok)) as Answer<{
				url: string;
				body: string;
			}>;
			if ('error' in request) throw request.error;
			let response: Response;
			try {
				response = await fetch(request.ok.url, {
					method: 'POST',
					headers: { 'content-type': 'application/json' },
					body: request.ok.body,
					credentials: 'omit',
					cache: 'no-store',
					referrerPolicy: 'no-referrer'
				});
			} catch {
				throw { kind: 'network' } satisfies LinkProblem;
			}
			const linked = JSON.parse(
				clientmcpLinkAccept(response.status, await response.text())
			) as Answer<Linked>;
			if ('error' in linked) throw linked.error;
			await apply(linked.ok);
			agentSettings.set({
				linked: {
					site: (kept.pending as { site: string }).site,
					login: linked.ok.login,
					clientId: linked.ok.client_id
				},
				agentUrl: linked.ok.agent_url ?? ''
			});
			return true;
		} catch (e) {
			this.problem = isProblem(e) ? e : { kind: 'malformed' };
			return false;
		} finally {
			this.phase = 'idle';
		}
	}

	private kept(): { pending: unknown; back: string } | null {
		try {
			const raw = JSON.parse(sessionStorage.getItem(PENDING_KEY) ?? 'null') as {
				pending?: unknown;
				back?: unknown;
			} | null;
			return raw && typeof raw.back === 'string' && raw.pending
				? { pending: raw.pending, back: raw.back }
				: null;
		} catch {
			return null;
		}
	}
}

function isProblem(e: unknown): e is LinkProblem {
	return typeof e === 'object' && e !== null && typeof (e as { kind?: unknown }).kind === 'string';
}

export const agentLink = new AgentLink();
