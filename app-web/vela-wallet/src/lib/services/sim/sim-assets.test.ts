/**
 * The simulation's balance changes, netted from its logs (`deriveAssetDeltas`):
 * a wrapped coin's own `Deposit` and `Withdrawal` count as the mint and burn
 * they are (cs-21: a WXDAI deposit showed "xDAI −1" and never "+1 WXDAI").
 */
import { describe, expect, it } from 'vitest';
import {
	deriveAssetDeltas,
	NATIVE_TRANSFER_SENTINEL,
	TRANSFER_TOPIC,
	WETH_DEPOSIT_TOPIC,
	WETH_WITHDRAWAL_TOPIC
} from './sim-assets';

const ME = '0x' + 'aa'.repeat(20);
const WXDAI = '0xe91d153e0b41518a2ce8dd3d7944fa863463a97d';
const word = (n: bigint) => '0x' + n.toString(16).padStart(64, '0');
const topic = (address: string) => '0x' + address.slice(2).padStart(64, '0');
const ONE = 10n ** 18n;

describe('deriveAssetDeltas', () => {
	it('reads a wrap as the coin out and the wrapped token in', () => {
		const deltas = deriveAssetDeltas(
			[
				{
					address: NATIVE_TRANSFER_SENTINEL,
					topics: [TRANSFER_TOPIC, topic(ME), topic(WXDAI)],
					data: word(ONE)
				},
				{ address: WXDAI, topics: [WETH_DEPOSIT_TOPIC, topic(ME)], data: word(ONE) }
			],
			ME
		);
		expect(deltas).toEqual([
			{ kind: 'native', token: undefined, delta: -ONE },
			{ kind: 'erc20', token: WXDAI, delta: ONE }
		]);
	});

	it('reads an unwrap as the wrapped token out and the coin in', () => {
		const deltas = deriveAssetDeltas(
			[
				{ address: WXDAI, topics: [WETH_WITHDRAWAL_TOPIC, topic(ME)], data: word(ONE) },
				{
					address: NATIVE_TRANSFER_SENTINEL,
					topics: [TRANSFER_TOPIC, topic(WXDAI), topic(ME)],
					data: word(ONE)
				}
			],
			ME
		);
		expect(deltas).toEqual([
			{ kind: 'erc20', token: WXDAI, delta: -ONE },
			{ kind: 'native', token: undefined, delta: ONE }
		]);
	});

	it("ignores another account's wrap, and an NFT's transfer", () => {
		const other = '0x' + 'bb'.repeat(20);
		expect(
			deriveAssetDeltas(
				[
					{ address: WXDAI, topics: [WETH_DEPOSIT_TOPIC, topic(other)], data: word(ONE) },
					{
						address: WXDAI,
						topics: [TRANSFER_TOPIC, topic(other), topic(ME), word(7n)],
						data: '0x'
					}
				],
				ME
			)
		).toEqual([]);
	});

	// r5reg-1: Arc's simulation logs each native move twice — traced from the
	// sentinel and from its own `0xffff…fffe` — once to an account, once to a
	// smart account (which logs `SafeReceived` too). Each move counts once.
	it('counts a native move the chain also logs itself once', () => {
		const CHAIN = '0xfffffffffffffffffffffffffffffffffffffffe';
		const smart = '0x' + 'cc'.repeat(20);
		const eoa = '0x' + 'dd'.repeat(20);
		const leg = (to: string) => [
			{
				address: NATIVE_TRANSFER_SENTINEL,
				topics: [TRANSFER_TOPIC, topic(ME), topic(to)],
				data: word(ONE)
			},
			{ address: CHAIN, topics: [TRANSFER_TOPIC, topic(ME), topic(to)], data: word(ONE) }
		];
		expect(deriveAssetDeltas(leg(eoa), ME)).toEqual([
			{ kind: 'native', token: undefined, delta: -ONE }
		]);
		expect(deriveAssetDeltas([...leg(smart), ...leg(eoa)], ME)).toEqual([
			{ kind: 'native', token: undefined, delta: -2n * ONE }
		]);
		// A receipt has no traced log: the chain's own is the record — the
		// coin, never a token at 0xffff…fffe.
		expect(
			deriveAssetDeltas(
				[{ address: CHAIN, topics: [TRANSFER_TOPIC, topic(ME), topic(smart)], data: word(ONE) }],
				ME
			)
		).toEqual([{ kind: 'native', token: undefined, delta: -ONE }]);
	});
});
