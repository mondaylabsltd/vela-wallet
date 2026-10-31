/**
 * An execution's preview in words: why it would fail, and what its fee is
 * and pays for. Pure, so the sheet and the home say the same thing, and a
 * test can read every line.
 */
import { feeAmountText } from '$lib/flows/fee-line';
import { fillTemplate } from '$lib/i18n/fill';
import type { MultisigMessages } from '../messages';
import type { ExecutionBlock, ExecutionPreview } from './execution';
import { formatUnits } from './units';

/**
 * r5reg-15: how many decimals a fee reads in — the wallet's own fee
 * formatter (`feeAmountText`) at its budget for a fee paid in a token. An
 * "≈" figure at six decimals ("0.035412 USDC") said more than anyone reads;
 * the formatter still widens until a tiny fee's first digit shows, so no
 * fee ever reads as "0".
 */
const FEE_DECIMALS = 4;

/** A fee (or what pays it) in the chain's coin, 18 decimals: "0.0354 USDC". */
export function feeCoin(amount: bigint, symbol: string): string {
	const text = feeAmountText(Number(amount) / 1e18, FEE_DECIMALS);
	return symbol ? `${text} ${symbol}` : text;
}

/** "the multisig holds 0.28 USDC, and this needs 0.5 USDC" / the network's reason. */
export function whyText(m: MultisigMessages, why: ExecutionBlock): string {
	if (why.kind === 'short') {
		const amount = (v: bigint) =>
			`${formatUnits(v, why.decimals)}${why.symbol ? ` ${why.symbol}` : ''}`;
		return fillTemplate(m.proposal.whyShort, { have: amount(why.have), need: amount(why.need) });
	}
	return why.reason
		? fillTemplate(m.proposal.whyReverts, { reason: why.reason })
		: m.proposal.whyRevertsPlain;
}

export interface FeeWords {
	/** "Network fee ≈ 0.035 USDC, paid by Alice" */
	fee: string;
	/** "Includes one-time setup on Arc Testnet ≈ 0.02 USDC:", or `null` with nothing set up. */
	setup: string | null;
	/** What that setup is, one line each. */
	items: string[];
}

/** The fee line of a priced execution, and what it pays for besides the transaction. */
export function feeWords(
	m: MultisigMessages,
	preview: Extract<ExecutionPreview, { kind: 'priced' }>,
	opts: {
		executor: string;
		symbol: string;
		network: string;
		label: (address: string) => string;
	}
): FeeWords {
	const coin = (v: bigint) => feeCoin(v, opts.symbol);
	const fee = fillTemplate(m.proposal.feeLine, {
		fee: coin(preview.fee.amount),
		name: opts.label(opts.executor)
	});
	const items: string[] = [];
	if (preview.setup.multisig) items.push(m.proposal.setupMultisig);
	for (const member of preview.setup.members) {
		items.push(fillTemplate(m.proposal.setupMember, { name: opts.label(member) }));
	}
	if (preview.setup.executor) {
		items.push(fillTemplate(m.proposal.setupExecutor, { name: opts.label(opts.executor) }));
	}
	if (items.length === 0) return { fee, setup: null, items };
	const setup =
		preview.setup.amount !== null && preview.setup.amount > 0n
			? fillTemplate(m.proposal.feeSetup, {
					network: opts.network,
					amount: coin(preview.setup.amount)
				})
			: fillTemplate(m.proposal.feeSetupPlain, { network: opts.network });
	return { fee, setup, items };
}
