<script lang="ts">
	import { listOf } from '$lib/i18n/list';
	/**
	 * One multisig transaction — what every member sees, and where each of
	 * them approves.
	 *
	 * What it does is the core's reading of the transaction itself
	 * (`reviewTx`), never text that came with it. A call to another contract
	 * (a dApp's) is read further, the way the signing sheet reads the same
	 * call sent by a site — the same machines, the same blocks (cs-12) — so
	 * every member approves what they can read. Each approval shown was
	 * verified here, and counts only if its owner is an owner on that chain
	 * now. There is one primary action, the next thing that moves it forward:
	 * approve, approve and execute, or execute — each one passkey prompt.
	 *
	 * A dApp's transaction (`dapp`, held by the machine) is drawn here before
	 * it exists — the machine's draft: the same reading, at the number it
	 * would take, with "Propose and approve" and Decline — nothing is signed
	 * until the person has seen what the site asks. Once proposed it is the
	 * transaction like any other; the machine answers its site.
	 *
	 * Everything it reads and does on the shown multisig is the core's
	 * proposals machine's (`proposals.view`, events through `proposalsHost`):
	 * the one review, each executor's look, how each member still to approve
	 * signs, its story, where approve / execute / reject / send again stand —
	 * this draws them, and words a failure as the corpus always has
	 * (`shell/proposal-failure.ts`).
	 *
	 * A link's transaction on a device with none of its members (`preview`,
	 * `watch`, ms-21) is the machine's too (`linked`), the multisig neither
	 * added nor shown: following it here, or signing with a member's wallet,
	 * are offered — each only on the person's tap.
	 */
	import { onMount, untrack } from 'svelte';
	import Button from '$lib/ui/Button.svelte';
	import { hostLabel } from '$lib/dapp/host';
	import { fillTemplate } from '$lib/i18n/fill';
	import { refusedLine, rejectHint } from '../core/approvals-words';
	import { nativeSymbol } from '$lib/services/networks';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import Identicon from '$lib/wallet/ui/Identicon.svelte';
	import { identiconSvgForClient } from '$lib/wallet/identicon';
	import type { SigningMessages } from '$lib/signing/messages';
	import type { MultisigMessages } from '../messages';
	import { readingBlocks, readingTitle, type ReadingContext } from '../core/reading-blocks';
	import { REVIEW_FRESH_MS, REVIEW_RETRY_MS, simEndedOf } from '../core/proposal-review.svelte';
	import type { SimEnded } from '$lib/signing/verdict';
	import type { MultisigReviewOut } from '../core/kernel';
	import {
		anywayLabel,
		partyAddresses,
		recipientBadges,
		recipientNames,
		reviewBlocks,
		reviewTech
	} from '../core/review-blocks';
	import BlockList from '$lib/signing/ui/BlockList.svelte';
	import { resolvedFormatKeys } from '$lib/services/locale-format';
	import {
		adviceStanding,
		openReview,
		proposalBeside,
		sheetBeside
	} from '$lib/agent/advice.svelte';
	import { adviceCard } from '$lib/agent/advisor';
	import ReviewTech from './ReviewTech.svelte';
	import MemberChangeRule from './MemberChangeRule.svelte';
	import { names } from '../core/names.svelte';
	import {
		actionRecipient,
		actionTitle,
		chainName,
		changeLines,
		explorerTxUrl,
		formatUnits,
		localName,
		memberWho,
		shortenAddress,
		tokenInfo,
		UNREAD_TOKEN
	} from '../core/display';
	import { GenesisRefusedError, type Finding } from '../core/refusal';
	import type { MultisigFrozenFacts as FrozenFacts } from '../generated/MultisigFrozenFacts';
	import { provenanceNote, type ProvenanceNote } from '$lib/services/token-provenance';
	import ProvenanceNoteView from '$lib/flows/ui/ProvenanceNote.svelte';
	import ProposalTimeline from './ProposalTimeline.svelte';
	import {
		ExecutionSpentError,
		ExecutionUncheckedError,
		ExecutionUnpricedError,
		ExecutionWouldFailError,
		ExecutorCannotPayError,
		type ExecutionPreview
	} from '../core/execution';
	import { feeCoin, feeWords, whyText } from '../core/execution-words';
	import {
		declinedRisk,
		gateOf,
		signingGate,
		SigningBlockedError,
		SigningRefusedError,
		SigningUncheckedError
	} from '../core/gate.svelte';
	import { refusalText } from './findings';
	import AuthorityNotice from './AuthorityNotice.svelte';
	import { ChainUnavailableError } from '../core/chain';
	import { progressLine } from '../core/progress';
	import { proposalsHost, ProposeFailedError } from '../shell/proposals-host.svelte';
	import {
		actionError,
		previewOf as previewFrom,
		shortToken,
		type TokenWords
	} from '../shell/proposal-failure';
	import type { MultisigActionState } from '../generated/MultisigActionState';
	import type { MultisigTimelineStep } from '../generated/MultisigTimelineStep';
	import { multisigUi } from '../core/ui.svelte';
	import { PasskeyCheckError } from '../core/owners';
	import { PageApprovalError } from '../core/page-approval.svelte';
	import { PageDeclinedError, PageSignFailedError } from '$lib/signing/sign-challenge';
	import { integrityLineModel } from '$lib/settings/venue';
	import { PasskeyError } from '$lib/onboarding/core/passkey';
	import type { ExecutionVerdict } from '../core/kernel';
	import { memberDrift, readMemberChanges, sameAddress } from '../core/kernel';
	import { connectLink, proposalLink, shareOrigin } from '../core/link';
	import { localOwners, type LocalOwner } from '../core/owners';
	import { itemOf, linkedSource, proposals, type ProposalItem } from '../core/proposals.svelte';
	import { multisig, type MultisigAccount } from '../core/store.svelte';
	import {
		MailboxRefusedError,
		MailboxUnreadableError,
		NoOutsideSignerError
	} from '../core/proposal-errors';
	import { typedName } from '../core/typed-name';
	import type { DappRequestSheet } from '../core/ui.svelte';
	import type { DraftAsk } from '../shell/proposals-host.svelte';
	import type { ProposalSource } from '../core/proposals.svelte';
	import CallReview from './CallReview.svelte';
	import Sheet from './Sheet.svelte';

	interface Props {
		m: MultisigMessages;
		/** The signing sheet's words, for a call's review (cs-12). */
		signing: SigningMessages;
		account: MultisigAccount;
		chainId: number;
		safeTxHash: string;
		locale: string;
		wide: boolean;
		closeLabel: string;
		onclose: () => void;
		/** It was executed from this device: the transaction is on chain. */
		onexecuted?: (done: { item: ProposalItem; txHash: string; userOpHash: string }) => void;
		/** A dApp's request, held by the machine: reviewed here before it is proposed. */
		dapp?: DappRequestSheet | null;
		/**
		 * §3.6: the proposer's own transaction (a transfer, a rename, a member
		 * change), built at its number and reviewed here before its passkey.
		 */
		prebuilt?: DraftAsk | null;
		/** ms-21: a link's transaction on a device with none of its members (the machine's `linked`). */
		preview?: boolean;
		/**
		 * ms-21: no member of it is on this device. `added`: it is kept here
		 * (to follow); `onwatch` keeps it, on the person's tap.
		 */
		watch?: { added: boolean; onwatch: () => void } | null;
	}

	let {
		m,
		signing,
		account,
		chainId,
		safeTxHash,
		locale,
		wide,
		closeLabel,
		onclose,
		onexecuted,
		dapp = null,
		prebuilt = null,
		preview = false,
		watch = null
	}: Props = $props();

	/**
	 * The shown multisig's transactions, or a link's read only (ms-21) — both
	 * the machine's. r4b-4: a link's is read again while it is open, as the
	 * shown multisig's are (the machine's pace while `linked` is up).
	 */
	const store = $derived<ProposalSource>(
		preview ? linkedSource(proposals.view?.linked ?? null) : proposals
	);

	/** A dApp's transaction not proposed yet, or the person's own (§3.6): the review, before any passkey. */
	const drafting = $derived((!!dapp && !dapp.held.safe_tx_hash) || !!prebuilt);

	/**
	 * §3.5: who can act for it on this network — the accounts machine's
	 * verdict (`check_on`: read once a session, also a network it is not used
	 * on yet), so a refusal or a risk is said before anyone taps. Never a
	 * chain read of the sheet's own.
	 */
	onMount(() => multisig.checkOn(account.address, chainId));
	const networkVerdict = $derived(multisig.verdictOn(account.address, chainId));
	const checked = $derived<Finding[] | null>(
		networkVerdict.checked ? networkVerdict.findings : null
	);

	/**
	 * Whether this device signs here: not at all where its genesis is
	 * refused (read-only), only after a confirmation naming the risk where
	 * more than its members can act for it — every signature asks.
	 */
	const gate = $derived(checked ? gateOf(checked, chainId) : signingGate(account, chainId));
	const readOnly = $derived(gate.kind === 'refused');

	/**
	 * The proposals machine's view: the shown multisig's transactions, their
	 * one review, each executor's look, where every action stands — of the
	 * shown multisig's, its draft (§3.6, a dApp's), or a link's (`linked`).
	 */
	const machine = $derived(proposals.view);
	/** The machine's draft, while this sheet is its review (§3.6, a dApp's request). */
	const machineDraft = $derived(drafting ? (machine?.draft ?? null) : null);
	/** Why the machine could not draw the draft (the mailbox did not answer), worded. */
	const draftFailure = $derived.by(() => {
		if (!drafting || machineDraft) return null;
		const state = machine?.actions.propose;
		if (!state || state.kind === 'working' || state.kind === 'done') return null;
		const error = actionError(state, chainId);
		return error ? reasonText(error) : null;
	});
	const machineDraftItem = $derived(machineDraft ? itemOf(machineDraft.item) : null);
	/** Its key in the machine's maps (`<chainId>:<safeTxHash>`). */
	const key = $derived(`${chainId}:${safeTxHash.toLowerCase()}`);

	const item = $derived(
		drafting
			? machineDraftItem
			: (store
					.chain(chainId)
					?.items.find((i) => i.safeTxHash.toLowerCase() === safeTxHash.toLowerCase()) ?? null)
	);
	const members = $derived(
		drafting ? (proposals.chain(chainId)?.members ?? null) : (store.chain(chainId)?.members ?? null)
	);
	// r5reg-14: the shown multisig's transaction, still waiting (or not read
	// yet — its network not answering), is read faster while its sheet is
	// open, and when the window comes back: another member's approval, or the
	// network's return, is seen here within seconds.
	const waitingHere = $derived(!preview && !drafting && item?.status !== 'done');
	$effect(() => {
		if (waitingHere) return proposals.watch();
	});
	const site = $derived(dapp ? hostLabel(dapp.held.origin) : '');

	/**
	 * A member as a change names it: the name typed for them here (r5reg-16),
	 * else the name this device knows, and the short address.
	 */
	const who = (address: string) =>
		memberWho(address, typedName(account, address) ?? names.label(address));

	let title = $state('');
	let lines = $state<{ label: string; to: string; note: ProvenanceNote | null }[]>([]);
	/** r4a-2: the token a transfer sends, when nobody vouches for it. */
	let tokenNote = $state<ProvenanceNote | null>(null);
	$effect(() => {
		const current = item;
		if (!current) return;
		void actionTitle(m, chainId, current.review.action, current.nonce, who).then(
			(t) => (title = t)
		);
		const action = current.review.action;
		if (action?.kind === 'transfer' && action.token) {
			const token = action.token;
			void tokenInfo(chainId, token)
				.catch(() => UNREAD_TOKEN)
				.then((info) => (tokenNote = provenanceNote(chainId, token, info.symbol, m.token)));
		} else tokenNote = null;
		// r8-1/r8-2: every batch is drawn from the core's reading (its tokens'
		// decimals read by the core, never assumed): no lines of its own here.
		lines = [];
	});

	/**
	 * r4a-1: what it changes about who controls the multisig — every member
	 * change, alone or in a batch, read by the core against this network's
	 * members now: who, the rule before and after, and whether it fits.
	 */
	const memberReading = $derived.by(() => {
		const action = item?.review.action ?? null;
		if (!action || !members) return null;
		try {
			return readMemberChanges(action, members);
		} catch {
			return null;
		}
	});
	const change = $derived(
		memberReading ? changeLines(m, memberReading, chainName(chainId), who) : null
	);
	/** The core's verb for a call it could read ("Approve"); else the multisig's line. */

	/*
	 * §3.6: the one review — the same on every member's device, for every
	 * action: what moves (simulated as the Safe, now), who each leg reaches,
	 * the state-diff guard, the hashes and the bytes, the AI's opinion. Read
	 * again while the sheet is open, so what a member signs from is the
	 * chain now; the gate reads it once more right before the passkey.
	 */
	const reviewKey = $derived(
		item && members
			? `${chainId}:${item.safeTxHash}:${members.deployed}:${members.threshold}:${members.owners.join(',')}:${item.status === 'done'}`
			: ''
	);
	/** r8-1: the core reads every call itself (its own "… anyway"); the shell adds nothing. */
	const blind = false;
	/*
	 * The shown multisig's: the machine's one review (`review`), asked when
	 * the sheet opens and whenever what it is of changes, again every
	 * REVIEW_FRESH_MS while it waits, and every REVIEW_RETRY_MS while the last
	 * could not reach the chain. Its draft's comes with the draft.
	 */
	const askKey = $derived(!drafting && reviewKey ? `${reviewKey}:${blind}` : '');
	$effect(() => {
		if (!askKey) return;
		const ask = () =>
			proposalsHost.send({
				type: 'review',
				chain_id: chainId,
				safe_tx_hash: untrack(() => item?.safeTxHash ?? safeTxHash),
				blind: untrack(() => blind)
			});
		ask();
		// Its hashes and fields are read once for a finished one; nothing is re-checked.
		if (untrack(() => item?.status === 'done')) return;
		const fresh = setInterval(ask, REVIEW_FRESH_MS);
		const retry = setInterval(() => {
			if (untrack(() => machineUnread)) ask();
		}, REVIEW_RETRY_MS);
		return () => {
			clearInterval(fresh);
			clearInterval(retry);
		};
	});
	/** The machine's review: absent while gathered, `null` when it could not be. */
	const machineReview = $derived.by((): MultisigReviewOut | null | undefined => {
		if (drafting) {
			const d = machine?.draft;
			return d && d.review !== null && d.review !== undefined
				? (d.review as MultisigReviewOut)
				: undefined;
		}
		const reviews = machine?.reviews ?? {};
		const hash = (item?.safeTxHash ?? safeTxHash).toLowerCase();
		const found = Object.entries(reviews).find(([k]) => k.toLowerCase() === `${chainId}:${hash}`);
		if (!found) return undefined;
		return (found[1] as MultisigReviewOut | null) ?? null;
	});
	/** The last review could not read the chain (its state or its simulation): asked again soon. */
	const machineUnread = $derived(
		machineReview === null ||
			(!!machineReview &&
				(machineReview.state.verdict.kind === 'unavailable' ||
					machineReview.simulation.kind === 'unreachable'))
	);
	/** The simulation as the signing sheet draws it, judged by the token rules for the Safe. */
	let machineSim = $state.raw<SimEnded | null>(null);
	$effect(() => {
		const review = machineReview;
		if (!review) return;
		let live = true;
		void simEndedOf(review, chainId, account.address).then((sim) => {
			if (live) machineSim = sim;
		});
		return () => {
			live = false;
		};
	});

	/** The core's review now, with the call's reading: blind (`sign_anyway`) or not. */
	const fullReview = $derived(machineReview ?? null);
	/** r8-1: the core's reading names a call or a batch ("Approve · USDC"); else the multisig's line. */
	const readingCtx = $derived<ReadingContext>({
		r: m.review.reading,
		callNumber: m.review.callNumber,
		nativeSymbol: nativeSymbol(chainId)
	});
	const shownTitle = $derived(readingTitle(fullReview, readingCtx) || title);
	/** The core read a call or a batch of calls: its reading heads the sheet. */
	const reading = $derived((fullReview?.reading?.calls.length ?? 0) > 0);
	/**
	 * A lone call the reading heads itself ("Send" · its amount · To): its
	 * verb and its recipient are not said twice above it (r5reg-15).
	 */
	const loneCall = $derived(
		fullReview?.reading?.calls.length === 1 &&
			fullReview.reading.calls[0].path.length === 0 &&
			fullReview.reading.calls[0].intent.kind !== 'batch'
	);
	/** Its simulation, as the signing sheet draws it. */
	const reviewSim = $derived(machineSim);
	/** The review could not be gathered at all. */
	const reviewFailed = $derived(machineReview === null);
	/** Read the review again now (the gate found it changed who controls the multisig). */
	function reviewAgain(): void {
		if (drafting || !item) return;
		proposalsHost.send({
			type: 'review',
			chain_id: chainId,
			safe_tx_hash: item.safeTxHash,
			blind
		});
	}
	const reviewContext = $derived({
		m,
		signing,
		network: chainName(chainId),
		nativeSymbol: nativeSymbol(chainId),
		preset: resolvedFormatKeys().number,
		nameOf: (address: string) => names.label(address) ?? localName(address),
		// r5reg-16: a member being added is named as typed when they were added.
		memberName: (address: string) => typedName(account, address)
	});
	/**
	 * r5reg-15: the addresses the call's reading names already (its
	 * "Recipient"): the review says each once — the call's row, wearing what
	 * the review knows of it ("First time sending here").
	 */
	const drawnParties = $derived(partyAddresses(readingBlocks(fullReview, readingCtx)));
	const partyBadges = $derived(recipientBadges(fullReview, reviewContext));
	const partyNames = $derived(recipientNames(fullReview, reviewContext));
	const tech = $derived(fullReview ? reviewTech(fullReview, reviewContext) : null);
	/** The review's rule: nothing signed (no override), "… anyway", or plain. */
	const signRule = $derived(fullReview?.sign ?? null);
	const signBlocked = $derived(signRule?.allowed === false);
	const anyway = $derived(signRule?.anyway === true);
	/** The signing sheet holds its confirm while its simulation is out; so does this. */
	const reviewHolding = $derived(!fullReview && !reviewFailed);
	/**
	 * r11-4: the core refuses it (`review.refused`: a batch nested too deep,
	 * a delegatecall, a refund, a state change) — or it is no kind Vela signs
	 * at all. Every member's Vela runs the same review: nobody signs it, so
	 * nobody is asked to approve it.
	 */
	const nobodySigns = $derived(
		!!item && (item.review.action === null || !!fullReview?.refused || signBlocked)
	);
	/**
	 * r9-9, r11-4: the ONE line that says why — the core's `refused.key` in
	 * the corpus' words, whatever the action, never its English `refusal`.
	 * A transaction no kind Vela signs whose review could not be read says
	 * the general reason; nothing is said while the review is still read.
	 */
	const refusedWords = $derived(
		fullReview?.refused
			? (refusedLine(m, fullReview.refused.key) ?? m.review.refusedWhy.other)
			: item?.review.action === null && !reviewHolding
				? m.review.refusedWhy.other
				: null
	);
	const valueVariant = $derived<'value' | 'danger'>(anyway ? 'danger' : 'value');

	// The AI's opinion on this proposal (spec 103): published for
	// `vela_review` while the sheet is up, bound to its safeTxHash and chain.
	const adviceId = $derived(
		fullReview ? ((fullReview.adviceRequest.id as string | undefined) ?? null) : null
	);
	$effect(() => {
		if (!fullReview) return;
		proposalBeside.publish({
			id: adviceId,
			signing: fullReview.adviceRequest,
			sheet: {
				clear: null,
				guard: null,
				fee: null,
				simulation: reviewSim,
				simulation_pending: !reviewSim
			}
		});
	});
	$effect(() => () => proposalBeside.withdraw());
	const adviceModel = $derived.by(() => {
		const held = sheetBeside.advice;
		if (!held || adviceId === null || held.requestId !== adviceId) return null;
		return adviceCard(held.advice, adviceStanding(held.advice, openReview()), signing.agentAdvice);
	});

	/** Whose hand does the work: the person picks among this device's members. */
	let chosen = $state<string | null>(null);
	const executors = $derived(item?.canExecute ?? []);
	const approvers = $derived(item?.canApprove ?? []);
	const candidates = $derived(executors.length > 0 ? executors : approvers);

	/*
	 * r4-5, r4-7: each executor's look at the execution — would it run, the
	 * fee in its coin and what that pays for, whether it holds it — taken
	 * before any tap, and again while the sheet is open, so the verdict and
	 * the fee are on screen before the passkey, never after a spinner.
	 */
	/** The executors the machine looks at for it (the shown multisig's, not a draft). */
	const looked = $derived(
		item && !drafting && item.review.action !== null ? executors.map((o) => o.address) : []
	);
	/** One executor's look, in the machine's `previews` (`<chainId>:<hash>:<executor>`). */
	const lookKey = (executor: string) =>
		`${chainId}:${(item?.safeTxHash ?? safeTxHash).toLowerCase()}:${executor.toLowerCase()}`;
	/** What the machine's look at `executor` says now (`undefined`: none yet). */
	const lookOf = (executor: string): unknown =>
		Object.entries(machine?.previews ?? {}).find(
			([k]) => k.toLowerCase() === lookKey(executor)
		)?.[1];
	/** How often an open sheet asks the machine to look (it decides whether a look is stale). */
	const ASK_LOOK_EVERY_MS = 5_000;
	function look(executor: string): void {
		// Asked from an effect: what it reads here is not what it depends on.
		untrack(() => {
			if (!item) return;
			proposalsHost.send({
				type: 'preview',
				chain_id: chainId,
				safe_tx_hash: item.safeTxHash,
				executor
			});
		});
	}
	const lookKeys = $derived(looked.join(','));
	$effect(() => {
		if (!lookKeys) return;
		const executors = untrack(() => looked);
		for (const e of executors) look(e);
		// r5reg-14: asked every few seconds while the sheet is open — the
		// machine decides whether a look is stale (a priced one stands its
		// 15 s, an unpriced one 5 s), never runs two at once, and says a look
		// past its cap as unchecked.
		const timer = setInterval(() => {
			for (const e of executors) look(e);
		}, ASK_LOOK_EVERY_MS);
		return () => clearInterval(timer);
	});
	/** The token a `short` look is about, worded (`tokenInfo`; the chain's coin by its symbol). */
	let tokenWords = $state<Record<string, TokenWords>>({});
	function wordsFor(value: unknown): TokenWords | null {
		const token = shortToken(value);
		if (token === undefined) return null;
		if (token === null) return { symbol: nativeSymbol(chainId), decimals: 18 };
		const known = tokenWords[token.toLowerCase()];
		if (!known) {
			void tokenInfo(chainId, token)
				.catch(() => UNREAD_TOKEN)
				.then((info) => (tokenWords = { ...tokenWords, [token.toLowerCase()]: info }));
		}
		return known ?? null;
	}
	function previewOf(owner: LocalOwner | null): ExecutionPreview | null {
		if (!owner || !looked.some((e) => sameAddress(e, owner.address))) return null;
		const value = lookOf(owner.address);
		return value === undefined ? null : previewFrom(value, chainId, wordsFor(value));
	}
	/** Someone here who can pay the fee — the one picked unless the person picks. */
	const payer = $derived(
		executors.find((o) => {
			const p = previewOf(o);
			return p?.kind === 'priced' && p.canPay;
		}) ?? null
	);
	/**
	 * r5reg-5: the person's pick; else one who can pay; else the member used
	 * here last; else the first — among those who have not approved yet (or
	 * can execute). With two or more, "As" shows who it is, drafting too.
	 */
	const usedLast = $derived(machineDraft?.defaultMember ?? machine?.last_actor ?? null);
	const actor = $derived<LocalOwner | null>(
		candidates.find((o) => sameAddress(o.address, chosen)) ??
			payer ??
			candidates.find((o) => sameAddress(o.address, usedLast)) ??
			candidates[0] ??
			null
	);
	const actorPreview = $derived(executors.length > 0 ? previewOf(actor) : null);
	/** The executor shown cannot run it: it would fail, or it cannot pay the fee. */
	const blocked = $derived(
		actorPreview?.kind === 'refused' ||
			actorPreview?.kind === 'fails' ||
			actorPreview?.kind === 'spent' ||
			(actorPreview?.kind === 'priced' && !actorPreview.canPay)
	);
	function recheck() {
		for (const e of looked) look(e);
	}
	const actorApproved = $derived(
		!!item && !!actor && item.approvals.some((a) => a.counts && sameAddress(a.owner, actor.address))
	);
	/** The member shown has not approved it and it is short of approvals: a press approves first. */
	const approveFirst = $derived(!!item && !actorApproved && item.approved < item.threshold);
	/**
	 * core-ms-2: the proposals machine offers this member "Approve and
	 * execute" in one press (`approve_execute`): theirs reaches the threshold
	 * and their look says they can pay.
	 */
	const oneTapOffered = $derived.by(() => {
		if (!item || !actor) return false;
		const offered = machine?.approve_execute[`${chainId}:${item.safeTxHash.toLowerCase()}`] ?? [];
		return offered.some((e) => sameAddress(e, actor.address));
	});

	/**
	 * Who can withdraw it: any member on this device, whether or not it has
	 * approved — the proposer above all. Rejecting proposes a cancellation at
	 * its number, which that member approves at once.
	 */
	const rejecters = $derived(localOwners(members?.owners ?? []));
	/** r4D-13: the person's own proposal (proposed from a member here) is withdrawn, not rejected. */
	const ownProposal = $derived(!!item?.proposer && localOwners([item.proposer]).length > 0);
	const rejectWord = $derived(ownProposal ? m.proposal.withdraw : m.proposal.reject);
	const rejecter = $derived<LocalOwner | null>(
		rejecters.find((o) => sameAddress(o.address, actor?.address)) ?? rejecters[0] ?? null
	);
	/** "Approve and execute" also asks for the fee: a member who cannot pay it still approves. */
	const canApproveOnly = $derived(
		!!actor && approvers.some((o) => sameAddress(o.address, actor.address))
	);

	type Phase =
		| { kind: 'idle' }
		| { kind: 'busy'; what: 'approve' | 'execute' | 'reject' | 'propose' }
		| { kind: 'sent'; txHash: string | null; by: string }
		/** r4b-1: how this device's execution ended when it was not the one that ran. */
		| { kind: 'ended'; verdict: ExecutionVerdict; by: string }
		| { kind: 'failed'; reason: string }
		/** r4-11: "Approve and execute" approved; only the execute part could not run. */
		| { kind: 'partial'; reason: string };
	let phase = $state<Phase>({ kind: 'idle' });
	/**
	 * r5reg-20: executed (here, or anywhere: its number is used) — a past
	 * fact. Nothing on it is a check of what running it WOULD do any more:
	 * no simulation, guard or first-send line, and no look read again.
	 */
	const past = $derived(item?.status === 'done' || (phase.kind === 'sent' && !!phase.txHash));
	const checks = $derived(
		past ? [] : reviewBlocks(fullReview, reviewSim, reviewContext, drawnParties)
	);
	let rejecting = $state(false);
	let copied = $state(false);

	// The link another member opens: the wallet's public address, never this
	// extension's own (`chrome-extension://…` opens nowhere else).
	const link = $derived(
		typeof location === 'undefined'
			? ''
			: proposalLink(shareOrigin(location.origin), locale, {
					chainId,
					multisig: account.address,
					safeTxHash
				})
	);

	/** A dismissal the browser reports as its own (a closed sheet, nothing said by the wallet). */
	function cancelled(error: unknown): boolean {
		return (
			error instanceof DOMException &&
			(error.name === 'NotAllowedError' || error.name === 'AbortError')
		);
	}

	/**
	 * r4b-12: why it did not go, always in the corpus' words — never a raw
	 * message — and true to what happens: a passkey closed (or not on this
	 * device: the browser tells the two apart for nobody), one that timed out,
	 * one whose signature did not check out; an approval kept and resent.
	 */
	/**
	 * Where a failed action leaves the sheet: back where it was when the
	 * person declined to sign on a high-risk network (§3.5) — nothing was
	 * asked of a passkey — else its reason.
	 */
	function failedPhase(error: unknown): Phase {
		return declinedRisk(error) ? { kind: 'idle' } : { kind: 'failed', reason: reasonOf(error) };
	}

	function reasonOf(error: unknown): string {
		console.warn('[multisig] not done:', error);
		return reasonText(error);
	}

	/** {@link reasonOf}'s words alone — drawn from a preview too, so it logs nothing. */
	function reasonText(error: unknown): string {
		// r4b-15 / r4D-10: offline, whatever could not be reached is said as
		// offline — one cause, one sentence — not as whichever read failed
		// first (Approve's authority check said "couldn't check"). A definite
		// answer, or the person's own passkey choice, is still said as itself.
		if (offline() && !definite(error)) return m.proposal.reason.offline;
		if (error instanceof ExecutorCannotPayError && actor) {
			const symbol = nativeSymbol(chainId);
			return fillTemplate(m.proposal.cannotPay, {
				name: names.label(actor.address),
				symbol,
				fee: feeCoin(error.need, symbol),
				balance: feeCoin(error.have, symbol)
			});
		}
		if (error instanceof ExecutionWouldFailError) {
			return fillTemplate(m.proposal.wouldFail, { reason: whyText(m, error.why) });
		}
		if (error instanceof ExecutionUnpricedError) return m.proposal.unpricedReason;
		if (error instanceof ExecutionUncheckedError) return m.proposal.uncheckedReason;
		if (error instanceof MailboxUnreadableError) {
			return offline() ? m.proposal.reason.offline : m.proposal.mailboxDown;
		}
		if (error instanceof NoOutsideSignerError) return m.proposal.noOutsideSigner;
		if (error instanceof SigningRefusedError) {
			const why = error.findings.map((f) =>
				f.from === 'genesis' ? refusalText(m, f.reason, f.chainId) : ''
			);
			return [
				fillTemplate(m.authority.refusedOn, { network: chainName(error.chainId) }),
				...new Set(why.filter(Boolean))
			].join(' ');
		}
		if (error instanceof SigningUncheckedError) {
			return fillTemplate(m.authority.signUnchecked, { network: chainName(error.chainId) });
		}
		// §3.6: read again right before the passkey, it changes who controls the multisig.
		if (error instanceof SigningBlockedError) {
			reviewAgain();
			return m.review.blocked;
		}
		// §3.5: the executor deploys only a genesis the core finds Verified there.
		if (error instanceof GenesisRefusedError) {
			const network = chainName(error.chainId);
			if (error.authority.kind === 'unknown') {
				return fillTemplate(m.authority.deployUnverified, { network });
			}
			const why = error.authority.reasons.map((r) => refusalText(m, r, error.chainId));
			return [fillTemplate(m.authority.deployRefused, { network }), ...new Set(why)].join(' ');
		}
		// 2026-10-29: the member approves on its trusted signing page.
		if (error instanceof PageApprovalError) {
			if (error.failure === 'not_opened' && error.line && m.handoff) {
				return (
					integrityLineModel(error.line, '', m.handoff)?.text ?? m.proposal.reason.passkeyFailed
				);
			}
			// Round 7 (r7-2, r7-7): what happened on the page, said as that.
			if (m.handoff && error.failure === 'unverified') return m.handoff.mismatch;
			if (m.handoff && error.failure === 'silent') return m.handoff.unanswered;
			if (m.handoff && error.failure === 'declined' && error.code === 'refused') {
				return m.handoff.refused;
			}
			return error.failure === 'unverified'
				? m.proposal.reason.passkeyFailed
				: m.proposal.reason.passkeyClosed;
		}
		// An executor's own operation, declined on its trusted signing page.
		if (error instanceof PageDeclinedError) return m.proposal.reason.passkeyClosed;
		// …or not signed there for another reason (round 7): said as what it was.
		if (error instanceof PageSignFailedError && m.handoff) {
			if (error.page === 'unanswered') return m.handoff.unanswered;
			if (error.page === 'mismatch') return m.handoff.mismatch;
			if (error.page === 'refused') return m.handoff.refused;
		}
		if (error instanceof PasskeyError) {
			if (error.kind === 'cancelled') {
				return error.timedOut ? m.proposal.reason.passkeyTimedOut : m.proposal.reason.passkeyClosed;
			}
			if (error.kind === 'not_discoverable') return m.proposal.reason.passkeyMissing;
			return m.proposal.reason.passkeyFailed;
		}
		if (cancelled(error)) return m.proposal.reason.passkeyClosed;
		if (error instanceof PasskeyCheckError) return m.proposal.reason.passkeyFailed;
		if (error instanceof MailboxRefusedError) return m.proposal.reason.serviceRefused;
		// The relay provably never took it (refused, or out of reach): nothing was sent.
		if (error instanceof Error && error.name === 'UserOpNotSentError') return m.proposal.notSent;
		if (error instanceof ChainUnavailableError || offline()) {
			return offline()
				? m.proposal.reason.offline
				: fillTemplate(m.proposal.reason.chainUnreachable, { network: chainName(chainId) });
		}
		return m.proposal.reason.unknown;
	}

	function offline(): boolean {
		return typeof navigator !== 'undefined' && navigator.onLine === false;
	}

	/** An outcome that is an answer, or the person's choice — not something out of reach. */
	function definite(error: unknown): boolean {
		return (
			error instanceof ExecutorCannotPayError ||
			error instanceof ExecutionWouldFailError ||
			error instanceof NoOutsideSignerError ||
			error instanceof SigningRefusedError ||
			error instanceof SigningBlockedError ||
			(error instanceof GenesisRefusedError && error.authority.kind !== 'unknown') ||
			error instanceof PageApprovalError ||
			error instanceof PasskeyError ||
			error instanceof PasskeyCheckError ||
			error instanceof MailboxRefusedError ||
			cancelled(error)
		);
	}

	/**
	 * A dApp's transaction that the proposer's own approval makes ready (a
	 * one-approval multisig, nothing ahead of it): proposed and executed in
	 * one go, so the site gets its transaction — not a wait for approvals
	 * nobody else has to give. The proposals machine's word
	 * (`draft.executeNow`), and its one press (`propose_execute`).
	 */
	const executeNow = $derived(drafting && !!dapp && (machineDraft?.executeNow ?? false));

	/** Where one of this sheet's actions stands in the machine (`approve:<hash>`, `execute:<hash>`). */
	const actionOf = (what: 'approve' | 'execute'): MultisigActionState | null => {
		const hash = (item?.safeTxHash ?? safeTxHash).toLowerCase();
		return machine?.actions[`${what}:${hash}`] ?? null;
	};

	/**
	 * Send `event` for action `key` and wait until it settles: `null` when it
	 * went, else the error the sheet words (`actionError`).
	 */
	async function act(
		key: string,
		event: Parameters<typeof proposalsHost.send>[0]
	): Promise<{ error: unknown; state: MultisigActionState | null }> {
		proposalsHost.send(event);
		const state = await proposalsHost.settled(key);
		return { error: state ? actionError(state, chainId) : null, state };
	}

	/** The person confirmed the transaction: propose it, approved by this device's member. */
	async function onPropose() {
		// §3.6: the proposer's own transaction goes on to its passkey (the host closes this).
		// r5reg-5: the member shown under "As" proposes it — never another, silently.
		if (prebuilt) {
			phase = { kind: 'busy', what: 'propose' };
			proposalsHost.confirmDraft(actor?.address ?? null);
			return;
		}
		if (!dapp || !item) return;
		const thenExecute = executeNow;
		phase = { kind: 'busy', what: 'propose' };
		// RB5: a page that is gone gets nothing signed (its request is withdrawn).
		if (!(await dapp.claim())) {
			phase = { kind: 'idle' };
			return;
		}
		const by = actor?.address ?? null;
		if (thenExecute) proposalsHost.proposeExecute(by);
		else proposalsHost.confirmDraft(by);
		// Proposed (by a member here, or an outside member's wallet), or why not.
		const state = await proposalsHost.until((view) => {
			const now = view.actions.propose;
			return now && now.kind !== 'working' && now.kind !== 'outsideAsked' ? now : null;
		});
		if (state?.kind !== 'done') {
			phase = failedPhase(actionError(state ?? { kind: 'cancelled' }, chainId));
			return;
		}
		// Ready on its own approval: the machine executes it now, in the same
		// press (`execute:<hash>`, working from the moment it was proposed).
		const key = `execute:${state.safe_tx_hash.toLowerCase()}`;
		if (!thenExecute || !machine?.actions[key]) {
			phase = { kind: 'idle' };
			return;
		}
		phase = { kind: 'busy', what: 'execute' };
		executedBy = by;
		ended = null;
		const ran = await proposalsHost.until((view) => {
			const now = view.actions[key];
			return now === undefined ? ('notNow' as const) : now.kind !== 'working' ? now : null;
		});
		if (ran === 'notNow' || ran === null) {
			phase = { kind: 'idle' };
			return;
		}
		const error = actionError(ran, chainId);
		if (error && !(error instanceof ExecutionSpentError)) phase = failedPhase(error);
		else if (error) phase = { kind: 'idle' };
	}

	/** `owner` approves it (one passkey prompt); `null` when it went, else why not. */
	async function approveAs(owner: LocalOwner): Promise<unknown> {
		if (!item) return null;
		const hash = item.safeTxHash;
		const { error } = await act(`approve:${hash.toLowerCase()}`, {
			type: 'approve',
			chain_id: chainId,
			safe_tx_hash: hash,
			owner: owner.address
		});
		return error;
	}

	async function onApprove() {
		if (!item || !actor) return;
		phase = { kind: 'busy', what: 'approve' };
		const error = await approveAs(actor);
		// The machine reads the list again after it (never a read begun before it).
		phase = error ? failedPhase(error) : { kind: 'idle' };
	}

	/** Who executed it from here (the sheet's tap, or a flight resumed after a reload). */
	let executedBy = $state<string | null>(null);
	/** The execution this sheet followed to its end: its verdict's side effects run once. */
	let ended = $state<string | null>(null);

	async function onExecute() {
		if (!item || !actor || !members) return;
		// "Approve and execute" is the machine's one press (`approve_execute`):
		// should only the execute part be unable to run (before its passkey —
		// a fee it cannot pay, a transaction that would fail), the machine
		// still gives and keeps the approval (r4-11).
		const oneTap = oneTapOffered;
		phase = { kind: 'busy', what: 'execute' };
		const by = actor.address;
		executedBy = by;
		ended = null;
		const hash = item.safeTxHash;
		const lower = hash.toLowerCase();
		// The machine looks again before the passkey unless its look is fresh,
		// signs that look's calls and fee, and follows the operation to its end.
		const { error } = await act(`execute:${lower}`, {
			type: oneTap ? 'approve_execute' : 'execute',
			chain_id: chainId,
			safe_tx_hash: hash,
			executor: by
		});
		if (!error) return;
		// r5reg-21: its number was used meanwhile — what ran is said, not a failure.
		if (error instanceof ExecutionSpentError) {
			phase = { kind: 'idle' };
			return;
		}
		// The approval the machine gave instead: said as approved, not executed.
		if (oneTap && machine?.actions[`approve:${lower}`]) {
			const reason =
				error instanceof ExecutionWouldFailError ? whyText(m, error.why) : reasonOf(error);
			phase = { kind: 'busy', what: 'approve' };
			const approved = await proposalsHost.settled(`approve:${lower}`);
			const again = approved ? actionError(approved, chainId) : null;
			phase = again ? failedPhase(again) : { kind: 'partial', reason };
			return;
		}
		phase = failedPhase(error);
	}

	/**
	 * r4b-1, r4b-7: how an execution from this device stands — the machine
	 * follows it (one it sent before a reload too) to the core's verdict on
	 * the chain's nonce, the relay's word and the transaction that executed
	 * it; a look that found its number used says what ran (r5reg-21).
	 */
	const verdict = $derived.by((): ExecutionVerdict | null => {
		const hash = (item?.safeTxHash ?? safeTxHash).toLowerCase();
		const found = Object.entries(machine?.executions ?? {}).find(
			([k]) => k.toLowerCase() === `${chainId}:${hash}`
		);
		return (found?.[1] as ExecutionVerdict | undefined) ?? null;
	});
	$effect(() => {
		if (drafting) return;
		const now = verdict;
		const flying = item?.executing === true && item.status !== 'done';
		untrack(() => {
			const by = executedBy ?? actor?.address ?? '';
			if (!now) {
				if (flying && (phase.kind === 'idle' || phase.kind === 'busy')) {
					phase = { kind: 'sent', txHash: null, by };
				}
				return;
			}
			if (now.kind === 'waiting') {
				if (phase.kind !== 'sent') phase = { kind: 'sent', txHash: null, by };
				return;
			}
			const mark = `${key}:${JSON.stringify(now)}`;
			if (ended === mark) return;
			// One this sheet did not see sent or followed: what ran is the list's to say.
			const ours =
				phase.kind === 'sent' ||
				(phase.kind === 'busy' && phase.what === 'execute') ||
				actionOf('execute')?.kind === 'working';
			const spent = previewOf(actor)?.kind === 'spent';
			if (!ours && !spent && !flying) return;
			ended = mark;
			switch (now.kind) {
				case 'executed':
					phase = { kind: 'sent', txHash: now.txHash, by };
					if (item) {
						// A rename: the record's new name is read from the chain now.
						if (item.review.action?.kind === 'rename') {
							multisig.send({ type: 'recheck_record', address: account.address });
						}
						onexecuted?.({ item, txHash: now.txHash, userOpHash: '' });
					}
					return;
				case 'executedByOther':
					phase = { kind: 'ended', verdict: now, by };
					return;
				case 'notSent':
					phase = { kind: 'failed', reason: m.proposal.notSent };
					return;
				case 'timedOut':
					phase = { kind: 'failed', reason: m.proposal.notLanded };
					return;
				default:
					phase = { kind: 'ended', verdict: now, by };
			}
		});
	});

	/** The words for how this device's execution ended (`ended`). */
	function endedLine(verdict: ExecutionVerdict, nonce: bigint): string {
		switch (verdict.kind) {
			case 'executedByOther':
				return verdict.paid ? m.proposal.lostRacePaid : m.proposal.lostRace;
			case 'replaced':
				return fillTemplate(
					verdict.paid ? m.proposal.replacedFirstPaid : m.proposal.replacedFirst,
					{
						nonce: nonce.toString()
					}
				);
			case 'failed':
				return m.proposal.execFailed;
			default:
				return fillTemplate(m.proposal.cantCheckUsed, { network: chainName(chainId) });
		}
	}

	/**
	 * Owners still to approve, on no device here — while their approval is
	 * still needed, and only for a transaction someone can sign (r11-4).
	 */
	const missingOwners = $derived(
		!item || item.status === 'done' || nobodySigns || readOnly
			? []
			: (members?.owners ?? []).filter(
					(o) => !item.approvals.some((a) => a.counts && sameAddress(a.owner, o))
				)
	);
	/** Enough members approved: nobody else's approval is needed (r4D-6). */
	const enough = $derived(!!item && item.approved >= item.threshold);
	/**
	 * How each member still to approve signs (`signers.kindOf`) — code at its
	 * address on any of the multisig's networks says a Vela account, over the
	 * kind kept for it (r7: evidence beats the label).
	 */
	/*
	 * The shown multisig's: the machine reads how each one signs
	 * (`read_signers`: code on the multisig's networks, Vela's passkey signer
	 * configuration) when the sheet opens and as the members still to approve
	 * change — a link's read-only one's too.
	 */
	const signersKey = $derived(
		!drafting && item && missingOwners.length > 0 ? `${key}:${missingOwners.join(',')}` : ''
	);
	$effect(() => {
		if (!signersKey) return;
		proposalsHost.send({
			type: 'read_signers',
			chain_id: chainId,
			safe_tx_hash: untrack(() => item?.safeTxHash ?? safeTxHash)
		});
	});
	const machineSigners = $derived.by(() => {
		const hash = (item?.safeTxHash ?? safeTxHash).toLowerCase();
		return (
			Object.entries(machine?.signers ?? {}).find(
				([k]) => k.toLowerCase() === `${chainId}:${hash}`
			)?.[1] ?? null
		);
	});
	const kinds = $derived(
		new Map<string, string | null>(
			missingOwners.map((o) => [
				o,
				localOwners([o]).length > 0
					? 'here'
					: (machineSigners?.rows.find((r) => sameAddress(r.owner, o))?.kind ?? null)
			])
		)
	);
	/**
	 * r7-8: the machine's one partition of the members still to approve —
	 * each said once: `in_vela` (something shows a Vela account: it approves
	 * in Vela, from the link — never an outside wallet, ms-11/r4-9), `outside`
	 * (the outside-wallet link: a plain account, or one nothing says, beside
	 * the Vela way), `either` (nothing says yet: both ways, its own line),
	 * `contract` (it approves from its own app, which Vela can't collect yet:
	 * said, nothing offered). Nothing once enough approved (r4D-6).
	 */
	const partOf = (list: readonly string[] | undefined): string[] =>
		(list ?? []).filter((o) => missingOwners.some((x) => sameAddress(x, o)));
	const outside = $derived(partOf(machineSigners?.outside));
	/** Vela members elsewhere still to approve: they approve in Vela, from the link. */
	const elsewhere = $derived(partOf(machineSigners?.in_vela));
	/** Members nothing says how they sign yet: either way, said once (r7-8). */
	const either = $derived(partOf(machineSigners?.either));

	/**
	 * r4C-13: the one link to send a member whose wallet is elsewhere — the
	 * connect page, naming the transaction, which it reads, reviews and signs
	 * from alone (r4C-2) — never the Vela link an outside wallet cannot use.
	 */
	const outsideLink = $derived(
		typeof location === 'undefined' || outside.length === 0
			? ''
			: connectLink(`${shareOrigin(location.origin)}/${locale}/connect`, null, outside, {
					chainId,
					multisig: account.address,
					safeTxHash
				})
	);
	/**
	 * Members still to approve the Vela link is for: Vela's, nothing-says-yet
	 * ones, and those still being read (never this device's, never a
	 * contract or a surely outside wallet).
	 */
	const velaMissing = $derived(
		enough
			? []
			: missingOwners.filter(
					(o) =>
						localOwners([o]).length === 0 &&
						(elsewhere.some((x) => sameAddress(x, o)) ||
							either.some((x) => sameAddress(x, o)) ||
							kinds.get(o) === null)
				)
	);
	/**
	 * r7: smart contract members still to approve — they approve from their
	 * own app (EIP-1271), which Vela can't collect yet: said, and neither the
	 * Vela link nor an outside wallet offered for them.
	 */
	const contractMissing = $derived(partOf(machineSigners?.contract));
	let outsideCopied = $state(false);
	async function copyOutsideLink() {
		try {
			await navigator.clipboard.writeText(outsideLink);
			outsideCopied = true;
			setTimeout(() => (outsideCopied = false), 1600);
		} catch {
			// nothing to copy into: the link stays on screen
		}
	}

	async function onExternal() {
		if (!item) return;
		// The machine's gate, then the outside member's wallet over a live
		// session (`Notice::OutsideSign`); its verified answer goes through the
		// machine's outbox (a link's read-only proposal too).
		const { error } = await act(`approve:${item.safeTxHash.toLowerCase()}`, {
			type: 'approve_outside',
			chain_id: chainId,
			safe_tx_hash: item.safeTxHash
		});
		// The outside wallet's own refusal is its sheet's to say.
		if (error && !cancelled(error)) phase = failedPhase(error);
	}

	/** A voiding transaction at its number, proposed and approved at once by `rejecter`. */
	async function onReject() {
		if (!item || !rejecter) return;
		phase = { kind: 'busy', what: 'reject' };
		try {
			await proposalsHost.propose(
				account,
				chainId,
				{ kind: 'cancel', nonce: `0x${item.nonce.toString(16)}` },
				{ review: false, as: rejecter.address }
			);
			rejecting = false;
			phase = { kind: 'idle' };
		} catch (error) {
			phase = failedPhase(
				error instanceof ProposeFailedError ? actionError(error.state, chainId) : error
			);
		}
	}

	async function copyLink() {
		try {
			await navigator.clipboard.writeText(link);
			copied = true;
			setTimeout(() => (copied = false), 1600);
		} catch {
			// nothing to copy into: the link stays on screen
		}
	}

	async function shareLink() {
		try {
			await navigator.share({ title: shownTitle || m.proposal.title, url: link });
		} catch {
			// dismissed, or no share sheet: copying is the fallback
		}
	}

	/*
	 * A transaction whose number is used, finished here or anywhere: executed
	 * (the transaction whose `ExecutionSuccess` names it), or voided by another
	 * transaction at its number — read from the chain, never guessed. `null`
	 * while it is being read or the chain cannot say.
	 */
	/*
	 * ms-9: a voided one says by what, in one wording — the cancellation
	 * proposed at its number when the chain shows that cancellation
	 * executed, else another transaction at its number (never a guess).
	 */
	let finished = $state<{ txHash: string | null; byCancel?: boolean } | null>(null);
	/** r4b-2: the chain's history could not be read: said, with Retry. */
	let unreadHistory = $state(false);
	let historyTry = $state(0);
	/** Unreadable histories asked again unprompted, and how far apart. */
	const HISTORY_AUTO_TRIES = 3;
	const HISTORY_AUTO_MS = 4_000;
	let autoTries = 0;
	const doneKey = $derived(item?.status === 'done' ? item.safeTxHash : '');
	/**
	 * The shown multisig's story, as the machine tells it (`timelines`): how
	 * it ended — executed by which transaction, or replaced at its number (by
	 * its cancellation or not) — read from the chain's history on its refresh.
	 */
	const machineSteps = $derived.by((): MultisigTimelineStep[] | null => {
		const hash = (item?.safeTxHash ?? safeTxHash).toLowerCase();
		return (
			Object.entries(machine?.timelines ?? {}).find(
				([k]) => k.toLowerCase() === `${chainId}:${hash}`
			)?.[1] ?? null
		);
	});
	const machineFinished = $derived.by(() => {
		const last = machineSteps?.at(-1);
		if (last?.kind === 'executed') return { txHash: last.tx_hash };
		if (last?.kind === 'replaced') return { txHash: null, byCancel: last.cancellation };
		return null;
	});
	// A done one's sheet: the machine reads how it ran (its receipt's facts).
	$effect(() => {
		if (!doneKey) return;
		proposalsHost.send({ type: 'timeline', chain_id: chainId, safe_tx_hash: doneKey });
	});
	$effect(() => {
		const key = doneKey;
		void historyTry;
		const found = machineFinished;
		const unread = !!key && !found && (machine?.history_unreadable ?? []).includes(chainId);
		finished = found;
		unreadHistory = unread;
		if (!unread) {
			if (found) autoTries = 0;
			return;
		}
		// Round 5: a node a few blocks behind cannot say yet — asked again by
		// itself a few times (Retry stays on screen).
		if (autoTries >= HISTORY_AUTO_TRIES) return;
		const again = setTimeout(() => {
			autoTries += 1;
			void proposals.refresh();
		}, HISTORY_AUTO_MS);
		return () => clearTimeout(again);
	});
	/** The person's Retry on a history that could not be read. */
	function retryHistory(): void {
		historyTry += 1;
		void proposals.retry();
	}
	/**
	 * r4a-8: an executed transaction's facts as they were when it ran — whose
	 * approvals counted, the threshold then, who sent it — read once its
	 * execution is known; never today's owners re-counted.
	 */
	const executedTx = $derived(
		phase.kind === 'sent' && phase.txHash ? phase.txHash : (finished?.txHash ?? null)
	);
	/** The machine's (`frozen`): from its receipt, else what this device recorded sending it. */
	const machineFrozen = $derived.by((): FrozenFacts | null => {
		if (!executedTx) return null;
		const hash = (item?.safeTxHash ?? safeTxHash).toLowerCase();
		const all = (machine?.frozen ?? {}) as Record<string, FrozenFacts>;
		return Object.entries(all).find(([k]) => k.toLowerCase() === `${chainId}:${hash}`)?.[1] ?? null;
	});
	// Executed from this sheet: the machine reads its facts once it is done.
	$effect(() => {
		if (!executedTx || item?.status !== 'done' || doneKey) return;
		proposalsHost.send({ type: 'timeline', chain_id: chainId, safe_tx_hash: item.safeTxHash });
	});
	const frozen = $derived(machineFrozen);
	/** Its story for the timeline (r4D-17): the machine's. */
	const steps = $derived<MultisigTimelineStep[]>(machineSteps ?? []);
	/**
	 * Whom the list names: the members whose approvals counted when it ran;
	 * a finished one's verified approvals while those are not known (never a
	 * "Waiting" row on a transaction that can no longer move); else this
	 * network's members now.
	 */
	const listed = $derived(
		frozen
			? frozen.approvers
			: item?.status === 'done'
				? item.approvals.filter((a) => a.counts).map((a) => a.owner)
				: (members?.owners ?? [])
	);

	let hashCopied = $state(false);
	async function copyHash() {
		if (!item) return;
		try {
			await navigator.clipboard.writeText(item.safeTxHash);
			hashCopied = true;
			setTimeout(() => (hashCopied = false), 1600);
		} catch {
			// nothing to copy into: the hash stays on screen
		}
	}

	/**
	 * r4a-16: every transaction it waits behind — each number between the
	 * chain's and its own — and, where one is here, the way to it (r4D-19).
	 */
	const blocking = $derived.by(() => {
		if (!item || item.status !== 'queued') return [];
		const from = store.chain(chainId)?.members.nonce ?? 0n;
		const out: { nonce: bigint; hash: string | null }[] = [];
		for (let n = from; n < item.nonce && out.length < 20; n += 1n) {
			const open = store.chain(chainId)?.items.find((i) => i.nonce === n && i.status !== 'done');
			out.push({ nonce: n, hash: open?.safeTxHash ?? null });
		}
		return out;
	});

	/** Approvals the mailbox holds from accounts that are not members on this network now. */
	const notCounted = $derived(
		(item?.approvals ?? [])
			.filter((a) => !a.counts && !(members?.owners ?? []).some((o) => sameAddress(o, a.owner)))
			.map((a) => a.owner)
	);

	/**
	 * Its first execution on a network it is not deployed on deploys it there
	 * with the original members. When another network's members now differ
	 * from those, the networks where they do — said before anyone executes.
	 */
	const startsOriginal = $derived.by(() => {
		if (!item || !members || members.deployed || item.status === 'done') return [];
		if (item.canExecute.length === 0 && item.canApprove.length === 0) return [];
		if (!sameAddress(proposals.followed, account.address)) return [];
		const original = { owners: account.owners, threshold: account.threshold };
		return proposals.chains
			.filter((c) => c.chainId !== chainId && c.members.deployed)
			.filter((c) => {
				try {
					return !memberDrift(original, c.members).same;
				} catch {
					return false;
				}
			})
			.map((c) => c.chainId);
	});

	/** Where what was signed here for it stands, while the mailbox lacks it. */
	const kept = $derived.by(() => {
		if (!item?.unsent) return null;
		const prefix = `${chainId}:${account.address}:${item.safeTxHash}:`.toLowerCase();
		return (
			Object.entries(machine?.outbox ?? {}).find(([k]) =>
				k.toLowerCase().startsWith(prefix)
			)?.[1] ?? null
		);
	});
	/** A second's tick while a resend is counted down. */
	let clock = $state(Date.now());
	$effect(() => {
		if (!kept?.retryAt) return;
		const timer = setInterval(() => (clock = Date.now()), 1000);
		return () => clearInterval(timer);
	});
	let resending = $state(false);
	async function sendAgain() {
		if (!item) return;
		resending = true;
		const was = kept;
		proposalsHost.send({ type: 'send_again', chain_id: chainId, safe_tx_hash: item.safeTxHash });
		// Until the machine says where it stands now (sent, or waiting again).
		await proposalsHost.until(() => (kept !== was || !item?.unsent ? true : null), 15_000);
		resending = false;
	}

	const canShare = typeof navigator !== 'undefined' && typeof navigator.share === 'function';
	const busy = $derived(phase.kind === 'busy');
	const recipient = $derived(item ? actionRecipient(item.review.action) : null);
	/**
	 * An owner executing approves as `msg.sender`: once its execution is on
	 * chain, it counts. Anyone else adds nothing — and nobody does while it is
	 * only on its way (the relay may yet refuse it).
	 */
	const shownApproved = $derived.by(() => {
		if (!item) return 0;
		const p = phase;
		if (p.kind !== 'sent' || !p.txHash) return item.approved;
		const owner = (members?.owners ?? []).some((o) => sameAddress(o, p.by));
		const approved = item.approvals.some((a) => a.counts && sameAddress(a.owner, p.by));
		return owner && !approved ? item.approved + 1 : item.approved;
	});

	/** r4b-3: the network did not answer the last read (or the device is offline). */
	const chainDown = $derived(store.unreadable.includes(chainId));
	let retrying = $state(false);
	/** Withdraw the person's own proposal on its review (§3.6), or decline a site's request. */
	function decline() {
		if (prebuilt) proposalsHost.withdrawDraft();
		// The machine answers its site: declined before it is proposed, else "proposed".
		else if (dapp) proposalsHost.send({ type: 'dapp_leave', id: dapp.rid });
	}

	async function retryRead() {
		retrying = true;
		try {
			await store.refresh(account, account.chainIds);
		} finally {
			retrying = false;
		}
	}
