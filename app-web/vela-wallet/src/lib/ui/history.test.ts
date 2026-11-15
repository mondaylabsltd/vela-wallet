/**
 * SvelteKit 3's shallow `goto` is asynchronous and refuses by REJECTING, where
 * SvelteKit 2's `pushState`/`replaceState` ran at once and threw. The writes
 * must still land in the order they were asked for, and on a refusal (before
 * the router has started; an address no route matches, the packaged
 * extension's `/en/wallet.html`) the platform's own history call must do the
 * job, with no unhandled rejection.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const goto = vi.fn();
vi.mock('$app/navigation', () => ({ goto: (...args: unknown[]) => goto(...args) }));
vi.mock('$app/state', () => ({ page: { state: { velaFlow: 2 } } }));

const { markHistory, replaceUrl } = await import('./history.ts');

const platform = { state: { kit: 'entry' }, pushState: vi.fn(), replaceState: vi.fn() };

beforeEach(() => {
	vi.stubGlobal('history', platform);
	goto.mockReset();
	platform.pushState.mockReset();
	platform.replaceState.mockReset();
});
afterEach(() => vi.unstubAllGlobals());

const settled = () => new Promise((resolve) => setTimeout(resolve, 0));

describe('markHistory', () => {
	it('is a shallow navigation to the same address, through the router', async () => {
		goto.mockResolvedValue(undefined);
		markHistory({ velaFlow: 3 });
		await settled();
		expect(goto).toHaveBeenCalledWith('', { shallow: true, state: { velaFlow: 3 } });
		expect(platform.pushState).not.toHaveBeenCalled();
	});

	it('falls back to the platform when the router refuses', async () => {
		goto.mockRejectedValue(new Error('No route found for /en/wallet.html'));
		markHistory({ velaFlow: 3 });
		await settled();
		expect(platform.pushState).toHaveBeenCalledWith({ velaFlow: 3 }, '');
	});
});

describe('replaceUrl', () => {
	it('replaces the entry through the router, keeping the page state', async () => {
		goto.mockResolvedValue(undefined);
		replaceUrl('/en/wallet?tab=1');
		await settled();
		expect(goto).toHaveBeenCalledWith('/en/wallet?tab=1', {
			shallow: true,
			replace: true,
			state: { velaFlow: 2 }
		});
		expect(platform.replaceState).not.toHaveBeenCalled();
	});

	it('falls back to the platform when the router refuses', async () => {
		goto.mockRejectedValue(new Error('Cannot call goto(...) before router is initialized'));
		replaceUrl('/en/wallet');
		await settled();
		expect(platform.replaceState).toHaveBeenCalledWith({ kit: 'entry' }, '', '/en/wallet');
	});
});

describe('in turn', () => {
	it('starts a write only once the one before it has landed', async () => {
		const order: string[] = [];
		let land!: () => void;
		goto.mockImplementationOnce(
			(url: string) =>
				new Promise<void>((resolve) => {
					order.push(`start ${url}`);
					land = () => {
						order.push(`land ${url}`);
						resolve();
					};
				})
		);
		goto.mockImplementation(async (url: string) => {
			order.push(`start ${url || '(here)'}`);
		});
		const dropped = replaceUrl('/en/wallet');
		const marked = markHistory({ velaFlow: 1 });
		await settled();
		expect(order).toEqual(['start /en/wallet']);
		land();
		await Promise.all([dropped, marked]);
		expect(order).toEqual(['start /en/wallet', 'land /en/wallet', 'start (here)']);
	});
});
