/**
 * A multisig's everyday flows on the real page, its real core and the
 * parallel space's passkeys — against a hermetic world (`stub-multisig.ts`:
 * the genesis service, its proposal mailbox and a chain where the multisig
 * is not deployed yet), so they run in seconds on every engine and nothing
 * leaves the machine. What needs a chain to execute is the fork suite's
 * (`e2e/onchain/`).
 *
 * - an invite link: what it names is shown before anything is added — its
 *   network, each member with their kind, the person's own account said —
 *   and one press adds it, the members' labels kept;
 * - a rejection: another member's proposal, rejected from its sheet — a way
 *   back first; then the cancellation at its number is reviewed before its
 *   passkey (r9-13: withdrawn there, nothing is signed), proposed, read as
 *   what it is — and both transactions then say that only one can run;
 * - what waits: the home says how many transactions need this member, in
 *   one line, and the switcher says it of the multisig not shown; a new one
 *   from another member is counted on the next read.
 */
import { expect, test, type Browser, type Page } from '@playwright/test';
import { multisigCore } from './core-facts';
import { en } from './live-helpers';
import {
	FIXTURES,
	MultisigWorld,
	OUTSIDE,
	enterSpace,
	holdMultisig,
	multisigOf,
	storedRow,
	transferTx,
	type Multisig
} from './stub-multisig';

test.setTimeout(150_000);

const RECIPIENT = '0x000000000000000000000000000000000000bEEF';

function thrownOn(page: Page): string[] {
	const seen: string[] = [];
	page.on('pageerror', (e) => seen.push(e.message));
	return seen;
}

async function device(browser: Browser, world: MultisigWorld, keep: string[]): Promise<Page> {
	const context = await browser.newContext({ viewport: { width: 430, height: 900 } });
	await world.install(context);
	const page = await context.newPage();
	await enterSpace(page, keep);
	return page;
}

/** The multisig's sheet on the transaction whose row reads `title`. */
async function openRow(page: Page, title: string): Promise<void> {
	await page.getByRole('main').getByText(title, { exact: true }).first().click();
	await expect(page.getByRole('heading', { name: 'Multisig transaction' })).toBeVisible();
}

async function closeSheet(page: Page): Promise<void> {
	await expect(async () => {
		await page.keyboard.press('Escape');
		await expect(page.getByRole('heading', { name: 'Multisig transaction' })).toHaveCount(0, {
			timeout: 1_000
		});
	}).toPass({ timeout: 15_000 });
}

