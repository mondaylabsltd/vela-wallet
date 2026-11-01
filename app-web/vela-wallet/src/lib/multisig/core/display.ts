/**
 * How a multisig and its transactions read on screen: member names, a
 * transaction's one-line summary, amounts in their token's units.
 *
 * Names are only ever this device's own (a Vela account's local name) or what
 * the member registered for itself in the passkey index — never text that came
 * with a link or a proposal (docs/multisig-vault-design.md §5.3).
 */
import { fillTemplate } from '$lib/i18n/fill';
import { requireApprovals } from './approvals-words';
import * as wasm from '../../../../../../rust/pkg-web/vela_core.js';
import { loadAccounts } from '$lib/onboarding/core/storage';
import { chainMeta } from '$lib/services/chains';
import { poolRpcCall } from '$lib/services/rpc-pool';
import { shortenAddress as shorten } from '$lib/wallet/identity';
import { sentAmountText } from '$lib/wallet/live';
import { eip55 } from '../external/eip55';
import type { MultisigMessages } from '../messages';
import {
	sameAddress,
	validName,
	type Action,
	type MemberChange,
	type MemberChanges
} from './kernel';
import { formatUnits as cutUnits, plainUnits } from './units';

export { parseUnits } from './units';

/**
 * "0xd9c79C…a20432": an address short, always in its EIP-55 case (r4D-19) —
 * the chain's logs and the mailbox hand addresses over lowercased, a member
 * typed in another case, and one home read "To 0xd9c79C…" beside "From
 * 0xd9c79c…" for one address. Every multisig screen shortens through here.
 */
export function shortenAddress(address: string): string {
	return shorten(eip55(address));
}

/**
 * An amount in base units as every money surface reads it (r4a-21): the
 * confirm's and Activity's own rule (`sentAmountText` — the ladder, cut,
 * the person's decimal mark and grouping), so "Send 1,000 USDC" on the sheet
 * and "1,000" on the confirm are one figure.
 */
export function formatUnits(amount: string | bigint, decimals: number | null): string {
	// r8-2: unknown decimals are never assumed — the raw integer, as it is.
	if (decimals === null) return amount.toString();
	try {
		return sentAmountText(plainUnits(amount.toString(), decimals));
	} catch {
		return cutUnits(amount, decimals);
	}
}

/**
 * A member as a change names it: its name and short address, never the name
 * alone — a name is a claim, the address is who gets the say.
 */
export function memberWho(address: string, name: string | null = localName(address)): string {
	const short = shortenAddress(address);
	return name && name !== short ? `${name} (${short})` : short;
}

/** A member's name on this device, or `null` when it is not one of its accounts. */
export function localName(address: string): string | null {
	const account = loadAccounts().find((a) => sameAddress(a.address, address));
	return account ? account.name || shortenAddress(account.address) : null;
}

export function memberLabel(address: string): string {
	return localName(address) ?? shortenAddress(address);
}

/**
 * A multisig's name: the one written with its record (a label anyone who
 * recorded it chose — shown only if it is a displayable name), else this
 * device's own label, else its rule ("Multisig 2/3").
 */
export function multisigName(
	account: { genesis: { name?: string }; label?: string; threshold: number; owners: string[] },
	m: MultisigMessages
): string {
	const recorded = account.genesis.name;
	return (
		(recorded && displayable(recorded) ? recorded : undefined) ??
		account.label ??
		fillTemplate(m.switcher.rowName, { m: account.threshold, n: account.owners.length })
	);
}

function displayable(name: string): boolean {
	try {
		return validName(name);
	} catch {
		return false;
	}
}

export interface TokenInfo {
	symbol: string;
	/** `null`: unknown (r8-2) — an amount is said raw, never in 18 decimals. */
	decimals: number | null;
}

/** A token nothing could be read of: no symbol, no decimals (r8-2). */
export const UNREAD_TOKEN: TokenInfo = { symbol: '', decimals: null };

/** The wallet's registry's word on a token (the core's), or `null`. */
function registryInfo(chainId: number, token: string): TokenInfo | null {
	try {
		const facts = JSON.parse(
			wasm.multisigLinkCall('review.token', JSON.stringify({ chainId, token }))
		) as { symbol: string | null; decimals: number | null; known: boolean };
		return facts.known && facts.decimals !== null
			? { symbol: facts.symbol ?? '', decimals: facts.decimals }
			: null;
	} catch {
		return null;
	}
}

const tokenCache = new Map<string, Promise<TokenInfo>>();

