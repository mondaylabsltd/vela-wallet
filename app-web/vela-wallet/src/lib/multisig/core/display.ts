/**
 * How a multisig and its transactions read on screen: member names, a
 * transaction's one-line summary, amounts in their token's units.
 *
 * Names are only ever this device's own (a Vela account's local name) or what
 * the member registered for itself in the passkey index — never text that came
 * with a link or a proposal (docs/multisig-vault-design.md §5.3).
 */
import { fillTemplate } from '$lib/i18n/fill';
import { loadAccounts } from '$lib/onboarding/core/storage';
import { chainMeta } from '$lib/services/chains';
import { poolRpcCall } from '$lib/services/rpc-pool';
import { shortenAddress } from '$lib/wallet/identity';
import type { MultisigMessages } from '../messages';
import { sameAddress, validName, type Action } from './kernel';
import { formatUnits } from './units';

export { formatUnits, parseUnits } from './units';

export { shortenAddress };

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
	decimals: number;
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
		hit = (async () => {
			const [symbol, decimals] = await Promise.all([
				poolRpcCall('eth_call', [{ to: token, data: '0x95d89b41' }, 'latest'], chainId),
				poolRpcCall('eth_call', [{ to: token, data: '0x313ce567' }, 'latest'], chainId)
			]);
			return {
				symbol:
					typeof symbol.result === 'string'
						? decodeString(symbol.result) || shortenAddress(token)
						: shortenAddress(token),
				decimals: typeof decimals.result === 'string' ? Number(BigInt(decimals.result)) : 18
			};
		})();
		hit.catch(() => tokenCache.delete(key));
		tokenCache.set(key, hit);
	}
	return hit;
}

/** A transaction's one line, e.g. "Send 500 USDC". */
export async function actionTitle(
	m: MultisigMessages,
	chainId: number,
	action: Action | null,
	nonce: bigint
): Promise<string> {
	if (action === null) return m.action.unsupported;
	switch (action.kind) {
		case 'transfer': {
			const info = await tokenInfo(chainId, action.token).catch(() => ({
				symbol: '',
				decimals: 18
			}));
			return fillTemplate(m.action.send, {
				amount: formatUnits(action.amount, info.decimals),
				symbol: info.symbol
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
		case 'addOwner':
			return m.action.addOwner;
		case 'removeOwner':
			return m.action.removeOwner;
		case 'swapOwner':
			return m.action.swapOwner;
		case 'changeThreshold':
			return fillTemplate(m.action.changeThreshold, { threshold: action.threshold });
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
				change = `${m.action.addOwner} ${shortenAddress(a.owner)}, ${fillTemplate(m.action.changeThreshold, { threshold: a.threshold })}`;
				break;
			case 'removeOwner':
				change = `${m.action.removeOwner} ${shortenAddress(a.owner)}, ${fillTemplate(m.action.changeThreshold, { threshold: a.threshold })}`;
				break;
			case 'swapOwner':
				change = `${m.action.swapOwner} ${shortenAddress(a.oldOwner)} → ${shortenAddress(a.newOwner)}`;
				break;
			case 'changeThreshold':
				change = fillTemplate(m.action.changeThreshold, { threshold: a.threshold });
				break;
			default:
				return [];
		}
		return [fillTemplate(m.proposal.changesMembers, { index: i + 1, change })];
	});
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
