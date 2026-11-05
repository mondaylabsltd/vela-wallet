/**
 * Round 13 on the web, through the core:
 *
 * - r13-1: where a multisig lives when none of its networks is in the space
 *   in front (`network_space::lives_elsewhere`) — the shared vectors
 *   (`vectors/multisig/other-space.json`) replayed through the wasm;
 * - r13-3: the step before a review names the read that failed — the link
 *   door's `review.opening`, the words each key says, and the error a
 *   proposal that was not built becomes (`ActionState.unread`).
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it, vi } from 'vitest';

vi.mock('$lib/services/networks', async (actual) => ({
	...(await actual<typeof import('$lib/services/networks')>()),
	getAllNetworksSync: () => [
		{ chainId: 1, isTestnet: false },
		{ chainId: 8453, isTestnet: false },
		{ chainId: 5042002, isTestnet: true },
		{ chainId: 84532, isTestnet: true }
	],
	isHiddenNetwork: (id: number) => id === 84532
}));
import * as wasm from '../../../../../../rust/pkg-web/vela_core.js';
import vectors from '../../../../../../rust/crates/vela-core/tests/vectors/multisig/other-space.json';
import { unreadWords } from './review-door';
import { ProposalUnreadError } from './proposal-errors';
import { actionError } from '../shell/proposal-failure';
import { livesElsewhere } from './elsewhere';

describe('r13-1: an account of the other space (shared vectors, through the wasm)', () => {
	for (const c of vectors.cases) {
		it(c.name, () => {
			const got = JSON.parse(
				wasm.networkLivesElsewhere(
					JSON.stringify({
						chain_ids: c.chain_ids,
						here: c.here,
						other: c.other,
						front: c.front
					})
				)
			);
			expect(got).toEqual(c.expect);
		});
	}

	it('the shell asks with the person’s lists, split by space', () => {
		expect(livesElsewhere([5042002], 'mainnet')).toMatchObject({
			chain_ids: [5042002],
			space: 'testnet',
			heading_key: 'networkSpace.headingTestnet',
			switch_key: 'networkSpace.toTestnet'
		});
		expect(livesElsewhere([5042002, 8453], 'mainnet')).toBeNull();
		expect(livesElsewhere([84532], 'mainnet')).toBeNull();
		expect(livesElsewhere([], 'mainnet')).toBeNull();
	});
});

describe('r13-3: the step before a review names the read that failed', () => {
	const proposal = {
		mailboxDown: 'service down',
		unread: {
			chain: 'no {{network}}',
			both: 'neither {{network}} nor the service',
			linkChain: 'link: no {{network}}',
			linkService: 'link: no service',
			linkBoth: 'link: neither {{network}} nor the service'
		}
	};

	it('the core names each read by its key', () => {
		const ask = (args: object) =>
			JSON.parse(wasm.multisigLinkCall('review.opening', JSON.stringify(args)));
		expect(ask({ chain: true })).toBe('home.multisig.proposal.unread.chain');
		expect(ask({ service: true })).toBe('home.multisig.proposal.mailboxDown');
		expect(ask({ chain: true, service: true })).toBe('home.multisig.proposal.unread.both');
		expect(ask({ chain: true, link: true })).toBe('home.multisig.proposal.unread.linkChain');
		expect(ask({})).toBeNull();
	});

	it('each key is said in its words, with the network named', () => {
		expect(unreadWords(proposal, 'home.multisig.proposal.unread.chain', 'Arc Testnet')).toBe(
			'no Arc Testnet'
		);
		expect(unreadWords(proposal, 'home.multisig.proposal.unread.both', 'Arc Testnet')).toBe(
			'neither Arc Testnet nor the service'
		);
		expect(unreadWords(proposal, 'home.multisig.proposal.unread.linkService', 'Arc Testnet')).toBe(
			'link: no service'
		);
		expect(unreadWords(proposal, 'home.multisig.proposal.mailboxDown', 'Arc Testnet')).toBe(
			'service down'
		);
	});

	it('a network that hung is never said as the service’s waiting transactions', () => {
		const error = actionError(
			{ kind: 'unread', reason_key: 'home.multisig.proposal.unread.chain' },
			5042002
		);
		expect(error).toBeInstanceOf(ProposalUnreadError);
		expect((error as ProposalUnreadError).reasonKey).toBe('home.multisig.proposal.unread.chain');
	});
});
