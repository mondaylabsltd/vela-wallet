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
	tokenInfo,
	UNREAD_TOKEN
} from './display';
import { checksumAddress } from '$lib/core/kernels';
import { readMemberChanges } from './kernel';
import { names } from './names.svelte';
import { provenanceNote, type ProvenanceNote } from '$lib/services/token-provenance';
import { proposals, type ProposalItem } from './proposals.svelte';
import { whyText } from './execution-words';
import { blockOf as wordedBlock, type TokenWords } from '../shell/proposal-failure';
import { nativeSymbol } from '$lib/services/networks';
import type { MultisigStanding } from '../generated/MultisigStanding';
import type { MultisigWaitingNote } from '../generated/MultisigWaitingNote';
import { asReview, readTx } from './reading';
import { readingTitle } from './reading-blocks';
import { readChainDoc } from '$lib/services/chain-doc';
import { catalogName } from './likely-networks';
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

/**
 * What `chainId` is called: the core's catalog's name (`networks.name`),
 * else the chain index's, or `null` (neither knows it, or the index did not
 * answer).
 */
export async function indexName(chainId: number): Promise<string | null> {
	const known = catalogName(chainId);
	if (known) return known;
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
	/** A `short` look's token, worded once read (symbol, decimals). */
	#tokens = $state<Record<string, TokenWords>>({});
	/** Titles and amounts are read from the chain (a token's symbol): kept once read. */
	#drawn = $state<Record<string, Drawn>>({});
	/** The chain index's names of the networks this device lacks, once read. */
	#names = $state<Record<number, string | null>>({});
	#asked = new Set<number>();

	/** The chain index's name of a network this device lacks (`null` until known, or unknown). */
	nameOf(chainId: number): string | null {
		return this.#names[chainId] ?? null;
	}

	#draw(m: MultisigMessages, item: ProposalItem, multisigAddress: string | null): void {
		const key = proposalRowId(item);
		if (this.#drawn[key]) return;
		this.#drawn = { ...this.#drawn, [key]: { title: '…', amount: '', unit: '' } };
		const action = item.review.action;
		// A contract call says what it is, as its proposal's sheet does — the
		// core's reading of it (r8-1, the same rule every platform reads) —
		// never only "Contract call".
		if (action?.kind === 'call' && multisigAddress !== null) {
			const read = readTx(item.chainId, multisigAddress, item.tx);
			const title = readingTitle(asReview(read), {
				r: m.review.reading,
				callNumber: m.review.callNumber,
				nativeSymbol: nativeSymbol(item.chainId)
			});
			if (title !== null && read?.reading.calls[0]?.intent.kind !== 'call') {
				this.#drawn = {
					...this.#drawn,
					[key]: { title, amount: '', unit: '', source: null }
				};
			}
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
	 * Read the titles of what is waiting. Call from an effect. A contract
	 * call is titled by the core's reading of it (r8-1).
	 */
	follow(m: MultisigMessages, multisigAddress: string | null = null): void {
		for (const item of proposals.open) this.#draw(m, item, multisigAddress);
		// r4-5: what is ready the machine looks at as it would run (each poll),
		// so the row never says "Ready to execute" for one that cannot; a
		// `short` look's token is worded here (its symbol and decimals).
		for (const item of proposals.open) {
			const block = standingOf(item);
			if (block?.kind !== 'blocked' || block.block.kind !== 'fails') continue;
			const why = block.block.why as { kind?: string; token?: string | null } | null;
			const token = why?.kind === 'short' ? (why.token ?? null) : undefined;
			if (!token || this.#tokens[token.toLowerCase()]) continue;
			void tokenInfo(item.chainId, token)
				.catch(() => UNREAD_TOKEN)
				.then((info) => (this.#tokens = { ...this.#tokens, [token.toLowerCase()]: info }));
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
		const said = !!proposals.view?.waiting_note;
		if (open.length === 0 && missing.length === 0 && !said) return null;
		const rows: ActivityRowModel[] = open.map((item) => {
			const drawn = this.#drawn[proposalRowId(item)];
			const change = memberChangeOf(m, item);
			const note = drawn?.note ?? null;
			return {
				id: proposalRowId(item),
				kind: 'sent',
				title: drawn?.title ?? '…',
				// r4D-13: ONE word about where it stands, so the line fits two
				// lines on a 320-wide phone. "Ready to execute" already says every
				// approval is in, "Needs your approval" is the one thing to do,
				// "Queued" waits on another; only a plain wait says its count
				// ("1/2 approved"), which says "waiting for others" by itself.
				subtitle: [
					drawn?.source,
					standing(m, item, this.#tokens),
					// r4b-6: proposals at one number — only one of them can run.
					item.sameNonce.length > 0 ? m.status.conflict : null,
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
		// What the group says of its list is the machine's one note (offline,
		// the service away, a network unanswered, the list kept, stale, the
		// history unread, still reading) — worded here, with a Retry where
		// one helps.
		const note = proposals.view?.waiting_note ?? null;
		if (note) {
			const said = noteText(m, note);
			return noteRetries(note)
				? { label: m.panel.pending, note: said, action: retry(), rows }
				: { label: m.panel.pending, note: said, rows };
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

/** The machine's one word on where `item` stands (`standings`), if it said one. */
function standingOf(item: ProposalItem): MultisigStanding | null {
	const key = `${item.chainId}:${item.safeTxHash}`.toLowerCase();
	const standings = proposals.view?.standings ?? {};
	for (const [k, v] of Object.entries(standings)) if (k.toLowerCase() === key) return v;
	return null;
}

/**
 * An Activity row's one word on where it stands — the machine's (`standing`):
 * a plain wait is said by its count, which already says "waiting for
 * others"; a ready one that cannot run here says why ("Can't run now — the
 * multisig holds 0.28 USDC, and this needs 0.5 USDC").
 */
function standing(
	m: MultisigMessages,
	item: ProposalItem,
	tokens: Record<string, TokenWords>
): string {
	const said = standingOf(item);
	switch (said?.kind) {
		case 'unsent':
			return m.status.unsent;
		case 'executing':
			return m.status.executing;
		case 'queued':
			return m.status.queued;
		case 'ready':
			return m.status.ready;
		case 'yourTurn':
			return m.status.yourTurn;
		case 'progress':
			// r4b-17: never "3/2" — the machine caps it.
			return progressLine(m, said.approved, said.threshold);
		case 'blocked': {
			if (said.block.kind === 'nobodyPays') {
				return fillTemplate(m.status.cannotRun, { reason: m.status.nobodyPays });
			}
			const why = said.block.why as { kind?: string; token?: string | null } | null;
			const token =
				why?.kind === 'short'
					? why.token
						? (tokens[why.token.toLowerCase()] ?? null)
						: { symbol: nativeSymbol(item.chainId), decimals: 18 }
					: null;
			return fillTemplate(m.status.cannotRun, {
				reason: whyText(m, wordedBlock(said.block.why, token))
			});
		}
		default:
			// Not said yet (the first frame): its count.
			return progressLine(m, Math.min(item.approved, item.threshold), item.threshold);
	}
}

/** The waiting group's note, worded. */
function noteText(m: MultisigMessages, note: MultisigWaitingNote): string {
	switch (note.kind) {
		case 'offline':
			return note.since
				? fillTemplate(m.service.offlineSince, { time: clockTime(note.since) })
				: m.service.offline;
		case 'unreachable':
			return m.service.unreachableNote;
		case 'chainUnreachable':
			return fillTemplate(m.service.chainUnreachable, {
				network: note.chain_ids.map((id) => chainName(id)).join(', ')
			});
		case 'lastKnown':
			return fillTemplate(m.panel.lastKnown, { time: clockTime(note.at) });
		case 'stale':
			return m.panel.stale;
		case 'historyUnreadable':
			return fillTemplate(m.panel.historyUnreadable, {
				network: note.chain_ids.map((id) => chainName(id)).join(', ')
			});
		case 'checking':
			return m.panel.checking;
	}
}

/** The note comes with a Retry (`WaitingNote::retry`). */
function noteRetries(note: MultisigWaitingNote): boolean {
	return (
		note.kind === 'unreachable' ||
		note.kind === 'chainUnreachable' ||
		note.kind === 'historyUnreadable'
	);
}

export const waitingRows = new WaitingRows();
