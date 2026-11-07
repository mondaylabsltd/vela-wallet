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
						'A3:8E:36:FE:5A:99:AE:30:73:F7:91:4C:3D:43:58:32:AE:BD:D9:7D:C1:A4:DF:EF:9E:4B:3A:6E:10:31:9D:53'
					]
				}
			}
		]);
	});
});
