import { json } from '@sveltejs/kit';
import type { RequestHandler } from './$types';

/**
 * getvela.app's Digital Asset Links. An app listed here can ask for every
 * person's getvela.app passkeys, so only the store app on its release key is
 * listed: no debug, cloud-build, device-test or demo identity (security review
 * 2026-11-07, P0). Test builds use a separate test RP domain. When Play App
 * Signing is set up, its certificate joins the release key here; nothing else.
 */
export const GET: RequestHandler = async () => {
	return json([
		{
			relation: [
				'delegate_permission/common.handle_all_urls',
				'delegate_permission/common.get_login_creds'
			],
			target: {
				namespace: 'android_app',
				package_name: 'app.getvela.wallet',
				sha256_cert_fingerprints: [
					// Release keystore
					'A3:8E:36:FE:5A:99:AE:30:73:F7:91:4C:3D:43:58:32:AE:BD:D9:7D:C1:A4:DF:EF:9E:4B:3A:6E:10:31:9D:53'
				]
			}
		}
	]);
};
