/**
 * The Worker's runtime environment (secrets and vars), read when the app
 * starts and imported from `$app/env/private`. None is public: nothing here
 * reaches a browser.
 *
 * Every variable is optional. An unset secret reads as `''`, which the route
 * that needs it answers as "not configured" rather than failing to start.
 */
import { defineEnvVars } from '@sveltejs/kit/env';

export const variables = defineEnvVars({
	PEERCONNECT_APP_SECRET: {
		description:
			'PeerConnect app secret (`wrangler secret put`). Unset: `/api/peerconnect-token` answers 503 not_configured.',
		schema: (value) => value ?? ''
	},
	PEERCONNECT_APP_ID: {
		description:
			'PeerConnect app id. Unset or empty: the id in `#lib/server/peerconnect-token.ts`.',
		schema: (value) => value || undefined
	},
	GITHUB_BUG_TOKEN: {
		description:
			'GitHub token that files bug reports (`wrangler secret put`). Unset: `/api/bug-report` answers not_configured.',
		schema: (value) => value ?? ''
	},
	GITHUB_BUG_REPO: {
		description: 'owner/name of the repository bug reports are filed in.',
		schema: (value) => value ?? 'mondaylabsltd/vela-wallet'
	}
});
