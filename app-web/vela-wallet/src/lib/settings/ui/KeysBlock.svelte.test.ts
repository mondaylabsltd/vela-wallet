/**
 * The key this device signs with stands out in Settings' keys list (founder,
 * 2026-09-26), in a real browser: the one FILLED pill, first among the row's
 * pills, with a check — and on no other row. A `.svelte.test.ts` because the
 * point is what a person sees; the node project does not render components.
 */
import { beforeAll, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { loadCore } from '$lib/core/client';
import type { CreateKeyRow } from '$lib/onboarding/generated/CreateKeyRow';
import type { WalletKeyRowModel, WalletKeysModel } from '../model';
import KeysBlock from './KeysBlock.svelte';

beforeAll(() => loadCore());

const key: CreateKeyRow = {
	name: '',
	authenticator_attachment: 'platform',
	transports: 'internal',
	confirmed: true,
	synced: false,
	synced_known: true,
	aaguid: '',
	provider_name: '',
	method: 'platform',
	kind: 'platform'
};

const row = (name: string, pills: WalletKeyRowModel['pills']): WalletKeyRowModel => ({
	name,
	holderFallback: 'Built-in passkey',
	fingerprint: '197d…647b',
	pills,
	details: [],
	key: { ...key, name }
});

const model: WalletKeysModel = {
	title: 'Keys',
	subtitle: 'Any one of these passkeys can sign for this wallet, on every network.',
	count: '3',
	loading: false,
	rows: [
		row('Parallel One', [{ text: 'Device-bound', tone: 'local' }]),
		row('Parallel Two', [
			{ text: 'Signed in', tone: 'signs_here' },
			{ text: 'Device-bound', tone: 'local' }
		]),
		row('Parallel Three', [{ text: 'Device-bound', tone: 'local' }])
	],
	copy: { action: 'Copy', done: 'Copied' }
};

describe('the key this device signs with', () => {
	it('wears the one filled pill, first in its row, and no other row does', async () => {
		const screen = render(KeysBlock, { props: { model } });
		const rows = [...screen.container.querySelectorAll('li.key')];
		const marked = rows.filter((li) => li.querySelector('.pill[data-tone="signs_here"]'));
		expect(marked).toHaveLength(1);
		expect(marked[0].textContent).toContain('Parallel Two');

		const pills = [...marked[0].querySelectorAll('.pill')];
		expect(pills[0].getAttribute('data-tone')).toBe('signs_here');
		expect(pills[0].textContent?.trim()).toBe('Signed in');
		expect(pills[0].querySelector('svg')).not.toBeNull();

		// Filled, where the others are outlined — and the same height as they are.
		const filled = getComputedStyle(pills[0]);
		const outlined = getComputedStyle(pills[1]);
		expect(filled.backgroundColor).not.toBe('rgba(0, 0, 0, 0)');
		expect(outlined.backgroundColor).toBe('rgba(0, 0, 0, 0)');
		expect(filled.fontWeight).not.toBe(outlined.fontWeight);
		expect(pills[0].getBoundingClientRect().height).toBeCloseTo(
			pills[1].getBoundingClientRect().height,
			0
		);
	});
});

/**
 * The row for the record's copy on Ethereum: the core says the words, the
 * tone and what a tap does (`registry_backup::BackupRow`), and this draws
 * exactly that. "Not copied yet" was the warning colour; a copy is optional
 * and costs a fee, so it is a state, never a defect.
 */
describe('the Ethereum copy’s row', () => {
	const EXPLAIN = 'Your wallet’s record on Gnosis is public…';
	/** The row as the model carries it: the core's paragraph rides on the row. */
	const withBackup = (backup: WalletKeysModel['backup']): WalletKeysModel => ({
		...model,
		backup
	});
	const TITLE = "Copy this wallet's record to Ethereum";
	const drawn = (backup: NonNullable<WalletKeysModel['backup']>) => {
		const onbackup = vi.fn();
		const screen = render(KeysBlock, { props: { model: withBackup(backup), onbackup } });
		const button = screen.container.querySelector('button.backup') as HTMLButtonElement;
		const state = button.querySelector('.state') as HTMLElement;
		const name = button.querySelector('.name') as HTMLElement;
		return { screen, button, state, name, onbackup };
	};
	/** The ink "not copied yet" must NOT wear. */
	const warning = () => {
		const probe = document.createElement('span');
		probe.style.color = 'var(--color-warning-base)';
		document.body.appendChild(probe);
		const color = getComputedStyle(probe).color;
		probe.remove();
		return color;
	};

	it('not copied yet: a calm line, a chevron, and a tap that starts the copy', async () => {
		const { button, state, onbackup } = drawn({
			title: TITLE,
			subtitle: 'Not copied yet (optional)',
			tone: 'neutral',
			action: 'copy'
		});
		expect(button.textContent).toContain(TITLE);
		expect(state.textContent).toBe('Not copied yet (optional)');
		expect(getComputedStyle(state).color).not.toBe(warning());
		expect(button.disabled).toBe(false);
		button.click();
		expect(onbackup).toHaveBeenCalledTimes(1);
	});

	it('couldn’t check: tappable, and the glyph says "again", not "go"', async () => {
		const retry = drawn({
			title: TITLE,
			subtitle: "Couldn't check. Tap to try again.",
			tone: 'neutral',
			action: 'retry'
		});
		expect(retry.button.disabled).toBe(false);
		expect(retry.button.getAttribute('data-action')).toBe('retry');
		retry.button.click();
		expect(retry.onbackup).toHaveBeenCalledTimes(1);
	});

	it('an older wallet that can never be copied: a statement — no tap, no glyph, no warning', async () => {
		const { button, state, onbackup } = drawn({
			title: TITLE,
			subtitle: "This older wallet can't be copied",
			tone: 'neutral',
			action: 'none'
		});
		expect(button.disabled).toBe(true);
		button.click();
		expect(onbackup).not.toHaveBeenCalled();
		// Only the row's own upload mark: nothing trailing invites a tap.
		expect(button.querySelectorAll('svg')).toHaveLength(1);
		expect(getComputedStyle(state).color).not.toBe(warning());
	});

	// PR 3 note 6: the paragraph describes making the copy, and the core names
	// none for a wallet that can never be copied. Then nothing is drawn — and
	// no empty slot is left where it stood: the block ends on its row.
	it('the explanation is drawn when the row carries one, and leaves no gap when it does not', async () => {
		const row = {
			title: TITLE,
			subtitle: 'Not copied yet (optional)',
			tone: 'neutral',
			action: 'copy'
		} as const;
		const told = drawn({ ...row, explain: EXPLAIN });
		const paragraph = told.screen.container.querySelector('p.explain') as HTMLElement;
		expect(paragraph.textContent).toBe(EXPLAIN);
		const section = told.screen.container.querySelector('section') as HTMLElement;
		// The paragraph is the block's last thing, under the row.
		expect(section.getBoundingClientRect().bottom).toBeCloseTo(
			paragraph.getBoundingClientRect().bottom,
			0
		);
		told.screen.unmount();

		const never = drawn({
			title: TITLE,
			subtitle: "This older wallet can't be copied",
			tone: 'neutral',
			action: 'none'
		});
		expect(never.screen.container.querySelector('p.explain')).toBeNull();
		expect(never.screen.container.textContent).not.toContain(EXPLAIN);
		// No room kept for it: the block's bottom is the row's bottom.
		const block = never.screen.container.querySelector('section') as HTMLElement;
		expect(block.getBoundingClientRect().bottom).toBeCloseTo(
			never.button.getBoundingClientRect().bottom,
			0
		);
	});

	it('copied: the success ink and a tick, and nothing to tap', async () => {
		const { button, state } = drawn({
			title: TITLE,
			subtitle: 'Copied to Ethereum',
			tone: 'positive',
			action: 'none'
		});
		expect(button.disabled).toBe(true);
		expect(button.querySelector('.done')).not.toBeNull();
		const probe = document.createElement('span');
		probe.style.color = 'var(--color-success-base)';
		document.body.appendChild(probe);
		expect(getComputedStyle(state).color).toBe(getComputedStyle(probe).color);
		probe.remove();
	});

	it('the title and its line each keep to their own line at a phone’s width', async () => {
		const { screen, name, state } = drawn({
			title: TITLE,
			subtitle: 'Not copied yet (optional)',
			tone: 'neutral',
			action: 'copy'
		});
		screen.container.style.width = '342px';
		await new Promise((r) => requestAnimationFrame(() => r(null)));
		// The state line is ONE line (under two font-sizes tall); the title may
		// wrap but never clips.
		expect(state.getBoundingClientRect().height).toBeLessThan(
			parseFloat(getComputedStyle(state).fontSize) * 2
		);
		expect(name.scrollWidth).toBeLessThanOrEqual(name.clientWidth + 1);
	});
});
