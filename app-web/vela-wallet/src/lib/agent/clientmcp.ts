/**
 * Vendored from the ClientMCP SDK (`clientmcp/sdk/js/src/index.ts`, protocol
 * `clientmcp.v1`, MIT OR Apache-2.0) — one dependency-free file, kept as one
 * file so it can be diffed against its source. Changes from upstream: repo
 * formatting, and `Record<string, unknown>` where upstream typed handler
 * arguments `any`.
 */
/**
 * ClientMCP SDK: expose a running application's functions as MCP tools.
 *
 * The application keeps one outbound WebSocket to a ClientMCP bridge; AI
 * agents call its tools through the bridge's standard MCP endpoint.
 * Protocol: https://github.com/clientmcp/clientmcp/blob/main/SPEC.md
 */

const SUBPROTOCOL = 'clientmcp.v1';
const PING = '{"type":"ping"}';
const PING_INTERVAL = 25_000;
const PONG_TIMEOUT = 10_000;
const MAX_BACKOFF = 30_000;
/** Largest message on the client channel (SPEC §6). */
const MAX_MESSAGE_BYTES = 4 * 1024 * 1024;

/** A JSON Schema for a tool's arguments. */
export interface ObjectSchema {
	type: 'object';
	[keyword: string]: unknown;
}

/** An MCP `CallToolResult`. */
export interface CallToolResult {
	content: unknown[];
	structuredContent?: unknown;
	isError?: boolean;
	_meta?: Record<string, unknown>;
}

export interface CallContext {
	/** The tool being called. */
	name: string;
	/** Aborted when the agent cancels the call or the connection ends. */
	signal: AbortSignal;
}

/**
 * How much a tool's action matters, on the application's own scale (SPEC §9.1):
 * `observe` changes nothing; `operate` is ordinary, reversible use; `manage` is
 * security-relevant or hard to undo; `transact` moves value or signs. The
 * bridge does not read it — the application decides what each level needs.
 */
export type Risk = 'observe' | 'operate' | 'manage' | 'transact';

/** The `_meta` key a tool's risk is published under (SPEC §9.1). */
export const RISK_META = 'io.github.clientmcp/risk';

/** The MCP annotations each risk implies, unless the tool sets its own. */
const RISK_HINTS: Record<Risk, Record<string, boolean>> = {
	observe: { readOnlyHint: true },
	operate: { readOnlyHint: false, destructiveHint: false },
	manage: { readOnlyHint: false, destructiveHint: true },
	transact: { readOnlyHint: false, destructiveHint: true }
};

/**
 * A tool: an MCP tool definition plus the function that runs it.
 *
 * The handler receives arguments written by a language model: validate them.
 * It may return a string (sent as text), any other JSON value (sent as text and,
 * for objects, as `structuredContent`), or `toolResult(...)` for full control.
 * A thrown error becomes a tool error the model can see.
 */
export interface Tool<Args = Record<string, unknown>> {
	title?: string;
	description?: string;
	/** Defaults to `{ type: "object" }`. */
	inputSchema?: ObjectSchema;
	outputSchema?: Record<string, unknown>;
	annotations?: Record<string, unknown>;
	icons?: unknown[];
	_meta?: Record<string, unknown>;
	/** Published in `_meta` and as the annotations it implies (SPEC §9.1). */
	risk?: Risk;
	handler: (args: Args, context: CallContext) => unknown;
}

export type Status = 'connecting' | 'online' | 'offline' | 'closed';

export interface Options {
	/** The bridge's client endpoint, e.g. `wss://bridge.example.com/connect`. */
	url: string;
	/** A client token, or a function returning a fresh one for every connection. */
	token: string | (() => string | Promise<string>);
	/** Guidance for the model on how to use the tools. */
	instructions?: string;
	/** Called whenever the connection status changes; `detail` names the close that caused it. */
	onStatus?: (status: Status, detail?: { code: number; reason: string }) => void;
	/** A WebSocket implementation, for runtimes without a global one. */
	WebSocket?: typeof WebSocket;
}

const RESULT = Symbol('clientmcp.result');

/** Marks a value as a complete `CallToolResult`, sent unchanged. */
export function toolResult(result: CallToolResult): CallToolResult {
	return Object.assign(Object.create(null), result, { [RESULT]: true });
}

/**
 * How a call ended (SPEC §9.2). A function that ran is not yet an operation
 * that was accepted, and an accepted operation is not yet one that succeeded:
 * - `completed` — the change holds (the application has seen it);
 * - `accepted` — it started, and goes on as `operation`, whose state the
 *   application reports elsewhere;
 * - `rejected` — nothing changed, for `reason.code`.
 */
export type Outcome =
	| { status: 'completed'; [key: string]: unknown }
	| { status: 'accepted'; operation: string; [key: string]: unknown }
	| { status: 'rejected'; reason: { code: string; detail?: string } };

function outcome(value: Outcome, isError = false): CallToolResult {
	const result: CallToolResult = {
		content: [{ type: 'text', text: JSON.stringify(value) }],
		structuredContent: value
	};
	if (isError) result.isError = true;
	return toolResult(result);
}

