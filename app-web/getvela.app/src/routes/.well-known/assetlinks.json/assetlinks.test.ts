/**
 * getvela.app's Digital Asset Links: the real app keeps links and passkeys;
 * the device-test build gets passkeys only, so it never competes with the
 * person's own Vela for a link.
 */
import { describe, expect, it } from 'vitest';
import { GET } from './+server';

type Statement = {
	relation: string[];
	target: { namespace: string; package_name: string; sha256_cert_fingerprints: string[] };
};

const DEBUG_KEY =
	'24:EA:D0:02:B2:AD:B1:D1:B4:9E:FC:B9:63:EB:9E:BC:22:60:76:6E:A5:0C:0B:57:BA:D3:DD:78:1B:0A:4A:B0';

async function statements(): Promise<Statement[]> {
	const response = await GET({} as Parameters<typeof GET>[0]);
	return (await response.json()) as Statement[];
}

describe('assetlinks.json', () => {
	it('keeps links and passkeys for the real app', async () => {
		const real = (await statements()).find((s) => s.target.package_name === 'app.getvela.wallet');
		expect(real?.relation).toEqual([
			'delegate_permission/common.handle_all_urls',
			'delegate_permission/common.get_login_creds'
		]);
	});

	it('gives the device-test build passkeys only, on the debug key', async () => {
		const test = (await statements()).find(
			(s) => s.target.package_name === 'app.getvela.wallet.devicetest'
		);
		expect(test?.relation).toEqual(['delegate_permission/common.get_login_creds']);
		expect(test?.target.sha256_cert_fingerprints).toEqual([DEBUG_KEY]);
	});
});
