import { execFileSync } from 'node:child_process';
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

	it('takes the commit from where spec 064 §3 says, in that order', () => {
		// Both branches assert. The first version of this test only asserted when
		// the variable was set, so on a developer's machine it checked nothing —
		// and the suite (which requires assertions) said so.
		const forced = process.env.VELA_GIT_COMMIT || process.env.WORKERS_CI_COMMIT_SHA;
		if (forced?.trim()) {
			expect(BUILD_COMMIT).toBe(forced.trim().slice(0, 7));
			return;
		}
		let head = '';
		try {
			head = execFileSync('git', ['rev-parse', 'HEAD'], { stdio: ['ignore', 'pipe', 'ignore'] })
				.toString()
				.trim();
		} catch {
			// no git here: the build says so, in a word
		}
		expect(BUILD_COMMIT).toBe(head ? head.slice(0, 7) : 'unknown');
	});
});