function decodeString(hex: string): string {
	const clean = hex.startsWith('0x') ? hex.slice(2) : hex;
	if (clean.length < 128) {
		// bytes32 symbols (old tokens): the non-zero bytes as text.
		const bytes = clean.match(/../g) ?? [];
		return bytes
			.map((b) => parseInt(b, 16))
			.filter((b) => b > 0)
			.map((b) => String.fromCharCode(b))
			.join('');
	}
	const length = parseInt(clean.slice(64, 128), 16);
	const data = clean.slice(128, 128 + length * 2);
	const bytes = new Uint8Array((data.match(/../g) ?? []).map((b) => parseInt(b, 16)));
	return new TextDecoder().decode(bytes);
}

/** A token's symbol and decimals: the native coin's from the chain table, an ERC-20's from the chain (cached). */
export function tokenInfo(chainId: number, token: string | null): Promise<TokenInfo> {
	if (!token) {
		return Promise.resolve({ symbol: chainMeta(chainId)?.nativeSymbol ?? 'ETH', decimals: 18 });
	}
	const key = `${chainId}:${token.toLowerCase()}`;
	let hit = tokenCache.get(key);
	if (!hit) {
		const known = registryInfo(chainId, token);
		if (known) {
			hit = Promise.resolve(known);
			tokenCache.set(key, hit);
			return hit;
		}
		hit = (async () => {
			const [symbol, decimals] = await Promise.all([
				poolRpcCall('eth_call', [{ to: token, data: '0x95d89b41' }, 'latest'], chainId),
				poolRpcCall('eth_call', [{ to: token, data: '0x313ce567' }, 'latest'], chainId)
			]);
			// r8-2: a contract that did not say its decimals (or a read that
			// failed) is not a reading: nothing is kept, and it is asked again.
			if (typeof decimals.result !== 'string' || decimals.result === '0x') {
				throw new Error(`no decimals for ${token} on ${chainId}`);
			}
			const places = Number(BigInt(decimals.result));
			if (!Number.isInteger(places) || places > 36) {
				throw new Error(`no decimals for ${token} on ${chainId}`);
			}
			return {
				symbol:
					typeof symbol.result === 'string'
						? decodeString(symbol.result) || shortenAddress(token)
						: shortenAddress(token),
				decimals: places
			};
		})();
		hit.catch(() => tokenCache.delete(key));
		tokenCache.set(key, hit);
	}
	return hit;
}

/** A transaction's one line, e.g. "Send 500 USDC", "Add Bob (0x12…34)". */
export async function actionTitle(
	m: MultisigMessages,
	chainId: number,
	action: Action | null,
	nonce: bigint,
	/** How a member is named (the sheet's: with the names this device knows). */
	who: (address: string) => string = (a) => memberWho(a)
): Promise<string> {
	if (action === null) return m.action.unsupported;
	switch (action.kind) {
		case 'transfer': {
			const info = await tokenInfo(chainId, action.token).catch(() => UNREAD_TOKEN);
			return fillTemplate(m.action.send, {
				amount:
					info.decimals === null
						? fillTemplate(m.review.reading.decimalsUnknown, { raw: action.amount })
						: formatUnits(action.amount, info.decimals),
				symbol: info.symbol || (action.token ? shortenAddress(action.token) : '')
			});
		}
		case 'batch':
			return fillTemplate(
				action.calls.every((c) => c.action.kind === 'transfer')
					? m.action.batch
					: m.action.batchCalls,
				{ total: action.calls.length }
			);
		case 'cancel':
			return fillTemplate(m.action.cancel, { nonce: nonce.toString() });
		// r4a-1: who it adds, removes or replaces — never only "Add a member".
		case 'addOwner':
			return fillTemplate(m.change.add, { member: who(action.owner) });
		case 'removeOwner':
			return fillTemplate(m.change.remove, { member: who(action.owner) });
		case 'swapOwner':
			return fillTemplate(m.change.replace, {
				old: who(action.oldOwner),
				new: who(action.newOwner)
			});
		case 'changeThreshold':
			return requireApprovals(m, action.threshold);
		case 'rename':
			return fillTemplate(m.action.rename, { name: action.name });
		case 'call': {
			if (BigInt(action.value) === 0n) return m.action.call;
			const info = await tokenInfo(chainId, null);
			return fillTemplate(m.action.callWithValue, {
				amount: formatUnits(action.value, info.decimals),
				symbol: info.symbol
			});
		}
	}
}