test('an invite link: its network and members by kind before anything is added, then added', async ({
	browser
}) => {
	const world = new MultisigWorld();
	const ms = await multisigOf([FIXTURES.one, FIXTURES.two, OUTSIDE.address], 2, 'Harbor fund');
	world.record(ms);
	// Fixture two's device: the invite names it without a label.
	const page = await device(browser, world, [FIXTURES.two]);
	const thrown = thrownOn(page);
	const link = await multisigCore<string>('multisigLinkCall', 'addLink', {
		origin: new URL(page.url()).origin,
		locale: 'en',
		multisig: ms.address,
		extra: {
			chainIds: [100],
			members: [
				{ address: FIXTURES.one, name: 'Ada', kind: 'passkey' },
				{ address: FIXTURES.two, kind: 'passkey' },
				{ address: OUTSIDE.address, name: 'Ops wallet', kind: 'wallet' }
			]
		}
	});
	// All in the fragment: nothing a server would log.
	expect(new URL(link).search).toBe('');
	expect(new URL(link).hash).toMatch(/^#msa=100:/);

	await page.goto(link);
	const sheet = page.getByRole('dialog', { name: 'Add a multisig' });
	const summary = sheet.getByTestId('invite-summary');
	await expect(summary).toBeVisible({ timeout: 30_000 });
	await expect(summary).toContainText('Used on Gnosis');
	await expect(summary).toContainText('Members, as the invite names them:');
	const members = summary.getByTestId('invite-member');
	await expect(members).toHaveCount(2);
	await expect(members.nth(0)).toHaveText(/^Ada\s*Vela account$/);
	await expect(members.nth(1)).toHaveText(/^Ops wallet\s*Outside wallet$/);
	// The member that is this device's own account, said as such.
	const you = summary.getByTestId('invite-you');
	await expect(you).toContainText('Parallel Two');
	await expect(you).toContainText("That's you");
	await expect(you).toContainText('Vela account');
	// Shown, not added: nothing is kept until the person says so.
	expect(await page.evaluate(() => localStorage.getItem('vela.multisigAccounts'))).toBeNull();

	await sheet.getByRole('button', { name: 'Add', exact: true }).click();
	await expect(sheet).toBeHidden({ timeout: 30_000 });
	await expect(page.getByRole('link', { name: 'Needs 2 of 3' }).first()).toBeVisible();
	await expect(page.getByText('Harbor fund', { exact: true }).first()).toBeVisible();
	const kept = JSON.parse(
		(await page.evaluate(() => localStorage.getItem('vela.multisigAccounts')))!
	) as {
		address: string;
		chainIds: number[];
		threshold: number;
		members: Record<string, { name?: string; kind?: string }>;
	}[];
	expect(kept).toHaveLength(1);
	expect(kept[0].address).toBe(ms.address);
	expect(kept[0].chainIds).toEqual([100]);
	expect(kept[0].threshold).toBe(2);
	// The labels and kinds ride with it; who may approve is the genesis's.
	expect(kept[0].members[OUTSIDE.address.toLowerCase()]).toEqual({
		name: 'Ops wallet',
		kind: 'wallet'
	});
	expect(kept[0].members[FIXTURES.one.toLowerCase()]).toEqual({ name: 'Ada', kind: 'passkey' });
	// Its genesis came from the service, and the core checked it gives this address.
	expect(world.asked).toContain(`GET /api/genesis/${ms.address}`);
	expect(thrown).toEqual([]);
	await page.context().close();
});

test('a rejection: a cancellation at the same number, read as one; only one of the two can run', async ({
	browser
}) => {
	const world = new MultisigWorld();
	const ms = await multisigOf([FIXTURES.one, FIXTURES.two, OUTSIDE.address], 2, 'Harbor fund');
	world.record(ms);
	world.balances.set(`100:${ms.address.toLowerCase()}`, 3n * 10n ** 18n);
	const a = await device(browser, world, [FIXTURES.one]);
	const b = await device(browser, world, [FIXTURES.two]);
	await holdMultisig(a, ms);
	await holdMultisig(b, ms);
	const thrownA = thrownOn(a);
	const thrownB = thrownOn(b);

	await test.step('member one proposes a send, reviewed before its passkey', async () => {
		await proposeSend(a, '0.25');
		await expect(a.getByText('1/2 approved').first()).toBeVisible({ timeout: 30_000 });
		await closeSheet(a);
	});
	expect(world.mailbox).toHaveLength(1);
	const original = world.mailbox[0];
	expect(original.safeTx.nonce).toBe('0x0');

	await test.step('member two finds it waiting, and Reject has a way back', async () => {
		// Read on its next poll, as a member's open wallet would.
		await expect(b.getByTestId('multisig-waiting')).toHaveText('1 needs your approval', {
			timeout: 45_000
		});
		await openRow(b, 'Send 0.25 xDAI');
		await b.getByRole('button', { name: 'Reject', exact: true }).click();
		// What rejecting does, before anything is signed.
		await expect(
			b.getByText(
				'Rejecting proposes a cancellation at the same number; it needs 2 approvals too, and whichever is executed first wins.'
			)
		).toBeVisible();
		await b.getByRole('button', { name: 'Cancel', exact: true }).click();
		await expect(b.getByText(/^Rejecting proposes a cancellation/)).toHaveCount(0);
		expect(world.mailbox).toHaveLength(1);
	});

	// r9-13: the cancellation is a proposal like any other — drawn at its
	// number with its review, on top, before any passkey.
	const review = b
		.getByRole('dialog')
		.filter({ hasText: 'Cancel transaction #0' })
		.filter({ has: b.getByTestId('review-hashes') });
	const rejectToReview = async () => {
		await b.getByRole('button', { name: 'Reject', exact: true }).click();
		await b.getByRole('button', { name: 'Reject', exact: true }).last().click();
		await expect(review).toBeVisible({ timeout: 30_000 });
		await expect(review.getByText('Cancel transaction #0').first()).toBeVisible();
		await expect(review.getByText('#0', { exact: true }).first()).toBeVisible();
		// Reviewed, not signed: the mailbox holds the one transaction still.
		expect(world.mailbox).toHaveLength(1);
		expect(world.submissions).toHaveLength(1);
	};

	await test.step("the cancellation's review, withdrawn: nothing signed, Reject offered again", async () => {
		await rejectToReview();
		await review.getByRole('button', { name: 'Don’t propose' }).click();
		await expect(review).toHaveCount(0);
		await expect(b.getByRole('button', { name: 'Reject', exact: true })).toBeVisible();
		expect(world.submissions).toHaveLength(1);
	});

	await test.step('rejected on its review: one cancellation at its number, approved by member two', async () => {
		await rejectToReview();
		await review.getByRole('button', { name: /^Propose( and approve)?( anyway)?$/ }).click();
		await expect.poll(() => world.mailbox.length, { timeout: 30_000 }).toBe(2);
		const cancellation = world.mailbox[1];
		// The same number, nothing moved: a call to itself, of nothing.
		expect(cancellation.safeTx.nonce).toBe('0x0');
		expect(cancellation.safeTx.to.toLowerCase()).toBe(ms.address.toLowerCase());
		expect(BigInt(cancellation.safeTx.value)).toBe(0n);
		expect(cancellation.safeTx.data).toBe('0x');
		expect(cancellation.approvals.map((x) => String(x.owner).toLowerCase())).toEqual([
			FIXTURES.two.toLowerCase()
		]);
		// The rejected transaction says so, and that only one can run.
		await expect(
			b.getByText('A cancellation of this transaction is waiting for approval.')
		).toBeVisible({
			timeout: 30_000
		});
		await expect(b.getByTestId('proposal-conflict')).toHaveText(
			'Only one of the transactions waiting at #0 can run: the first one executed replaces the others.'
		);
		await closeSheet(b);
	});

	await test.step('the cancellation, opened: what it is, and no rejection of a rejection', async () => {
		await openRow(b, 'Cancel transaction #0');
		const sheet = b.getByRole('dialog').last();
		await expect(sheet.getByText('Cancel transaction #0').first()).toBeVisible();
		await expect(sheet.getByTestId('review-hashes')).toBeVisible();
		await expect(sheet.getByText('1/2 approved').first()).toBeVisible();
		await expect(sheet.getByTestId('proposal-conflict')).toBeVisible();
		await expect(sheet.getByRole('button', { name: 'Reject', exact: true })).toHaveCount(0);
		await closeSheet(b);
	});

	await test.step('both rows, on both devices: only one of these can run', async () => {
		for (const page of [b, a]) {
			for (const title of ['Send 0.25 xDAI', 'Cancel transaction #0']) {
				const row = page.getByRole('main').getByRole('button', { name: new RegExp(`^${title}`) });
				await expect(row.first()).toContainText('Only one of these can run', { timeout: 45_000 });
			}
		}
	});

	expect(thrownA).toEqual([]);
	expect(thrownB).toEqual([]);
	await a.context().close();
	await b.context().close();
});

/** The send every account has, whose confirm for a multisig proposes — reviewed first (§3.6). */
async function proposeSend(page: Page, amount: string): Promise<void> {
	await page
		.getByRole('button', { name: en('componentsUi.dock.send'), exact: true })
		.first()
		.click();
	await expect(page.getByRole('heading', { name: en('send.selectTokenTitle') })).toBeVisible();
	await page
		.getByRole('button', { name: /^xDAI Gnosis/ })
		.last()
		.click({ timeout: 60_000 });
	await page.getByRole('textbox', { name: en('send.recipientLabel') }).fill(RECIPIENT);
	await page.getByRole('textbox', { name: 'xDAI' }).fill(amount);
	const advance = page.getByRole('button', { name: en('send.continueBtn') });
	await expect(advance).toBeEnabled({ timeout: 30_000 });
	await advance.click();
	await expect(page.getByRole('heading', { name: en('send.confirmTitle') })).toBeVisible();
	await page.getByRole('button', { name: /^Propose( and approve)?( anyway)?$/ }).click();
	const hashes = page.getByTestId('review-hashes');
	await expect(hashes).toBeVisible({ timeout: 60_000 });
	const sheet = hashes.locator('xpath=ancestor::*[@role="dialog"][1]');
	// D1 (real-device pass 2, r13-2): this world's node offers neither a
	// state-change check nor a simulation — the review is drawn, said in
	// danger with Try again, and proposed only "… anyway".
	await expect(sheet.getByTestId('review-unchecked')).toBeVisible();
	await sheet.getByRole('button', { name: /^Propose( and approve)? anyway$/ }).click();
}

test('what waits: one line on the home, the other multisig in the switcher, a new one counted', async ({
	browser
}) => {
	const world = new MultisigWorld();
	const harbor = await multisigOf([FIXTURES.one, OUTSIDE.address], 2, 'Harbor fund', '0x1');
	const night = await multisigOf([FIXTURES.one, OUTSIDE.address], 2, 'Night fund', '0x2');
	world.record(harbor);
	world.record(night);
	const page = await device(browser, world, [FIXTURES.one]);
	await holdTwo(page, harbor, night);
	const thrown = thrownOn(page);
	const strip = page.getByTestId('multisig-waiting');

	await test.step('nothing waits: nothing is said', async () => {
		await expect(page.getByText('Harbor fund', { exact: true }).first()).toBeVisible();
		// A read of the mailbox has answered, and said nothing.
		await expect.poll(() => world.asked.some((q) => q.includes(harbor.address))).toBe(true);
		await expect(strip).toHaveCount(0);
	});

	await test.step('another member proposes: said once, in one line', async () => {
		await world.proposeAsOutsideOwner(100, harbor.address, transferTx(RECIPIENT, 10n ** 17n, 0));
		// Read on the multisig's next poll, nothing reloaded.
		await expect(strip).toHaveText('1 needs your approval', { timeout: 45_000 });
		await expect(strip).toHaveCount(1);
		await expect(page.getByText('Waiting for approval').first()).toBeVisible();
	});

	await test.step('a second one arrives while shown: counted on the next read', async () => {
		await world.proposeAsOutsideOwner(
			100,
			harbor.address,
			transferTx(RECIPIENT, 2n * 10n ** 17n, 1)
		);
		await expect(strip).toHaveText('2 need your approval', { timeout: 45_000 });
		await expect(strip).toHaveCount(1);
	});

	await test.step('the multisig not shown: read apart, said in the switcher', async () => {
		await world.proposeAsOutsideOwner(100, night.address, transferTx(RECIPIENT, 10n ** 17n, 0));
		// By keyboard: the parallel space's badge sits over the header's middle
		// on a narrow WebKit page, and would take a pointer's press.
		await page
			.getByRole('button', { name: /^Harbor fund/ })
			.first()
			.focus();
		await page.keyboard.press('Enter');
		const switcher = page.getByRole('dialog');
		const row = (name: string) =>
			switcher.getByRole('button', { name: new RegExp(`^${name}`) }).getByTestId('row-waiting');
		await expect(row('Night fund')).toHaveText('1 needs your approval', { timeout: 45_000 });
		await expect(row('Harbor fund')).toHaveText('2 need your approval');
	});
	expect(thrown).toEqual([]);
	await page.context().close();
});

/**
 * §18 item 3: a multisig's history that cannot be read is said where the
 * natives say it — Activity's empty state, with Try again — once: the
 * waiting group says nothing of it. Try again, the chain answering, reads it.
 */
test('an unread history: said on Activity with Try again, once, and read again', async ({
	browser
}) => {
	const world = new MultisigWorld();
	const harbor = await multisigOf([FIXTURES.one, OUTSIDE.address], 2, 'Harbor fund', '0x3');
	world.record(harbor);
	// Transaction #0 ran on another member's device: the chain's number is 1,
	// and the search for its execution is refused.
	await world.proposeAsOutsideOwner(100, harbor.address, transferTx(RECIPIENT, 10n ** 17n, 0));
	world.safes.set(`100:${harbor.address.toLowerCase()}`, {
		owners: harbor.owners,
		threshold: harbor.threshold,
		nonce: 1
	});
	world.refuseHistory = true;
	const page = await device(browser, world, [FIXTURES.one]);
	await holdMultisig(page, harbor);
	const thrown = thrownOn(page);
	const unread = page.getByText(/Couldn't read this multisig's history on Gnosis/);
	const tryAgain = page.getByTestId('activity-empty-retry');

	await test.step('said once, with Try again on Activity', async () => {
		await expect(unread).toHaveCount(1, { timeout: 60_000 });
		await expect(tryAgain).toBeVisible();
		await expect(page.getByTestId('multisig-waiting')).toHaveCount(0);
		await expect(page.getByRole('button', { name: 'Retry' })).toHaveCount(0);
	});

	await test.step('Try again, the chain answering: read', async () => {
		world.refuseHistory = false;
		await tryAgain.click();
		await expect(unread).toHaveCount(0, { timeout: 60_000 });
		await expect(tryAgain).toHaveCount(0);
	});
	expect(thrown).toEqual([]);
	await page.context().close();
});

/** Keep both multisigs here, `shown` in front. */
async function holdTwo(page: Page, shown: Multisig, other: Multisig): Promise<void> {
	await page.evaluate(
		({ rows, address }) => {
			localStorage.setItem('vela.multisigAccounts', JSON.stringify(rows));
			localStorage.setItem('vela.activeMultisig', address);
		},
		{ rows: [storedRow(shown), storedRow(other)], address: shown.address }
	);
	await page.reload();
}
