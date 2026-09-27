/**
 * THE bottom sheet's behaviour, in a browser (founder, 2026-09-27: one sheet,
 * like Android and iOS): the drag from the grabber and from the content, the
 * dismissible gate, scrim / ✕ / Escape through one close path, and focus in
 * and back out. The numbers themselves are sheet-gesture.test.ts's.
 */
import { createRawSnippet, tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import BottomSheet from './BottomSheet.svelte';
import { SLOP } from './sheet-gesture';

type Variant = 'wallet' | 'menu' | 'signing' | 'prompt';

const body = (html: string) => createRawSnippet(() => ({ render: () => html }));
const pause = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

async function drawn(
	props: {
		variant?: Variant;
		dismissible?: boolean;
		html?: string;
		closeLabel?: string;
	} = {}
) {
	const onclose = vi.fn();
	const host = document.createElement('div');
	host.style.cssText = 'position: relative; width: 390px; height: 844px; overflow: hidden;';
	document.body.appendChild(host);
	const opener = document.createElement('button');
	opener.textContent = 'open';
	document.body.appendChild(opener);
	opener.focus();
	const screen = render(BottomSheet, {
		target: host,
		props: {
			title: 'Test sheet',
			closeLabel: props.closeLabel ?? 'Close',
			variant: props.variant ?? 'wallet',
			dismissible: props.dismissible ?? true,
			onclose,
			children: body(
				props.html ??
					'<div><p>Body</p><button type="button" class="a">A</button><button type="button" class="b">B</button></div>'
			)
		}
	});
	await tick();
	// Past the entry animation.
	await pause(300);
	const sheet = host.querySelector<HTMLElement>('[role="dialog"]')!;
	const grip = sheet.querySelector<HTMLElement>('.handle')!;
	const content = sheet.querySelector<HTMLElement>('.content')!;
	const scrim = host.querySelector<HTMLElement>('.scrim')!;
	const top = () => sheet.getBoundingClientRect().top;
	const rest = top();
	const height = sheet.getBoundingClientRect().height;
	return { screen, host, opener, sheet, grip, content, scrim, onclose, top, rest, height };
}

/** A finger on the grabber, dragged `dy` over `steps` moves `gap` ms apart. */
async function dragGrip(grip: HTMLElement, dy: number, steps = 10, gap = 16) {
	const box = grip.getBoundingClientRect();
	const x = box.left + box.width / 2;
	const y0 = box.top + 6;
	const fire = (type: string, y: number) =>
		grip.dispatchEvent(
			new PointerEvent(type, {
				pointerId: 7,
				pointerType: 'touch',
				isPrimary: true,
				clientX: x,
				clientY: y,
				bubbles: true,
				cancelable: true
			})
		);
	fire('pointerdown', y0);
	for (let i = 1; i <= steps; i++) {
		fire('pointermove', y0 + (dy * i) / steps);
		if (gap) await pause(gap);
	}
	fire('pointerup', y0 + dy);
	await tick();
}

/** A finger on the content, dragged `dy` (touch events, as a phone sends them). */
async function dragContent(target: HTMLElement, dy: number, steps = 10, gap = 16) {
	const box = target.getBoundingClientRect();
	const x = box.left + box.width / 2;
	const y0 = box.top + Math.min(20, box.height / 2);
	const point = (y: number) =>
		new Touch({ identifier: 3, target, clientX: x, clientY: y, pageX: x, pageY: y });
	const fire = (type: string, y: number | null) => {
		const touches = y === null ? [] : [point(y)];
		target.dispatchEvent(
			new TouchEvent(type, {
				touches,
				targetTouches: touches,
				changedTouches: [point(y ?? y0 + dy)],
				bubbles: true,
				cancelable: true
			})
		);
	};
	fire('touchstart', y0);
	for (let i = 1; i <= steps; i++) {
		fire('touchmove', y0 + (dy * i) / steps);
		if (gap) await pause(gap);
	}
	fire('touchend', null);
	await tick();
}

describe('BottomSheet — one sheet, every skin', () => {
	for (const variant of ['wallet', 'menu', 'signing', 'prompt'] as const) {
		it(`${variant}: draws a grabber and closes on a long drag from it`, async () => {
			const view = await drawn({ variant });
			expect(view.sheet.querySelector('.handle')).not.toBeNull();
			await dragGrip(view.grip, view.height * 0.5);
			await vi.waitFor(() => expect(view.onclose).toHaveBeenCalledOnce(), { timeout: 1500 });
			await view.screen.unmount();
		});
	}
});

describe('BottomSheet — the drag', () => {
	it('follows the finger, and a short drag springs back without closing', async () => {
		const view = await drawn();
		// Short: well under the 28% line, slowly (no flick).
		await dragGrip(view.grip, Math.max(SLOP + 6, view.height * 0.15), 10, 24);
		await pause(500);
		expect(view.onclose).not.toHaveBeenCalled();
		expect(Math.abs(view.top() - view.rest)).toBeLessThan(1);
		await view.screen.unmount();
	});

	it('closes on a fast downward flick, even a short one', async () => {
		const view = await drawn();
		await dragGrip(view.grip, 70, 4, 0);
		await vi.waitFor(() => expect(view.onclose).toHaveBeenCalledOnce(), { timeout: 1500 });
		await view.screen.unmount();
	});

	it('rubber-bands upward and comes back', async () => {
		const view = await drawn();
		const box = view.grip.getBoundingClientRect();
		const fire = (type: string, y: number) =>
			view.grip.dispatchEvent(
				new PointerEvent(type, {
					pointerId: 9,
					pointerType: 'touch',
					clientX: box.left + 10,
					clientY: y,
					bubbles: true
				})
			);
		fire('pointerdown', box.top + 6);
		for (let i = 1; i <= 10; i++) {
			fire('pointermove', box.top + 6 - 20 * i);
			await pause(10);
		}
		// Pulled up 200: it moved, but by less.
		const lifted = view.rest - view.top();
		expect(lifted).toBeGreaterThan(10);
		expect(lifted).toBeLessThan(200);
		fire('pointerup', box.top + 6 - 200);
		await pause(500);
		expect(Math.abs(view.top() - view.rest)).toBeLessThan(1);
		expect(view.onclose).not.toHaveBeenCalled();
		await view.screen.unmount();
	});

	it('drags from the content when it is at its top', async () => {
		const view = await drawn({ html: '<p style="height: 40px">Short body</p>' });
		await dragContent(view.content.firstElementChild as HTMLElement, view.height * 0.5);
		await vi.waitFor(() => expect(view.onclose).toHaveBeenCalledOnce(), { timeout: 1500 });
		await view.screen.unmount();
	});

	it('leaves a pull-down to the content while it is scrolled', async () => {
		const view = await drawn({
			html: '<div style="height: 3000px">Long body</div>'
		});
		view.content.scrollTop = 200;
		await dragContent(view.content.firstElementChild as HTMLElement, view.height * 0.5);
		await pause(500);
		expect(view.onclose).not.toHaveBeenCalled();
		await view.screen.unmount();
	});

	it('never takes a drag that starts in a text field', async () => {
		const view = await drawn({ html: '<input class="field" value="hello" />' });
		await dragContent(view.content.querySelector('.field') as HTMLElement, view.height * 0.5);
		await pause(500);
		expect(view.onclose).not.toHaveBeenCalled();
		expect(Math.abs(view.top() - view.rest)).toBeLessThan(1);
		await view.screen.unmount();
	});
});

describe('BottomSheet — the dismissible gate', () => {
	it('resists a long drag and comes back, with nothing closing it', async () => {
		const view = await drawn({ dismissible: false });
		const box = view.grip.getBoundingClientRect();
		const fire = (type: string, y: number) =>
			view.grip.dispatchEvent(
				new PointerEvent(type, {
					pointerId: 5,
					pointerType: 'touch',
					clientX: box.left + 10,
					clientY: y,
					bubbles: true
				})
			);
		fire('pointerdown', box.top + 6);
		for (let i = 1; i <= 10; i++) {
			fire('pointermove', box.top + 6 + view.height * 0.06 * i);
			await pause(16);
		}
		// Pulled 60% of its height: it gave, but far less than the finger moved.
		const moved = view.top() - view.rest;
		expect(moved).toBeGreaterThan(5);
		expect(moved).toBeLessThan(view.height * 0.6 * 0.7);
		fire('pointerup', box.top + 6 + view.height * 0.6);
		await pause(500);
		expect(view.onclose).not.toHaveBeenCalled();
		expect(Math.abs(view.top() - view.rest)).toBeLessThan(1);

		// A flick, the scrim, Escape, the ✕: nothing.
		await dragGrip(view.grip, 90, 3, 0);
		view.scrim.click();
		window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
		const close = view.sheet.querySelector<HTMLButtonElement>('.close')!;
		expect(close.disabled).toBe(true);
		await pause(500);
		expect(view.onclose).not.toHaveBeenCalled();
		expect(view.sheet.isConnected).toBe(true);
		await view.screen.unmount();
	});

	it('the host can still close it (its own confirm)', async () => {
		const view = await drawn({ dismissible: false });
		(view.screen.component as unknown as { requestClose: () => void }).requestClose();
		await vi.waitFor(() => expect(view.onclose).toHaveBeenCalledOnce(), { timeout: 1500 });
		await view.screen.unmount();
	});
});

describe('BottomSheet — the other doors, one close path', () => {
	it('the scrim closes it (after the exit)', async () => {
		const view = await drawn();
		view.scrim.click();
		expect(view.onclose).not.toHaveBeenCalled(); // the exit plays first
		await vi.waitFor(() => expect(view.onclose).toHaveBeenCalledOnce(), { timeout: 1500 });
		await view.screen.unmount();
	});

	it('the ✕ closes it', async () => {
		const view = await drawn();
		view.sheet.querySelector<HTMLButtonElement>('.close')!.click();
		await vi.waitFor(() => expect(view.onclose).toHaveBeenCalledOnce(), { timeout: 1500 });
		await view.screen.unmount();
	});

	it('Escape closes it — but not while an IME is composing', async () => {
		const view = await drawn();
		window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', isComposing: true }));
		await pause(400);
		expect(view.onclose).not.toHaveBeenCalled();
		window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
		await vi.waitFor(() => expect(view.onclose).toHaveBeenCalledOnce(), { timeout: 1500 });
		await view.screen.unmount();
	});

	it('closes once, however many doors are knocked on', async () => {
		const view = await drawn();
		view.scrim.click();
		view.sheet.querySelector<HTMLButtonElement>('.close')!.click();
		window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
		await pause(700);
		expect(view.onclose).toHaveBeenCalledOnce();
		await view.screen.unmount();
	});
});

describe('BottomSheet — focus', () => {
	it('moves focus in, keeps Tab inside, and gives it back to the opener', async () => {
		const view = await drawn();
		expect(view.sheet.contains(document.activeElement)).toBe(true);
		const buttons = [...view.sheet.querySelectorAll<HTMLButtonElement>('button')];
		const last = buttons[buttons.length - 1];
		last.focus();
		window.dispatchEvent(
			new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true })
		);
		expect(document.activeElement).toBe(buttons[0]);
		buttons[0].focus();
		window.dispatchEvent(
			new KeyboardEvent('keydown', { key: 'Tab', shiftKey: true, bubbles: true, cancelable: true })
		);
		expect(document.activeElement).toBe(last);
		await view.screen.unmount();
		expect(document.activeElement).toBe(view.opener);
	});
});
