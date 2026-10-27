/**
 * Connecting the AI agent with clientmcp.dev (spec 106): one button, the
 * person signs in there, and the wallet comes back with its bridge and
 * token. clientmcp.dev is played here — its `/connect` sends the tab back
 * with a code, its `/api/token` trades the code (CORS, as the real one) — so
 * the test can hold the wallet to the protocol: the S256 challenge is the
 * verifier's, the state is the one it sent, and the code leaves the address
 * bar.
 */
import { createHash } from 'node:crypto';
import { expect, test, type Page } from '@playwright/test';
import { en } from './live-helpers';
import { BRIDGE, playBridge } from './agent-bridge';
import { denyOffOrigin } from './stub-chain';

test.use({ viewport: { width: 390, height: 844 } });
test.setTimeout(120_000);

const SITE = 'https://clientmcp.dev';
const AGENT_URL = 'http://localhost:18999/mcp/agent-token-e2e';
const CORS = { 'access-control-allow-origin': '*', 'access-control-allow-headers': 'content-type' };

type Played = {
	/** What `/connect` was asked. */
	connect: () => URLSearchParams | null;
	/** What `/api/token` was sent. */
	exchanged: () => Record<string, string> | null;
};

/**
 * clientmcp.dev, played. `back` decides the return: by default the code and
 * the state it was given.
 */
async function playSite(
	page: Page,
	back: (query: URLSearchParams) => Record<string, string> = (q) => ({
		code: 'the-code',
		state: q.get('state') ?? ''
	}),
	answer: { status: number; body: unknown } = {
		status: 200,
		body: {
			token_type: 'clientmcp',
			client_id: 'c-e2e',
			client_token: 'linked-token',
			expires_at: 1_830_000_000,
			bridge_url: BRIDGE,
			mcp_url: 'http://localhost:18999/mcp',
			agent_url: AGENT_URL,
			login: 'alice'
		}
	}
): Promise<Played> {
	let connect: URLSearchParams | null = null;
	let exchanged: Record<string, string> | null = null;
	await page.route(`${SITE}/**`, async (route) => {
		const request = route.request();
		const url = new URL(request.url());
		if (url.pathname === '/connect') {
			connect = url.searchParams;
			const to = new URL(url.searchParams.get('redirect_uri') ?? '');
			for (const [k, v] of Object.entries(back(url.searchParams))) to.searchParams.set(k, v);
			return route.fulfill({ status: 302, headers: { location: to.toString() } });
		}
		if (url.pathname === '/api/token') {
			if (request.method() === 'OPTIONS') return route.fulfill({ status: 204, headers: CORS });
			exchanged = request.postDataJSON() as Record<string, string>;
			return route.fulfill({
				status: answer.status,
				headers: { ...CORS, 'content-type': 'application/json' },
				body: JSON.stringify(answer.body)
			});
		}
		return route.abort('failed');
	});
	return { connect: () => connect, exchanged: () => exchanged };
}

async function openAgentSettings(page: Page): Promise<void> {
	await page.goto('/en/parallel');
	await page.locator('li code').first().waitFor({ timeout: 60_000 });
	await page.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
	await page.waitForURL(/\/en\/wallet$/);
	await page.goto('/en/settings');
	// Its row is in Advanced, folded on the phone layout.
	await page.getByRole('button', { name: 'Advanced' }).click();
	await page.getByText(en('settings.agent.title'), { exact: true }).first().click();
	await expect(page.getByTestId('agent-connect')).toBeVisible();
}

const stored = (page: Page) =>
	page.evaluate(() => JSON.parse(localStorage.getItem('vela.agent') ?? 'null'));

