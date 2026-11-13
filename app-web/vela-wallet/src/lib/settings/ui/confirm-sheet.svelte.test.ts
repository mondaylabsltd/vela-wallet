/**
 * ob-2: the confirm sheet's explicit acknowledgement.
 *
 * Erasing means everything, so a wallet not yet synced would go with it.
 * The sheet then draws that loss by name (`warning`) and keeps its confirm
 * disabled until the person ticks that they understand it (`ack`). Without
 * either, it is the sheet it always was.
 */
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { tick } from 'svelte';
import '$lib/tokens/tokens.css';
import ConfirmSheet from './ConfirmSheet.svelte';
import type { ConfirmSheetModel } from '../model';

const base: ConfirmSheetModel = {
	title: 'Erase This Device',
	body: 'Everything Vela stored on this device is deleted.',
	confirm: 'Erase Everything',
	cancel: 'Cancel',
	tone: 'danger'
};

function drawn(sheet: ConfirmSheetModel) {
	const onconfirm = vi.fn();
	const screen = render(ConfirmSheet, { props: { sheet, onconfirm } });
	const confirm = () =>
		[...screen.container.querySelectorAll('button')].find(
			(button) => button.textContent?.trim() === sheet.confirm
		) as HTMLButtonElement;
	return { root: screen.container, onconfirm, confirm };
}

describe('the confirm sheet', () => {
	it('confirms at once when nothing needs acknowledging', () => {
		const { confirm, onconfirm, root } = drawn(base);
		expect(root.querySelector('input[type="checkbox"]')).toBeNull();
		expect(confirm().disabled).toBe(false);
		confirm().click();
		expect(onconfirm).toHaveBeenCalledOnce();
	});

	it('names the loss, and waits for the box before it confirms', async () => {
		const { confirm, onconfirm, root } = drawn({
			...base,
			warning: 'Not finished syncing: Everyday.',
			ack: 'I understand an unsynced wallet may be lost for good'
		});
		expect(root.textContent).toContain('Not finished syncing: Everyday.');
		expect(confirm().disabled).toBe(true);
		confirm().click();
		expect(onconfirm).not.toHaveBeenCalled();

		root.querySelector<HTMLInputElement>('input[type="checkbox"]')?.click();
		await tick();
		expect(confirm().disabled).toBe(false);
		confirm().click();
		expect(onconfirm).toHaveBeenCalledOnce();
	});
});
