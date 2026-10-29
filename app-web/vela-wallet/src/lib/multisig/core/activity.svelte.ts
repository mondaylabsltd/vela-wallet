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
import {
	actionTitle,
	changeLines,
	formatUnits,
	localName,
	memberWho,
	shortenAddress,
	tokenInfo
} from './display';
import { checksumAddress } from '$lib/core/kernels';
import { readMemberChanges } from './kernel';
import { names } from './names.svelte';
import { provenanceNote, type ProvenanceNote } from '$lib/services/token-provenance';
import { executionInput, proposals, type ProposalItem } from './proposals.svelte';
import { blockOf, preflight } from './preflight.svelte';
import { multisig } from './store.svelte';
import { whyText } from './execution-words';
import { readCall } from './call-review.svelte';
import { readChainDoc } from '$lib/services/chain-doc';
import type { SigningMessages } from '$lib/signing/messages';
import { ROW_PREFIX, transactionOfRow, waitingRowId } from './row-id';
import { progressLine } from './progress';

/** The id an Activity row carries for a waiting transaction (`row-id.ts`). */
export const PROPOSAL_ROW_PREFIX = ROW_PREFIX;

/** The waiting group's action while the multisig service (or a network) is away: ask it now. */
export const MULTISIG_RETRY = 'multisig-retry';

export function proposalRowId(item: Pick<ProposalItem, 'chainId' | 'safeTxHash'>): string {
	return waitingRowId(item.chainId, item.safeTxHash);
}

export { progressLine };

/** A time of day the way this browser shows one ("4:36 PM"). */
function clockTime(at: number): string {
	try {
		return new Date(at).toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' });
	} catch {
		return new Date(at).toISOString().slice(11, 16);
	}
}

/** The id of a row for proposals on a network this device does not have. */
export const MISSING_ROW_PREFIX = 'multisig-network:';

/** The network a missing-network row names, or `null` for any other row. */
export function missingOfRow(id: string | undefined): number | null {
	if (!id?.startsWith(MISSING_ROW_PREFIX)) return null;
	const chainId = Number(id.slice(MISSING_ROW_PREFIX.length));
	return Number.isSafeInteger(chainId) && chainId > 0 ? chainId : null;
}

/**
 * "On Foo (chain 123) — a network this device doesn't have": the chain
 * index's name when it knows the chain, else the number alone.
 */
export function missingTitle(m: MultisigMessages, chainId: number, name: string | null): string {
	return name
		? fillTemplate(m.proposal.missingNetworkNamed, { name, chainId })
		: fillTemplate(m.proposal.missingNetwork, { chainId });
}

/** What the chain index calls `chainId`, or `null` (it does not know it, or did not answer). */
export async function indexName(chainId: number): Promise<string | null> {
	const doc = await readChainDoc(chainId).catch(() => null);
	const name = doc?.kind === 'doc' ? doc.body.name : null;
	return typeof name === 'string' && name.trim() !== '' ? name.trim() : null;
}

/**
 * The transaction a row id names — a waiting one's row, or one its execution
 * wrote (r4a-4) — or `null` for any other row: nothing but the one scheme
 * parses (`row-id.ts`).
 */
export function proposalOfRow(
	id: string | undefined
): { chainId: number; safeTxHash: string } | null {
	const read = transactionOfRow(id);
	return read ? { chainId: read.chainId, safeTxHash: read.safeTxHash } : null;
}

interface Drawn {
	title: string;
	amount: string;
	unit: string;
	/** Who read a contract call its title names ("Read by Vela, from the transaction"). */
	source?: string | null;
	/** r4a-2: the token a transfer sends, when nobody vouches for it. */
	note?: ProvenanceNote | null;
}

/** A member as a row names it: the name this device knows, and the short address. */
const who = (address: string) => memberWho(address, names.label(address));

class WaitingRows {
	/** The multisig `follow` was last told of: whose executions the rows were looked at for. */
	#multisig = $state<string | null>(null);
	/** Titles and amounts are read from the chain (a token's symbol): kept once read. */
	#drawn = $state<Record<string, Drawn>>({});
	/** The chain index's names of the networks this device lacks, once read. */
	#names = $state<Record<number, string | null>>({});
	#asked = new Set<number>();

