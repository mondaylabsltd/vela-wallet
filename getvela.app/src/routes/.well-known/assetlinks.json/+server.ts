import { json } from '@sveltejs/kit';
import type { RequestHandler } from './$types';

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
					'A3:8E:36:FE:5A:99:AE:30:73:F7:91:4C:3D:43:58:32:AE:BD:D9:7D:C1:A4:DF:EF:9E:4B:3A:6E:10:31:9D:53',
					// Debug keystore (development)
					'24:EA:D0:02:B2:AD:B1:D1:B4:9E:FC:B9:63:EB:9E:BC:22:60:76:6E:A5:0C:0B:57:BA:D3:DD:78:1B:0A:4A:B0',
					// EAS-managed keystore — signs the cloud dev builds used by the
					// PR-preview flow. Extracted from the built APK via
					// `apksigner verify --print-certs`. Without it, passkey
					// registration on those builds fails with
					// "[50152] RP ID cannot be validated".
					'BE:1E:CA:F3:B4:69:20:3F:37:7F:6A:7F:C3:63:2E:86:D5:ED:7D:3E:C8:16:82:CA:C5:6C:94:6E:D1:78:99:21'
				]
			}
		}
	]);
};
