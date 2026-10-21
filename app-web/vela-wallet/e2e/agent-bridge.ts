/**
 * The agent's side of a ClientMCP bridge, played in a test (spec 103): the
 * wallet dials what Settings says, registers its tools, and calls arrive as a
 * remote agent's would — `page.routeWebSocket` stands where the bridge is.
 */
import { expect, type Page, type WebSocketRoute } from '@playwright/test';

export const BRIDGE = 'ws://localhost:18999/connect';

/** Settings → AI agent as the person leaves it: on, and this bridge. */
export async function enableAgent(page: Page): Promise<void> {
	await page.evaluate(
		(url) =>
			localStorage.setItem(
				'vela.agent',
				JSON.stringify({ enabled: true, url, token: 'e2e-token' })
			),
		BRIDGE
	);
}

type ToolResult = { isError?: boolean; structuredContent?: Record<string, unknown> };

/** The bridge, played: the wallet's registration, and calls as an agent makes them. */
export async function playBridge(page: Page) {
	let socket: WebSocketRoute | null = null;
	let tools: string[] = [];
	let next = 0;
	const waiting = new Map<string, (result: ToolResult) => void>();
	await page.routeWebSocket(BRIDGE, (ws) => {
		socket = ws;
		ws.onMessage((raw) => {
			const message = JSON.parse(String(raw)) as {
				type: string;
				id?: string;
				tools?: { name: string }[];
				result?: ToolResult;
			};
			if (message.type === 'ping') ws.send('{"type":"pong"}');
			if (message.type === 'register') {
				tools = (message.tools ?? []).map((tool) => tool.name);
				ws.send('{"type":"registered"}');
			}
			if (message.type === 'result' && message.id) waiting.get(message.id)?.(message.result ?? {});
		});
	});
	const send = (message: object) => {
		expect(socket, 'the wallet dialled the bridge').not.toBeNull();
		socket!.send(JSON.stringify(message));
	};
	return {
		tools: () => tools,
		/** Start a call and leave it running; `cancel()` is the bridge giving up on it. */
		start(name: string, args: Record<string, unknown> = {}) {
			const id = `call-${++next}`;
			let settled = false;
			waiting.set(id, () => (settled = true));
			send({ type: 'call', id, name, arguments: args });
			return { cancel: () => send({ type: 'cancel', id }), answered: () => settled };
		},
		async call(name: string, args: Record<string, unknown> = {}) {
			expect(socket, 'the wallet dialled the bridge').not.toBeNull();
			const id = `call-${++next}`;
			const answered = new Promise<ToolResult>((resolve) => waiting.set(id, resolve));
			socket!.send(JSON.stringify({ type: 'call', id, name, arguments: args }));
			const result = await answered;
			return { error: result.isError === true, ...(result.structuredContent ?? {}) } as {
				error: boolean;
				status?: string;
				operation?: string;
				transfer?: { id: string; state: string; user_op_hash: string | null };
				closed_receipt?: string;
				reason?: { code: string; detail: string };
				[key: string]: unknown;
			};
		}
	};
}
