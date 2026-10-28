/**
 * ob-2 / ob-3: the retry screen after a publish that did not land.
 *
 * The account is saved on this device before the publish (ob-2, owner
 * 2026-10-28), so the screen says exactly that — saved here, not yet
 * findable elsewhere — never "Wallet created" and never "not saved". It
 * offers the wallet, not a start-over over a wallet that exists, names no
 * field that is not there, and does not wait for a tap: the publish replays
 * the keys' own proofs (no prompt), so it is retried on its own — on a
 * widening timer it counts down, and the moment the browser is back online.
 * The strings are the REAL corpus.
 */
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import en from '../../../../../../../assets/i18n/en.json';
import RetryScreen from './RetryScreen.svelte';

const strings = (key: string): string => {
	const value = key
		.split('.')
		.reduce<unknown>((node, part) => (node as Record<string, unknown>)?.[part], en);
	if (typeof value !== 'string') throw new Error(`no corpus value for "${key}"`);
	return value;
};

function drawn(
	props: {
		busy?: boolean;
		round?: number;
		retryDelaysMs?: number[];
		showStartOver?: boolean;
	} = {}
) {
	const onRetry = vi.fn();
	const onEnter = vi.fn();
	const screen = render(RetryScreen, {
		props: {
			detail: 'Failed to fetch',
			busy: props.busy ?? false,
			strings,
			onRetry,
			onEnter,
			showStartOver: props.showStartOver ?? false,
			onStartOver: () => {},
			round: props.round ?? 0,
			retryDelaysMs: props.retryDelaysMs ?? [60_000]
		}
	});
	const button = (label: string) =>
		[...screen.container.querySelectorAll('button')].find(
			(candidate) => candidate.textContent?.trim() === label
		);
	return { root: screen.container, onRetry, onEnter, button };
}

describe('the retry screen', () => {
	it('says the wallet is saved on this device, and names no field that is not there', () => {
		const { root } = drawn();
		const text = root.textContent ?? '';
		expect(strings('onboarding.create.syncFailedTitle')).toBe('Saved on this device');
		expect(text).toContain(strings('onboarding.create.syncFailedTitle'));
		expect(text).toContain(strings('onboarding.create.syncFailedMessage'));
		expect(text).not.toMatch(/Wallet created/i);
		expect(text).not.toMatch(/not saved|discards/i);
		expect(text).not.toMatch(/endpoint below/i);
	});

	it('offers the saved wallet, and no start-over over a wallet that exists', async () => {
		const { onEnter, button } = drawn();
		const enter = button(strings('onboarding.create.enterWalletBtn'));
		expect(enter).toBeDefined();
		enter?.click();
		expect(onEnter).toHaveBeenCalledOnce();
		expect(button(strings('onboarding.create.startOverBtn'))).toBeUndefined();
	});

	it('counts down to the next try on its own — never a silent wait', () => {
		const { root } = drawn({ retryDelaysMs: [20_000] });
		expect(root.querySelector('[role="status"]')?.textContent).toBe(
			strings('onboarding.create.syncFailedNext').replace('{{seconds}}', '20')
		);
	});

	it('retries on its own after the wait for this round', async () => {
		const { onRetry } = drawn({ round: 1, retryDelaysMs: [60_000, 20] });
		await vi.waitFor(() => expect(onRetry).toHaveBeenCalledOnce());
	});

	it('retries the moment the browser is back online', () => {
		const { onRetry } = drawn();
		window.dispatchEvent(new Event('online'));
		expect(onRetry).toHaveBeenCalledOnce();
	});

	it('does not stack a retry on one already running', () => {
		const { onRetry } = drawn({ busy: true });
		window.dispatchEvent(new Event('online'));
		expect(onRetry).not.toHaveBeenCalled();
	});
});
