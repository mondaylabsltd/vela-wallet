import { json } from '@sveltejs/kit';
import type { RequestHandler } from './$types';

export const GET: RequestHandler = async () => {
	return json({
		webcredentials: {
			apps: ['F9W689P9NE.app.getvela.VelaWallet']
		}
	});
};
