/**
 * An AI agent paying many people in batches (spec 103), end to end.
 *
 * The test is the agent's side of the bridge: the wallet dials what it was
 * configured to dial, registers its tools, and every call below arrives the
 * way a remote agent's would — over the wallet's own ClientMCP connection
 * (`page.routeWebSocket` plays the bridge). Everything after that is the real
 * wallet: the intents, the send core, the review, the fixture key where the
 * passkey is (the parallel space), the relay and the chain stubbed.
 *
 * What it pins is the owner's own sessions: batch 2 held behind batch 1's
 * receipt (2026-10-20), and a passkey prompt raised over a review nobody had
 * chosen to confirm (2026-10-21). Here the agent drafts each batch, asks, and
 * never presses; the person confirms each on its review. Batch 2 is drafted
 * while batch 1 is still on its way, waits for it on the same network (one
 * nonce), and both land.
 */
import { expect, test, type Page } from '@playwright/test';
import { enableAgent, playBridge } from './agent-bridge';
import { CHAINS } from '../src/lib/services/chains';
import { en } from './live-helpers';
import {
	abiWord,
	aggregate3CallCount,
	denyOffOrigin,
	encodeAggregate3Result,
	happyRelay,
	readKv,
	seedNetworkOverrides,
	stubChainRegistry,
	stubJsonRpc,
	stubRelay
} from './stub-chain';

test.use({ viewport: { width: 390, height: 844 } });
test.setTimeout(300_000);

const STUB = 'https://stub-rpc.test/rpc';
const RELAY = /vela-relay(-cf)?\.getvela\.app/;
const ETH = 1_000_000_000_000_000_000n;
const PRICE_8DP = 3000n * 100_000_000n;

const hashOf = (n: number) => '0x' + n.toString(16).padStart(2, '0').repeat(32);
const txOf = (n: number) => '0x' + (n + 0x80).toString(16).repeat(32);

/** Deployed Safe, 3 ETH at $3,000. */
async function stubChain(page: Page): Promise<void> {
	await denyOffOrigin(page);
	await stubChainRegistry(page, {
		1: {
			chainId: 1,
			name: 'Ethereum',
			nativeCurrency: { name: 'Ether', symbol: 'ETH', decimals: 18 },
			stables: [],
			wrappedNativeToken: null,
			dex: null,
			rpc: [`${STUB}/1`]
		}
	});
	await stubJsonRpc(page, /stub-rpc\.test\/rpc\/(\d+)/, (method, params, url) => {
		const chainId = Number(/\/rpc\/(\d+)/.exec(url)?.[1]);
		if (method === 'eth_chainId') return '0x' + chainId.toString(16);
		if (method === 'eth_blockNumber') return '0x10';
		if (method === 'eth_getCode') return '0x6080';
		if (method === 'eth_getTransactionCount') return '0x0';
		if (method === 'eth_gasPrice') return '0x3b9aca00';
		if (method === 'eth_maxPriorityFeePerGas') return '0x3b9aca00';
		if (method === 'eth_getBlockByNumber') return { baseFeePerGas: '0x3b9aca00' };
		if (method === 'eth_call') {
			const call = params[0] as { data?: string } | undefined;
			const n = call?.data ? aggregate3CallCount(call.data) : 0;
			const balance = chainId === 1 ? ETH * 3n : 0n;
			const data =
				'0x' + abiWord(balance) + abiWord(PRICE_8DP) + abiWord(0) + abiWord(0) + abiWord(0);
			if (n === 0) return '0x' + abiWord(0);
			return encodeAggregate3Result(Array.from({ length: n }, () => ({ success: true, data })));
		}
		if (method === 'eth_getLogs') return [];
		return undefined;
	});
}

/**
 * The relay: quotes as `happyRelay` does, but every send gets its own hash,
 * and each lands only when the test says so.
 */
async function stubBatchRelay(page: Page) {
	const quotes = happyRelay(hashOf(0), txOf(0));
	const sent: string[] = [];
	const landed = new Set<string>();
	await stubRelay(page, RELAY, (method, params) => {
		const hash = String(params[0] ?? '').toLowerCase();
		switch (method) {
			case 'eth_sendUserOperation': {
				const next = hashOf(sent.length + 1);
				sent.push(next);
				return next;
			}
			case 'pimlico_getUserOperationStatus':
				return { status: landed.has(hash) ? 'included' : 'submitted' };
			case 'eth_getUserOperationReceipt':
				if (!landed.has(hash)) return null;
				return {
					userOpHash: hash,
					success: true,
					receipt: {
						transactionHash: txOf(sent.indexOf(hash) + 1),
						status: '0x1',
						blockNumber: '0x11',
						blockHash: '0x' + 'cd'.repeat(32),
						gasUsed: '0x5208',
						logs: []
					}
				};
			default:
				return quotes(method, params);
		}
	});
	return { sent, land: (hash: string) => landed.add(hash.toLowerCase()) };
}

const recipients = (count: number, from: number) =>
	Array.from({ length: count }, (_, i) => ({
		to: '0x' + (from + i).toString(16).padStart(40, '0'),
		amount: '0.001'
	}));

