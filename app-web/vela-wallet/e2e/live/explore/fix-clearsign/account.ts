import type { BrowserContext, Page } from '@playwright/test';
import { createAccount } from '../../live-helpers';
import { EXT, attachAuthenticator, harvest, seedNetwork, setBalance, shot } from './ext';

/** A fresh virtual-passkey account in this profile's extension, funded on the fork. */
export async function makeAccount(context: BrowserContext, name: string): Promise<{ page: Page; address: string }> {
	const page = await context.newPage();
	await page.goto(`${EXT}/manifest.json`);
	await attachAuthenticator(context, page);
	await seedNetwork(page);
	await page.goto(`${EXT}/en.html`);
	await page.getByRole('button', { name: 'Create Wallet' }).or(page.getByRole('link', { name: 'Create Wallet' })).first().click();
	const address = await createAccount(page, name);
	await setBalance(address, 5n * 10n ** 18n);
	await harvest(page);
	await page.reload();
	await page.getByText('Total balance').first().waitFor({ timeout: 30_000 }).catch(() => {});
	// The worker needs the snapshot and the chain catalog.
	await page.waitForFunction(
		() =>
			(window as any).chrome.storage.local
				.get(['vela.ext.cache', 'vela.ext.chains'])
				.then((all: any) => all['vela.ext.cache'] !== undefined && all['vela.ext.chains'] !== undefined),
		null,
		{ timeout: 30_000 }
	);
	await shot(page, `${name.replace(/\W+/g, '-')}-wallet`);
	return { page, address };
}

export async function buttonsOf(page: Page): Promise<string[]> {
	return page
		.evaluate(() =>
			[...document.querySelectorAll('button, [role=button], a')]
				.filter((b) => (b as HTMLElement).offsetParent !== null)
				.map((b) => ((b as HTMLElement).innerText || b.getAttribute('aria-label') || '').trim().replace(/\s+/g, ' '))
				.filter(Boolean)
		)
		.catch(() => []);
}

export async function textOf(page: Page): Promise<string> {
	return page.evaluate(() => document.body.innerText).catch(() => '<closed>');
}