	/** The chain index's name of a network this device lacks (`null` until known, or unknown). */
	nameOf(chainId: number): string | null {
		return this.#names[chainId] ?? null;
	}

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
			actionTitle(m, item.chainId, action, item.nonce, who),
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
			const note =
				action?.kind === 'transfer' && token
					? provenanceNote(item.chainId, action.token, token.symbol, m.token)
					: null;
			this.#drawn = {
				...this.#drawn,
				[key]: read
					? { ...read, amount, unit: token?.symbol ?? '', note }
					: { title: own, amount, unit: token?.symbol ?? '', note }
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
		// r4-5: what is ready is looked at as it would run, so the row never
		// says "Ready to execute" for one that cannot (the sheet's same looks).
		this.#multisig = multisigAddress;
		for (const item of proposals.open) {
			for (const input of readyInputs(multisigAddress, item)) {
				void preflight.check(input, HOME_FRESH_MS);
			}
		}
		for (const missing of proposals.missing) {
			if (this.#asked.has(missing.chainId)) continue;
			this.#asked.add(missing.chainId);
			void indexName(missing.chainId).then((name) => {
				this.#names = { ...this.#names, [missing.chainId]: name };
			});
		}
	}

	/**
	 * The group to put first in Activity, or `null` when nothing is waiting.
	 * A mailbox that is down or slow is said here (`panel.stale`), over the
	 * last transactions it gave — never "No activity yet" over ones it has
	 * not answered for. Reads only.
	 */
	group(m: MultisigMessages, toName?: string): ActivityGroupModel | null {
		const open = proposals.open;
		const missing = proposals.missing;
		const said =
			proposals.stale ||
			proposals.offline ||
			proposals.unreadable.length > 0 ||
			proposals.historyUnreadable.length > 0 ||
			proposals.loading;
		if (open.length === 0 && missing.length === 0 && !said) return null;
		const rows: ActivityRowModel[] = open.map((item) => {
			const drawn = this.#drawn[proposalRowId(item)];
			const change = memberChangeOf(m, item);
			const note = drawn?.note ?? null;
			return {
				id: proposalRowId(item),
				kind: 'sent',
				title: drawn?.title ?? '…',
				subtitle: [
					drawn?.source,
					statusOf(m, item, this.#multisig),
					// r4b-6: proposals at one number — only one of them can run.
					item.sameNonce.length > 0 ? m.status.conflict : null,
					progressLine(m, item.approved, item.threshold),
					// r4a-1: the rule it meets and the one it leaves.
					change?.rule,
					// vis-18: whom a transfer goes to, as a Vela account's row
					// says it; the chain logo on the glyph already names the
					// network, so a transfer's line has room for the person. A
					// member change names its network: that is where it applies.
					change
						? chainName(item.chainId)
						: (recipientLine(item, toName) ?? chainName(item.chainId))
				]
					.filter(Boolean)
					.join(' · '),
				// What must be read first: it changes who controls the multisig,
				// or it sends a token nobody vouches for (its contract, and a
				// lookalike's "not the real USDC").
				...(change
					? { caution: m.change.rowCaution }
					: note
						? { caution: `${note.text} · ${note.short}` }
						: {}),
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
		// Proposals on a network this device lacks: said, with the way to add
		// it (the row opens it) — never judged, never counted as anyone's turn.
		for (const gap of missing) {
			rows.push({
				id: `${MISSING_ROW_PREFIX}${gap.chainId}`,
				kind: 'sent',
				title: missingTitle(m, gap.chainId, this.nameOf(gap.chainId)),
				subtitle: [
					fillTemplate(m.proposal.missingCount, { n: gap.count }),
					m.proposal.addNetwork
				].join(' · '),
				amount: '',
				unit: '',
				positive: false,
				masked: false,
				badgeColor: chainColor(gap.chainId)
			});
		}
		const retry = () => ({ id: MULTISIG_RETRY, label: m.service.retry });
		// r4b-15: offline is said as offline — with when the list shown was read.
		if (proposals.offline) {
			const at = proposals.readAt;
			return {
				label: m.panel.pending,
				note: at
					? fillTemplate(m.service.offlineSince, { time: clockTime(at) })
					: m.service.offline,
				rows
			};
		}
		// The service away is said as itself, once for every network, with a
		// Retry: its own schedule retries too, but nobody should have to wait.
		if (proposals.unreachable) {
			return { label: m.panel.pending, note: m.service.unreachableNote, action: retry(), rows };
		}
		// r4b-3: a network that did not answer is named, never "nothing waits".
		const unread = proposals.unreadable;
		if (unread.length > 0) {
			return {
				label: m.panel.pending,
				note: fillTemplate(m.service.chainUnreachable, {
					network: unread.map((id) => chainName(id)).join(', ')
				}),
				action: retry(),
				rows
			};
		}
		// r4b-11: the list kept from an earlier page, shown at once — said as
		// the last known while it is checked (or while the mailbox is slow).
		if (proposals.cached) {
			return {
				label: m.panel.pending,
				note: fillTemplate(m.panel.lastKnown, { time: clockTime(proposals.cached.at) }),
				rows
			};
		}
		if (proposals.stale) return { label: m.panel.pending, note: m.panel.stale, rows };
		// r4b-2: a history that could not be read is said, with Retry.
		const history = proposals.historyUnreadable;
		if (history.length > 0) {
			return {
				label: m.panel.pending,
				note: fillTemplate(m.panel.historyUnreadable, {
					network: history.map((id) => chainName(id)).join(', ')
				}),
				action: retry(),
				rows
			};
		}
		// r4b-11, r4D-9: still reading, nothing to show yet — said from the
		// first frame, never an empty space the list then jumps into.
		if (rows.length === 0 && proposals.loading) {
			return { label: m.panel.pending, note: m.panel.checking, rows };
		}
		return { label: m.panel.pending, rows };
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

/**
 * r4a-1: a waiting transaction's member changes on its Home row — the rule
 * it meets and the one it leaves, against its network's members now (the
 * core's reading) — or `null` when it changes none.
 */
function memberChangeOf(m: MultisigMessages, item: ProposalItem): { rule: string } | null {
	const action = item.review.action;
	const members = proposals.chain(item.chainId)?.members;
	if (!action || !members) return null;
	try {
		const reading = readMemberChanges(action, members);
		return reading ? { rule: changeLines(m, reading, chainName(item.chainId), who).rule } : null;
	} catch {
		return null;
	}
}

/** How long the home lets an execution's look stand: it is re-read on every poll. */
const HOME_FRESH_MS = 60_000;

/** A ready transaction's execution by each of this device's executors. */
function readyInputs(multisigAddress: string | null, item: ProposalItem) {
	if (item.status !== 'ready' || item.canExecute.length === 0) return [];
	const account = multisigAddress ? multisig.find(multisigAddress) : undefined;
	const members = account ? proposals.chain(item.chainId)?.members : undefined;
	if (!account || !members) return [];
	return item.canExecute.map((o) => executionInput(account, item, o, members));
}

/** "Can't run now — …" for a ready one that cannot, else `null`. */
function blockedLine(
	m: MultisigMessages,
	item: ProposalItem,
	multisigAddress: string | null
): string | null {
	const block = blockOf(readyInputs(multisigAddress, item).map((input) => preflight.get(input)));
	if (!block) return null;
	return fillTemplate(m.status.cannotRun, {
		reason: block.kind === 'fails' ? whyText(m, block.why) : m.status.nobodyPays
	});
}

function statusOf(m: MultisigMessages, item: ProposalItem, multisigAddress: string | null): string {
	// Signed here, not in the mailbox yet: the one thing to know about it.
	if (item.unsent) return m.status.unsent;
	// r4b-7: sent from here and may still land — not offered again.
	if (item.executing) return m.status.executing;
	if (item.status === 'queued') return m.status.queued;
	if (item.status === 'ready') return blockedLine(m, item, multisigAddress) ?? m.status.ready;
	if (item.canApprove.length > 0) return m.status.yourTurn;
	return m.status.waiting;
}

export const waitingRows = new WaitingRows();