/**
 * A batch's calls into the multisig itself, one line each: every one changes
 * who controls it (a member added, removed or replaced, the approvals it
 * needs), so a member reads it first and loudly — "Call #2 changes this
 * multisig: Add a member 0x12…34, Require 2 approvals". Nothing for anything
 * else (a transaction that is only a member change is titled as one).
 */
export function memberChanges(m: MultisigMessages, action: Action | null): string[] {
	if (action?.kind !== 'batch') return [];
	return action.calls.flatMap((call, i) => {
		const a = call.action;
		let change: string;
		switch (a.kind) {
			case 'addOwner':
				change = `${m.action.addOwner} ${shortenAddress(a.owner)}, ${requireApprovals(m, a.threshold)}`;
				break;
			case 'removeOwner':
				change = `${m.action.removeOwner} ${shortenAddress(a.owner)}, ${requireApprovals(m, a.threshold)}`;
				break;
			case 'swapOwner':
				change = `${m.action.swapOwner} ${shortenAddress(a.oldOwner)} → ${shortenAddress(a.newOwner)}`;
				break;
			case 'changeThreshold':
				change = requireApprovals(m, a.threshold);
				break;
			default:
				return [];
		}
		return [fillTemplate(m.proposal.changesMembers, { index: i + 1, change })];
	});
}

/** "2 of 3": a rule as the create sheet writes one. */
export function ruleText(m: MultisigMessages, rule: { threshold: number; owners: number }): string {
	return fillTemplate(m.create.thresholdValue, { m: rule.threshold, n: rule.owners });
}

/**
 * The rule as a requirement (r4D-13): "Needs 2 of 3 members" — and, with one
 * member, "Needs its only member's approval", never "Needs 1 of 1 members".
 */
export function ruleTitle(m: MultisigMessages, rule: { m: number; n: number }): string {
	return rule.n === 1 ? m.panel.titleSolo : fillTemplate(m.panel.title, rule);
}

/**
 * The header's rule chip (r4D-13, r5reg-10): what it needs, short — "Needs
 * 2 of 3", "需 3 位中的 2 位" — never a bare "2 of 3" that reads as a count
 * so far; "One member" for a multisig of one.
 */
export function ruleChipText(m: MultisigMessages, rule: { m: number; n: number }): string {
	return rule.n === 1 ? m.panel.chipSolo : fillTemplate(m.panel.chip, rule);
}

/** One member change, in words: "Add Bob (0x12…34)", "Replace A with B". */
export function changeText(
	m: MultisigMessages,
	change: Pick<MemberChange, 'kind' | 'added' | 'removed'>,
	who: (address: string) => string = (a) => memberWho(a)
): string {
	switch (change.kind) {
		case 'add':
			return fillTemplate(m.change.add, { member: who(change.added ?? '') });
		case 'remove':
			return fillTemplate(m.change.remove, { member: who(change.removed ?? '') });
		case 'replace':
			return fillTemplate(m.change.replace, {
				old: who(change.removed ?? ''),
				new: who(change.added ?? '')
			});
		case 'threshold':
			return m.change.threshold;
	}
}

/**
 * A transaction's member changes as a member must read them before
 * approving (r4a-1): each change ("#2 · Add Bob (0x12…34)" in a batch), the
 * rule it meets and the one it leaves ("Approvals: 2 of 3 → 2 of 4"), the
 * network, and the caution — the core's reading against the network's
 * members now. `doesNotFit`: the Safe would refuse it there.
 */
export function changeLines(
	m: MultisigMessages,
	reading: MemberChanges,
	network: string,
	who: (address: string) => string = (a) => memberWho(a)
): { changes: string[]; rule: string; caution: string; doesNotFit: string | null } {
	return {
		changes: reading.changes.map((c) => {
			const text = changeText(m, c, who);
			return c.call === null ? text : `#${c.call} · ${text}`;
		}),
		rule: fillTemplate(m.change.rule, {
			before: ruleText(m, reading.before),
			after: ruleText(m, reading.after)
		}),
		caution: fillTemplate(m.change.caution, { network }),
		doesNotFit: reading.fits ? null : fillTemplate(m.change.doesNotFit, { network })
	};
}

/** Who a transaction pays, when it pays one address. */
export function actionRecipient(action: Action | null): string | null {
	if (action?.kind === 'transfer' || action?.kind === 'call') return action.to;
	return null;
}

export function chainName(chainId: number): string {
	return chainMeta(chainId)?.displayName ?? `Chain ${chainId}`;
}

export function explorerTxUrl(chainId: number, txHash: string): string | null {
	const base = chainMeta(chainId)?.explorerURL;
	return base ? `${base.replace(/\/+$/, '')}/tx/${txHash}` : null;
}
