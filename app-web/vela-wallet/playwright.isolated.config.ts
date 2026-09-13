import base from './playwright.config';
export default {
	...base,
	webServer: {
		...base.webServer,
		command:
			'npm run build && npm run build:extension && npx wrangler dev .svelte-kit/cloudflare/_worker.js --port 4174',
		port: 4174,
		timeout: 600_000,
		reuseExistingServer: false
	},
	use: { ...base.use, baseURL: 'http://localhost:4174' }
};