/** The outcome's own members (SPEC §9.2): data merged into it cannot carry them. */
const OUTCOME_MEMBERS = ['status', 'operation', 'reason'];

function merged(data: Record<string, unknown>): Record<string, unknown> {
	const taken = OUTCOME_MEMBERS.find((key) => key in data);
	if (taken)
		throw new TypeError(
			`outcome data cannot carry "${taken}": the outcome's own member (SPEC §9.2)`
		);
	return data;
}

/** The change holds. `data` is merged into the outcome. */
export function completed(data: Record<string, unknown> = {}): CallToolResult {
	return outcome({ ...merged(data), status: 'completed' });
}

/** It started; follow it as `operation` (its id). `data` is merged into the outcome. */
export function accepted(operation: string, data: Record<string, unknown> = {}): CallToolResult {
	return outcome({ ...merged(data), status: 'accepted', operation });
}

/** Nothing changed. `code` is for the agent to act on; `detail` for a reader. */
export function rejected(code: string, detail?: string): CallToolResult {
	return outcome(
		{ status: 'rejected', reason: detail === undefined ? { code } : { code, detail } },
		true
	);
}

/** Close codes after which reconnecting cannot help (SPEC §3.3). */
const FATAL: Record<number, string> = {
	1009: 'message too large',
	4400: 'protocol error',
	4409: 'replaced by a newer connection with the same client ID'
};
const UNAUTHORIZED = 4401;

