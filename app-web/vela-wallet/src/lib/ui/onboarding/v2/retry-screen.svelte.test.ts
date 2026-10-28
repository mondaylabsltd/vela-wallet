/**
 * ob-2 / ob-3: the retry screen after a publish that did not land.
 *
 * Nothing is saved until the publish lands, so the screen says "not saved
 * yet" (never "Wallet created"), names no field that is not there, and does
 * not wait for a tap: the publish replays the keys' own proofs (no prompt),
 * so it is retried on its own — on a widening timer, and the moment the
 * browser is back online. The strings are the REAL corpus.
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

function drawn(props: { busy?: boolean; round?: number; retryDelaysMs?: number[] } = {}) {
	const onRetry = vi.fn();
	const screen = render(RetryScreen, {
		props: {
			detail: 'Failed to fetch',
			busy: props.busy ?? false,
			strings,
			onRetry,
			onStartOver: () => {},
			round: props.round ?? 0,
			retryDelaysMs: props.retryDelaysMs ?? [60_000]
		}
	});
	return { root: screen.container, onRetry };
}

describe('the retry screen', () => {
	it('says the wallet is not saved yet, and names no field that is not there', () => {
		const { root } = drawn();
		const text = root.textContent ?? '';
		expect(text).toContain(strings('onboarding.create.syncFailedTitle'));
		expect(text).not.toMatch(/Wallet created/i);
		expect(text).not.toMatch(/endpoint below/i);
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
