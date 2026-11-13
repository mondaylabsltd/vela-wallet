import { createHash } from 'node:crypto';
import { describe, expect, it } from 'vitest';
import { getTemplate, type OgImageParams } from './og-templates';
import fixture from './og-templates.fixture.json';

// Golden outputs captured from seo-sveltekit 0.1.0 before removing it.
// Ignore React's key field, which is layout-independent and now typed explicitly.
describe('OG template compatibility', () => {
	for (const [type, cases] of Object.entries(fixture.hashes)) {
		for (const [name, expected] of Object.entries(cases)) {
			it(`${type || 'empty type'} / ${name} preserves the previous layout`, () => {
				const input = fixture.cases[name as keyof typeof fixture.cases];
				const tree = getTemplate(type)({ ...input, params: input.params as OgImageParams });
				const json = JSON.stringify(tree, (key, value) => (key === 'key' ? undefined : value));
				expect(createHash('sha256').update(json).digest('hex')).toBe(expected);
			});
		}
	}

	it('falls back to the website layout for inherited object property names', () => {
		for (const type of ['__proto__', 'constructor', 'toString']) {
			expect(getTemplate(type)).toBe(getTemplate('website'));
		}
	});
});
