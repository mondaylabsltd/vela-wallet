import { json } from '@sveltejs/kit';
import type { RequestHandler } from './$types';

/**
 * getvela.app's universal-link association. The iOS lane's record of what the
 * app claims on this host is
 * `app-ios/universal-links/getvela.app.apple-app-site-association.json`;
 * `association.test.ts` holds this response equal to it, so a change there
 * that is not made here fails a test rather than a person's link.
 *
 * - `/sign` — the Safari extension's sign hand-off.
 * - `/pay`, `/pay/` — a payment link the app shares. With the app installed
 *   the OS opens it there, and the app reads it through the core's
 *   `payment_request`; without it, `src/routes/pay` forwards it to the web
 *   wallet's `/pay` (spec 039 Part B).
 * - `webcredentials` — passkeys for getvela.app (rpId), unchanged.
 */
const ASSOCIATION = {
	applinks: {
		details: [
			{
				appIDs: ['F9W689P9NE.app.getvela.VelaWallet'],
				components: [{ '/': '/sign' }, { '/': '/pay' }, { '/': '/pay/' }]
			}
		]
	},
	webcredentials: {
		apps: ['F9W689P9NE.app.getvela.VelaWallet', 'F9W689P9NE.app.aikeys.apppasskeysdemo-ios']
	}
};

export const GET: RequestHandler = async () => {
	return json(ASSOCIATION);
};
