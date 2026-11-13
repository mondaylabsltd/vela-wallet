/**
 * The wallet's ClientMCP client, over a fake socket: a call still out when
 * the wallet hangs up is answered first — the agent hears why (agent-21),
 * and the handler it gave up on answers nothing after.
 */
import { describe, expect, it } from 'vitest';
import { ClientMCP, rejected } from './clientmcp';

class FakeSocket {
	static last: FakeSocket | null = null;
	readyState = 0;
	sent: string[] = [];
	onopen: (() => void) | null = null;
	onmessage: ((event: { data: string }) => void) | null = null;
	onclose: ((event: { code: number; reason: string }) => void) | null = null;
	onerror: (() => void) | null = null;
	constructor() {
		FakeSocket.last = this;
	}
	send(text: string) {
		this.sent.push(text);
	}
	close() {
		this.readyState = 3;
	}
	open() {
		this.readyState = 1;
		this.onopen?.();
	}
	receive(message: unknown) {
		this.onmessage?.({ data: JSON.stringify(message) });
	}
}

const tick = () => new Promise((done) => setTimeout(done, 0));

describe('answerAll', () => {
	it('answers each call still out, and its handler answers nothing after', async () => {
		const client = new ClientMCP({
			url: 'ws://bridge.example/connect',
			token: 'tok',
			WebSocket: FakeSocket as unknown as typeof WebSocket
		});
		let given: AbortSignal | null = null;
		let finish: (value: unknown) => void = () => {};
		client.expose('vela_wait', {
			handler: (_args, { signal }) => {
				given = signal;
				return new Promise((done) => (finish = done));
			}
		});
		void client.connect();
		await tick();
		const socket = FakeSocket.last!;
		socket.open();
		socket.receive({ type: 'registered' });
		socket.receive({ type: 'call', id: 'c1', name: 'vela_wait', arguments: {} });
		await tick();
		socket.sent.length = 0;

		client.answerAll(rejected('stopped', 'the person stopped the agent'));
		expect(socket.sent).toHaveLength(1);
		const message = JSON.parse(socket.sent[0]);
		expect(message).toMatchObject({ type: 'result', id: 'c1', result: { isError: true } });
		expect(message.result.structuredContent).toMatchObject({
			status: 'rejected',
			reason: { code: 'stopped' }
		});
		expect(given!.aborted).toBe(true);

		finish({ timed_out: true });
		await tick();
		expect(socket.sent).toHaveLength(1);
		client.close();
	});
});

describe('a call to a tool that is not exposed', () => {
	async function online(client: ClientMCP): Promise<FakeSocket> {
		void client.connect();
		await tick();
		const socket = FakeSocket.last!;
		socket.open();
		socket.receive({ type: 'registered' });
		socket.sent.length = 0;
		return socket;
	}

	it('is answered by `unknownTool` — a retired action says what to use instead', async () => {
		const client = new ClientMCP({
			url: 'ws://bridge.example/connect',
			token: 'tok',
			WebSocket: FakeSocket as unknown as typeof WebSocket
		});
		const asked: string[] = [];
		client.unknownTool = (name) => {
			asked.push(name);
			return rejected('not_found', 'use set_network_endpoint');
		};
		const socket = await online(client);
		socket.receive({ type: 'call', id: 'c1', name: 'vela_set_provider_key', arguments: {} });
		await tick();
		expect(asked).toEqual(['vela_set_provider_key']);
		const message = JSON.parse(socket.sent[0]);
		expect(message.result.structuredContent).toMatchObject({
			status: 'rejected',
			reason: { code: 'not_found', detail: 'use set_network_endpoint' }
		});
		client.close();
	});

	it('is "Unknown tool" without one', async () => {
		const client = new ClientMCP({
			url: 'ws://bridge.example/connect',
			token: 'tok',
			WebSocket: FakeSocket as unknown as typeof WebSocket
		});
		const socket = await online(client);
		socket.receive({ type: 'call', id: 'c2', name: 'vela_nothing', arguments: {} });
		await tick();
		const message = JSON.parse(socket.sent[0]);
		expect(message.result).toMatchObject({ isError: true });
		expect(JSON.stringify(message.result)).toContain('Unknown tool: vela_nothing');
		client.close();
	});
});
