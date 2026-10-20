import { describe, expect, it } from 'vitest';
import { bridgeUrlProblem, mcpEndpoint } from './settings.svelte';

describe('the bridge address (spec 103)', () => {
	it('takes wss anywhere and ws only to this machine', () => {
		expect(bridgeUrlProblem('wss://bridge.example.com/connect')).toBeNull();
		expect(bridgeUrlProblem('ws://127.0.0.1:8700/connect')).toBeNull();
		expect(bridgeUrlProblem('ws://localhost:8700/connect')).toBeNull();
		expect(bridgeUrlProblem('ws://[::1]:8700/connect')).toBeNull();
		expect(bridgeUrlProblem('ws://bridge.example.com/connect')).toBe('insecure');
		expect(bridgeUrlProblem('https://bridge.example.com/connect')).toBe('invalid');
		expect(bridgeUrlProblem('not a url')).toBe('invalid');
		expect(bridgeUrlProblem('')).toBe('invalid');
	});

	it("derives the agent's MCP endpoint from it", () => {
		expect(mcpEndpoint('wss://bridge.example.com/connect')).toBe('https://bridge.example.com/mcp');
		expect(mcpEndpoint(' ws://127.0.0.1:8700/connect/ ')).toBe('http://127.0.0.1:8700/mcp');
		expect(mcpEndpoint('wss://example.com/bridge/connect?x=1')).toBe(
			'https://example.com/bridge/mcp'
		);
		expect(mcpEndpoint('wss://example.com')).toBe('https://example.com/mcp');
		expect(mcpEndpoint('ws://bridge.example.com/connect')).toBeNull();
	});
});
