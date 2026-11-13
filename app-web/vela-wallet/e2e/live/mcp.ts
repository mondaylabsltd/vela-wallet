/**
 * An AI agent, as a real one reaches the wallet: over MCP (Streamable HTTP,
 * the 2025-06-18 revision) through a ClientMCP bridge the wallet's tab is
 * connected to. Nothing here is the wallet's code: the bridge is
 * `clientmcp serve` (LIVE_MCP_BRIDGE), the tokens come from `clientmcp token`
 * (MCP_CLIENT_TOKEN for the wallet, MCP_AGENT_TOKEN for the agent).
 */
import type { Page } from '@playwright/test';

export const MCP_BRIDGE = process.env.LIVE_MCP_BRIDGE ?? 'http://127.0.0.1:8095';
const CLIENT_TOKEN = process.env.MCP_CLIENT_TOKEN ?? '';
const AGENT_TOKEN = process.env.MCP_AGENT_TOKEN ?? '';
export const mcpReady = CLIENT_TOKEN !== '' && AGENT_TOKEN !== '';

export type ToolResult = {
	isError?: boolean;
	content?: { type: string; text?: string }[];
	structuredContent?: Record<string, unknown>;
};

/** Settings → AI agent, as the person leaves it: on, this bridge, these wallets allowed. */
export async function linkAgent(page: Page, wallets: string[]): Promise<void> {
	await page.evaluate(
		({ url, token, wallets }) =>
			localStorage.setItem('vela.agent', JSON.stringify({ enabled: true, url, token, wallets })),
		{ url: `${MCP_BRIDGE.replace(/^http/, 'ws')}/connect`, token: CLIENT_TOKEN, wallets }
	);
}

export class Agent {
	#id = 0;
	#session: string | null = null;

	async #rpc(method: string, params?: unknown): Promise<unknown> {
		const res = await fetch(`${MCP_BRIDGE}/mcp`, {
			method: 'POST',
			headers: {
				authorization: `Bearer ${AGENT_TOKEN}`,
				'content-type': 'application/json',
				accept: 'application/json',
				'mcp-protocol-version': '2025-06-18',
				...(this.#session ? { 'mcp-session-id': this.#session } : {})
			},
			body: JSON.stringify({ jsonrpc: '2.0', id: ++this.#id, method, params }),
			// Past the bridge's own call timeout (120 s): a call never hangs a test.
			signal: AbortSignal.timeout(130_000)
		});
		this.#session = res.headers.get('mcp-session-id') ?? this.#session;
		const answer = (await res.json()) as { result?: unknown; error?: { message: string } };
		if (answer.error) throw new Error(`${method}: ${answer.error.message}`);
		return answer.result;
	}

	async initialize(): Promise<{ instructions?: string }> {
		return (await this.#rpc('initialize', {
			protocolVersion: '2025-06-18',
			capabilities: {},
			clientInfo: { name: 'vela-live-agent', version: '1' }
		})) as { instructions?: string };
	}

	async tools(): Promise<string[]> {
		const listed = (await this.#rpc('tools/list')) as { tools: { name: string }[] };
		return listed.tools.map((t) => t.name);
	}

	async call(name: string, args: Record<string, unknown> = {}): Promise<ToolResult> {
		return (await this.#rpc('tools/call', { name, arguments: args })) as ToolResult;
	}

	/** A tool's answer as data: its structured content, else its text read as JSON. */
	async data(name: string, args: Record<string, unknown> = {}): Promise<Record<string, unknown>> {
		const result = await this.call(name, args);
		if (result.structuredContent) return { ...result.structuredContent, isError: result.isError };
		const text = result.content?.find((c) => c.type === 'text')?.text ?? '{}';
		try {
			return { ...(JSON.parse(text) as Record<string, unknown>), isError: result.isError };
		} catch {
			return { text, isError: result.isError };
		}
	}
}
