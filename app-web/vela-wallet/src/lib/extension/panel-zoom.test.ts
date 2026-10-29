import { describe, expect, it } from 'vitest';
import { holdPanelScale } from './panel-zoom';

/** A window's event surface, without a window. */
function target() {
	const events = new EventTarget();
	return {
		addEventListener: events.addEventListener.bind(events),
		removeEventListener: events.removeEventListener.bind(events),
		fire(type: string, init: { ctrlKey?: boolean } = {}): boolean {
			const event = new Event(type, { cancelable: true });
			Object.defineProperty(event, 'ctrlKey', { value: init.ctrlKey ?? false });
			events.dispatchEvent(event);
			return event.defaultPrevented;
		}
	};
}

describe('the side panel keeps its scale (round 3)', () => {
	it('a pinch — a wheel with ctrlKey, or Safari’s gesture — is refused', () => {
		const panel = target();
		holdPanelScale(panel as unknown as Window);
		expect(panel.fire('wheel', { ctrlKey: true })).toBe(true);
		expect(panel.fire('gesturestart')).toBe(true);
		expect(panel.fire('gesturechange')).toBe(true);
	});

	it('a plain wheel still scrolls', () => {
		const panel = target();
		holdPanelScale(panel as unknown as Window);
		expect(panel.fire('wheel')).toBe(false);
	});

	it('let go, it refuses nothing', () => {
		const panel = target();
		holdPanelScale(panel as unknown as Window)();
		expect(panel.fire('wheel', { ctrlKey: true })).toBe(false);
		expect(panel.fire('gesturestart')).toBe(false);
	});
});
