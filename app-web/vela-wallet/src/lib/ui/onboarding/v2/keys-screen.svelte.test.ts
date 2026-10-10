/**
 * The create flow's "Add passkeys" screen, in a real browser (issue 475).
 *
 * With no key yet the screen drew a "+ Add a passkey" button ABOVE the three
 * places, which were already open and could not fold: the button said what
 * the places said and did nothing. The core now names ONE heading for the
 * three places (`CreateView.add_heading_key`) and says when they are held
 * open (`methods_pinned`); this draws exactly that, and nothing beside it.
 *
 * A `.svelte.test.ts` because it is about what a person sees and can tap; the
 * strings are the REAL corpus.
 */
import { tick } from 'svelte';
import { beforeAll, describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { loadCore } from '$lib/core/client';
import { CREATE_FIXTURES } from '$lib/onboarding/v2-fixtures';
import type { CreateView } from '$lib/onboarding/generated/CreateView';
import type { KeyMethod } from '$lib/onboarding/generated/KeyMethod';
import en from '../../../../../../../assets/i18n/en.json';
import zh from '../../../../../../../assets/i18n/zh.json';
import KeysScreen from './KeysScreen.svelte';

beforeAll(() => loadCore());

const stringsIn =
	(catalog: unknown) =>
	(key: string, params?: Record<string, string | number>): string => {
		const value = key
			.split('.')
			.reduce<unknown>((node, part) => (node as Record<string, unknown>)?.[part], catalog);
		if (typeof value !== 'string') throw new Error(`no corpus value for "${key}"`);
		return Object.entries(params ?? {}).reduce(
			(text, [name, fill]) => text.replaceAll(`{{${name}}}`, String(fill)),
			value
		);
	};

/** A board from the fixtures — views the core emits (`v2-fixtures.ts`). */
const board = (code: string): CreateView => {
	const found = CREATE_FIXTURES.find((f) => f.code === code);
	if (!found) throw new Error(`no fixture ${code}`);
	return found.view;
};

function drawn(view: CreateView, catalog: unknown = en) {
	const picked: KeyMethod[] = [];
	const screen = render(KeysScreen, {
		props: {
			keys: view.keys,
			canAddKey: view.can_add_key,
			canFinish: view.can_finish,
			needsSecondKey: view.needs_second_key,
			busy: view.busy,
			maxKeys: 7,
			addMethods: view.add_methods,
			addHeadingKey: view.add_heading_key,
			methodsPinned: view.methods_pinned,
			strings: stringsIn(catalog),
			onAddKey: (method: KeyMethod) => picked.push(method),
			onConfirmKey: () => {},
			onRemoveKey: () => {},
			onFinish: () => {}
		}
	});
	const root = screen.container;
	return {
		root,
		picked,
		methods: () => [...root.querySelectorAll<HTMLButtonElement>('button.method')],
		fold: () => root.querySelector<HTMLButtonElement>('button.fold'),
		heading: () => root.querySelector<HTMLElement>('.add > h2'),
		/** Every "+" glyph drawn on the screen. */
		pluses: () => [...root.querySelectorAll('.plus')]
	};
}

describe('no key yet: one plain heading over the three places, open', () => {
	it('draws the core’s heading as a heading — not a button — and no "+" anywhere', () => {
		const view = drawn(board('K0'));
		expect(board('K0').methods_pinned).toBe(true);
		expect(view.heading()?.textContent).toBe('Add a passkey');
		// The three places stand open, and they are the only things to tap
		// besides the screen's own Create.
		expect(view.methods().map((b) => b.querySelector('.name')?.textContent)).toEqual([
			'This device',
			'Phone or tablet',
			'USB security key'
		]);
		expect(view.fold()).toBeNull();
		expect(view.pluses()).toEqual([]);
		// The duplicate is gone: nothing between the heading and the places.
		const add = view.root.querySelector('.add') as HTMLElement;
		expect([...add.querySelectorAll('button')]).toEqual(view.methods());
	});

	it('the first place tapped is the one the core hears', () => {
		const view = drawn(board('K0'));
		view.methods()[1].click();
		expect(view.picked).toEqual(['hybrid']);
	});

	it('reads the heading in the person’s language', () => {
		expect(drawn(board('K0'), zh).heading()?.textContent).toBe('添加通行密钥');
	});
});

describe('a key or more: the heading is the fold', () => {
	it('"+ Add another", closed until tapped — then the three places, and closed again after a pick', async () => {
		const view = drawn(board('K1'));
		expect(board('K1').methods_pinned).toBe(false);
		const fold = view.fold()!;
		expect(view.heading()).toBeNull();
		expect(fold.textContent?.replace(/\s+/g, ' ').trim()).toBe('+ Add another');
		expect(view.pluses()).toHaveLength(1);
		expect(fold.getAttribute('aria-expanded')).toBe('false');
		expect(view.methods()).toEqual([]);

		fold.click();
		await tick();
		expect(fold.getAttribute('aria-expanded')).toBe('true');
		expect(view.methods()).toHaveLength(3);

		view.methods()[2].click();
		await tick();
		expect(view.picked).toEqual(['security_key']);
		expect(view.methods()).toEqual([]);
		expect(fold.getAttribute('aria-expanded')).toBe('false');
	});

	it('the same tap that opened it closes it', async () => {
		const view = drawn(board('K3'));
		const fold = view.fold()!;
		fold.click();
		await tick();
		expect(view.methods()).toHaveLength(3);
		fold.click();
		await tick();
		expect(view.methods()).toEqual([]);
	});

	it('says "再添加一把" in Chinese', () => {
		expect(drawn(board('K1'), zh).fold()?.textContent).toContain('再添加一把');
	});
});

describe('seven keys: a statement, and nothing under it', () => {
	it('"Limit of 7 reached": no "+", no fold to open, no places', async () => {
		const view = drawn(board('K5'));
		const fold = view.fold()!;
		expect(fold.textContent?.trim()).toBe('Limit of 7 reached');
		expect(fold.disabled).toBe(true);
		expect(fold.hasAttribute('aria-expanded')).toBe(false);
		expect(view.pluses()).toEqual([]);
		fold.click();
		await tick();
		expect(view.methods()).toEqual([]);
	});
});

describe('the words are the core’s key, never this screen’s choice', () => {
	it('draws whatever key the view names', () => {
		const view = drawn({ ...board('K1'), add_heading_key: 'onboarding.create.keyLimitReached' });
		expect(view.fold()?.textContent).toContain('Limit of 7 reached');
		// …and holds the places open exactly when the core says so, whatever
		// the list holds.
		const pinned = drawn({ ...board('K1'), methods_pinned: true });
		expect(pinned.fold()).toBeNull();
		expect(pinned.methods()).toHaveLength(3);
		expect(pinned.heading()?.textContent).toBe('Add another');
	});

	it('the fixtures’ headings are the ones the core computes for their key counts', () => {
		const heading = (code: string) => [
			board(code).keys.length,
			board(code).add_heading_key,
			board(code).methods_pinned
		];
		expect(heading('K0')).toEqual([0, 'onboarding.create.addKeyBtn', true]);
		expect(heading('K1')).toEqual([1, 'onboarding.create.addMethodLabel', false]);
		expect(heading('K3')).toEqual([3, 'onboarding.create.addMethodLabel', false]);
		expect(heading('K5')).toEqual([7, 'onboarding.create.keyLimitReached', false]);
	});
});
