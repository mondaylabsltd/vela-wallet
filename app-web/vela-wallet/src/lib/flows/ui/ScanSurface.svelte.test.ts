/**
 * home-18, in a real browser: a scanner whose camera was refused says how to
 * allow it and offers to ask again; the lamp and the flip, which need a live
 * camera, are unavailable rather than buttons that do nothing.
 */
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { page } from 'vitest/browser';
import '$lib/tokens/tokens.css';
import type { ScanModel } from '../model';
import ScanSurface from './ScanSurface.svelte';

const model = {
	title: 'Scan QR',
	hint: 'Point camera at a QR code',
	closeLabel: 'Close',
	tools: [
		{ id: 'gallery', label: 'Photos' },
		{ id: 'torch', label: 'Torch' },
		{ id: 'flip', label: 'Flip' }
	]
} as unknown as ScanModel;

describe('a refused camera (home-18)', () => {
	it('offers to ask again, and the lamp and flip rest; the photo picker still works', async () => {
		const onpress = vi.fn();
		const ontool = vi.fn();
		render(ScanSurface, {
			props: {
				model,
				notice:
					'Camera access is blocked. Allow it for this site in your browser’s settings, then try again.',
				retry: { label: 'Try again', onpress },
				dimmed: ['torch', 'flip'],
				ontool
			}
		});
		await page.getByRole('button', { name: 'Try again' }).click();
		expect(onpress).toHaveBeenCalledOnce();
		await expect.element(page.getByRole('button', { name: 'Torch' })).toBeDisabled();
		await expect.element(page.getByRole('button', { name: 'Flip' })).toBeDisabled();
		await page.getByRole('button', { name: 'Photos' }).click();
		expect(ontool).toHaveBeenCalledWith('gallery');
	});

	it('a live camera: no retry, every tool armed', async () => {
		const screen = render(ScanSurface, { props: { model } });
		expect(screen.container.querySelector('[data-testid="scan-retry"]')).toBeNull();
		await expect.element(page.getByRole('button', { name: 'Torch' })).toBeEnabled();
	});
});
