/**
 * The artwork opens the viewer wherever it is drawn (founder call,
 * 2026-09-05): given its seed it is a button that asks the resident store;
 * without one it stays a picture. A `.svelte.test.ts` for the browser
 * project, because the button is the browser's to click.
 */
import { afterEach, describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { identiconViewer } from '../identicon-viewer.svelte';
import Identicon from './Identicon.svelte';

const SVG = '<svg viewBox="0 0 1 1"></svg>';
const ADDRESS = '0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c';

afterEach(() => {
	identiconViewer.close();
	identiconViewer.openLabel = '';
});

describe('Identicon', () => {
	it('is a picture without a seed', () => {
		const screen = render(Identicon, { props: { svg: SVG, label: 'Alice' } });
		expect(screen.container.querySelector('button')).toBeNull();
		expect(screen.container.querySelector('[role="img"]')?.getAttribute('aria-label')).toBe(
			'Alice'
		);
	});

	it('opens the viewer on its seed when it has one', async () => {
		identiconViewer.openLabel = 'View identicon';
		const screen = render(Identicon, { props: { svg: SVG, address: ADDRESS, label: 'Alice' } });
		const button = screen.container.querySelector('button');
		expect(button).not.toBeNull();
		// Named by the corpus, never by the row's own name — a row beside it is
		// the button that carries the name, and two buttons named "Alice" would
		// be one control to a screen reader.
		expect(button?.getAttribute('aria-label')).toBe('View identicon');
		button?.click();
		expect(identiconViewer.current).toEqual({ address: ADDRESS, identiconSvg: SVG });
	});

	it('stays a picture for a blank seed or blank artwork', () => {
		const blankSeed = render(Identicon, { props: { svg: SVG, address: '' } });
		expect(blankSeed.container.querySelector('button')).toBeNull();
		const blankArt = render(Identicon, { props: { svg: '', address: ADDRESS } });
		expect(blankArt.container.querySelector('button')).toBeNull();
	});
});
