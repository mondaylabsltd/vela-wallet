// Round 3: the recipient's first-transfer tell is never built on a history
// read that failed — "unknown", said as that, not "first time sending here".
import { beforeEach, describe, expect, it, vi } from 'vitest';

const seams = vi.hoisted(() => ({
	history: vi.fn<() => Promise<unknown[]>>(),
	rpc: vi.fn()
}));

vi.mock('./records', () => ({ readTransactionHistory: seams.history }));
vi.mock('./rpc-pool', () => ({ poolRpcCall: seams.rpc }));

import { clearRecipientRiskCache, resolveRecipientRisk } from './recipient-risk';

const TO = '0x' + 'b0'.repeat(20);

beforeEach(() => {
	clearRecipientRiskCache();
	seams.history.mockReset();
	seams.rpc.mockReset();
	seams.rpc.mockResolvedValue({ result: '0x' });
});

describe('the recipient’s first-transfer tell', () => {
	it('a history that could not be read is unknown, never a first transfer', async () => {
		seams.history.mockRejectedValue(new Error('IndexedDB blocked'));
		await expect(resolveRecipientRisk(100, TO)).resolves.toEqual({
			isContract: false,
			firstInteraction: null,
			historyUnread: true
		});
	});

	it('a history read finds the earlier send, or finds none', async () => {
		seams.history.mockResolvedValue([{ to: TO.toUpperCase().replace('0X', '0x'), type: 'send' }]);
		await expect(resolveRecipientRisk(100, TO)).resolves.toMatchObject({
			firstInteraction: false,
			historyUnread: false
		});
		seams.history.mockResolvedValue([]);
		await expect(resolveRecipientRisk(100, TO)).resolves.toMatchObject({
			firstInteraction: true,
			historyUnread: false
		});
	});

	it('a receive is not an earlier send', async () => {
		seams.history.mockResolvedValue([{ to: TO, type: 'receive' }]);
		await expect(resolveRecipientRisk(100, TO)).resolves.toMatchObject({
			firstInteraction: true
		});
	});
});
