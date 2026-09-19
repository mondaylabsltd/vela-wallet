import { describe, expect, it } from 'vitest';
import manifest from '../../../extension/manifest.json';
import { resolveSettingsMessages } from '$lib/i18n/engine.server';
import { buildDesktopState } from '$lib/settings/fixtures';
import { BUILD_COMMIT, BUILD_VERSION, MOCK_COMMIT, MOCK_VERSION } from './info';

/**
 * Spec 064 FR-C1. Until this existed every web and extension build said
 * `v1.0.0 (6ab8f)` — the design mock's numbers — and 0.9.2 shipped saying so.
 */
describe('the build says which build it is', () => {
	it('reports the declared version, not the mock’s', () => {
		expect(BUILD_VERSION).toBe(manifest.version);
	});

	it('reports a commit, or says it does not know — never the mock’s', () => {
		expect(BUILD_COMMIT).not.toBe(MOCK_COMMIT);
		expect(BUILD_COMMIT).toMatch(/^([0-9a-f]{7}|unknown)$/);
	});

	it('is what the About page actually shows', () => {
		const m = resolveSettingsMessages('en');
		const about = buildDesktopState('dst8', m, () => '').about.version;
		expect(about).toContain(BUILD_VERSION);
		expect(about).toContain(BUILD_COMMIT);
		expect(about).not.toContain(MOCK_COMMIT);
		if (BUILD_VERSION !== MOCK_VERSION) expect(about).not.toContain(MOCK_VERSION);
	});

	it('honours VELA_GIT_COMMIT over git — what CI and the release set', () => {
		// vitest.setup cannot re-run vite's `define`; what CAN be pinned is that
		// when the variable was set for this run, it is what we got.
		const forced = process.env.VELA_GIT_COMMIT;
		if (forced) expect(BUILD_COMMIT).toBe(forced.slice(0, 7));
	});
});
