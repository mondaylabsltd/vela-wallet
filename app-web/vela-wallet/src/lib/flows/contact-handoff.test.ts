import { describe, expect, it } from 'vitest';
import { flowHandoffQuery, readFlowHandoff } from './contact-handoff';

const ADDR = '0x' + 'a1'.repeat(20);

describe('the contacts → wallet hand-off', () => {
	it('round-trips each of the three journeys through the URL', () => {
		for (const handoff of [
			{ kind: 'send', recipient: ADDR } as const,
			{ kind: 'receive' } as const,
			{ kind: 'group-send', groupId: 'grp_3' } as const
		]) {
			const query = flowHandoffQuery(handoff);
			expect(query.startsWith('?')).toBe(true);
			expect(readFlowHandoff(query)).toEqual(handoff);
		}
	});

	it('a plain visit, or a recipient that is not an address, hands nothing off', () => {
		expect(readFlowHandoff('')).toBeNull();
		expect(readFlowHandoff('?to=vitalik.eth')).toBeNull();
		expect(readFlowHandoff('?flow=send')).toBeNull();
	});
});
