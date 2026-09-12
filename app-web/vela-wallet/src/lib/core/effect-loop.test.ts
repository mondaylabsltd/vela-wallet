/**
 * Spec 048: an answer the core refuses to read is not a hang. The loop tells
 * `onError` and answers the same effect with its failure — once.
 */
import { describe, expect, it, vi } from 'vitest';
import { createEffectLoop, type CoreResult, type EffectCore } from './effect-loop';

type View = { stage: string };
type Effect = { id: number; operation: { type: string } };
type Result = { type: string };

function refusingCore(): EffectCore<View, string, Effect, Result> & { resolved: Result[] } {
	const resolved: Result[] = [];
	let stage = 'idle';
	const done = (effects: Effect[] = []): CoreResult<View, Effect> => ({ view: { stage }, effects });
	return {
		resolved,
		view: () => ({ stage }),
		dispatch(event) {
			stage = event;
			return done(event === 'load' ? [{ id: 7, operation: { type: 'load_accounts' } }] : []);
		},
		resolve(_id, result) {
			resolved.push(result);
			if (result.type === 'accounts_loaded') throw new Error('invalid result from shell: missing field');
			stage = result.type;
			return done();
		}
	};
}

describe('createEffectLoop', () => {
	it('answers a refused result with the effect’s failure, once, and reports it', async () => {
		const core = refusingCore();
		const onError = vi.fn();
		const views: View[] = [];
		const loop = createEffectLoop(core, {
			onView: (v) => views.push(v),
			execute: async () => ({ type: 'accounts_loaded' }),
			toFailure: (effect) => ({ type: `${effect.operation.type}_failed` }),
			onError
		});
		loop.start('load');
		await new Promise((r) => setTimeout(r, 0));
		expect(onError).toHaveBeenCalledTimes(1);
		expect(core.resolved.map((r) => r.type)).toEqual(['accounts_loaded', 'load_accounts_failed']);
		expect(views.at(-1)?.stage).toBe('load_accounts_failed');
		loop.dispose();
	});

	it('does not loop when the failure itself is refused', async () => {
		const core = refusingCore();
		core.resolve = () => {
			throw new Error('refused');
		};
		const onError = vi.fn();
		const loop = createEffectLoop(core, {
			onView: () => {},
			execute: async () => ({ type: 'accounts_loaded' }),
			toFailure: () => ({ type: 'storage_failed' }),
			onError
		});
		loop.start('load');
		await new Promise((r) => setTimeout(r, 0));
		expect(onError).toHaveBeenCalledTimes(2);
		loop.dispose();
	});
});