export class ClientMCP {
	#options: Options;
	#tools = new Map<string, Tool>();
	#instructions: string | undefined;
	#socket: WebSocket | undefined;
	#status: Status = 'offline';
	#running = false;
	#attempt = 0;
	#registerQueued = false;
	#calls = new Map<string, AbortController>();
	#timers: {
		retry?: ReturnType<typeof setTimeout>;
		ping?: ReturnType<typeof setInterval>;
		pong?: ReturnType<typeof setTimeout>;
	} = {};
	#online: { resolve: () => void; reject: (e: Error) => void; promise: Promise<void> } | undefined;

	constructor(options: Options) {
		this.#options = options;
		this.#instructions = options.instructions;
	}

	get status(): Status {
		return this.#status;
	}

	/** Exposes a tool. Returns a function that removes it again. */
	expose<Args = Record<string, unknown>>(name: string, tool: Tool<Args>): () => void {
		this.#tools.set(name, tool as Tool);
		this.#scheduleRegister();
		return () => {
			if (this.#tools.get(name) === tool) this.unexpose(name);
		};
	}

	unexpose(name: string): void {
		if (this.#tools.delete(name)) this.#scheduleRegister();
	}

	set instructions(text: string | undefined) {
		this.#instructions = text;
		this.#scheduleRegister();
	}

	/** Connects, and keeps reconnecting. Resolves once online. */
	connect(): Promise<void> {
		if (!this.#online) {
			let resolve!: () => void, reject!: (e: Error) => void;
			const promise = new Promise<void>((res, rej) => ((resolve = res), (reject = rej)));
			promise.catch(() => {}); // a caller that does not await must not see an unhandled rejection
			this.#online = { resolve, reject, promise };
		}
		if (!this.#running) {
			this.#running = true;
			void this.#open();
		}
		return this.#online.promise;
	}

	/** Disconnects for good. Calls in flight are aborted. */
	close(): void {
		this.#stop({ code: 1000, reason: 'closed by the application' });
		this.#socket?.close(1000);
	}

	async #open(): Promise<void> {
		this.#setStatus('connecting');
		let token: string;
		try {
			const { token: source } = this.#options;
			token = typeof source === 'function' ? await source() : source;
		} catch {
			return this.#retry();
		}
		if (!this.#running) return;
		const WS = this.#options.WebSocket ?? globalThis.WebSocket;
		let socket: WebSocket;
		try {
			socket = new WS(this.#options.url, [SUBPROTOCOL, token]);
		} catch {
			return this.#retry();
		}
		this.#socket = socket;

		socket.onopen = () => {
			// Online once the bridge acknowledges this registration (SPEC §3.2).
			this.#send(this.#registration());
			this.#registerQueued = false;
			this.#timers.ping = setInterval(() => {
				this.#send(PING);
				this.#timers.pong ??= setTimeout(() => socket.close(), PONG_TIMEOUT);
			}, PING_INTERVAL);
		};
		socket.onmessage = (event) => {
			clearTimeout(this.#timers.pong);
			this.#timers.pong = undefined;
			if (typeof event.data === 'string') this.#receive(event.data);
		};
		socket.onclose = (event) => {
			if (this.#socket !== socket) return;
			this.#socket = undefined;
			clearInterval(this.#timers.ping);
			clearTimeout(this.#timers.pong);
			this.#timers.pong = undefined;
			this.#abortCalls();
			const fatal = FATAL[event.code];
			const tokenRefreshes = typeof this.#options.token === 'function';
			const detail = { code: event.code, reason: event.reason || fatal || '' };
			if (fatal || (event.code === UNAUTHORIZED && !tokenRefreshes)) {
				this.#stop({ ...detail, reason: detail.reason || 'unauthorized' });
			} else if (this.#running) {
				this.#retry(detail);
			}
		};
	}

	#retry(detail?: { code: number; reason: string }): void {
		if (!this.#running) return;
		this.#setStatus('offline', detail);
		const delay = Math.min(MAX_BACKOFF, 500 * 2 ** this.#attempt++) * (0.5 + Math.random() / 2);
		this.#timers.retry = setTimeout(() => void this.#open(), delay);
	}

	#stop(detail: { code: number; reason: string }): void {
		this.#running = false;
		clearTimeout(this.#timers.retry);
		clearInterval(this.#timers.ping);
		clearTimeout(this.#timers.pong);
		this.#abortCalls();
		this.#online?.reject(
			new Error(`ClientMCP connection closed: ${detail.reason} (${detail.code})`)
		);
		this.#online = undefined;
		this.#setStatus('closed', detail);
	}

	#setStatus(status: Status, detail?: { code: number; reason: string }): void {
		if (status === this.#status && !detail) return;
		this.#status = status;
		this.#options.onStatus?.(status, detail);
	}

	#send(text: string): void {
		if (this.#socket?.readyState === 1) this.#socket.send(text);
	}

	#scheduleRegister(): void {
		if (this.#registerQueued || this.#socket?.readyState !== 1) return;
		this.#registerQueued = true;
		queueMicrotask(() => {
			this.#registerQueued = false;
			this.#send(this.#registration());
		});
	}

	#registration(): string {
		const tools = [...this.#tools].map(([name, tool]) => ({
			name,
			title: tool.title,
			description: tool.description,
			inputSchema: tool.inputSchema ?? { type: 'object' },
			outputSchema: tool.outputSchema,
			annotations: tool.risk ? { ...RISK_HINTS[tool.risk], ...tool.annotations } : tool.annotations,
			icons: tool.icons,
			_meta: tool.risk ? { ...tool._meta, [RISK_META]: tool.risk } : tool._meta
		}));
		return JSON.stringify({ type: 'register', tools, instructions: this.#instructions });
	}

	#receive(text: string): void {
		let message: { type?: string; id?: string; name?: string; arguments?: Record<string, unknown> };
		try {
			message = JSON.parse(text);
		} catch {
			return;
		}
		if (message.type === 'registered' && this.#status !== 'online') {
			this.#attempt = 0;
			this.#setStatus('online');
			this.#online?.resolve();
		} else if (message.type === 'call' && typeof message.id === 'string') {
			void this.#call(message.id, String(message.name), message.arguments ?? {});
		} else if (message.type === 'cancel' && typeof message.id === 'string') {
			this.#calls.get(message.id)?.abort(new Error('cancelled by the agent'));
			this.#calls.delete(message.id);
		}
	}

	async #call(id: string, name: string, args: Record<string, unknown>): Promise<void> {
		const controller = new AbortController();
		this.#calls.set(id, controller);
		let result: CallToolResult;
		try {
			const tool = this.#tools.get(name);
			if (!tool) throw new Error(`Unknown tool: ${name}`);
			result = toCallToolResult(await tool.handler(args, { name, signal: controller.signal }));
		} catch (error) {
			const text = error instanceof Error ? error.message : String(error);
			result = { content: [{ type: 'text', text }], isError: true };
		}
		if (this.#calls.get(id) !== controller) return; // cancelled, or the connection ended
		this.#calls.delete(id);
		let message = JSON.stringify({ type: 'result', id, result });
		if (byteLength(message) > MAX_MESSAGE_BYTES) {
			const text = `The tool result is too large to deliver (over ${MAX_MESSAGE_BYTES} bytes).`;
			message = JSON.stringify({
				type: 'result',
				id,
				result: { content: [{ type: 'text', text }], isError: true }
			});
		}
		this.#send(message);
	}

	#abortCalls(): void {
		for (const controller of this.#calls.values()) controller.abort(new Error('disconnected'));
		this.#calls.clear();
	}
}

/** UTF-8 length of `text`; exact only when it might exceed the limit. */
function byteLength(text: string): number {
	return text.length * 3 <= MAX_MESSAGE_BYTES ? text.length : new TextEncoder().encode(text).length;
}

function toCallToolResult(value: unknown): CallToolResult {
	// The marker is a symbol key, which JSON leaves out: the result goes as it is.
	if (value && typeof value === 'object' && RESULT in value)
		return value as unknown as CallToolResult;
	if (value === undefined || value === null) return { content: [] };
	if (typeof value === 'string') return { content: [{ type: 'text', text: value }] };
	const result: CallToolResult = { content: [{ type: 'text', text: JSON.stringify(value) }] };
	// Only objects: MCP revisions before 2026-07-28 require structuredContent to be one.
	if (typeof value === 'object' && !Array.isArray(value)) result.structuredContent = value;
	return result;
}
