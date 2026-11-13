/**
 * The AI agent's bar (spec 103) on a wide screen (agent-19): at the foot of
 * the sidebar rail — which keeps that room while the bar is up — never over
 * the pages' own header controls; on a phone, in the top right corner.
 */
import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import type { AgentMessages } from './messages';

vi.mock('$lib/intents/index.svelte', () => ({
	intents: { approvals: [], activity: [], stop: () => {}, invoke: () => {}, answer: () => {} },
	world: () => ({})
}));
vi.mock('$lib/core/client', () => ({ intentApprovalDetailIn: () => 'null' }));
vi.mock('$lib/contacts/core/contacts-book.svelte', () => ({ contactsBook: { view: null } }));
vi.mock('$lib/settings/core/network-admin.svelte', () => ({
	networkAdmin: { view: { networks: [], wizard: { chain_info: null } } }
}));
vi.mock('$lib/wallet/core/feed.svelte', () => ({ feed: { view: { transactions: [] } } }));
vi.mock('$lib/network-space/network-space.svelte', () => ({ networkSpace: { words: null } }));

const { default: AgentLayer } = await import('./AgentLayer.svelte');

const messages = {
	title: 'AI agent',
	connected: 'Connected',
	elsewhere: 'In use in another tab',
	useHere: 'Use here',
	stop: 'Stop',
	actions: {}
} as unknown as AgentMessages;

afterEach(async () => {
	await page.viewport(414, 896);
});

describe('the agent bar', () => {
	it('stands at the foot of the sidebar rail on a wide screen, and the rail keeps its room', async () => {
		await page.viewport(1440, 900);
		const screen = render(AgentLayer, { props: { messages, status: 'online' } });
		await tick();
		expect(document.documentElement.hasAttribute('data-agent-bar')).toBe(true);
		const bar = document.querySelector<HTMLElement>('[data-testid="agent-bar"]')!;
		const box = bar.getBoundingClientRect();
		// Inside the 240-wide rail, at the bottom of the window.
		expect(box.left).toBeGreaterThanOrEqual(0);
		expect(box.right).toBeLessThanOrEqual(240);
		expect(window.innerHeight - box.bottom).toBeLessThanOrEqual(24);
		expect(box.top).toBeGreaterThan(window.innerHeight / 2);
		await screen.unmount();
		expect(document.documentElement.hasAttribute('data-agent-bar')).toBe(false);
	});

	it('keeps to the top right corner on a phone', async () => {
		await page.viewport(414, 896);
		const screen = render(AgentLayer, { props: { messages, status: 'online' } });
		await tick();
		const box = document.querySelector('[data-testid="agent-bar"]')!.getBoundingClientRect();
		expect(box.top).toBeLessThan(60);
		expect(window.innerWidth - box.right).toBeLessThanOrEqual(24);
		await screen.unmount();
	});

	it('is not up, and claims no room, while the agent is off', async () => {
		const screen = render(AgentLayer, { props: { messages, status: 'off' } });
		await tick();
		expect(document.querySelector('[data-testid="agent-bar"]')).toBeNull();
		expect(document.documentElement.hasAttribute('data-agent-bar')).toBe(false);
		await screen.unmount();
	});
});
