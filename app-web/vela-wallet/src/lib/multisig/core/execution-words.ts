/**
 * An execution's preview in words: why it would fail, and what its fee is
 * and pays for. Pure, so the sheet and the home say the same thing, and a
 * test can read every line.
 */
import { feeAmountText } from '$lib/flows/fee-line';
import { fillTemplate } from '$lib/i18n/fill';
import type { MultisigMessages } from '../messages';
import type { ExecutionBlock, ExecutionPreview } from './execution';
import type { MultisigAmount } from '../generated/MultisigAmount';

/**
 * r5reg-15: how many decimals a fee reads in — the wallet's own fee
 * formatter (`feeAmountText`) at its budget for a fee paid in a token. An
 * "≈" figure at six decimals ("0.035412 USDC") said more than anyone reads;
 * a tiny fee keeps its first two significant digits, so no fee ever reads
 * as "0".
 */
const FEE_DECIMALS = 4;

/**
 * A fee (or the setup it pays for) in the chain's coin, 18 decimals:
 * "0.0354 USDC". r21-2: the core's figure, rounded UP — never below what the
 * execution is charged.
 */
export function feeCoin(amount: bigint, symbol: string): string {
	const text = feeAmountText(amount, 18, FEE_DECIMALS);
	return symbol ? `${text} ${symbol}` : text;
}

/**
 * What an executor HOLDS, beside a fee it cannot pay: the same places, cut
 * DOWN exactly (no float), so the balance never reads as more than there is
 * — a fee rounded up beside a balance rounded up could read alike while the
 * one cannot pay the other. A balance under the last place keeps its first
 * two significant digits, cut too.
 */
export function balanceCoin(amount: bigint, symbol: string): string {
	const text = cutDown(amount < 0n ? 0n : amount, 18, FEE_DECIMALS);
	return symbol ? `${text} ${symbol}` : text;
}

function cutDown(units: bigint, decimals: number, places: number): string {
	if (units === 0n) return '0';
	const raw = units.toString().padStart(decimals + 1, '0');
	const whole = raw.slice(0, raw.length - decimals);
	const fraction = raw.slice(raw.length - decimals);
	const first = fraction.search(/[1-9]/);
	const keep = whole === '0' && first >= places ? Math.min(first + 2, fraction.length) : places;
	const kept = fraction.slice(0, keep).replace(/0+$/, '');
	return kept === '' ? whole : `${whole}.${kept}`;
}

/**
 * r12-1: an amount as the core says it — its `text` ("0.01 USDC"), or, when
 * nobody said the token's decimals, its base units with "decimals unknown".
 * Nothing here scales base units.
 */
export function amountText(m: MultisigMessages, amount: MultisigAmount): string {
	return amount.said.kind === 'known'
		? amount.said.text
		: fillTemplate(m.review.reading.decimalsUnknown, { raw: amount.said.raw });
}

/** "the multisig holds 0.28 USDC, and this needs 0.5 USDC" / the network's reason. */
export function whyText(m: MultisigMessages, why: ExecutionBlock): string {
	if (why.kind === 'short') {
		return fillTemplate(m.proposal.whyShort, {
			have: amountText(m, why.have),
			need: amountText(m, why.need)
		});
	}
	return why.reason
		? fillTemplate(m.proposal.whyReverts, { reason: why.reason })
		: m.proposal.whyRevertsPlain;
}

export interface FeeWords {
	/** "Network fee ≈ 0.035 USDC, paid by Alice" */
	fee: string;
	/**
	 * r18-5 (§32): the mark under the fee line while a press is out on it —
	 * "Signing at this fee — it stays fixed until you confirm or cancel." —
	 * else `null`.
	 */
	signing: string | null;
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
	const signing = preview.signing === true ? m.proposal.feeSigning : null;
	const items: string[] = [];
	if (preview.setup.multisig) items.push(m.proposal.setupMultisig);
	for (const member of preview.setup.members) {
		items.push(fillTemplate(m.proposal.setupMember, { name: opts.label(member) }));
	}
	if (preview.setup.executor) {
		items.push(fillTemplate(m.proposal.setupExecutor, { name: opts.label(opts.executor) }));
	}
	if (items.length === 0) return { fee, signing, setup: null, items };
	const setup =
		preview.setup.amount !== null && preview.setup.amount > 0n
			? fillTemplate(m.proposal.feeSetup, {
					network: opts.network,
					amount: coin(preview.setup.amount)
				})
			: fillTemplate(m.proposal.feeSetupPlain, { network: opts.network });
	return { fee, signing, setup, items };
}
