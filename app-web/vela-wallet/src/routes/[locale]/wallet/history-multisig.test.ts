/**
 * r4D-17 / r5reg-9: one multisig execution, two screens — the home's row
 * opened the multisig's sheet (its timeline), the History screen's row the
 * plain "Transaction Details" with "Delete record". Both open the sheet now:
 * the History screen's tap (`onnavigate('tx-detail', index)`, on the phone's
 * flow host and the desktop's) asks `openedAsMultisig` before it pushes the
 * plain detail. Read from its source, the way `sheet-close.test.ts` pins
 * wiring.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const source = readFileSync(join(import.meta.dirname, '+page.svelte'), 'utf8');
/** The code, not its comments. */
const code = (text: string) => text.replace(/^\s*\/\/.*$/gm, '');

describe('a multisig execution on the History screen', () => {
	it('every flow host opens the multisig sheet before it pushes the plain detail', () => {
		const handlers = [...source.matchAll(/onnavigate=\{\(to, index\) => \{([\s\S]*?)\n\t*\}\}/g)];
		expect(handlers.length, 'the phone and the desktop flow hosts').toBe(2);
		for (const [, body] of handlers) {
			const lines = code(body).trim().split('\n');
			expect(lines[0]).toMatch(/noteTarget\(to, index\)/);
			expect(lines[1]).toMatch(/if \(openedAsMultisig\(to\)\) return;/);
		}
	});

	it('opens the sheet the home row opens, for the row the History screen named', () => {
		const fn = /function openedAsMultisig\(to: string\): boolean \{([\s\S]*?)\n\t\}/.exec(source);
		expect(fn).not.toBeNull();
		const body = code(fn![1]);
		expect(body).toMatch(/to !== 'tx-detail'/);
		expect(body).toMatch(/multisigOfRow\(selectedTxId/);
		expect(body).toMatch(/multisigUi\.open\(\{ kind: 'proposal', \.\.\.proposal \}\)/);
		// The home's tap reads the row the same way.
		const home = /function openActivity\(row: \{ id\?: string \}\) \{([\s\S]*?)\n\t\}/.exec(source);
		expect(code(home![1])).toMatch(/multisigOfRow\(row\.id\)/);
	});
});
