/**
 * About (owner, 2026-10-29): the mark, the name and the build — the build
 * copies itself for a support request — then help, the policy, the terms and
 * the code, and one slogan. No technical rows, no tagline, no switch.
 */
import { tick } from 'svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { loadCore } from '$lib/core/client';
import AboutPanel from './AboutPanel.svelte';
import { buildDesktopState, buildMobileState } from '../fixtures';
import { words } from '../__fixtures__/words';
import type { AboutModel } from '../model';

const m = words();
const IDENTICON = (seed: string) => `<svg data-seed="${seed}"></svg>`;
// The fixtures' venue rows are the core's sentences: built over the loaded core.
beforeAll(() => loadCore());

const PANEL: AboutModel = {
	title: 'About',
	name: 'Vela',
	version: 'v0.9.8 (ced8298)',
	versionCopy: 'Vela v0.9.8 (ced8298)',
	copyLabel: 'Copy',
	copiedLabel: 'Copied',
	links: [{ label: 'Privacy Policy', value: 'getvela.app/privacy' }],
	footer: 'The Ethereum wallet that’s truly yours'
};

afterEach(() => vi.restoreAllMocks());

describe('About', () => {
	it('shows the name and the build, and no switch', async () => {
		const screen = render(AboutPanel, { props: { panel: PANEL } });
		await tick();
		const root = screen.container;
		expect(root.querySelector('.name')?.textContent).toBe('Vela');
		expect(root.querySelector('button.version')?.textContent).toContain('v0.9.8 (ced8298)');
		expect(root.querySelector('[role="switch"]')).toBe(null);
		expect(root.querySelector('.footer')?.textContent).toBe(PANEL.footer);
		await screen.unmount();
	});

	it('a tap on the build copies it, and says so in a neutral word', async () => {
		const write = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue();
		const screen = render(AboutPanel, { props: { panel: PANEL } });
		await tick();
		const status = screen.container.querySelector('[role="status"]');
		expect(status?.textContent).toBe('');
		(screen.container.querySelector('button.version') as HTMLButtonElement).click();
		await vi.waitFor(() => expect(status?.textContent).toBe('Copied'));
		expect(write).toHaveBeenCalledWith('Vela v0.9.8 (ced8298)');
		// Neutral: the confirmation is the muted text colour, never the accent.
		const tone = getComputedStyle(status as Element).color;
		const accent = getComputedStyle(document.documentElement)
			.getPropertyValue('--color-accent-base')
			.trim();
		expect(tone).not.toBe(accent);
		await screen.unmount();
	});

	it('lists help, the policy, the terms and the code — no website, no Safe, no technical rows', () => {
		for (const about of [
			buildMobileState('st14', m, IDENTICON).about,
			buildDesktopState('dst8', m, IDENTICON).about
		]) {
			expect(about.links.map((link) => link.href)).toEqual([
				'https://getvela.app/support',
				'https://getvela.app/privacy',
				'https://getvela.app/terms',
				'https://github.com/mondaylabsltd/vela-wallet'
			]);
			expect(about.links.at(-1)?.label).toBe(m.about.linkGitHub);
			expect(about.name).toBe('Vela');
			expect(about.versionCopy).toBe(`Vela ${about.version}`);
			expect(about.footer).toBe(m.about.footer);
			expect(Object.keys(about)).not.toContain('rows');
		}
	});
});
