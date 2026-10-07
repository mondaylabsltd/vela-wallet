/**
 * About carries "Share anonymous usage statistics" under the privacy policy
 * that describes it — a switch that says where it stands and reports a flip.
 */
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import AboutPanel from './AboutPanel.svelte';
import type { AboutModel } from '../model';

const PANEL: AboutModel = {
	title: 'About',
	tagline: 'The passkey wallet',
	version: '0.9.6',
	sectionTechnical: 'Technical',
	rows: [],
	links: [{ label: 'Privacy Policy', value: 'getvela.app/privacy' }],
	footer: 'Made with care'
};
const ROW = {
	id: 'settings-analytics',
	title: 'Share anonymous usage statistics',
	subtitle: 'Which screens open and which steps finish — never addresses, amounts or names.'
};

describe('About → usage statistics', () => {
	it('is absent where no state is given (the gallery)', async () => {
		const screen = render(AboutPanel, { props: { panel: PANEL } });
		await tick();
		expect(document.body.querySelector('[role="switch"]')).toBe(null);
		await screen.unmount();
	});

	it('shows where it stands and reports the flip', async () => {
		const flips: boolean[] = [];
		const screen = render(AboutPanel, {
			props: { panel: PANEL, analytics: { ...ROW, on: true }, onanalytics: (on) => flips.push(on) }
		});
		await tick();
		const toggle = document.body.querySelector('[role="switch"]') as HTMLButtonElement;
		expect(toggle.getAttribute('aria-checked')).toBe('true');
		expect(toggle.textContent).toContain(ROW.title);
		expect(toggle.textContent).toContain(ROW.subtitle);
		toggle.click();
		expect(flips).toEqual([false]);
		await screen.unmount();
	});
});
