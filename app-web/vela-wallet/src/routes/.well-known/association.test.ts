/**
 * The association files the Worker serves ARE the platform lanes' files —
 * byte for byte, as JSON, with no redirect (Apple's and Google's verifiers
 * follow none). The e2e asks the built Worker the same.
 */
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { GET as aasa } from './apple-app-site-association/+server';
import { GET as assetLinks } from './assetlinks.json/+server';

const repo = (path: string) =>
	readFileSync(fileURLToPath(new URL(`../../../../../${path}`, import.meta.url)), 'utf8');

const IOS = repo('app-ios/universal-links/wallet.getvela.app.apple-app-site-association.json');
const ANDROID = repo('app-android/vela-wallet/app-links/assetlinks.json');

describe('/.well-known/apple-app-site-association', () => {
	it('is the iOS lane’s file, as application/json', async () => {
		const response = aasa();
		expect(response.status).toBe(200);
		expect(response.headers.get('content-type')).toBe('application/json');
		const body = await response.text();
		expect(body).toBe(IOS);
		expect(JSON.parse(body)).toEqual(JSON.parse(IOS));
	});

	it('claims the pay link for the app', async () => {
		const parsed = JSON.parse(await aasa().text()) as {
			applinks: { details: { components: { '/': string }[] }[] };
		};
		const paths = parsed.applinks.details.flatMap((d) => d.components.map((c) => c['/']));
		expect(paths).toContain('/pay');
	});
});

describe('/.well-known/assetlinks.json', () => {
	it('is the Android lane’s file, as application/json', async () => {
		const response = assetLinks();
		expect(response.status).toBe(200);
		expect(response.headers.get('content-type')).toBe('application/json');
		const body = await response.text();
		expect(body).toBe(ANDROID);
		expect(JSON.parse(body)).toEqual(JSON.parse(ANDROID));
	});
});