</script>

<Sheet title={m.proposal.title} {closeLabel} {wide} {busy} {onclose}>
	{#if drafting && !item}
		<!-- The dApp's transaction is being read: said, never a blank window. -->
		{#if draftFailure}
			<p class="warn" role="alert">{draftFailure}</p>
			<div class="two">
				<Button variant="secondary" shape="rounded" onclick={decline}>{m.external.decline}</Button>
				<Button variant="secondary" shape="rounded" onclick={() => dapp?.retry()}
					>{m.external.retry}</Button
				>
			</div>
		{:else}
			<p class="quiet" aria-busy="true">{m.proposal.opening}</p>
		{/if}
	{:else if !item}
		{#if !store.loading && (chainDown || offline())}
			<!-- r4b-3: the network (or this device) is not answering: said as
			     that, with Retry — never "it hasn't been shared". -->
			<p class="warn" role="alert" data-testid="proposal-chain-down">
				{offline()
					? m.proposal.reason.offline
					: fillTemplate(m.proposal.cantCheck, { network: chainName(chainId) })}
			</p>
			<Button variant="secondary" shape="rounded" disabled={retrying} onclick={retryRead}
				>{m.service.retry}</Button
			>
		{:else}
			<!-- Read from the mailbox and still not there: said as that (ms-22). -->
			<p class="quiet" aria-busy={!store.chain(chainId)?.mailbox}>
				{store.loading
					? m.proposal.opening
					: store.chain(chainId)?.mailbox
						? m.proposal.notInMailbox
						: m.proposal.notFound}
			</p>
		{/if}
	{:else}
		{#if dapp}
			<p class="site">{fillTemplate(m.proposal.siteAsks, { host: site })}</p>
		{/if}
		<header class="what">
			{#if !loneCall && !(change && memberReading?.changes.length === 1)}
				<!-- r5reg-15: a call read by the signing sheet's machines heads itself
				     ("Send −0.05 USDC"): its verb is not said twice above it. S4 (r7): nor
				     is a lone member change, which its own list names ("Add Carol (…)"). -->
				<p class="title">{shownTitle || '…'}</p>
			{/if}
			{#if recipient && !(loneCall && item.review.action?.kind === 'transfer')}
				<!-- A transfer read by the signing sheet's machines names its own recipient (§3.6). -->
				{@const named = localName(recipient)}
				<p class="to">
					<Identicon svg={identiconSvgForClient(recipient)} size="compact" address={recipient} />
					<!-- A call names the contract it calls, in the signing sheet's word. -->
					<span class="to-label">{reading ? signing.labelInteracting : m.action.to}</span>
					{#if named}<span class="to-who">{named}</span>{/if}
					<span class="to-address">{shortenAddress(recipient)}</span>
				</p>
			{/if}
			{#if tokenNote}
				<!-- r4a-2: which token this is, when nobody vouches for its name. -->
				<ProvenanceNoteView
					note={tokenNote}
					contractLabel={m.token.contract}
					copyLabel={m.proposal.copy}
					explorerLabel={m.proposal.explorer}
				/>
			{/if}
			{#if change && memberReading}
				<!-- r4a-1 / r7-11: who it adds, removes or replaces, the rule before and
				     after, on which network — said before anything else, drawn as every
				     signer reads it (the connect page too). -->
				<MemberChangeRule
					{m}
					changes={memberReading}
					network={chainName(chainId)}
					{who}
					done={item.status === 'done'}
				/>
			{/if}
			<CallReview
				review={fullReview}
				{m}
				nativeSymbol={nativeSymbol(chainId)}
				badges={partyBadges}
				names={partyNames}
			/>
			{#if lines.length > 0 && !reading}
				<ul class="lines">
					{#each lines as line, i (i)}
						<li>
							<span>{line.label}</span><span class="to-address">{shortenAddress(line.to)}</span>
							{#if line.note}
								<span class="line-note">
									<ProvenanceNoteView
										note={line.note}
										copyLabel={m.proposal.copy}
										explorerLabel={m.proposal.explorer}
									/>
								</span>
							{/if}
						</li>
					{/each}
				</ul>
			{/if}
			<!-- §3.6: the state-diff guard, who each leg reaches, what moves — the signing sheet's blocks. -->
			<div class="review-checks" data-testid="review-checks">
				<BlockList blocks={checks} />
			</div>
			<dl class="facts">
				<div>
					<dt>{m.action.network}</dt>
					<dd>{chainName(chainId)}</dd>
				</div>
				<div>
					<dt>{m.action.number}</dt>
					<!-- r4D-13: "#0", "Nr. 0": a number, never "Transaction # 0", a count. -->
					<dd>{fillTemplate(m.action.numberValue, { nonce: item.nonce.toString() })}</dd>
				</div>
				<div class="hash-row">
					<dt>{m.proposal.hash}</dt>
					<!-- r4a-22: the whole hash, to compare with another wallet's. -->
					<dd class="mono">
						<details class="hash" data-testid="proposal-hash">
							<summary>{item.safeTxHash.slice(0, 10)}…{item.safeTxHash.slice(-8)}</summary>
							<code>{item.safeTxHash}</code>
						</details>
						<button
							type="button"
							class="hash-copy"
							aria-label={m.proposal.copy}
							data-testid="proposal-hash-copy"
							onclick={copyHash}
						>
							<Icon icon={hashCopied ? UTILITY_ICONS.check : UTILITY_ICONS.copy} size="sm" />
						</button>
					</dd>
				</div>
			</dl>
			{#if item.review.action?.kind === 'rename'}
				<!-- A rename runs on the registry's chain, where only the Safe itself
				     can call it; it never deploys it there (owner, 2026-10-29). -->
				<p class="fine" data-testid="rename-where">
					{fillTemplate(m.proposal.renameRuns, { network: chainName(chainId) })}
				</p>
				{#if item.renameUndeployed}
					<p class="warn" role="alert" data-testid="rename-undeployed">
						{fillTemplate(m.proposal.renameUndeployed, { network: chainName(chainId) })}
					</p>
				{/if}
			{/if}
			{#if item.review.action?.kind === 'call' && item.review.action.data !== '0x'}
				<!-- What the call carries, as it will be signed: every member can read it. -->
				<details class="data">
					<summary>{m.proposal.data}</summary>
					<code>{item.review.action.data}</code>
				</details>
			{:else if item.review.action?.kind === 'batch' && reading}
				<!-- Every call's own bytes, in the batch's order. -->
				<details class="data">
					<summary>{m.proposal.data}</summary>
					{#each item.review.action.calls as call, i (i)}
						<code>#{i + 1} {call.to} {call.data}</code>
					{/each}
				</details>
			{/if}
		</header>

		{#if fullReview && tech}
			<ReviewTech
				{m}
				review={fullReview}
				{tech}
				advice={adviceModel}
				executed={item.status === 'done'}
			/>
		{/if}

		{#if gate.kind === 'refused'}
			<!-- §3.5: its genesis refused here — read-only, nothing to sign. -->
			<AuthorityNotice {m} kind="refused" findings={gate.findings} readOnlyOn={[chainId]} />
		{:else if gate.kind === 'risk'}
			<!-- §3.5: more than its members can act for it here — each signature asks. -->
			<AuthorityNotice {m} kind="risk" findings={gate.findings} />
		{/if}

		{#if refusedWords}
			<p class="warn" role="alert" data-testid="proposal-refused">
				<Icon icon={UTILITY_ICONS['triangle-alert']} size="sm" />
				<!-- r9-9, r11-4: the core's one line for why, whatever the action (never its English reason). -->
				{refusedWords}
			</p>
		{/if}

		<section class="approvals" aria-label={m.proposal.approvals}>
			<h3>
				{m.proposal.approvals}
				<!-- r4a-8: a finished one is never re-counted against today's rule. -->
				<span class="count" data-testid="approvals-count"
					>{executedTx
						? m.proposal.executed
						: finished
							? m.proposal.voided
							: item.status === 'done'
								? ''
								: progressLine(m, shownApproved, item.threshold)}</span
				>
			</h3>
			<div class="bar" aria-hidden="true">
				<span
					class="fill"
					style:width="{executedTx
						? 100
						: item.status === 'done'
							? 0
							: Math.min(100, (shownApproved / Math.max(1, item.threshold)) * 100)}%"
				></span>
			</div>
			<ul>
				{#each listed as owner (owner)}
					{@const row =
						frozen || item.status === 'done'
							? true
							: item.approvals.find((a) => a.counts && sameAddress(a.owner, owner))}
					{@const executedBy = frozen
						? frozen.executorApproved && sameAddress(frozen.executor, owner)
						: phase.kind === 'sent' && !!phase.txHash && sameAddress(phase.by, owner)}
					{@const mine = localOwners([owner]).length > 0}
					<li class:done={!!row || executedBy}>
						<!-- Beside the row's own control (Sign with another wallet), never in it. -->
						<Identicon svg={identiconSvgForClient(owner)} size="compact" address={owner} />
						<span class="who"
							>{names.label(owner)}{#if mine}<span class="here">
									· {m.panel.thisDevice}</span
								>{/if}</span
						>
						<span class="state">
							{#if executedBy}
								<Icon icon={UTILITY_ICONS.check} size="sm" />{m.proposal.executed}
							{:else if row}
								<Icon icon={UTILITY_ICONS.check} size="sm" />{m.proposal.approved}
							{:else if item.status !== 'done' && enough}
								<!-- Enough approved: theirs is not needed, and nothing asks for it. -->
								{m.proposal.notNeeded}
							{:else if !drafting && kinds.get(owner) === 'wallet' && !busy}
								<button type="button" class="external" onclick={onExternal}
									>{m.external.signWithWallet}</button
								>
							{:else if !drafting && (elsewhere.some( (o) => sameAddress(o, owner) ) || either.some( (o) => sameAddress(o, owner) ))}
								<!-- A Vela member elsewhere approves in Vela, from the link (r4-9);
								     one nothing says yet is offered the Vela way first (r4a-9)
								     and its own wallet's beside it (r7), worded as either (r7-8). -->
								{m.proposal.waiting} ·
								<button
									type="button"
									class="external"
									title={fillTemplate(
										either.some((o) => sameAddress(o, owner))
											? m.proposal.eitherWay
											: m.proposal.approvesInVela,
										{ names: names.label(owner) }
									)}
									onclick={canShare ? shareLink : copyLink}>{m.proposal.sendLink}</button
								>{#if kinds.get(owner) === 'unsure' && !busy}
									·
									<button type="button" class="external" onclick={onExternal}
										>{m.external.signWithWallet}</button
									>{/if}
							{:else if !drafting && kinds.get(owner) === 'contract'}
								<!-- r7: approves from its own app; nothing here can collect it yet. -->
								<span data-testid="contract-member"
									>{m.proposal.waiting} · {m.members.kindContract}</span
								>
							{:else if item.status !== 'done'}
								{m.proposal.waiting}
							{/if}
						</span>
					</li>
				{/each}
			</ul>
		</section>

		{#if frozen}
			<p class="fine" data-testid="executed-facts">
				{[
					frozen.executor
						? fillTemplate(m.proposal.executedBy, { name: names.label(frozen.executor) })
						: '',
					fillTemplate(m.proposal.countedThen, {
						approved: frozen.approvers.length,
						threshold: frozen.threshold
					})
				]
					.filter(Boolean)
					.join(' · ')}
			</p>
		{:else if executedTx && phase.kind !== 'sent'}
			<p class="fine" data-testid="executed-facts-unknown">{m.proposal.approvalsSigned}</p>
		{/if}

		{#if !drafting && contractMissing.length > 0 && item.status !== 'done'}
			<!-- r7: said honestly — their approvals (EIP-1271) aren't collected yet. -->
			<p class="fine" data-testid="contract-members">
				{fillTemplate(m.proposal.contractMembers, {
					names: listOf(
						contractMissing.map((o) => names.label(o)),
						locale
					)
				})}
			</p>
		{/if}

		{#if notCounted.length > 0 && item.status !== 'done'}
			<!-- The mailbox takes any valid signature; only this network's members count. -->
			<p class="fine" data-testid="not-counted">
				{fillTemplate(m.proposal.notCounted, {
					network: chainName(chainId),
					names: listOf(
						notCounted.map((o) => names.label(o)),
						locale
					)
				})}
			</p>
		{/if}

		{#if startsOriginal.length > 0}
			<!-- Executing here deploys it with its genesis: said when its members
			     changed on another network, so nobody is surprised. -->
			<section class="starts" data-testid="starts-original">
				<p class="starts-title">{m.proposal.startsOriginal}</p>
				<p class="fine">
					{fillTemplate(m.proposal.startsOriginalNote, {
						m: account.threshold,
						n: account.owners.length,
						networks: listOf(
							startsOriginal.map((id) => chainName(id)),
							locale
						)
					})}
				</p>
				<ul class="starts-members">
					{#each account.owners as owner (owner)}
						<li>
							<Identicon svg={identiconSvgForClient(owner)} size="compact" address={owner} />
							<span class="who">{names.label(owner)}</span>
						</li>
					{/each}
				</ul>
			</section>
		{/if}

		{#if item.unsent && phase.kind !== 'sent' && phase.kind !== 'ended'}
			<!-- Signed here, not in the mailbox yet: kept, and sent again — said live. -->
			<div class="unsent" role="status" data-testid="proposal-unsent">
				{#if kept?.sending}
					<!-- r8-16: on its way now — nothing was refused, nothing to send again. -->
					<p class="fine" data-testid="proposal-sending">{m.proposal.outboxSending}</p>
				{:else if kept?.retryAt}
					<p class="fine">
						{fillTemplate(kept.why === 'busy' ? m.proposal.outboxBusy : m.proposal.outboxAway, {
							seconds: Math.max(1, Math.ceil((kept.retryAt - clock) / 1000))
						})}
					</p>
				{:else}
					<p class="fine">{m.proposal.outboxNotSent}</p>
					<Button variant="secondary" shape="rounded" disabled={resending} onclick={sendAgain}
						>{m.proposal.outboxSendAgain}</Button
					>
				{/if}
			</div>
		{/if}

		{#if item.status === 'done' && phase.kind !== 'sent' && phase.kind !== 'ended'}
			<!-- Said once it is read from the chain: executed (where), or voided. -->
			{#if unreadHistory}
				<!-- r4b-2: the history could not be read — said, with Retry. -->
				<p class="quiet" role="status" data-testid="proposal-history-unread">
					{fillTemplate(m.proposal.cantCheckUsed, { network: chainName(chainId) })}
				</p>
				<Button variant="secondary" shape="rounded" onclick={retryHistory}>{m.service.retry}</Button
				>
			{:else if finished?.txHash}
				<p class="quiet">
					{m.proposal.executed}
					{#if explorerTxUrl(chainId, finished.txHash)}
						·
						<!-- eslint-disable svelte/no-navigation-without-resolve -- an external block explorer -->
						<a
							href={explorerTxUrl(chainId, finished.txHash)}
							target="_blank"
							rel="noopener noreferrer">{m.proposal.explorer}</a
						>
						<!-- eslint-enable svelte/no-navigation-without-resolve -->
					{/if}
				</p>
			{:else if finished}
				<p class="quiet" data-testid="proposal-voided">
					{fillTemplate(finished.byCancel ? m.proposal.voidedNote : m.proposal.voidedByOther, {
						nonce: item.nonce.toString()
					})}
				</p>
			{:else}
				<p class="quiet">{m.proposal.doneNote}</p>
			{/if}
		{:else if item.status === 'queued'}
			<!-- r4a-16: every number it waits behind; each one here opens (r4D-19). -->
			<p class="quiet" data-testid="proposal-queued">
				{blocking.length > 1
					? fillTemplate(m.proposal.queuedNoteMany, {
							nonces: listOf(
								blocking.map((b) => `#${b.nonce}`),
								locale
							)
						})
					: fillTemplate(m.proposal.queuedNote, {
							nonce: (blocking[0]?.nonce ?? store.chain(chainId)?.members.nonce ?? 0n).toString()
						})}
			</p>
			{#if !preview && blocking.some((b) => b.hash)}
				<div class="blocking">
					{#each blocking as b (b.nonce)}
						{#if b.hash}
							{@const hash = b.hash}
							<button
								type="button"
								class="text-button"
								data-tap
								onclick={() => multisigUi.open({ kind: 'proposal', chainId, safeTxHash: hash })}
								>{fillTemplate(m.proposal.openNumber, { nonce: b.nonce.toString() })}</button
							>
						{/if}
					{/each}
				</div>
			{/if}
		{/if}
		{#if executedTx && (item.status === 'done' || phase.kind === 'sent')}
			<!-- r4D-17: its story, from the facts this wallet holds. -->
			<ProposalTimeline {m} {chainId} {steps} label={(a) => names.label(a)} />
		{/if}
		{#if item.sameNonce.length > 0 && item.status !== 'done'}
			<!-- r4b-6: proposals at one number — only one of them can run. -->
			<p class="fine" role="note" data-testid="proposal-conflict">
				{fillTemplate(m.proposal.conflict, { nonce: item.nonce.toString() })}
			</p>
		{/if}
		{#if item.cancelledBy && item.status !== 'done'}
			<p class="quiet">{m.proposal.cancelPending}</p>
		{/if}

		{#if watch && !drafting}
			<!-- ms-21: no member here — said, and the two ways on, each a tap. -->
			<section class="preview" data-testid="proposal-preview">
				<p class="quiet">{m.proposal.previewNote}</p>
				{#if watch.added}
					<p class="fine" role="status">{m.proposal.watching}</p>
				{/if}
				<div class="preview-actions">
					{#if !watch.added}
						<Button variant="secondary" shape="rounded" onclick={watch.onwatch}
							>{m.proposal.watch}</Button
						>
					{/if}
					{#if outside.length > 0 && phase.kind === 'idle'}
						<Button variant="secondary" shape="rounded" onclick={onExternal}
							>{m.external.signWithWallet}</Button
						>
					{/if}
				</div>
			</section>
		{/if}

		{#snippet actorChoice()}
			<!-- r5reg-5: which of this device's members does it — never picked silently. -->
			<fieldset class="actor" data-testid="actor-choice">
				<legend>{m.proposal.as}</legend>
				{#each candidates as owner (owner.address)}
					<!-- An executor's coin here, and whether it covers the fee (r4-10). -->
					{@const look = executors.length > 0 ? previewOf(owner) : null}
					<label class:on={actor?.address === owner.address}>
						<input
							type="radio"
							name="actor"
							value={owner.address}
							checked={actor?.address === owner.address}
							onchange={() => (chosen = owner.address)}
						/>
						<span class="actor-name">{owner.name}</span>
						{#if look?.kind === 'priced' && look.balance !== null}
							<span class="actor-balance" data-testid="actor-balance"
								>{formatUnits(look.balance, 18)}
								{nativeSymbol(
									chainId
								)}{#if !look.canPay}{` · ${m.proposal.pickerCantPay}`}{/if}</span
							>
						{/if}
					</label>
				{/each}
			</fieldset>
		{/snippet}

		{#if drafting}
			<!-- The review of what the site asks: nothing is signed before this. -->
			{#if phase.kind === 'failed'}
				<p class="warn" role="alert" data-testid="proposal-failed">{phase.reason}</p>
			{/if}
			{#if candidates.length > 1 && item.review.action !== null && !readOnly && !signBlocked}
				{@render actorChoice()}
			{/if}
			<!-- r4D-19: in the desktop's capped dialog the actions stay in view. -->
			<div class="two" class:actions={wide} data-testid="proposal-actions">
				<!-- r4D-13: a site's request is declined; S4 (r7): the person's own, not
				     proposed yet, is simply not proposed — nothing to withdraw. -->
				<Button variant="secondary" shape="rounded" disabled={busy} onclick={decline}
					>{prebuilt ? m.proposal.dontPropose : m.external.decline}</Button
				>
				{#if item.review.action !== null && !readOnly && !signBlocked}
					<Button
						variant={valueVariant}
						shape="rounded"
						loading={busy && phase.kind === 'busy' && phase.what === 'propose'}
						disabled={busy || reviewHolding}
						onclick={onPropose}
						>{executeNow
							? anywayLabel('approveExecute', m.proposal.approveAndExecute, fullReview, m)
							: rejecters.length > 0
								? anywayLabel('propose', m.send.propose, fullReview, m)
								: m.send.proposeOnly}</Button
					>
				{/if}
			</div>
			{#if executeNow && item.review.action !== null && !readOnly && !signBlocked}
				<p class="fine">{m.proposal.feeNote}</p>
			{/if}
		{:else if phase.kind === 'ended'}
			<!-- r4b-1: this device's execution was not the one that ran — said
			     within seconds, with the transaction that did. -->
			{@const other =
				phase.verdict.kind === 'executedByOther' || phase.verdict.kind === 'failed'
					? phase.verdict.txHash
					: null}
			<div class="sent" role="status" data-testid="proposal-ended">
				<span>{endedLine(phase.verdict, item.nonce)}</span>
				{#if other && explorerTxUrl(chainId, other)}
					<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- an external block explorer -->
					<a href={explorerTxUrl(chainId, other)} target="_blank" rel="noopener noreferrer"
						>{m.proposal.explorer}</a
					>
				{/if}
			</div>
			{#if phase.verdict.kind === 'unknown'}
				<Button
					variant="secondary"
					shape="rounded"
					onclick={() => {
						phase = { kind: 'idle' };
						ended = null;
						retryHistory();
					}}>{m.service.retry}</Button
				>
			{/if}
		{:else if phase.kind === 'sent'}
			<div class="sent" class:pending={!phase.txHash} role="status" aria-busy={!phase.txHash}>
				<!-- The check is for what is on chain; on its way, it is only said. -->
				{#if phase.txHash}<Icon icon={UTILITY_ICONS.check} size="md" />{/if}
				<span>{phase.txHash ? m.proposal.executed : m.proposal.executing}</span>
				{#if phase.txHash && explorerTxUrl(chainId, phase.txHash)}
					<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- an external block explorer -->
					<a href={explorerTxUrl(chainId, phase.txHash)} target="_blank" rel="noopener noreferrer"
						>{m.proposal.explorer}</a
					>
				{/if}
			</div>
		{:else if item.status !== 'done' && item.review.action !== null && !readOnly && !signBlocked}
			{#if candidates.length > 1}
				{@render actorChoice()}
			{/if}

			{#if phase.kind === 'failed'}
				<p class="warn" role="alert" data-testid="proposal-failed">
					{fillTemplate(m.proposal.failed, { reason: phase.reason })}
				</p>
			{:else if phase.kind === 'partial'}
				<p class="fine" role="status" data-testid="approved-not-executed">
					{fillTemplate(m.proposal.approvedNotExecuted, { reason: phase.reason })}
				</p>
			{/if}

			<!-- r4D-19: in the desktop's capped dialog the actions stay in view. -->
			<div class="primary" class:actions={wide} data-testid="proposal-actions">
				{#if executors.length > 0 && actor}
					{#if actorPreview?.kind === 'refused'}
						<!-- §3.5: it would deploy from a genesis the core does not find Verified here. -->
						<p class="warn" role="alert" data-testid="execute-refused">
							{reasonText(actorPreview.error)}
						</p>
					{:else if actorPreview?.kind === 'fails'}
						<!-- r4-5: it would fail: said why, before any passkey, and not offered. -->
						<p class="warn" role="alert" data-testid="execute-would-fail">
							{fillTemplate(m.proposal.wouldFail, { reason: whyText(m, actorPreview.why) })}
						</p>
					{:else if actorPreview?.kind === 'spent'}
						<!-- r5reg-21: its number was used meanwhile — what ran is being read. -->
						<p class="quiet" role="status" data-testid="execute-spent">{m.proposal.doneNote}</p>
					{:else if actorPreview?.kind === 'priced' && !actorPreview.canPay}
						<!-- r4D-1: who cannot pay, and who here can — before the tap. -->
						<p class="warn" role="alert" data-testid="execute-cant-pay">
							{payer && !sameAddress(payer.address, actor.address)
								? fillTemplate(m.proposal.cantPayOther, {
										name: names.label(actor.address),
										other: names.label(payer.address)
									})
								: fillTemplate(m.proposal.cannotPay, {
										name: names.label(actor.address),
										symbol: nativeSymbol(chainId),
										fee: feeCoin(actorPreview.fee.amount, nativeSymbol(chainId)),
										balance: feeCoin(actorPreview.balance ?? 0n, nativeSymbol(chainId))
									})}
						</p>
					{/if}
					{#if blocked}
						<!-- It cannot run by this executor: approving is the one thing left to do. -->
						{#if approveFirst && canApproveOnly && actorPreview?.kind !== 'spent'}
							<Button
								variant={valueVariant}
								shape="rounded"
								loading={busy && phase.kind === 'busy' && phase.what === 'approve'}
								disabled={busy || reviewHolding}
								onclick={onApprove}
							>
								{anywayLabel('approve', m.proposal.approve, fullReview, m)}
							</Button>
						{/if}
					{:else}
						<div class:two={approveFirst && canApproveOnly}>
							{#if approveFirst && canApproveOnly}
								<!-- Executing pays the fee; approving alone does not. -->
								<Button
									variant="secondary"
									shape="rounded"
									loading={busy && phase.kind === 'busy' && phase.what === 'approve'}
									disabled={busy || reviewHolding}
									onclick={onApprove}
								>
									{m.proposal.approve}
								</Button>
							{/if}
							<Button
								variant={valueVariant}
								shape="rounded"
								loading={busy && phase.kind === 'busy' && phase.what === 'execute'}
								disabled={busy ||
									reviewHolding ||
									actorPreview === null ||
									(approveFirst && !oneTapOffered)}
								onclick={onExecute}
							>
								{approveFirst
									? anywayLabel('approveExecute', m.proposal.approveAndExecute, fullReview, m)
									: anywayLabel('execute', m.proposal.execute, fullReview, m)}
							</Button>
						</div>
					{/if}
					{#if actorPreview?.kind === 'priced'}
						<!-- r4-7: the fee before the passkey, in the executor's coin, and what it pays for. -->
						{@const words = feeWords(m, actorPreview, {
							executor: actor.address,
							symbol: nativeSymbol(chainId),
							network: chainName(chainId),
							label: (a) => names.label(a)
						})}
						<div class="fee" data-testid="execute-fee">
							<p class="fine">{words.fee}</p>
							{#if words.setup}
								<p class="fine">{words.setup}</p>
								<ul class="fine setup">
									{#each words.items as line, i (i)}
										<li>{line}</li>
									{/each}
								</ul>
							{/if}
						</div>
					{:else if actorPreview?.kind === 'unchecked'}
						<!-- Round 5: the look did not answer in time — said, and nothing held:
						     approving goes ahead, and Execute looks again first. -->
						<p class="fine" role="status" data-testid="execute-unchecked">
							{m.proposal.feeUnchecked}
							<button type="button" class="text-button" onclick={recheck}
								>{m.proposal.feeRetry}</button
							>
						</p>
					{:else if actorPreview?.kind === 'unpriced'}
						<p class="fine" data-testid="execute-unpriced">
							{m.proposal.feeUnpriced}
							<button type="button" class="text-button" onclick={recheck}
								>{m.proposal.feeRetry}</button
							>
						</p>
					{:else if actorPreview === null}
						<p class="fine" aria-busy="true" data-testid="execute-checking">
							{m.proposal.feeChecking}
						</p>
					{/if}
				{:else if approvers.length > 0 && actor}
					<Button
						variant={valueVariant}
						shape="rounded"
						loading={busy && phase.kind === 'busy' && phase.what === 'approve'}
						disabled={busy || reviewHolding}
						onclick={onApprove}
					>
						{anywayLabel('approve', m.proposal.approve, fullReview, m)}
					</Button>
				{:else if localOwners(members?.owners ?? []).length === 0 && !watch}
					<p class="quiet">{m.proposal.notMember}</p>
				{/if}
			</div>
		{/if}

		{#if dapp && !drafting && item.status !== 'done'}
			<!-- A batch's site was given its id; a transaction's waits for its hash
			     (until the request's limit, when it is told it was proposed). -->
			{#if dapp.held.method !== 'wallet_sendCalls' && !dapp.held.told && shownApproved < item.threshold}
				<p class="fine" role="status">
					{fillTemplate(m.proposal.stillNeeded, { missing: item.threshold - shownApproved })}
				</p>
			{/if}
			<p class="fine">
				{fillTemplate(
					dapp.held.method === 'wallet_sendCalls'
						? m.proposal.siteFollows
						: dapp.held.told
							? m.proposal.siteTold
							: m.proposal.siteWaits,
					{ host: site }
				)}
			</p>
		{/if}

		<!-- Only while approvals are missing: not once it is being executed, and
		     never for a transaction nobody can sign (r11-4). -->
		{#if !drafting && !nobodySigns && item.status !== 'done' && phase.kind !== 'sent' && phase.kind !== 'ended' && shownApproved < item.threshold && (velaMissing.length > 0 || (outside.length === 0 && contractMissing.length === 0))}
			<section class="invite">
				<h3>{m.proposal.invite}</h3>
				<p class="fine">{m.proposal.inviteHint}</p>
				{#if elsewhere.length > 0}
					<!-- Vela members elsewhere: never an outside wallet — the link, in Vela (r4-9). -->
					<p class="fine" data-testid="approves-in-vela">
						{fillTemplate(m.proposal.approvesInVela, {
							names: listOf(
								elsewhere.map((o) => names.label(o)),
								locale
							)
						})}
					</p>
				{/if}
				{#if either.length > 0}
					<!-- r7-8: nothing says how they sign yet — said once, both ways offered. -->
					<p class="fine" data-testid="either-way">
						{fillTemplate(m.proposal.eitherWay, {
							names: listOf(
								either.map((o) => names.label(o)),
								locale
							)
						})}
					</p>
				{/if}
				<div class="link-row">
					<code class="link">{link}</code>
				</div>
				<div class="link-actions">
					<Button variant="secondary" shape="rounded" onclick={copyLink}>
						<span class="label"
							><Icon icon={copied ? UTILITY_ICONS.check : UTILITY_ICONS.copy} size="sm" />{copied
								? m.proposal.copied
								: m.proposal.copyLink}</span
						>
					</Button>
					{#if canShare}
						<Button variant="secondary" shape="rounded" onclick={shareLink}
							>{m.proposal.share}</Button
						>
					{/if}
				</div>
			</section>
		{/if}

		{#if !drafting && !nobodySigns && item.status !== 'done' && phase.kind !== 'sent' && phase.kind !== 'ended' && shownApproved < item.threshold && outsideLink}
			<!-- r4C-13: an outside member's own link: it signs from the connect page alone. -->
			<section class="invite" data-testid="outside-invite">
				<h3>{m.external.outsideLinkTitle}</h3>
				<p class="fine">{m.external.outsideLinkHint}</p>
				<div class="link-row">
					<code class="link" data-testid="outside-link">{outsideLink}</code>
				</div>
				<div class="link-actions">
					<Button variant="secondary" shape="rounded" onclick={copyOutsideLink}>
						<span class="label"
							><Icon
								icon={outsideCopied ? UTILITY_ICONS.check : UTILITY_ICONS.copy}
								size="sm"
							/>{outsideCopied ? m.proposal.copied : m.proposal.copyLink}</span
						>
					</Button>
				</div>
			</section>
		{/if}

		<!-- Any member here may withdraw it, the proposer included, approved or not. -->
		{#if !drafting && !readOnly && item.status !== 'done' && item.review.action?.kind !== 'cancel' && !item.cancelledBy && rejecter && phase.kind !== 'sent' && phase.kind !== 'ended' && !(phase.kind === 'busy' && phase.what === 'execute')}
			<!-- Not while it is being executed; and a way back from it (r4D-14). -->
			<div class="reject">
				{#if rejecting}
					<p class="fine">{rejectHint(m, item.threshold)}</p>
					<div class="two">
						<Button
							variant="secondary"
							shape="rounded"
							disabled={busy}
							onclick={() => (rejecting = false)}>{m.proposal.rejectCancel}</Button
						>
						<Button
							variant="danger"
							shape="rounded"
							loading={busy && phase.kind === 'busy' && phase.what === 'reject'}
							disabled={busy}
							onclick={onReject}>{rejectWord}</Button
						>
					</div>
				{:else}
					<button type="button" class="text-button" data-tap onclick={() => (rejecting = true)}
						>{rejectWord}</button
					>
				{/if}
			</div>
		{/if}

		{#if !drafting && item.status !== 'done' && phase.kind !== 'sent' && phase.kind !== 'ended'}
			<p class="fine subtle">{m.proposal.noExpiry}</p>
		{/if}
	{/if}
</Sheet>

<style>
	.starts {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		padding: var(--space-md);
		border: var(--border-hairline) solid var(--color-border-strong);
		border-radius: var(--radius-lg);
	}

	.starts-title {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.starts-members {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	.starts-members li {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
	}

	.blocking {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-md);
	}

	.unsent {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	/* r4D-12: a link-styled button still measures the target floor (--size-targetMin). */
	.external {
		min-height: var(--size-targetMin);
		padding: 0;
		border: 0;
		background: none;
		font: inherit;
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
		text-decoration: underline;
		cursor: pointer;
	}

	.what {
		padding-block-end: var(--space-lg);
		border-bottom: var(--border-hairline) solid var(--color-border-base);
	}

	.title {
		margin: 0 0 var(--space-md);
		font-family: var(--font-display);
		font-size: calc(var(--text-2xl) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
		font-variant-numeric: tabular-nums;
	}

	.to {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		margin: 0 0 var(--space-md);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
	}

	.to-label {
		color: var(--color-fg-muted);
	}

	.to-who {
		color: var(--color-fg-base);
		font-weight: var(--weight-medium);
	}

	.to-address,
	.mono {
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.lines {
		list-style: none;
		margin: 0 0 var(--space-md);
		padding: 0;
	}

	.lines li {
		display: flex;
		flex-wrap: wrap;
		justify-content: space-between;
		padding-block: var(--space-xs);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	.facts {
		display: grid;
		gap: var(--space-xs);
		margin: 0;
	}

	.facts div {
		display: flex;
		justify-content: space-between;
		gap: var(--space-md);
	}

	dt {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	dd {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	h3 {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		margin: var(--space-xl) 0 var(--space-sm);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.count {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-regular);
		color: var(--color-fg-muted);
	}

	.bar {
		height: var(--space-xs);
		border-radius: var(--radius-full);
		background: var(--color-bg-sunken);
		overflow: hidden;
		margin-block-end: var(--space-md);
	}

	.fill {
		display: block;
		height: 100%;
		background: var(--color-success-base);
		transition: width var(--motion-duration-normal);
	}

	.approvals ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	/* r4D-15: a member's short address is already the short form, never cut
	   a second time ("0x08bdcA…f6…"): the row wraps instead — the name line
	   first, the state under it at the end — and a name wraps at its spaces. */
	.approvals li {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-xs) var(--space-md);
		padding-block: var(--space-sm);
	}

	.who {
		flex: 1 1 auto;
		min-width: 0;
		overflow-wrap: break-word;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	/* "· On this device" moves as one, onto the next line if it must. */
	.here {
		white-space: nowrap;
		color: var(--color-fg-muted);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
	}

	.state {
		display: inline-flex;
		align-items: center;
		margin-inline-start: auto;
		gap: var(--space-xs);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	li.done .state {
		color: var(--color-success-text);
	}

	.quiet {
		margin: var(--space-lg) 0 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.line-note {
		flex-basis: 100%;
	}

	.hash-row dd {
		display: flex;
		align-items: flex-start;
		gap: var(--space-xs);
		min-width: 0;
	}

	.hash {
		min-width: 0;
	}

	.hash summary {
		cursor: pointer;
	}

	.hash code {
		display: block;
		overflow-wrap: anywhere;
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
	}

	.hash-copy {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		flex: none;
		/* r4D-12: never smaller than --size-targetMin (the icon alone was 14). */
		min-width: var(--size-targetMin);
		min-height: var(--size-targetMin);
		padding: 0;
		border: none;
		background: none;
		color: var(--color-fg-muted);
		cursor: pointer;
	}

	.warn {
		display: flex;
		gap: var(--space-sm);
		align-items: flex-start;
		margin: var(--space-lg) 0 0;
		padding: var(--space-md);
		border-radius: var(--radius-lg);
		background: var(--color-warning-soft);
		border: var(--border-hairline) solid var(--color-warning-border);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	.actor {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-sm);
		margin: var(--space-xl) 0 0;
		padding: 0;
		border: 0;
	}

	.actor legend {
		margin-block-end: var(--space-sm);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.actor label {
		display: inline-flex;
		align-items: center;
		gap: var(--space-xs);
		padding: var(--space-sm) var(--space-md);
		border-radius: var(--radius-full);
		border: var(--border-hairline) solid var(--color-border-base);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-base);
		cursor: pointer;
	}

	.actor label.on {
		border-color: var(--color-border-strong);
		background: var(--color-bg-sunken);
	}

	.actor input {
		position: absolute;
		opacity: 0;
		pointer-events: none;
	}

	.primary {
		margin-block-start: var(--space-xl);
	}

	/* r4D-19: the desktop's dialog is capped (80% high, 520 wide) and a
	   review is long: its actions sat below the fold. They ride the
	   dialog's bottom edge while the review scrolls under them, and settle
	   in their place at its end. */
	.actions {
		position: sticky;
		/* The dialog's scroller keeps a little room under its content
		   (Dialog's `.content`); the bar covers it, so nothing scrolls
		   past beneath the actions. */
		bottom: calc(var(--space-sm) * -1);
		z-index: 1;
		padding-block: var(--space-md) calc(var(--space-sm) * 2);
		border-top: var(--border-hairline) solid var(--color-border-base);
		background: var(--color-bg-raised);
	}

	.actor-balance {
		color: var(--color-fg-muted);
		font-variant-numeric: tabular-nums;
	}

	.primary > .warn {
		margin-block-end: var(--space-md);
	}

	.setup {
		margin-block-start: var(--space-xs);
		padding-inline-start: var(--space-lg);
	}

	.fine {
		margin: var(--space-sm) 0 0;
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.fine.subtle {
		margin-block-start: var(--space-xl);
		color: var(--color-fg-muted);
	}

	.sent {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		margin-block-start: var(--space-xl);
		padding: var(--space-md);
		border-radius: var(--radius-lg);
		background: var(--color-success-soft);
		color: var(--color-success-text);
		font-weight: var(--weight-medium);
	}

	/* On its way, not on chain: no success colour for what may yet be refused. */
	.sent.pending {
		background: var(--color-bg-sunken);
		color: var(--color-fg-base);
	}

	.two {
		display: flex;
		gap: var(--space-sm);
		margin-block-start: var(--space-xl);
	}

	.primary .two {
		margin-block-start: 0;
	}

	.two > :global(*) {
		flex: 1;
	}

	.site {
		margin: 0 0 var(--space-md);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.data {
		margin-block-start: var(--space-md);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.data summary {
		cursor: pointer;
	}

	.data code {
		display: block;
		margin-block-start: var(--space-sm);
		padding: var(--space-md);
		max-height: calc(var(--space-3xl) * 4);
		overflow: auto;
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
		font-family: var(--font-mono);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		overflow-wrap: anywhere;
	}

	.sent a {
		margin-inline-start: auto;
		color: var(--color-fg-base);
		font-weight: var(--weight-regular);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
	}

	.link-row {
		margin-block-start: var(--space-md);
		padding: var(--space-md);
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
		overflow-wrap: anywhere;
	}

	.link {
		font-family: var(--font-mono);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.link-actions {
		display: flex;
		gap: var(--space-sm);
		margin-block-start: var(--space-md);
	}

	.link-actions > :global(*) {
		flex: 1;
	}

	.label {
		display: inline-flex;
		align-items: center;
		gap: var(--space-sm);
	}

	.reject {
		margin-block-start: var(--space-xl);
		text-align: center;
	}

	.preview {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
		margin-block-start: var(--space-xl);
	}

	.preview-actions {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-sm);
	}

	.preview-actions > :global(*) {
		flex: 1;
	}

	.text-button {
		padding: var(--space-sm);
		border: 0;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-error-base);
		cursor: pointer;
	}
</style>
