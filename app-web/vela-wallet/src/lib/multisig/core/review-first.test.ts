/**
 * §3.6: the proposer's own review before its passkey — drawn by the host,
 * answered once: "Propose and approve" goes on, a decline or a closed sheet
 * is a cancel, and with no host up to draw it nothing is signed.
 */
import { describe, expect, it } from 'vitest';
import { reviewFirst, reviewFirstHost, REVIEW_DECLINED } from './review-first.svelte';
import type { ChainState, ProposalItem } from './proposals.svelte';

const chain = { chainId: 100 } as ChainState;
const item = { safeTxHash: `0x${'ab'.repeat(32)}` } as ProposalItem;

describe('the review before the proposer signs (§3.6)', () => {
	it('confirmed: it goes on, and the review is gone', async () => {
		const detach = reviewFirstHost.attach();
		const asked = reviewFirst('0x1', 100, chain, item);
		expect(reviewFirstHost.request?.item).toBe(item);
		reviewFirstHost.request!.confirm();
		await expect(asked).resolves.toBeNull();
		expect(reviewFirstHost.request).toBeNull();
		detach();
	});

	it('declined, or the host goes while it asks: a cancel, nothing signed', async () => {
		const detach = reviewFirstHost.attach();
		const declined = reviewFirst('0x1', 100, chain, item);
		reviewFirstHost.request!.decline();
		await expect(declined).rejects.toMatchObject({ name: 'AbortError', message: REVIEW_DECLINED });
		const left = reviewFirst('0x1', 100, chain, item);
		detach();
		await expect(left).rejects.toMatchObject({ name: 'AbortError' });
		expect(reviewFirstHost.request).toBeNull();
	});

	it('no host up to draw it: never signed unreviewed', async () => {
		await expect(reviewFirst('0x1', 100, chain, item)).rejects.toMatchObject({
			name: 'AbortError'
		});
	});
});
