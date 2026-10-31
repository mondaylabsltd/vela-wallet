/**
 * Spec 102 R8 through the web's door: the shared vectors
 * (`rust/crates/vela-core/tests/vectors/signing-route/surface.json`, also
 * read by vela-core's own test) answered by the wasm this build ships
 * (`signingRoute`) — and the surface this build says it is.
 *
 * On the Safari Web Extension, whose pages cannot use a `getvela.app` passkey,
 * nothing is signed in Vela: every kind, a key ceremony included, goes to a
 * page. Every other surface answers exactly as R7 did.
 */
import '$lib/i18n/wasm-init.server';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { multisigApprovalRoute, signingRoute } from '$lib/core/kernels';
import type { SigningSurface } from '$lib/core/generated/SigningSurface';
import { SIGNING_SURFACE } from './surface';

interface Vectors {
	accounts: Record<string, unknown>;
	cases: { account: string; method: string; surface: SigningSurface; route: unknown }[];
}

const VECTORS = JSON.parse(
	readFileSync(
		fileURLToPath(
			new URL(
				'../../../../../rust/crates/vela-core/tests/vectors/signing-route/surface.json',
				import.meta.url
			)
		),
		'utf8'
	)
) as Vectors;

describe('where each signature goes, per surface (R7 + R8)', () => {
	it('answers every shared vector as the core does', () => {
		expect(VECTORS.cases.length).toBeGreaterThanOrEqual(100);
		for (const c of VECTORS.cases) {
			expect(
				signingRoute(VECTORS.accounts[c.account], c.method, c.surface),
				JSON.stringify(c)
			).toEqual(c.route);
		}
	});

	it('never answers in_vela on Safari', () => {
		const onSafari = VECTORS.cases.filter((c) => c.surface === 'safari_extension');
		expect(onSafari.some((c) => c.route === null)).toBe(true);
		for (const c of onSafari) {
			expect(
				(
					signingRoute(VECTORS.accounts[c.account], c.method, 'safari_extension') as {
						type?: string;
					} | null
				)?.type
			).not.toBe('in_vela');
		}
	});

	it("this build is the standard surface, and its own route is the vectors' standard one", () => {
		// The web wallet and the Chrome extension: `pnpm build:extension:safari`
		// is the only build whose pages load from `safari-web-extension:`.
		expect(SIGNING_SURFACE).toBe('standard');
		const account = VECTORS.accounts.in_vela;
		expect(signingRoute(account, 'personal_sign')).toEqual({ type: 'in_vela' });
		expect(multisigApprovalRoute(account)).toEqual({ type: 'in_vela' });
	});
});
