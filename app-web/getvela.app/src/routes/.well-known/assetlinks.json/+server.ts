import { json } from '@sveltejs/kit';
import type { RequestHandler } from './$types';

/**
 * getvela.app's Digital Asset Links. An app listed here can ask for every
 * person's getvela.app passkeys, so only the owner's release-grade keys are
 * listed: no debug, cloud-build, device-test or demo identity (security review
 * 2026-11-07, P0). A test build is signed with the upload key on the owner's
 * machine to use them (never a debug key). When Play App Signing is set up,
 * its certificate joins here; nothing else.
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
					// Upload key (~/.android/keys/upload-key.jks, created 2026-11-14; it
					// replaced the earlier release keystore A3:8E:36:FE…, retired the same
					// day): the release build's key, and the key the owner's own test
					// builds are signed with (app/build.gradle.kts,
					// velaSignTestBuildsWithUploadKey) so they can use getvela.app's
					// passkeys like the store app (hardening.md §6, revised 2026-11-14).
					'7D:74:AF:4A:A4:78:CD:B1:C4:02:01:7F:7D:12:7A:99:46:5E:DF:93:25:40:73:0F:5A:19:EE:4D:B9:BA:C8:8B'
				]
			}
		}
	]);
};
