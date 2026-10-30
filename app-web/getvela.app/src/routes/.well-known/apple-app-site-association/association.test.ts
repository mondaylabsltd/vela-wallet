/**
 * getvela.app's universal-link association: the iOS lane's file, as JSON, and
 * nothing the site claimed before lost — `/sign` and the passkey apps.
 */
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { GET } from './+server';

type Component = { '/': string; '#'?: string; comment?: string };
type Association = {
	applinks: { details: { appIDs: string[]; components: Component[] }[] };
	webcredentials: { apps: string[] };
};

/** The record without its comments — Apple ignores keys it does not know; the claims are what count. */
function claims(a: Association): Association {
	return {
		...a,
		applinks: {
			details: a.applinks.details.map((d) => ({
				...d,
				components: d.components.map((c) => {
					const rest: Component = { ...c };
					delete rest.comment;
					return rest;
				})
			}))
		}
	};
}

const IOS = JSON.parse(
	readFileSync(
		fileURLToPath(
			new URL(
				'../../../../../../app-ios/universal-links/getvela.app.apple-app-site-association.json',
				import.meta.url
			)
		),
		'utf8'
	)
) as Association;

async function served(): Promise<{ response: Response; body: Association }> {
	const response = await GET({} as Parameters<typeof GET>[0]);
	return { response, body: (await response.clone().json()) as Association };
}

describe('/.well-known/apple-app-site-association (getvela.app)', () => {
	it('is served as JSON, not a redirect', async () => {
		const { response } = await served();
		expect(response.status).toBe(200);
		expect(response.headers.get('content-type')).toBe('application/json');
	});

	it('claims exactly what the iOS lane’s file claims', async () => {
		const { body } = await served();
		expect(claims(body)).toEqual(claims(IOS));
	});

	it('claims the pay link, and keeps /sign and the passkey apps', async () => {
		const { body } = await served();
		const paths = body.applinks.details.flatMap((d) => d.components.map((c) => c['/']));
		expect(paths).toEqual(expect.arrayContaining(['/sign', '/pay', '/pay/']));
		expect(body.webcredentials.apps).toEqual([
			'F9W689P9NE.app.getvela.VelaWallet',
			'F9W689P9NE.app.aikeys.apppasskeysdemo-ios'
		]);
	});
});
