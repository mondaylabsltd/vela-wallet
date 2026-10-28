/* eslint-disable @typescript-eslint/no-explicit-any -- a stand-in bridge and wallet */
/**
 * The connect page in a browser (ms-14): it says which account is
 * connecting, and an account that is not one of the members the link named
 * is refused aloud — Connect is off, and the page says what to do — never a
 * calm "Connected. Keep this page open".
 */
import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { MULTISIG_SHAPE, type MultisigMessages } from '$lib/multisig/messages';

const MEMBER = '0x70997970c51812dc3a010c7d01b50e0d17dc79c8';
const STRANGER = '0x3c44cdddb6a900fa2b585dd299e03d12fa4293bc';
const INVITE = `wss://relay.getvela.app/r/abc#${'A'.repeat(86)}`;

const fake = vi.hoisted(() => ({
	accounts: [] as string[],
	joined: [] as unknown[][],
	bridge: null as any,
	approved: false
}));

vi.mock('$lib/ui/history', () => ({ replaceUrl: () => {} }));
vi.mock('$lib/multisig/external/injected', () => ({ currentChain: async () => 'eip155:100' }));
vi.mock('$lib/multisig/external/eip6963', () => ({
	discoverProviders: async () => [
		{
			info: { uuid: 'u', name: 'MetaMask', icon: '', rdns: '' },
			provider: { request: async () => fake.accounts }
		}
	]
}));
vi.mock('$lib/multisig/external/bridge', () => ({
	Bridge: class extends EventTarget {
		signer: { accounts: string[]; chain: string };
		activity: unknown[] = [];
		prompt: unknown = undefined;
		constructor(_provider: unknown, accounts: string[], chain: string) {
			super();
			this.signer = { accounts, chain };
			fake.bridge = this;
		}
		get sessions() {
			return fake.joined.length
				? [{ id: 's', state: 'connected', approved: fake.approved, declined: false, created: 1 }]
				: [];
		}
		resumeAll() {}
		join(...args: unknown[]) {
			fake.joined.push(args);
			this.prompt = { session: 's', params: { name: 'Vela' }, answer: () => {} };
			return 's';
		}
		end() {}
	}
}));

const { default: Page } = await import('./+page.svelte');

function words(): MultisigMessages {
	const out: any = {};
	for (const [group, keys] of Object.entries(MULTISIG_SHAPE)) {
		out[group] = {};
		for (const key of Object.keys(keys)) out[group][key] = `${group}.${key}`;
	}
	out.external.connectingAs = 'as {{account}}';
	out.external.expectedMember = 'for {{accounts}}';
	out.external.notExpectedMember = '{{account}} is not {{accounts}}';
	return out;
}

async function open(accounts: string[], hash: string) {
	fake.accounts = accounts;
	fake.joined = [];
	fake.approved = false;
	history.replaceState(null, '', `${location.pathname}${hash}`);
	const screen = render(Page, {
		props: { data: { multisigMessages: words(), locale: 'en' } as any }
	});
	await vi.waitFor(() => expect(fake.joined).toHaveLength(1), { timeout: 3000 });
	await tick();
	return screen;
}

const connectButton = () =>
	[...document.querySelectorAll('button')].find(
		(b) => b.textContent?.trim() === 'external.connect'
	) as HTMLButtonElement | undefined;

afterEach(() => {
	history.replaceState(null, '', location.pathname);
});

describe('the connect page names who connects (ms-14)', () => {
	it('joins with the members the link named, and says which account connects', async () => {
		const screen = await open([MEMBER], `#${encodeURIComponent(INVITE)}&for=${MEMBER}`);
		expect(fake.joined[0]).toEqual([INVITE, [MEMBER]]);
		await vi.waitFor(() =>
			expect(document.querySelector('[data-testid="connect-account"]')?.textContent).toContain(
				'as 0x7099'
			)
		);
		expect(document.querySelector('[data-testid="connect-wrong-account"]')).toBeNull();
		expect(connectButton()?.disabled).toBe(false);
		await screen.unmount();
	});

	it('refuses an account that is not one of them: aloud, with what to do, Connect off', async () => {
		const screen = await open([STRANGER], `#${encodeURIComponent(INVITE)}&for=${MEMBER}`);
		await vi.waitFor(() =>
			expect(document.querySelector('[data-testid="connect-wrong-account"]')).not.toBeNull()
		);
		const alert = document.querySelector('[data-testid="connect-wrong-account"]')!;
		expect(alert.getAttribute('role')).toBe('alert');
		expect(alert.textContent).toContain('0x3C44');
		expect(alert.textContent).toContain('0x7099');
		expect(connectButton()?.disabled).toBe(true);
		await screen.unmount();
	});

	it('connected as a stranger: the refusal, never "keep this page open"', async () => {
		const screen = await open([STRANGER], `#${encodeURIComponent(INVITE)}&for=${MEMBER}`);
		fake.approved = true;
		fake.bridge.prompt = undefined;
		fake.bridge.dispatchEvent(new Event('change'));
		await tick();
		await vi.waitFor(() =>
			expect(document.querySelector('[data-testid="connect-wrong-account"]')).not.toBeNull()
		);
		expect(document.querySelector('[data-testid="connect-connected"]')).toBeNull();
		// The wallet switches to the member: the page carries on.
		fake.bridge.signer.accounts.splice(0, 1, MEMBER);
		fake.bridge.dispatchEvent(new Event('change'));
		await vi.waitFor(() =>
			expect(document.querySelector('[data-testid="connect-connected"]')).not.toBeNull()
		);
		expect(document.querySelector('[data-testid="connect-wrong-account"]')).toBeNull();
		await screen.unmount();
	});

	it('an older link names no one: nothing to refuse', async () => {
		const screen = await open([STRANGER], `#${encodeURIComponent(INVITE)}`);
		expect(fake.joined[0]).toEqual([INVITE, []]);
		await tick();
		expect(document.querySelector('[data-testid="connect-wrong-account"]')).toBeNull();
		expect(connectButton()?.disabled).toBe(false);
		await screen.unmount();
	});
});
