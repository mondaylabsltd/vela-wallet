/**
 * getvela.app's Digital Asset Links are pinned exactly: any added app or key
 * would be able to ask for every person's getvela.app passkeys.
 */
import { describe, expect, it } from 'vitest';
import { GET } from './+server';

describe('assetlinks.json', () => {
	it('lists only the store app on its release key', async () => {
		const response = await GET({} as Parameters<typeof GET>[0]);
		expect(await response.json()).toEqual([
			{
				relation: [
					'delegate_permission/common.handle_all_urls',
					'delegate_permission/common.get_login_creds'
				],
				target: {
					namespace: 'android_app',
					package_name: 'app.getvela.wallet',
					sha256_cert_fingerprints: [
						'7D:74:AF:4A:A4:78:CD:B1:C4:02:01:7F:7D:12:7A:99:46:5E:DF:93:25:40:73:0F:5A:19:EE:4D:B9:BA:C8:8B'
					]
				}
			}
		]);
	});
});
