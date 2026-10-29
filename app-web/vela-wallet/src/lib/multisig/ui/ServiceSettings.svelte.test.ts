/**
 * Settings → Multisig service, in a browser: what is in use is shown and
 * asked; Save asks the typed address and says what answered — saved at once
 * for a multisig service, "Save anyway" for nothing at all, refused for
 * something else — and Use default forgets the person's own. The key is the
 * one the live tests seed (`vela.genesisServiceURL`).
 */
import { tick } from 'svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { loadCore } from '$lib/core/client';
import type { MultisigMessages } from '../messages';
import { GENESIS_SERVICE_KEY, genesisService } from '../core/service-state.svelte';
import ServiceSettings from './ServiceSettings.svelte';

const service = {
	title: 'Multisig service',
	subtitle: 'sub',
	description: 'desc',
	privacy: 'The service sees your multisigs.',
	inUse: 'In use',
	default: 'Default',
	field: 'Service address',
	save: 'Save',
	useDefault: 'Use default',
	checking: 'Checking…',
	reachable: 'Reachable',
	unreachable: 'Not reachable',
	notService: 'Not a multisig service',
	serves: 'Serves {{networks}}',
	unsavedUnreachable: 'Nothing answered.',
	unsavedNotService: 'Not a multisig service, not saved.',
	saveAnyway: 'Save anyway',
	invalid: 'Enter an https address.',
	insecure: 'Use https.',
	saved: 'Saved.',
	customNote: 'Using a custom multisig service ({{host}})',
	unreachableNote: 'away',
	retry: 'Retry',
	noNetworks: 'none'
};
const m = { service } as unknown as MultisigMessages;

const HEALTH = JSON.stringify({
	service: 'safe-genesis-registry',
	chains: [{ chainId: 5042002, name: 'Arc Testnet' }, { chainId: 100 }]
});

/** What answers, by address: a body, `null` for nothing at all. */
let answers: Record<string, { status: number; body: string } | null> = {};
const asked: string[] = [];

beforeAll(() => loadCore());
beforeEach(() => {
	genesisService.set(null);
	answers = {};
	asked.length = 0;
	vi.stubGlobal(
		'fetch',
		vi.fn(async (input: string) => {
			asked.push(input);
			const base = input.replace(/\/api\/health$/, '');
			const answer = answers[base];
			if (!answer) throw new TypeError('Failed to fetch');
			return new Response(answer.body, { status: answer.status });
		})
	);
});
afterEach(() => {
	vi.unstubAllGlobals();
	genesisService.set(null);
});

const settle = async () => {
	for (let i = 0; i < 5; i += 1) {
		await new Promise((r) => setTimeout(r, 0));
		await tick();
	}
};

async function drawn() {
	const screen = render(ServiceSettings, { props: { m, locale: 'en' } });
	await settle();
	const root = screen.container;
	const button = (text: string) =>
		[...root.querySelectorAll('button')].find((b) => b.textContent?.trim() === text);
	return {
		root,
		button,
		text: () => root.textContent ?? '',
		async type(value: string) {
			const input = root.querySelector('input');
			expect(input).toBeTruthy();
			input!.value = value;
			input!.dispatchEvent(new Event('input', { bubbles: true }));
			await tick();
		},
		async click(text: string) {
			const b = button(text);
			expect(b, `${text} is drawn`).toBeTruthy();
			b!.click();
			await settle();
		}
	};
}

describe('Settings → Multisig service', () => {
	it('shows the default in use, asked: reachable, and the networks it serves', async () => {
		answers['https://safe-genesis.getvela.app'] = { status: 200, body: HEALTH };
		const page = await drawn();
		const inUse = page.root.querySelector('[data-testid="genesis-service-in-use"]')!;
		expect(inUse.textContent).toContain('https://safe-genesis.getvela.app');
		expect(inUse.textContent).toContain('Default');
		expect(inUse.textContent).toContain('Reachable');
		expect(inUse.textContent).toContain('Serves Arc Testnet and Gnosis');
		// The privacy note is the field's hint.
		expect(page.text()).toContain('The service sees your multisigs.');
		expect(page.button('Use default')).toBeUndefined();
	});

	it('says the default is not reachable when it is not', async () => {
		const page = await drawn();
		expect(page.text()).toContain('Not reachable');
	});

	it('saves a multisig service at once, in the key the live tests seed', async () => {
		answers['http://127.0.0.1:8788'] = { status: 200, body: HEALTH };
		const page = await drawn();
		await page.type(' http://127.0.0.1:8788/ ');
		await page.click('Save');
		expect(localStorage.getItem(GENESIS_SERVICE_KEY)).toBe('http://127.0.0.1:8788');
		expect(page.text()).toContain('Saved.');
		expect(page.text()).not.toContain('Default');
		expect(page.button('Use default')).toBeTruthy();

		await page.click('Use default');
		expect(localStorage.getItem(GENESIS_SERVICE_KEY)).toBeNull();
		expect(page.text()).toContain('Default');
	});

	it('says nothing answered, and saves it only on "Save anyway"', async () => {
		const page = await drawn();
		await page.type('http://127.0.0.1:9999');
		await page.click('Save');
		expect(page.text()).toContain('Nothing answered.');
		expect(localStorage.getItem(GENESIS_SERVICE_KEY)).toBeNull();
		await page.click('Save anyway');
		expect(localStorage.getItem(GENESIS_SERVICE_KEY)).toBe('http://127.0.0.1:9999');
		expect(page.text()).toContain('Not reachable');
	});

	it('does not save something that is not a multisig service', async () => {
		answers['https://relay.example'] = {
			status: 200,
			body: JSON.stringify({ service: 'vela-relay' })
		};
		const page = await drawn();
		await page.type('https://relay.example');
		await page.click('Save');
		expect(page.text()).toContain('Not a multisig service, not saved.');
		expect(page.button('Save anyway')).toBeUndefined();
		expect(localStorage.getItem(GENESIS_SERVICE_KEY)).toBeNull();
	});

	it("refuses an address the core's rule refuses, without asking it", async () => {
		const page = await drawn();
		const before = asked.length;
		await page.type('http://192.168.1.5:8788');
		await page.click('Save');
		expect(page.text()).toContain('Use https.');
		await page.type('safe-genesis');
		await page.click('Save');
		expect(page.text()).toContain('Enter an https address.');
		expect(asked.length).toBe(before);
		expect(localStorage.getItem(GENESIS_SERVICE_KEY)).toBeNull();
	});
});
