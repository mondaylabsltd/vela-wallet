/**
 * vis-19: a desktop dialog leaves with the motion it came in with — and a
 * person who asked for reduced motion gets none either way.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { MOTION } from '$lib/tokens/tokens';
import { dialogPop, scrimFade } from './dialog-motion';

const node = {} as Element;

afterEach(() => vi.unstubAllGlobals());

describe('dialog motion', () => {
	it('takes the fast motion token, and none under reduced motion', () => {
		vi.stubGlobal('matchMedia', () => ({ matches: false }));
		expect(scrimFade().duration).toBe(MOTION.fast);
		expect(dialogPop(node).duration).toBe(MOTION.fast);
		vi.stubGlobal('matchMedia', () => ({ matches: true }));
		expect(scrimFade().duration).toBe(0);
		expect(dialogPop(node).duration).toBe(0);
	});

	it('keeps a centred card on its centre while it scales', () => {
		vi.stubGlobal('matchMedia', () => ({ matches: false }));
		const css = dialogPop(node, { centred: true }).css!;
		expect(css(0, 1)).toBe('opacity: 0; transform: translate(-50%, -50%) scale(0.97)');
		expect(css(1, 0)).toBe('opacity: 1; transform: translate(-50%, -50%) scale(1)');
		expect(dialogPop(node).css!(1, 0)).toBe('opacity: 1; transform: scale(1)');
	});
});
