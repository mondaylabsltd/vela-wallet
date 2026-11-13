/**
 * Settings → AI agent, "Use your own bridge" (set-10): the fold stays the way
 * the person left it when a saved field re-renders the panel.
 */
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import AgentPanel from './AgentPanel.svelte';
import type { AgentPanelModel } from '../model';

function panel(url: string, ownOpen: boolean): AgentPanelModel {
	return {
		title: 'AI agent',
		description: 'An AI agent can use this wallet.',
		enable: { id: 'settings-agent', title: 'Allow an AI agent', subtitle: '', on: false },
		bridge: { id: 'agent-bridge', label: 'Bridge address', value: url },
		token: { id: 'agent-token', label: 'Client token', value: '', secret: true },
		status: { tone: 'neutral', label: 'Off', dot: true },
		link: {
			connect: 'Connect with clientmcp.dev',
			connectHint: 'Sign in with GitHub.',
			reconnect: 'Reconnect',
			disconnect: 'Disconnect',
			ownBridge: 'Use your own bridge',
			ownOpen
		},
		wallets: { title: 'Wallets the agent may use', hint: '', rows: [] }
	};
}

const fold = () => document.body.querySelector('details.own') as HTMLDetailsElement;

describe('Use your own bridge (set-10)', () => {
	it('stays open after a field is saved and the panel re-renders', async () => {
		const saved: string[] = [];
		const screen = render(AgentPanel, {
			props: { panel: panel('', false), onbridge: (url) => saved.push(url) }
		});
		await tick();
		expect(fold().open).toBe(false);

		fold().querySelector('summary')?.click();
		await tick();
		expect(fold().open).toBe(true);

		const input = document.body.querySelector(
			'input[aria-label="Bridge address"]'
		) as HTMLInputElement;
		input.value = 'wss://bridge.example.com/connect';
		input.dispatchEvent(new Event('input', { bubbles: true }));
		input.dispatchEvent(new FocusEvent('blur'));
		await tick();
		expect(saved).toEqual(['wss://bridge.example.com/connect']);

		// The save comes back as a new model; the fold must not snap shut, and
		// the address just applied is there to see.
		await screen.rerender({ panel: panel('wss://bridge.example.com/connect', false) });
		await tick();
		expect(fold().open).toBe(true);
		expect(
			(document.body.querySelector('input[aria-label="Bridge address"]') as HTMLInputElement).value
		).toBe('wss://bridge.example.com/connect');
		await screen.unmount();
	});

	it('opens by itself when a bridge is already configured', async () => {
		const screen = render(AgentPanel, { props: { panel: panel('wss://b.example/connect', true) } });
		await tick();
		expect(fold().open).toBe(true);
		await screen.unmount();
	});
});
