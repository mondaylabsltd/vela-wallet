/**
 * Engine-integration gate (research.md D6): the real wasm engine must resolve
 * every Welcome key in every locale — no key-echo, no silent English where a
 * translation exists — and the page-facing resolver must agree with the raw
 * engine (the 004/005 differential spirit at this architecture's altitude).
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { describe, expect, it } from 'vitest';
import { SUPPORTED_LOCALES, textDirectionOf } from './locales';
import { FLOW_KEYS, FLOW_PLURAL_KEYS, WELCOME_KEYS } from './messages';
import { fillTemplate } from './fill';
import {
	rawResolve,
	resolveFlowMessages,
	resolveWelcomeMessages,
	textDirection
} from './engine.server';

describe('welcome messages resolve through the vela-core engine', () => {
	for (const locale of SUPPORTED_LOCALES) {
		it(`${locale}: every key resolves to a non-empty string`, () => {
			const messages = resolveWelcomeMessages(locale);
			expect(messages.metaTitle.length).toBeGreaterThan(0);
			expect(messages.metaDescription.length).toBeGreaterThan(0);
			expect(messages.tagline.length).toBeGreaterThan(0);
			expect(messages.createWallet.length).toBeGreaterThan(0);
			expect(messages.alreadyHaveWallet.length).toBeGreaterThan(0);
		});
	}

	it('actually loads per-locale catalogs (en/zh/ja taglines all differ)', () => {
		const taglines = (['en', 'zh', 'ja'] as const).map(
			(locale) => resolveWelcomeMessages(locale).tagline
		);
		expect(new Set(taglines).size).toBe(3);
	});

	it('differential: resolveWelcomeMessages equals raw engine resolution', () => {
		for (const locale of ['en', 'zh', 'ru', 'zh-HK'] as const) {
			const messages = resolveWelcomeMessages(locale);
			expect(messages.tagline).toBe(rawResolve(locale, 'onboarding.welcomeWeb.tagline'));
			expect(messages.createWallet).toBe(rawResolve(locale, 'onboarding.welcome.createWallet'));
			expect(messages.alreadyHaveWallet).toBe(
				rawResolve(locale, 'onboarding.welcome.alreadyHaveWallet')
			);
		}
	});

	it('no key echoes through the raw engine for any locale × key', () => {
		for (const locale of SUPPORTED_LOCALES) {
			for (const key of WELCOME_KEYS) {
				expect(rawResolve(locale, key), `${locale} ${key}`).not.toBe(key);
			}
		}
	});

	it('static direction registry agrees with the engine for all 15 locales', () => {
		for (const locale of SUPPORTED_LOCALES) {
			expect(textDirection(locale), locale).toBe(textDirectionOf(locale));
		}
	});
});

describe('onboarding flow messages resolve through the vela-core engine (spec 014 T033)', () => {
	it('FLOW_KEYS covers the whole per-state key map, without duplicates', () => {
		expect(new Set(FLOW_KEYS).size).toBe(FLOW_KEYS.length);
		// One sentinel per screen the v2 flow can show, plus the prompts.
		//
		// The outcome-catalog sentinels are gone with spec 019: the eighteen
		// `OutcomeKind`s were a design taxonomy the core does not express — a
		// transport failure and a 503 both arrive as `CreateFailed { detail }`,
		// so telling them apart would mean classifying error strings in
		// TypeScript. `onboarding.common.*` still holds that copy for a shell
		// that IS handed a classification; it is not a state the core emits.
		for (const sentinel of [
			'onboarding.common.close',
			'onboarding.create.ack2PrivacyPolicy',
			'onboarding.create.keysTitleBlocked',
			'onboarding.create.methodSecurityKeyBody',
			'onboarding.create.taskDeriveAddress',
			'onboarding.create.successTitle',
			'onboarding.create.statusSyncingKey',
			'onboarding.login.statusAwaitingPasskey',
			'onboarding.login.recoverOfferBody',
			'onboarding.login.registryUnreachableTitle',
			'onboarding.login.registryUnreachableBody',
			'onboarding.common.notDiscoverableTitle'
		]) {
			expect(FLOW_KEYS, sentinel).toContain(sentinel);
		}
	});

	it('no key echoes through the raw engine for any locale × flow key', () => {
		for (const locale of SUPPORTED_LOCALES) {
			for (const key of FLOW_KEYS) {
				expect(rawResolve(locale, key), `${locale} ${key}`).not.toBe(key);
			}
		}
	});

	it('resolveFlowMessages serializes every key, agreeing with the raw engine', () => {
		for (const locale of ['en', 'zh', 'ru', 'zh-HK'] as const) {
			const flow = resolveFlowMessages(locale);
			const forms = Object.keys(flow).filter((key) => !FLOW_KEYS.includes(key));
			expect(Object.keys(flow)).toHaveLength(FLOW_KEYS.length + forms.length);
			for (const key of forms) {
				expect(
					FLOW_PLURAL_KEYS.some((plural) => key.startsWith(`${plural}_`)),
					key
				).toBe(true);
				expect(flow[key], `${locale} ${key}`).toBe(rawResolve(locale, key));
			}
			for (const key of FLOW_KEYS) {
				expect(flow[key].length, `${locale} ${key}`).toBeGreaterThan(0);
				expect(flow[key], `${locale} ${key}`).toBe(rawResolve(locale, key));
			}
		}
	});

	// Issue 409: a plural key ships as the forms its locale has — the core's
	// categories — never as a bare key (it has none), and never as forms the
	// locale's rule cannot pick.
	it('plural flow keys ship each locale’s own forms', () => {
		const forms = (locale: 'en' | 'zh' | 'ru' | 'fr') =>
			Object.keys(resolveFlowMessages(locale))
				.filter((key) => key.startsWith('onboarding.create.successMessage'))
				.sort();
		const base = 'onboarding.create.successMessage';
		expect(forms('en')).toEqual([`${base}_one`, `${base}_other`]);
		expect(forms('zh')).toEqual([`${base}_other`]);
		expect(forms('fr')).toEqual([`${base}_many`, `${base}_one`, `${base}_other`]);
		expect(forms('ru')).toEqual([`${base}_few`, `${base}_many`, `${base}_one`, `${base}_other`]);
		const en = resolveFlowMessages('en');
		expect(fillTemplate(en[`${base}_one`], { count: 1 })).toBe(
			'Your key can sign in on its own. The contract deploys with your first transaction.'
		);
		expect(fillTemplate(en[`${base}_other`], { count: 2 })).toBe(
			'Any of your 2 keys can sign in on its own. The contract deploys with your first transaction.'
		);
	});

	it('zh copy is the mocks’ verbatim source (contracts/i18n-keys.md)', () => {
		const flow = resolveFlowMessages('zh');
		expect(flow['onboarding.create.keysTitleBlocked']).toBe('再加一把才能创建');
		expect(flow['onboarding.create.taskDeriveAddress']).toBe('推导账户地址');
		expect(flow['onboarding.login.statusAwaitingPasskey']).toBe('正在等待通行密钥');
		expect(flow['onboarding.common.notDiscoverableTitle']).toBe('这台设备上没有可用的通行密钥');
	});

	// PR 3 note 17: with no key yet the keys screen's title and the heading
	// over its three places read the same words (zh 「添加通行密钥」 twice; en
	// "Add passkeys" / "Add a passkey"). The heading has its own key now —
	// "Choose where it lives" — and the old one is retired: `t()` throws on it.
	it('the keys screen’s first heading adds information: never the title’s words again, in any language', () => {
		expect(FLOW_KEYS).toContain('onboarding.create.keyPlaceHeading');
		expect(FLOW_KEYS).not.toContain('onboarding.create.addKeyBtn');
		for (const locale of SUPPORTED_LOCALES) {
			const flow = resolveFlowMessages(locale);
			const heading = flow['onboarding.create.keyPlaceHeading'];
			expect(heading, locale).toBeTruthy();
			expect(heading, locale).not.toBe(flow['onboarding.create.keysTitle']);
			// …and it is not the "Add another" of the keys after the first.
			expect(heading, locale).not.toBe(flow['onboarding.create.addMethodLabel']);
			expect(flow, locale).not.toHaveProperty('onboarding.create.addKeyBtn');
		}
		expect(resolveFlowMessages('en')['onboarding.create.keyPlaceHeading']).toBe(
			'Choose where it lives'
		);
		expect(resolveFlowMessages('zh')['onboarding.create.keyPlaceHeading']).toBe('选择存放位置');
	});

	it('interpolation templates ship raw and fill client-side (FR-011 frozen numbers)', () => {
		const flow = resolveFlowMessages('en');
		expect(flow['onboarding.create.keyCount']).toContain('{{current}}');
		expect(flow['onboarding.create.progressSubtitle']).toContain('{{count}}');
		const filled = fillTemplate(flow['onboarding.create.keyCount'], { current: 1, max: 7 });
		expect(filled).not.toContain('{{');
		expect(filled).toContain('1');
		expect(filled).toContain('7');
	});
});

describe('mock annotation strings never ship (spec 014 FR-002 / SC-006)', () => {
	// The designer directives visible in some mocks. Assembled from halves so
	// this test file itself can never trip its own scan.
	const NEEDLES = ['新增' + ' i18n', '展开' + '态', '兜底' + '集合'];

	const SRC_ROOT = join(import.meta.dirname, '..', '..');

	const collect = (dir: string): string[] =>
		readdirSync(dir).flatMap((name) => {
			const path = join(dir, name);
			return statSync(path).isDirectory() ? collect(path) : [path];
		});

	it('appear nowhere under src/', () => {
		const files = collect(SRC_ROOT);
		expect(files.length).toBeGreaterThan(50);
		for (const file of files) {
			const text = readFileSync(file, 'utf8');
			for (const needle of NEEDLES) {
				expect(text.includes(needle), `${relative(SRC_ROOT, file)} contains "${needle}"`).toBe(
					false
				);
			}
		}
	});
});

describe('every key the onboarding surfaces can request is in FLOW_KEYS', () => {
	// The flow dict echoes unknown keys (`data.flow[key] ?? key`), so a key
	// missing from FLOW_KEYS ships as raw text with no gate tripping — the
	// spec-019 rail did exactly that. This scan closes the loop: every
	// `strings('…')` literal in the onboarding components and the two routes
	// that resolve through `data.flow` must be prerendered.
	const SRC_ROOT = join(import.meta.dirname, '..', '..');
	const SURFACES = [
		join(SRC_ROOT, 'lib', 'ui', 'onboarding'),
		join(SRC_ROOT, 'routes', '[locale]', '+page.svelte'),
		join(SRC_ROOT, 'routes', '[locale]', 'create', '+page.svelte')
	];

	const collect = (path: string): string[] =>
		statSync(path).isDirectory()
			? readdirSync(path).flatMap((name) => collect(join(path, name)))
			: [path];

	// The rail builds its step keys from a template, which hides them from the
	// literal scan — so the template's shape is pinned here and expanded by hand.
	const RAIL_TEMPLATE = /^onboarding\.create\.step\$\{key\}(Label|Detail)$/;
	const RAIL_STEPS = ['Naming', 'Keys', 'Create'];

	it('scan finds the surfaces and no literal falls outside FLOW_KEYS', () => {
		const requested = new Set<string>();
		const files = SURFACES.flatMap(collect).filter(
			(f) => f.endsWith('.svelte') || f.endsWith('.ts')
		);
		expect(files.length).toBeGreaterThan(5);
		for (const file of files) {
			const text = readFileSync(file, 'utf8');
			for (const m of text.matchAll(/strings\(\s*'([^']+)'/g)) requested.add(m[1]);
			for (const m of text.matchAll(/strings\(\s*`([^`]+)`/g)) {
				const suffix = RAIL_TEMPLATE.exec(m[1])?.[1];
				expect(
					suffix,
					`${relative(SRC_ROOT, file)}: strings(\`${m[1]}\`) is not the pinned rail template — ` +
						'list its expansions here and in FLOW_KEYS'
				).toBeDefined();
				for (const step of RAIL_STEPS) requested.add(`onboarding.create.step${step}${suffix}`);
			}
		}
		expect(requested.size).toBeGreaterThan(30);
		// A plural key (issue 409) is requested bare and shipped as its forms.
		const provided = new Set<string>([...FLOW_KEYS, ...FLOW_PLURAL_KEYS]);
		for (const key of requested) {
			expect(
				provided.has(key),
				`"${key}" is requested but missing from FLOW_KEYS / FLOW_PLURAL_KEYS`
			).toBe(true);
		}
	});
});
