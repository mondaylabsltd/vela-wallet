/**
 * Issue #328: the wallet page's half of a closed flow sheet. `FlowsMobile`
 * reports every close (FlowsMobile.svelte.test.ts); the page must pop the
 * sheet's step, and end what the machines hold open for it, or the next tap
 * on a row — or on 导入 — finds the sheet still "open" and opens nothing.
 * Read from its source, the way `feed-filter.test.ts` pins wiring.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

describe('a closed flow sheet, on the wallet page', () => {
	it('pops the sheet’s step and closes what the machines hold open', () => {
		const source = readFileSync(join(import.meta.dirname, '+page.svelte'), 'utf8');
		const handler = /onsheetclose=\{\(\) => \{([\s\S]*?)\n\t*\}\}/.exec(source);
		expect(handler, 'the phone flow host hears a closed sheet').not.toBeNull();
		// The handler's code, not its comments.
		const body = handler![1].replace(/^\s*\/\/.*$/gm, '');
		expect(body).toMatch(/nav\.sheetClosed\(flowState\)/);
		expect(body).toMatch(/closeAddToken\(\)/);
		expect(body).toMatch(/close_contact_picker/);
		expect(body).toMatch(/feeSheetOpen = false/);
		expect(body).toMatch(/closeBatch\(\)/);
	});
});
