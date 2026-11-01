/**
 * r11-2: a request the side panel took, whose sheet could not be built, is
 * answered — never left owed behind the plain wallet with the site waiting
 * for good. The answer is the truth: nothing was shown or signed, and the
 * site may ask again; it is never "the user rejected it" (4001).
 */
import { describe, expect, it, vi } from 'vitest';

const answers: { rid: string; answer: unknown }[] = [];
vi.mock('./panel-surface.svelte', () => ({
	panelSurface: {
		caller: true,
		answer: vi.fn(async (rid: string, answer: unknown) => {
			answers.push({ rid, answer });
			return true;
		})
	}
}));

const { answerNotShown, NOT_SHOWN_ERROR, USER_REJECTED } = await import('./transport');

describe('a request taken and not shown (r11-2)', () => {
	it('is answered with an internal error the site may retry, never a rejection', async () => {
		expect(await answerNotShown('rid-1')).toBe(true);
		expect(answers).toEqual([{ rid: 'rid-1', answer: { error: { ...NOT_SHOWN_ERROR } } }]);
		expect(NOT_SHOWN_ERROR.code).toBe(-32603);
		expect(NOT_SHOWN_ERROR.code).not.toBe(USER_REJECTED);
	});
});