test('one button: signed in on clientmcp.dev, the wallet is connected', async ({ page }) => {
	await denyOffOrigin(page);
	const bridge = await playBridge(page);
	const site = await playSite(page);
	await openAgentSettings(page);

	await page.getByTestId('agent-connect').click();
	await expect(page.getByText('Connected as @alice')).toBeVisible({ timeout: 30_000 });

	// The request the wallet made, and the exchange it finished with.
	const asked = site.connect()!;
	expect(asked.get('app')).toBe('vela');
	expect(asked.get('code_challenge_method')).toBe('S256');
	expect(asked.get('redirect_uri')).toBe(new URL('/clientmcp', page.url()).toString());
	expect(asked.get('installation')).toMatch(/^[A-Za-z0-9_-]{22}$/);
	const sent = site.exchanged()!;
	expect(sent).toMatchObject({
		grant_type: 'authorization_code',
		code: 'the-code',
		redirect_uri: asked.get('redirect_uri')
	});
	const challenge = createHash('sha256').update(sent.code_verifier).digest('base64url');
	expect(challenge, 'the challenge is the verifier’s S256').toBe(asked.get('code_challenge'));

	// The code is gone from the address bar; the token and whose it is are kept.
	expect(page.url()).not.toContain('code=');
	expect(await stored(page)).toMatchObject({
		enabled: true,
		url: BRIDGE,
		token: 'linked-token',
		linked: { site: SITE, login: 'alice', clientId: 'c-e2e' }
	});
	await expect.poll(() => bridge.tools().length, { timeout: 30_000 }).toBeGreaterThan(30);
	// Spec 107 E8: the first connection offers the wallet in front, and only it.
	expect((await stored(page)).wallets).toEqual(['0xD400866e00B055B20752a826CD5C89b811de130b']);
	await expect(page.getByTestId('agent-wallets-none')).toHaveCount(0);
	// Which client this wallet is.
	await expect(page.getByTestId('agent-client-id')).toHaveText('Client c-e2e');

	// The agent's address, here to copy: masked until Show.
	const address = page.getByTestId('agent-address-value');
	await expect(address).not.toContainText('agent-token-e2e');
	await page.getByRole('button', { name: en('settings.agent.show') }).click();
	await expect(address).toHaveText(AGENT_URL);
	expect((await stored(page)).agentUrl).toBe(AGENT_URL);
	await expect(page.getByRole('link', { name: /Connect your AI agent/ })).toHaveAttribute(
		'href',
		`${SITE}/account`
	);

	// Signing in again from here is the same installation.
	const installation = asked.get('installation');
	await page.getByRole('button', { name: en('settings.agent.reconnect') }).click();
	await expect(page.getByText('Connected as @alice')).toBeVisible({ timeout: 30_000 });
	expect(site.connect()!.get('installation')).toBe(installation);

	// Disconnecting forgets the token here.
	await page.getByRole('button', { name: en('settings.agent.disconnect') }).click();
	await expect(page.getByTestId('agent-connect')).toBeVisible();
	expect(await stored(page)).toMatchObject({
		enabled: false,
		token: '',
		linked: null,
		agentUrl: ''
	});
});

test('a return that is not this sign-in’s is refused, and nothing is traded', async ({ page }) => {
	await denyOffOrigin(page);
	const site = await playSite(page, () => ({ code: 'stolen', state: 'someone-elses-state' }));
	await openAgentSettings(page);
	await page.getByTestId('agent-connect').click();
	await expect(page.getByTestId('agent-link-problem')).toHaveText(
		en('settings.agent.linkNotOurs'),
		{
			timeout: 30_000
		}
	);
	expect(site.exchanged(), 'the code was never sent').toBeNull();
	expect(page.url()).not.toContain('code=');
	expect((await stored(page))?.token ?? '').toBe('');
});

test('the site’s refusal is said in its words', async ({ page }) => {
	await denyOffOrigin(page);
	await playSite(page, undefined, {
		status: 400,
		body: { error: 'invalid_grant', error_description: 'The code is unknown, used or expired.' }
	});
	await openAgentSettings(page);
	await page.getByTestId('agent-connect').click();
	await expect(page.getByTestId('agent-link-problem')).toHaveText(
		'clientmcp.dev said no: The code is unknown, used or expired.',
		{ timeout: 30_000 }
	);
});

test('the person saying no on clientmcp.dev is said too', async ({ page }) => {
	await denyOffOrigin(page);
	await playSite(page, (q) => ({
		error: 'access_denied',
		error_description: 'Not now',
		state: q.get('state') ?? ''
	}));
	await openAgentSettings(page);
	await page.getByTestId('agent-connect').click();
	await expect(page.getByTestId('agent-link-problem')).toHaveText(
		'clientmcp.dev said no: Not now',
		{ timeout: 30_000 }
	);
});
