/**
 * A multisig's transactions waiting for approval, as rows of the home's own
 * Activity — the way a Vela account's pending transfer is drawn, with how far
 * its approvals have come where the status goes. A waiting transaction is the
 * one thing a multisig adds to the home, so it is drawn with what is already
 * there rather than as a panel of its own.
 */
import { fillTemplate } from '$lib/i18n/fill';
import { chainLogoURL } from '$lib/flows/marks';
import { chainName } from '$lib/services/networks';
import { chainColor } from '$lib/wallet/fixtures';
import type { ActivityGroupModel, ActivityRowModel } from '$lib/wallet/model';
import type { MultisigMessages } from '../messages';
import { actionTitle, formatUnits, localName, shortenAddress, tokenInfo } from './display';
import { checksumAddress } from '$lib/core/kernels';
import { proposals, type ProposalItem } from './proposals.svelte';
import { readCall } from './call-review.svelte';
import type { SigningMessages } from '$lib/signing/messages';

/** The id an Activity row carries for a waiting transaction. */
export const PROPOSAL_ROW_PREFIX = 'multisig:';

export function proposalRowId(item: Pick<ProposalItem, 'chainId' | 'safeTxHash'>): string {
	return `${PROPOSAL_ROW_PREFIX}${item.chainId}:${item.safeTxHash}`;
}

/** The proposal a row id names, or `null` for any other row. */
export function proposalOfRow(
	id: string | undefined
): { chainId: number; safeTxHash: string } | null {
	if (!id?.startsWith(PROPOSAL_ROW_PREFIX)) return null;
	const [chain, hash] = id.slice(PROPOSAL_ROW_PREFIX.length).split(':');
	const chainId = Number(chain);
	return Number.isInteger(chainId) && hash ? { chainId, safeTxHash: hash } : null;
}

interface Drawn {
	title: string;
	amount: string;
	unit: string;
	/** Who read a contract call its title names ("Read by Vela, from the transaction"). */
	source?: string | null;
}

class WaitingRows {
	/** Titles and amounts are read from the chain (a token's symbol): kept once read. */
	#drawn = $state<Record<string, Drawn>>({});

	#draw(
		m: MultisigMessages,
		item: ProposalItem,
		multisigAddress: string | null,
		signing: SigningMessages | null
	): void {
		const key = proposalRowId(item);
		if (this.#drawn[key]) return;
		this.#drawn = { ...this.#drawn, [key]: { title: '…', amount: '', unit: '' } };
		const action = item.review.action;
		// A contract call says what it is, as its proposal's sheet does — the
		// core's reading of it, and who read it — never only "Contract call".
		if (action?.kind === 'call' && signing !== null && multisigAddress !== null) {
			void readCall(item.chainId, multisigAddress, action, signing)
				.catch(() => null)
				.then((read) => {
					if (read === null) return;
					const drawn = this.#drawn[key];
					this.#drawn = {
						...this.#drawn,
						[key]: {
							...(drawn ?? { amount: '', unit: '' }),
							title: read.title,
							source: read.source
						}
					};
				});
		}
		void Promise.all([
			actionTitle(m, item.chainId, action, item.nonce),
			action?.kind === 'transfer'
				? tokenInfo(item.chainId, action.token).catch(() => null)
				: // A call's own coin, so a read title keeps what the call sends.
					action?.kind === 'call' && BigInt(action.value) > 0n
					? tokenInfo(item.chainId, null).catch(() => null)
					: Promise.resolve(null)
		]).then(([title, token]) => {
			const amount = !token
				? ''
				: action?.kind === 'transfer'
					? formatUnits(action.amount, token.decimals)
					: action?.kind === 'call'
						? formatUnits(action.value, token.decimals)
						: '';
			// A call's reading, landed first, keeps its words; the coin a call
			// sends stands in the amount, not twice in its title.
			const read = this.#drawn[key]?.source !== undefined ? this.#drawn[key] : null;
			const own = action?.kind === 'call' && amount !== '' ? m.action.call : title;
			this.#drawn = {
				...this.#drawn,
				[key]: read
					? { ...read, amount, unit: token?.symbol ?? '' }
					: { title: own, amount, unit: token?.symbol ?? '' }
			};
		});
	}

	/**
	 * Read the titles of what is waiting. Call from an effect. With the
	 * signing sheet's words, a contract call is read as its sheet reads it.
	 */
	follow(
		m: MultisigMessages,
		signing: SigningMessages | null = null,
		multisigAddress: string | null = null
	): void {
		for (const item of proposals.open) this.#draw(m, item, multisigAddress, signing);
	}

	/**
	 * The group to put first in Activity, or `null` when nothing is waiting.
	 * A mailbox that is down or slow is said here (`panel.stale`), over the
	 * last transactions it gave — never "No activity yet" over ones it has
	 * not answered for. Reads only.
	 */
	group(m: MultisigMessages, toName?: string): ActivityGroupModel | null {
		const open = proposals.open;
		if (open.length === 0 && !proposals.stale) return null;
		const rows: ActivityRowModel[] = open.map((item) => {
			const drawn = this.#drawn[proposalRowId(item)];
			return {
				id: proposalRowId(item),
				kind: 'sent',
				title: drawn?.title ?? '…',
				subtitle: [
					drawn?.source,
					statusOf(m, item),
					fillTemplate(m.status.progress, {
						approved: item.approved,
						threshold: item.threshold
					}),
					// vis-18: whom a transfer goes to, as a Vela account's row
					// says it; the chain logo on the glyph already names the
					// network, so a transfer's line has room for the person.
					recipientLine(item, toName) ?? chainName(item.chainId)
				]
					.filter(Boolean)
					.join(' · '),
				// The typographic minus every other row's amount carries (vis-18).
				amount: drawn?.amount
					? drawn.amount.startsWith('<')
						? drawn.amount
						: `\u2212${drawn.amount}`
					: '',
				unit: drawn?.unit ?? '',
				positive: false,
				masked: false,
				badgeColor: chainColor(item.chainId),
				badgeLogoUrl: chainLogoURL(item.chainId)
			};
		});
		return proposals.stale
			? { label: m.panel.pending, note: m.panel.stale, rows }
			: { label: m.panel.pending, rows };
	}
}

/**
 * "To Alice" / "To 0x1234AB…34abcd" for a transfer to one address — this
 * device's name for it when it is one of its accounts, else its checksummed
 * short form; `null` for anything else (a batch, a member change).
 */
function recipientLine(item: ProposalItem, toName: string | undefined): string | null {
	const action = item.review.action;
	if (toName === undefined || action?.kind !== 'transfer') return null;
	let address = action.to;
	try {
		address = checksumAddress(action.to);
	} catch {
		// Not an address the core can read: shown as it came.
	}
	return fillTemplate(toName, { name: localName(address) ?? shortenAddress(address) });
}

function statusOf(m: MultisigMessages, item: ProposalItem): string {
	if (item.status === 'queued') return m.status.queued;
	if (item.status === 'ready') return m.status.ready;
	if (item.canApprove.length > 0) return m.status.yourTurn;
	return m.status.waiting;
}

export const waitingRows = new WaitingRows();
