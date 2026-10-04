/**
 * Issue 409 — when a publish is answered.
 *
 * A one-key create is answered at the registry's 202 with the task it was
 * queued under (the session's landing watch confirms the landing later); a
 * multi-key create and every re-publish still wait for the landing; a group
 * already on-chain is landed either way.
 */
import { describe, expect, it } from 'vitest';
import { afterRegister } from './publish';

describe('a publish that asked is answered on acceptance (issue 409)', () => {
	it('answers a one-key create with its task at the 202', () => {
		expect(afterRegister({ id: 't1', status: 'pending' }, true)).toEqual({
			kind: 'accepted',
			taskId: 't1'
		});
	});

	it('still waits for the landing when it was not asked to', () => {
		expect(afterRegister({ id: 't1', status: 'pending' }, false)).toEqual({
			kind: 'await_landing',
			taskId: 't1'
		});
	});

	it('treats a group already on-chain as landed, and a 202 without a task as a failure', () => {
		for (const asked of [true, false]) {
			expect(afterRegister({ status: 'done' }, asked)).toEqual({ kind: 'landed' });
			// Nothing to confirm the landing by.
			expect(() => afterRegister({ status: 'pending' }, asked)).toThrow(/without a task id/);
		}
	});
});
