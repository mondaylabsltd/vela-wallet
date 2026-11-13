/**
 * The words the connect page and the outside review put on the core facts
 * (`multisig_outside::reading`): a transaction title, a row (what it was,
 * how it ended), a token line. Nothing here reads a transaction or decides
 * what it is: each kind maps to its corpus words, the amounts formatted as
 * every money surface formats them.
 */
import { fillTemplate } from '$lib/i18n/fill';
import { requireApprovals } from '../core/approvals-words';
import { nativeSymbol } from '$lib/services/networks';
import type { MultisigMessages } from '../messages';
import { chainName, formatUnits, memberWho, shortenAddress } from '../core/display';
import type { OutsideRowLabel } from '../generated/OutsideRowLabel';
import type { OutsideRowOutcome } from '../generated/OutsideRowOutcome';
import type { OutsideTokenFacts } from '../generated/OutsideTokenFacts';
import type { OutsideTxTitle } from '../generated/OutsideTxTitle';
import { problemText } from './errors';

/** A token by the name an amount is said in: its symbol, the network coin, else its short address. */
export function symbolOf(chainId: number, facts: OutsideTokenFacts): string {
	if (facts.symbol) return facts.symbol;
	return facts.token ? shortenAddress(facts.token) : nativeSymbol(chainId);
}

/** "500 USDC": an amount in base units, in its token units when the token said them. */
export function amountText(chainId: number, amount: string, facts: OutsideTokenFacts): string {
	const figure = facts.decimals === null ? amount : formatUnits(amount, facts.decimals);
	return `${figure} ${symbolOf(chainId, facts)}`.trim();
}

/**
 * Which token a transfer sends, when it is a contract: its symbol (the
 * registry name, else the contract own word) and its address; the network
 * coin, by the network.
 */
export function tokenLine(m: MultisigMessages, chainId: number, facts: OutsideTokenFacts): string {
	return facts.token
		? fillTemplate(m.external.tokenContract, {
				symbol: symbolOf(chainId, facts),
				address: shortenAddress(facts.token)
			})
		: fillTemplate(m.external.tokenNative, {
				symbol: nativeSymbol(chainId),
				network: chainName(chainId)
			});
}

/** A transaction one line: "Send 500 USDC", "Add Bob (0x12…34)", "Cancel #7". */
export function titleText(
	m: MultisigMessages,
	chainId: number,
	title: OutsideTxTitle,
	nonce: string,
	who: (address: string) => string = (a) => memberWho(a)
): string {
	switch (title.kind) {
		case 'send': {
			const facts = title.token;
			return fillTemplate(m.action.send, {
				amount: facts.decimals === null ? title.amount : formatUnits(title.amount, facts.decimals),
				symbol: symbolOf(chainId, facts)
			});
		}
		case 'batch':
			return fillTemplate(title.transfersOnly ? m.action.batch : m.action.batchCalls, {
				total: title.total
			});
		case 'cancel':
			return fillTemplate(m.action.cancel, { nonce });
		case 'addOwner':
			return fillTemplate(m.change.add, { member: who(title.owner) });
		case 'removeOwner':
			return fillTemplate(m.change.remove, { member: who(title.owner) });
		case 'swapOwner':
			return fillTemplate(m.change.replace, {
				old: who(title.oldOwner),
				new: who(title.newOwner)
			});
		case 'changeThreshold':
			return requireApprovals(m, title.threshold);
		case 'rename':
			return fillTemplate(m.action.rename, { name: title.name });
		case 'call':
			return BigInt(title.value) === 0n
				? m.action.call
				: fillTemplate(m.action.callWithValue, {
						amount: formatUnits(title.value, 18),
						symbol: nativeSymbol(chainId)
					});
		case 'unsupported':
			return m.action.unsupported;
	}
}

/** A number as Vela writes it: "#7". */
export function numbered(m: MultisigMessages, nonce: string): string {
	return fillTemplate(m.action.numberValue, { nonce });
}

/** What a row was: "Approve #7 · Send 0.02 USDC to 0x…", "Reject #8", "Execute #7". */
export function rowLabelText(m: MultisigMessages, label: OutsideRowLabel): string {
	const w = m.external;
	switch (label.kind) {
		case 'approve':
			return fillTemplate(w.rowApprove, {
				number: numbered(m, label.nonce),
				what: titleText(m, label.chainId, label.title, label.nonce)
			});
		case 'approveTo':
			return fillTemplate(w.rowApproveTo, {
				number: numbered(m, label.nonce),
				what: titleText(m, label.chainId, label.title, label.nonce),
				to: shortenAddress(label.to)
			});
		case 'reject':
			return fillTemplate(w.rowReject, { number: numbered(m, label.nonce) });
		case 'execute':
			return fillTemplate(w.rowExecute, { number: numbered(m, label.nonce) });
		case 'signTx':
			return w.signTx;
		case 'sendTx':
			return w.sendTx;
		case 'switchChain':
			return w.switchChain;
		case 'other':
			return label.method;
	}
}

/** How a row ended, in words; `wallet`: the wallet name, for its own refusal. */
export function rowOutcomeText(
	m: MultisigMessages,
	outcome: OutsideRowOutcome,
	wallet: string
): string {
	const w = m.external;
	switch (outcome.kind) {
		case 'waiting':
			return w.outcomeWaiting;
		case 'inWallet':
			return w.outcomeInWallet;
		case 'done':
			return w.outcomeDone;
		case 'signedHash':
			return w.outcomeSignedHash;
		case 'sent':
			return w.outcomeSent;
		case 'switched':
			return w.outcomeSwitched;
		case 'withdrawn':
			return w.outcomeWithdrawn;
		case 'declined':
			return w.outcomeDeclined;
		case 'walletDeclined':
			return w.walletDeclined;
		case 'failed':
			return outcome.problem
				? problemText(outcome.problem, w, fillTemplate, wallet, '')
				: w.outcomeFailed;
		case 'notSent':
			return w.outcomeNotSent;
	}
}
