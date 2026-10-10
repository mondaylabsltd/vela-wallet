/**
 * The create flow's "Add passkeys" screen, in a real browser (issue 475).
 *
 * With no key yet the screen drew a "+ Add a passkey" button ABOVE the three
 * places, which were already open and could not fold: the button said what
 * the places said and did nothing. The core now names ONE heading for the
 * three places (`CreateView.add_heading_key`), says when they are held open
 * (`methods_pinned`) and when "Added n / 7" is drawn (`key_count_shown`);
 * this draws exactly that, and nothing beside it.
 *
 * A `.svelte.test.ts` because it is about what a person sees and can tap; the
 * strings are the REAL corpus.
 */
import { tick } from 'svelte';
import { beforeAll, describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { CreateWalletCore, loadCore } from '$lib/core/client';
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
			keyCountShown: view.key_count_shown,
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
		/** The "Added n / 7" head over the keys, when it is drawn. */
		counter: () => root.querySelector<HTMLElement>('.listhead'),
		title: () => root.querySelector<HTMLElement>('h1.title')!,
		subtitle: () => root.querySelector<HTMLElement>('.subtitle')!,
		/** Every "+" glyph drawn on the screen. */
		pluses: () => [...root.querySelectorAll('.plus')]
	};
}

describe('no key yet: one plain heading over the three places, open', () => {
	it('draws the core’s heading as a heading — not a button — and no "+" anywhere', () => {
		const view = drawn(board('K0'));
		expect(board('K0').methods_pinned).toBe(true);
		// "Choose where it lives" — the title above already says "Add passkeys",
		// and the heading used to say it again (PR 3 note 17).
		expect(view.heading()?.textContent).toBe('Choose where it lives');
		expect(view.heading()?.textContent).not.toBe(view.title().textContent);
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

	it('reads the heading in the person’s language — and not the title’s words twice', () => {
		const view = drawn(board('K0'), zh);
		expect(view.heading()?.textContent).toBe('选择存放位置');
		// 「添加通行密钥」 stood here too, right under a title that reads the same.
		expect(view.title().textContent?.trim()).toBe('添加通行密钥');
		expect(view.heading()?.textContent).not.toBe(view.title().textContent?.trim());
	});
});

/**
 * "Added n / 7" (PR 3 note 22). With no key it read "Added 0 / 7" over an
 * empty list — a count of nothing, beside a subtitle that already says "up to
 * 7" — and one app hid it while three showed it. The core says when it is
 * drawn (`CreateView.key_count_shown`: from the first key on); the screen has
 * no condition of its own.
 */
describe('the "Added n / 7" counter is the core’s to show', () => {
	it('no key: no counter, no "0 / 7", and no empty block left where the list would be', () => {
		expect(board('K0').key_count_shown).toBe(false);
		const view = drawn(board('K0'));
		expect(view.counter()).toBeNull();
		expect(view.root.textContent).not.toContain('0 / 7');
		expect(view.root.textContent).not.toMatch(/\bAdded\b/);
		// The list block takes no room: the heading over the three places sits
		// one gap of the screen's rhythm under the subtitle, not two.
		const list = view.root.querySelector('.list') as HTMLElement;
		expect(list.getBoundingClientRect().height).toBe(0);
		const screen = view.root.querySelector('.screen') as HTMLElement;
		const gap = parseFloat(getComputedStyle(screen).rowGap);
		expect(gap).toBeGreaterThan(0);
		const subtitleBottom = view.subtitle().getBoundingClientRect().bottom;
		const headingTop = view.heading()!.getBoundingClientRect().top;
		expect(headingTop - subtitleBottom).toBeCloseTo(gap, 0);
	});

	it('the first key: "Added 1 / 7" over it — and the title and subtitle have not moved', () => {
		expect(board('K1').key_count_shown).toBe(true);
		const none = drawn(board('K0'));
		const y = (el: HTMLElement) => Math.round(el.getBoundingClientRect().top * 10) / 10;
		const before = { title: y(none.title()), subtitle: y(none.subtitle()) };
		const one = drawn(board('K1'));
		// Rendered side by side in one page: compare each to its own root.
		const within = (view: ReturnType<typeof drawn>, el: HTMLElement) =>
			Math.round((el.getBoundingClientRect().top - view.root.getBoundingClientRect().top) * 10) /
			10;
		expect(within(one, one.title())).toBe(within(none, none.title()));
		expect(within(one, one.subtitle())).toBe(within(none, none.subtitle()));
		expect(before.title).toBeGreaterThanOrEqual(0);

		const counter = one.counter()!;
		expect(counter.textContent?.replace(/\s+/g, ' ').trim()).toBe('Added 1 / 7');
		// The counter is the first thing under the subtitle, the key right under it.
		const row = one.root.querySelector('li.row') as HTMLElement;
		expect(counter.getBoundingClientRect().top).toBeGreaterThan(
			one.subtitle().getBoundingClientRect().bottom
		);
		expect(row.getBoundingClientRect().top).toBeGreaterThan(counter.getBoundingClientRect().bottom);
	});

	it('a key more changes the figure and nothing else: the head and the first row stay where they are', () => {
		const one = drawn(board('K1'));
		const three = drawn(board('K3'));
		const place = (view: ReturnType<typeof drawn>, selector: string) => {
			const el = view.root.querySelector(selector) as HTMLElement;
			const root = view.root.getBoundingClientRect();
			const box = el.getBoundingClientRect();
			return [box.top - root.top, box.height].map((n) => Math.round(n * 10) / 10);
		};
		expect(three.counter()?.textContent).toContain('3 / 7');
		expect(place(three, '.listhead')).toEqual(place(one, '.listhead'));
		expect(place(three, 'li.row')).toEqual(place(one, 'li.row'));
		// A count that jitters in width reads as the layout moving: the figure
		// is set in the mono face, so "1 / 7" and "3 / 7" are one width.
		const width = (view: ReturnType<typeof drawn>) =>
			(view.root.querySelector('.count') as HTMLElement).getBoundingClientRect().width;
		expect(width(three)).toBeCloseTo(width(one), 1);
	});

	it('draws it exactly when the view says — the screen has no rule of its own', () => {
		// Keys, and the core says no counter: none is drawn (the keys still are).
		const silent = drawn({ ...board('K1'), key_count_shown: false });
		expect(silent.counter()).toBeNull();
		expect(silent.root.querySelectorAll('li.row')).toHaveLength(1);
		// No key, and the core says a counter: it is drawn, as told.
		const told = drawn({ ...board('K0'), key_count_shown: true });
		expect(told.counter()?.textContent).toContain('0 / 7');
	});

	it('says "已添加 1 / 7" in Chinese', () => {
		expect(drawn(board('K1'), zh).counter()?.textContent?.replace(/\s+/g, ' ').trim()).toBe(
			'已添加 1 / 7'
		);
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
		expect(heading('K0')).toEqual([0, 'onboarding.create.keyPlaceHeading', true]);
		expect(heading('K1')).toEqual([1, 'onboarding.create.addMethodLabel', false]);
		expect(heading('K3')).toEqual([3, 'onboarding.create.addMethodLabel', false]);
		expect(heading('K5')).toEqual([7, 'onboarding.create.keyLimitReached', false]);
		// …and the counter's rule: from the first key on.
		expect(['K0', 'K1', 'K3', 'K5'].map((code) => board(code).key_count_shown)).toEqual([
			false,
			true,
			true,
			true
		]);
	});

	it('a wallet with no key yet, as the REAL core draws it: the new heading, the places open, no counter', () => {
		const core = new CreateWalletCore();
		const view = JSON.parse(core.view()) as CreateView;
		core.free();
		expect(view.keys).toEqual([]);
		expect(view.add_heading_key).toBe('onboarding.create.keyPlaceHeading');
		expect(view.key_count_shown).toBe(false);
		// The fixture's K0 is that view's three rules, not an invention.
		expect(board('K0').add_heading_key).toBe(view.add_heading_key);
		expect(board('K0').key_count_shown).toBe(view.key_count_shown);
		// The retired key is in no corpus: `t()` throws on it.
		expect(() => stringsIn(en)('onboarding.create.addKeyBtn')).toThrow();
		expect(() => stringsIn(zh)('onboarding.create.addKeyBtn')).toThrow();
	});
});
