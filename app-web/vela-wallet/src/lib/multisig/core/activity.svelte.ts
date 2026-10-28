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
import { actionTitle, formatUnits, tokenInfo } from './display';
import { proposals, type ProposalItem } from './proposals.svelte';

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
}

class WaitingRows {
	/** Titles and amounts are read from the chain (a token's symbol): kept once read. */
	#drawn = $state<Record<string, Drawn>>({});

	#draw(m: MultisigMessages, item: ProposalItem): void {
		const key = proposalRowId(item);
		if (this.#drawn[key]) return;
		this.#drawn = { ...this.#drawn, [key]: { title: '…', amount: '', unit: '' } };
		const action = item.review.action;
		void Promise.all([
			actionTitle(m, item.chainId, action, item.nonce),
			action?.kind === 'transfer'
				? tokenInfo(item.chainId, action.token).catch(() => null)
				: Promise.resolve(null)
		]).then(([title, token]) => {
			const amount =
				action?.kind === 'transfer' && token ? formatUnits(action.amount, token.decimals) : '';
			this.#drawn = {
				...this.#drawn,
				[key]: { title, amount, unit: token?.symbol ?? '' }
			};
		});
	}

	/** Read the titles of what is waiting. Call from an effect. */
	follow(m: MultisigMessages): void {
		for (const item of proposals.open) this.#draw(m, item);
	}

	/**
	 * The group to put first in Activity, or `null` when nothing is waiting.
	 * A mailbox that is down or slow is said here (`panel.stale`), over the
	 * last transactions it gave — never "No activity yet" over ones it has
	 * not answered for. Reads only.
	 */
	group(m: MultisigMessages): ActivityGroupModel | null {
		const open = proposals.open;
		if (open.length === 0 && !proposals.stale) return null;
		const rows: ActivityRowModel[] = open.map((item) => {
			const drawn = this.#drawn[proposalRowId(item)];
			return {
				id: proposalRowId(item),
				kind: 'sent',
				title: drawn?.title ?? '…',
				subtitle: `${statusOf(m, item)} · ${fillTemplate(m.status.progress, {
					approved: item.approved,
					threshold: item.threshold
				})} · ${chainName(item.chainId)}`,
				amount: drawn?.amount
					? drawn.amount.startsWith('<')
						? drawn.amount
						: `-${drawn.amount}`
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

function statusOf(m: MultisigMessages, item: ProposalItem): string {
	if (item.status === 'queued') return m.status.queued;
	if (item.status === 'ready') return m.status.ready;
	if (item.canApprove.length > 0) return m.status.yourTurn;
	return m.status.waiting;
}

export const waitingRows = new WaitingRows();
