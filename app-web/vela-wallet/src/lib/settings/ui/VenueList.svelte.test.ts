/**
 * "Where you review and sign" (spec 102), in a real browser.
 *
 * What a person must be able to see and do: which venue is in force, that a
 * choice which cannot reach the account's keys is OFF and why (R1 — the row
 * stays, with the core's reason), that a trusted page carries the one line
 * that backs the word "trusted", and that on the web — which opens no page —
 * the list is a statement, never a control that quietly does nothing: where
 * it signs is marked, and every page row is off with the core's reason
 * (D-16).
 */
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import type { SigningVenue } from '$lib/core/generated/SigningVenue';
import type { VenueModel } from '../model';
import VenueList from './VenueList.svelte';

const OFFICIAL = 'https://sign.getvela.app/';
const OWN = 'https://sign.example.com/';

const APP: VenueModel = {
	title: 'Where you review and sign',
	subtitle: "For this account's transactions and messages. Your keys don't change.",
	value: 'On a trusted page',
	note: 'sign.getvela.app',
	summary: 'On a trusted page · sign.getvela.app',
	domainLine: 'Keys on getvela.app',
	rows: [
		{
			id: 'in_vela',
			venue: { type: 'in_vela' },
			icon: 'wallet',
			title: 'In Vela',
			body: "Vela's own signing sheet",
			keysOn: 'Keys on getvela.app',
			active: false
		},
		{
			id: OFFICIAL,
			venue: { type: 'page', url: OFFICIAL },
			icon: 'shield-check',
			title: 'On a trusted page',
			body: 'A zero-dependency page shows exactly what you sign',
			page: {
				name: "Vela's official signing page",
				host: 'sign.getvela.app',
				official: true,
				hostShown: true
			},
			keysOn: 'Keys on getvela.app',
			integrity: {
				text: "Version 0ba8ee8c · matches Vela's published build list · checked 14:32",
				tone: 'ok'
			},
			active: true
		},
		{
			id: OWN,
			venue: { type: 'page', url: OWN },
			icon: 'shield-check',
			title: 'On a trusted page',
			body: 'A zero-dependency page shows exactly what you sign',
			page: {
				name: 'Self-hosted · sign.example.com',
				host: 'sign.example.com',
				official: false,
				hostShown: false
			},
			keysOn: 'Keys on sign.example.com',
			active: false,
			blocked: "This page is on sign.example.com; this account's keys are on getvela.app."
		}
	]
};

function drawn(venue: VenueModel) {
	const picked: SigningVenue[] = [];
	const screen = render(VenueList, {
		props: { venue, onpick: (choice: SigningVenue) => picked.push(choice) }
	});
	const radios = () => [...screen.container.querySelectorAll<HTMLButtonElement>('[role="radio"]')];
	return { root: screen.container, picked, radios };
}

describe('Where you review and sign', () => {
	it('marks the venue in force, and draws the pages under what a trusted page IS', () => {
		const view = drawn(APP);
		expect(view.radios().map((radio) => radio.getAttribute('aria-checked'))).toEqual([
			'false',
			'true',
			'false'
		]);
		const text = view.root.textContent ?? '';
		// Said once, as the heading over the pages — not repeated on every row.
		expect(text.split('A zero-dependency page shows exactly what you sign')).toHaveLength(2);
		expect(text).toContain('sign.getvela.app');
		expect(text).toContain('Keys on getvela.app');
	});

	it('backs "trusted" with its integrity line: the version, set in the mono face', () => {
		const view = drawn(APP);
		const line = view.radios()[1].querySelector('.integrity');
		expect(line?.textContent).toContain("matches Vela's published build list");
		const version = line?.querySelector('.version');
		expect(version?.textContent).toBe('0ba8ee8c');
		expect(getComputedStyle(version as Element).fontFamily).toContain('IBM Plex Mono');
	});

	it('a page that cannot reach the keys is off, and says why — it is never hidden', () => {
		const view = drawn(APP);
		const blocked = view.radios()[2];
		expect(blocked.disabled).toBe(true);
		expect(blocked.textContent).toContain(
			"This page is on sign.example.com; this account's keys are on getvela.app."
		);
		blocked.click();
		expect(view.picked).toEqual([]);
	});

	it('choosing a reachable venue hands the core that venue', async () => {
		const view = drawn(APP);
		view.radios()[0].click();
		await tick();
		expect(view.picked).toEqual([{ type: 'in_vela' }]);
		// The one in force is not chosen again.
		view.radios()[1].click();
		expect(view.picked).toHaveLength(1);
	});

	it('names a self-hosted page once: its name already says the host', () => {
		const view = drawn(APP);
		const own = view.radios()[2].querySelector('.title');
		expect(own?.textContent).toBe('Self-hosted · sign.example.com');
		expect(own?.querySelector('.host')).toBeNull();
		// The official page's name does not say it, so the host is drawn beside it.
		expect(view.radios()[1].querySelector('.title .host')?.textContent).toBe('sign.getvela.app');
	});

	it('the web: stated, every page row off with its reason, and tapping does nothing', () => {
		const reason = 'Signing pages open from the Vela apps, not the web.';
		const web: VenueModel = {
			...APP,
			value: 'Review and sign in Vela',
			note: undefined,
			summary: 'Review and sign in Vela',
			readOnly: true,
			rows: [
				{ ...APP.rows[0], active: true },
				{ ...APP.rows[1], active: false, integrity: undefined, blocked: reason }
			]
		};
		const view = drawn(web);
		expect(view.radios()).toHaveLength(2);
		expect(view.radios().map((radio) => radio.getAttribute('aria-checked'))).toEqual([
			'true',
			'false'
		]);
		expect(view.radios().map((radio) => radio.getAttribute('aria-disabled'))).toEqual([
			'true',
			'true'
		]);
		expect(view.radios()[1].disabled).toBe(true);
		expect(view.radios()[1].textContent).toContain(reason);
		for (const radio of view.radios()) radio.click();
		expect(view.picked).toEqual([]);
	});

	it('fits a 320-wide phone: nothing scrolls sideways', async () => {
		const screen = render(VenueList, { props: { venue: APP } });
		const root = screen.container;
		// 6 × 48 + 32 = 320, in tokens (the audit reads test files too).
		root.style.width = 'calc(var(--space-5xl) * 6 + var(--space-4xl))';
		await tick();
		expect(root.clientWidth).toBeGreaterThan(0);
		expect(root.scrollWidth).toBeLessThanOrEqual(root.clientWidth);
	});
});
