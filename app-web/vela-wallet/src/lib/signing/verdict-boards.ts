/**
 * The simulation verdict's boards (SV1–SV8): the web signing sheet with each
 * thing its verdict's place can say, drawn over a CS board's own request.
 *
 * A board cannot show a verdict the core would not give. Each one starts as
 * a node's `eth_simulateV1` reply, written out (`sim-replies.ts`), read by
 * the core (`simOutcome`); a check's moves are judged by a real `token_trust`
 * machine told what Ethereum's registry would tell it (`judgedByFreshMachine`);
 * and the block is built by the sheet's own builder (`landedVerdict`), over
 * the rooms the live sheet keeps (`verdictRooms`). Only the request above the
 * place is borrowed — a CS board's blocks, as the mocks drew them.
 *
 * - SV1 — the simulation is out: "Checking…" in the place, the confirm held
 *   with the core's line under it.
 * - SV2 — a send: one row, what leaves.
 * - SV3 — a swap: what leaves and what comes back.
 * - SV4 — a token nobody vouches for arrives: its row is a direction and no
 *   figure, with the one warning under the card.
 * - SV5 — four rows and that warning: taller than the place keeps room for,
 *   drawn whole.
 * - SV6 — checked, and nothing of the person's moves: "No asset changes".
 * - SV7 — the call fails: "Expected to fail", with the reason the core cleaned.
 * - SV8 — nothing could be checked: the caution.
 */
import { judgedByFreshMachine, simOutcome, type TrustFacts } from '$lib/core/kernels';
import { IDENTITY } from '$lib/wallet/fixtures';
import type { SigningMessages } from './messages';
import type { Block, SigningModel, SigningStateId, VerdictBlock } from './model';
import { NOT_OFFERED, nativeLog, ran, reverted, transferLog } from './sim-replies';
import { checkingCard, landedVerdict, verdictRooms, type SimEnded } from './verdict';

export const VERDICT_STATES = ['sv1', 'sv2', 'sv3', 'sv4', 'sv5', 'sv6', 'sv7', 'sv8'] as const;
export type VerdictStateId = (typeof VERDICT_STATES)[number];

/** The core's line under a confirm held for the simulation. */
const HELD_KEY = 'componentsUi.signing.confirmBlock.simChecking';

const ME = IDENTITY.addressFull;
const SITE = '0x' + '51'.repeat(20);
// Ethereum's own, as the core's registry knows them.
const USDC = '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48';
const DAI = '0x6b175474e89094c44da98b954eedeac495271d0f';
const WETH = '0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2';
/** A contract the site controls. It answers "USDC" — as any contract can. */
const LURE = '0x' + 'ba'.repeat(20);

/** What Ethereum's registry tells the trust machine, and what a stranger's token answers. */
const FACTS: TrustFacts = {
	stables: [USDC, DAI],
	wrapped: WETH,
	meta: { [LURE]: { symbol: 'USDC', decimals: 18 } }
};

const unit = (decimals: number) => 10n ** BigInt(decimals);
const out = (token: string, amount: bigint) => transferLog(token, ME, SITE, amount);
const into = (token: string, amount: bigint) => transferLog(token, SITE, ME, amount);

/** Each board: the CS board whose request it is drawn over, and the node's reply (none: still out). */
const BOARDS: Record<VerdictStateId, { over: SigningStateId; reply: string | null }> = {
	sv1: { over: 'cs1', reply: null },
	sv2: { over: 'cs1', reply: ran(out(USDC, 1_000n * unit(6))) },
	sv3: {
		over: 'cs11',
		reply: ran(out(USDC, 1_000n * unit(6)), into(WETH, 305_100_000_000_000_000n))
	},
	sv4: {
		over: 'cs2',
		reply: ran(nativeLog(ME, SITE, 10n * unit(18)), into(LURE, 5_000n * unit(18)))
	},
	sv5: {
		over: 'cs11',
		reply: ran(
			out(USDC, 1_000n * unit(6)),
			into(WETH, 305_100_000_000_000_000n),
			into(LURE, 5_000n * unit(18)),
			out(DAI, 18_500_000_000_000_000_000n)
		)
	},
	sv6: { over: 'cs6', reply: ran() },
	sv7: { over: 'cs1', reply: reverted('ERC20: transfer amount exceeds balance') },
	sv8: { over: 'cs1', reply: NOT_OFFERED }
};

/** Which CS board each verdict board borrows its request from. */
export function verdictBoardOver(state: VerdictStateId): SigningStateId {
	return BOARDS[state].over;
}

/** A reply as the host would hold it once the simulation has ended. */
function ended(reply: string): SimEnded {
	const verdict = simOutcome(ME, reply);
	return {
		verdict,
		judged: verdict?.kind === 'deltas' ? judgedByFreshMachine(ME, 1, verdict.deltas, FACTS) : null
	};
}

/** What stands in the place on `state`'s board. */
function inner(state: VerdictStateId, m: SigningMessages): VerdictBlock {
	const reply = BOARDS[state].reply;
	if (reply === null) return checkingCard(m);
	// The boards' Ethereum and the mocks' number style ("1,000").
	return landedVerdict(ended(reply), { m, nativeSymbol: 'ETH', preset: 'comma_dot' }).block;
}

/**
 * `state`'s board over `base` — the CS board named by {@link verdictBoardOver}.
 * The request's own blocks stay; the place is added after them, where the
 * live sheet puts it. Nothing a CS mock says of a simulation is kept beside
 * it (a drawn card, the technical row): the place is the one voice.
 */
export function buildVerdictState(
	state: VerdictStateId,
	base: SigningModel,
	m: SigningMessages
): SigningModel {
	const landed = BOARDS[state].reply !== null;
	const blocks: Block[] = [
		...base.blocks.filter((block) => block.kind !== 'balances' && block.kind !== 'verdict'),
		{ kind: 'verdict', inner: [inner(state, m)], rooms: verdictRooms(m), landed }
	];
	return {
		...base,
		blocks,
		tech: { ...base.tech, simResult: undefined },
		// Held for the verdict, with the core's line — or open beside it.
		confirm: landed
			? { action: base.confirm.action, enabled: true }
			: { action: base.confirm.action, enabled: false, note: m.confirmBlock[HELD_KEY] }
	};
}