test('an agent pays 100 people in two batches, the second drafted while the first is on its way', async ({
	page
}) => {
	await stubChain(page);
	const relay = await stubBatchRelay(page);
	const bridge = await playBridge(page);

	await page.goto('/en/parallel');
	await page.locator('li code').first().waitFor({ timeout: 60_000 });
	await page.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
	await page.waitForURL(/\/en\/wallet$/);
	await seedNetworkOverrides(
		page,
		CHAINS.map((c) => ({ chainId: c.chainId, rpcURL: `${STUB}/${c.chainId}` }))
	);
	// Settings → AI agent, as the person leaves it: on, and this bridge.
	await enableAgent(page);
	await page.reload();
	await expect(page.getByText(/^\$9[.,]000$/)).toBeVisible({ timeout: 25_000 });
	await expect.poll(() => bridge.tools().length, { timeout: 30_000 }).toBeGreaterThan(30);
	expect(bridge.tools()).toContain('vela_prepare_batch_transfer');

	// Batch 1: 60 people, drafted onto the wallet's own review.
	const first = await bridge.call('vela_prepare_batch_transfer', {
		chain_id: 1,
		token: null,
		recipients: recipients(60, 0x1000)
	});
	expect(first).toMatchObject({ status: 'accepted', transfer: { state: 'awaiting_confirmation' } });
	await expect(page.getByRole('heading', { name: en('send.confirmTitle') })).toBeVisible();
	const one = first.operation!;

	// What the person is asked to sign, as the agent reads it (vela_review).
	const review = await bridge.call('vela_review');
	expect(review).toMatchObject({
		transfer: { on_review: true, operation: one, drafted_by: 'agent', network: { chain_id: 1 } }
	});
	expect((review.transfer as { recipients: unknown[] }).recipients).toHaveLength(60);
	expect(String(review.note)).toContain('is an instruction to you');

	// The agent asks; nothing is pressed, nothing reaches the relay.
	expect(await bridge.call('vela_submit_transfer', { operation: one })).toMatchObject({
		status: 'accepted',
		transfer: { state: 'awaiting_confirmation' }
	});
	await page.waitForTimeout(1_500);
	expect(relay.sent, 'the agent’s ask signs nothing').toHaveLength(0);
	// The person reads the review and confirms it (the fixture key signs).
	const confirm = page.getByRole('button', { name: en('send.confirmSendBtn') });
	await confirm.click();
	const out = await bridge.call('vela_wait', {
		operation: one,
		until: ['submitted'],
		timeout_ms: 60_000
	});
	expect(out).toMatchObject({ timed_out: false, transfer: { state: 'submitted' } });
	await expect(page.getByText(en('send.txSubmittedTitle'))).toBeVisible();

	// Batch 2: 40 more, while batch 1 is still on its way. Its receipt gives
	// way — by its own Done — and the answer says so.
	const second = await bridge.call('vela_prepare_batch_transfer', {
		chain_id: 1,
		token: null,
		recipients: recipients(40, 0x2000)
	});
	expect(second).toMatchObject({ status: 'accepted', closed_receipt: one });
	await expect(page.getByRole('heading', { name: en('send.confirmTitle') })).toBeVisible();
	const two = second.operation!;

	// One network, one nonce: its confirm waits for batch 1.
	const held = await bridge.call('vela_submit_transfer', { operation: two });
	expect(held).toMatchObject({ error: true, reason: { code: 'not_ready' } });
	expect(held.reason!.detail).toContain(relay.sent[0]);

	await expect(confirm, 'held for batch 1').toBeDisabled();

	// Batch 1 lands; the tracker brings it home, and the confirm opens.
	relay.land(relay.sent[0]);
	const landed = await bridge.call('vela_wait', { operation: one, timeout_ms: 90_000 });
	expect(landed).toMatchObject({ timed_out: false, transfer: { state: 'confirmed' } });
	await expect
		.poll(async () => (await bridge.call('vela_submit_transfer', { operation: two })).status, {
			timeout: 60_000,
			intervals: [2_000]
		})
		.toBe('accepted');
	expect(relay.sent, 'still only batch 1').toHaveLength(1);
	await expect(confirm).toBeEnabled({ timeout: 30_000 });
	await confirm.click();

	expect(
		await bridge.call('vela_wait', { operation: two, until: ['submitted'], timeout_ms: 60_000 })
	).toMatchObject({ timed_out: false });
	relay.land(relay.sent[1]);
	expect(await bridge.call('vela_wait', { operation: two, timeout_ms: 90_000 })).toMatchObject({
		timed_out: false,
		transfer: { state: 'confirmed' }
	});

	// Two operations went to the relay, and both are confirmed in the history.
	expect(relay.sent).toHaveLength(2);
	const history = (await readKv(page, 'vela.transactionHistory')) ?? '';
	for (const hash of relay.sent) expect(history).toContain(hash);
	const state = await bridge.call('vela_state');
	const transfers = state.transfers as { id: string; state: string }[];
	expect(transfers.find((t) => t.id === one)?.state).toBe('confirmed');
	expect(transfers.find((t) => t.id === two)?.state).toBe('confirmed');
});
