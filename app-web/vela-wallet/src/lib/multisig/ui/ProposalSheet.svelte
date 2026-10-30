<script lang="ts">
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
	 * A dApp's transaction (`draft`) is drawn here before it exists: the same
	 * reading, at the number it would take, with "Propose and approve" and
	 * Decline — nothing is signed until the person has seen what the site
	 * asks. Once proposed it is the transaction like any other.
	 *
	 * A link's transaction on a device with none of its members (`source`,
	 * `watch`, ms-21) is read from its own source, the multisig neither
	 * added nor shown: following it here, or signing with a member's wallet,
	 * are offered — each only on the person's tap.
	 */
	import { onMount, tick, untrack } from 'svelte';
	import Button from '$lib/ui/Button.svelte';
	import { hostLabel } from '$lib/dapp/host';
	import { fillTemplate } from '$lib/i18n/fill';
	import { nativeSymbol } from '$lib/services/networks';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import Identicon from '$lib/wallet/ui/Identicon.svelte';
	import { identiconSvgForClient } from '$lib/wallet/identicon';
	import type { SigningMessages } from '$lib/signing/messages';
	import type { MultisigMessages } from '../messages';
	import {
		CallReading,
		callReviewBlocks,
		callTitle,
		readableCalls,
		reviewedCalls
	} from '../core/call-review.svelte';
	import { ProposalReview } from '../core/proposal-review.svelte';
	import {
		anywayLabel,
		partyAddresses,
		recipientBadges,
		recipientNames,
		reviewBlocks,
		reviewTech
	} from '../core/review-blocks';
	import { currency } from '$lib/settings/core/currency.svelte';
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
		tokenInfo
	} from '../core/display';
	import { GenesisRefusedError, type Finding } from '../core/refusal';
	import {
		factsOfExecution,
		frozenFacts,
		rememberExecution,
		type FrozenFacts
	} from '../core/executed';
	import { provenanceNote, type ProvenanceNote } from '$lib/services/token-provenance';
	import ProvenanceNoteView from '$lib/flows/ui/ProvenanceNote.svelte';
	import ProposalTimeline from './ProposalTimeline.svelte';
	import { timelineOf } from '../core/timeline';
	import {
		ExecutionSpentError,
		ExecutionUncheckedError,
		ExecutionUnpricedError,
		ExecutionWouldFailError,
		ExecutorCannotPayError,
		stoppedBeforeSigning,
		type ExecutionPreview
	} from '../core/execute';
	import { feeCoin, feeWords, whyText } from '../core/execution-words';
	import { preflight, PREVIEW_FRESH_MS, UNPRICED_FRESH_MS } from '../core/preflight.svelte';
	import { signers } from '../core/signers.svelte';
	import { defaultActor, lastActor } from '../core/actor';
	import {
		declinedRisk,
		gateOf,
		signingGate,
		SigningBlockedError,
		SigningRefusedError,
		SigningUncheckedError
	} from '../core/gate.svelte';
	import { checkOn } from '../core/recheck';
	import { refusalText } from './findings';
	import AuthorityNotice from './AuthorityNotice.svelte';
	import { findExecution } from '../core/records';
	import { ChainUnavailableError } from '../core/chain';
	import { executions, watchExecution, type InFlight } from '../core/execution-watch.svelte';
	import { progressLine } from '../core/progress';
	import { multisigUi } from '../core/ui.svelte';
	import { PasskeyCheckError } from '../core/owners';
	import { PageApprovalError } from '../core/page-approval.svelte';
	import { PageDeclinedError, PageSignFailedError } from '$lib/signing/sign-challenge';
	import { integrityLineModel } from '$lib/settings/venue';
	import { ServiceUnavailableError } from '../core/service';
	import { PasskeyError } from '$lib/onboarding/core/passkey';
	import type { ExecutionVerdict } from '../core/kernel';
	import {
		executionVerdict,
		memberDrift,
		readMemberChanges,
		sameAddress,
		transfersOf
	} from '../core/kernel';
	import { proposalLink, shareOrigin } from '../core/link';
	import { connectLink } from '../external/connect-link';
	import { eip55 } from '../external/eip55';
	import { localOwners, type LocalOwner } from '../core/owners';
	import {
		approve,
		approveExternally,
		draftCalls,
		execute,
		MailboxRefusedError,
		executionInput,
		MailboxUnreadableError,
		NoOutsideSignerError,
		outbox,
		proposals,
		proposeCancel,
		type ChainState,
		type ProposalItem
	} from '../core/proposals.svelte';
	import { multisig, type MultisigAccount } from '../core/store.svelte';
	import { typedName } from '../core/typed-name';
	import type { DappProposal } from '../core/ui.svelte';
	import type { ReviewFirstRequest } from '../core/review-first.svelte';
	import type { ProposalSource } from '../core/linked.svelte';
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
		/** A dApp's transaction: reviewed here before it is proposed (`DappRequestHost`). */
		draft?: DappProposal | null;
		/**
		 * §3.6: the proposer's own transaction (a transfer, a rename, a member
		 * change), built at its number and reviewed here before its passkey.
		 */
		prebuilt?: ReviewFirstRequest | null;
		/** ms-21: where the transaction is read from when it is not the shown multisig's. */
		source?: ProposalSource | null;
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
		draft = null,
		prebuilt = null,
		source = null,
		watch = null
	}: Props = $props();

	/** The shown multisig's transactions, or the link's own source (ms-21). */
	const store = $derived<ProposalSource>(source ?? proposals);
	// r4b-4: a link's transaction is read again while it is shown, as the
	// shown multisig's are — an execution elsewhere is seen here too.
	$effect(() => source?.watch?.());

	/** A dApp's transaction not proposed yet: the review, before any passkey. */
	const drafting = $derived((!!draft && !draft.safeTxHash) || !!prebuilt);
	let drafted = $state<{ chain: ChainState; item: ProposalItem } | null>(null);
	let draftFailure = $state<string | null>(null);

	async function loadDraft() {
		if (!draft) return;
		draftFailure = null;
		try {
			drafted = await draftCalls(draft.address, chainId, draft.calls);
		} catch (error) {
			draftFailure = reasonOf(error);
		}
	}

	/** This session's check of this network, once it has answered. */
	let checked = $state<Finding[] | null>(null);

	onMount(() => {
		if (draft && !draft.safeTxHash) void loadDraft();
		// §3.5: who can act for it on this network, read now (once a session)
		// — so a refusal or a risk is said before anyone taps.
		void checkOn(account, chainId).then((found) => {
			if (found) checked = found;
		});
	});

	/**
	 * Whether this device signs here: not at all where its genesis is
	 * refused (read-only), only after a confirmation naming the risk where
	 * more than its members can act for it — every signature asks.
	 */
	const gate = $derived(checked ? gateOf(checked, chainId) : signingGate(account, chainId));
	const readOnly = $derived(gate.kind === 'refused');

	const item = $derived(
		drafting
			? (prebuilt?.item ?? drafted?.item ?? null)
			: (store
					.chain(chainId)
					?.items.find((i) => i.safeTxHash.toLowerCase() === safeTxHash.toLowerCase()) ?? null)
	);
	const members = $derived(
		drafting
			? (prebuilt?.chain.members ?? drafted?.chain.members ?? null)
			: (store.chain(chainId)?.members ?? null)
	);
	// r5reg-14: the shown multisig's transaction, still waiting (or not read
	// yet — its network not answering), is read faster while its sheet is
	// open, and when the window comes back: another member's approval, or the
	// network's return, is seen here within seconds.
	const waitingHere = $derived(!source && !drafting && item?.status !== 'done');
	$effect(() => {
		if (waitingHere) return proposals.watch();
	});
	const site = $derived(draft ? hostLabel(draft.origin) : '');

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
				.catch(() => ({ symbol: '', decimals: 18 }))
				.then((info) => (tokenNote = provenanceNote(chainId, token, info.symbol, m.token)));
		} else tokenNote = null;
		// A batch of transfers alone keeps its lines; one with calls is read
		// call by call below, as the signing sheet reads a batch.
		if (action?.kind === 'batch' && readableCalls(action) === null) {
			void Promise.all(
				transfersOf(action).map(async (t) => {
					const info = await tokenInfo(chainId, t.token).catch(() => ({
						symbol: '',
						decimals: 18
					}));
					return {
						label: `${formatUnits(t.amount, info.decimals)} ${info.symbol}`,
						to: t.to,
						note: provenanceNote(chainId, t.token, info.symbol, m.token)
					};
				})
			).then((l) => (lines = l));
		} else lines = [];
	});

	/*
	 * A call to another contract, read by the signing sheet's machines. Keyed
	 * by the call itself: the list is re-read every few seconds, and the same
	 * call must not be read again (and flash "Loading…") each time.
	 */
	const callKey = $derived.by(() => {
		const calls = item ? reviewedCalls(item.review.action, item.tx) : null;
		return calls
			? `${chainId}:${account.address}:${calls.map((c) => `${c.to}:${c.value}:${c.data}`).join(';')}`
			: '';
	});
	let reading = $state<CallReading | null>(null);
	$effect(() => {
		if (!callKey) return;
		const { action, tx, chain, address } = untrack(() => ({
			action: item?.review.action,
			tx: item?.tx,
			chain: chainId,
			address: account.address
		}));
		const calls = tx ? reviewedCalls(action ?? null, tx) : null;
		if (!calls) return;
		const next = new CallReading(chain, address, calls);
		reading = next;
		void next.start();
		return () => {
			next.dispose();
			if (reading === next) reading = null;
		};
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
	const shownTitle = $derived((reading && callTitle(reading.clear, signing)) || title);

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
	let proposalReview = $state<ProposalReview | null>(null);
	$effect(() => {
		if (!reviewKey) return;
		const subject = untrack(() => ({
			safe: account.address,
			chainId,
			tx: item!.tx,
			members: members!,
			genesis: account.genesis,
			origin: draft?.origin ?? null
		}));
		const next = new ProposalReview(subject);
		proposalReview = next;
		void next.refresh();
		// Its hashes and fields are read once for a finished one; nothing is re-checked.
		const done = untrack(() => item?.status === 'done');
		const stop = done ? undefined : next.keepFresh();
		return () => {
			stop?.();
			next.dispose();
			if (proposalReview === next) proposalReview = null;
		};
	});
	/** The core's review now, with the call's reading: blind (`sign_anyway`) or not. */
	const fullReview = $derived(proposalReview?.read(reading?.clear.sign_anyway === true) ?? null);
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
	const drawnParties = $derived(
		reading ? partyAddresses(callReviewBlocks(reading, signing, currency.view)) : []
	);
	const partyBadges = $derived(recipientBadges(fullReview, reviewContext));
	const partyNames = $derived(recipientNames(fullReview, reviewContext));
	const tech = $derived(fullReview ? reviewTech(fullReview, reviewContext) : null);
	/** The review's rule: nothing signed (no override), "… anyway", or plain. */
	const signRule = $derived(fullReview?.sign ?? null);
	const signBlocked = $derived(signRule?.allowed === false);
	const anyway = $derived(signRule?.anyway === true);
	/** The signing sheet holds its confirm while its simulation is out; so does this. */
	const reviewHolding = $derived(!fullReview && proposalReview?.failed !== true);
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
				clear: reading?.clear ?? null,
				guard: reading?.guard ?? null,
				fee: null,
				simulation: proposalReview?.sim ?? null,
				simulation_pending: !proposalReview?.sim
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
	const previewInputs = $derived(
		item && members && !drafting && item.review.action !== null
			? executors.map((o) => executionInput(account, item, o, members))
			: []
	);
	$effect(() => {
		const inputs = previewInputs;
		if (inputs.length === 0) return;
		for (const input of inputs) void preflight.check(input);
		// r5reg-14: asked every 5 s — a priced look still stands its 15 s
		// (`check` keeps a fresh one); one that could not price or check it
		// (an RPC outage) is taken again within 5 s of the network's return.
		const timer = setInterval(() => {
			for (const input of inputs) void preflight.check(input);
		}, UNPRICED_FRESH_MS);
		return () => clearInterval(timer);
	});
	function previewOf(owner: LocalOwner | null): ExecutionPreview | null {
		const input = previewInputs.find(
			(i) => owner && sameAddress(i.executor.address, owner.address)
		);
		return input ? preflight.get(input) : null;
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
	const actor = $derived<LocalOwner | null>(
		candidates.find((o) => sameAddress(o.address, chosen)) ??
			payer ??
			defaultActor(candidates, account.address) ??
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
		for (const input of previewInputs) void preflight.check(input, 0);
	}
	const actorApproved = $derived(
		!!item && !!actor && item.approvals.some((a) => a.counts && sameAddress(a.owner, actor.address))
	);

	/**
	 * Who can withdraw it: any member on this device, whether or not it has
	 * approved — the proposer above all. Rejecting proposes a cancellation at
	 * its number, which that member approves at once.
	 */
	const rejecters = $derived(localOwners(members?.owners ?? []));
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
		past ? [] : reviewBlocks(fullReview, proposalReview?.sim ?? null, reviewContext, drawnParties)
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
			proposalReview?.refresh();
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
		if (error instanceof ServiceUnavailableError) {
			return offline() ? m.proposal.reason.offline : m.proposal.reason.serviceKept;
		}
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
	 * nobody else has to give.
	 */
	const executeNow = $derived(drafting && !prebuilt && !!item && item.canExecute.length > 0);

	/** The person confirmed the dApp's transaction: propose it, approved by this device's member. */
	async function onPropose() {
		// §3.6: the proposer's own transaction goes on to its passkey (the host closes this).
		// r5reg-5: the member shown under "As" proposes it — never another, silently.
		if (prebuilt) {
			phase = { kind: 'busy', what: 'propose' };
			prebuilt.confirm(actor?.address ?? null);
			return;
		}
		if (!draft || !item) return;
		const thenExecute = executeNow;
		phase = { kind: 'busy', what: 'propose' };
		try {
			await draft.propose(actor?.address ?? null);
			phase = { kind: 'idle' };
		} catch (error) {
			phase = failedPhase(error);
			return;
		}
		// The proposal as the mailbox has it now: executed if it is ready.
		await tick();
		if (thenExecute && item?.status === 'ready' && actor) await onExecute();
	}

	async function onApprove() {
		if (!item || !actor) return;
		phase = { kind: 'busy', what: 'approve' };
		try {
			await approve(account, item, actor);
			if (candidates.length > 1) lastActor.note(account.address, actor.address);
			// r5reg-14: read after the approval — never a read begun before it.
			await store.refresh(account, account.chainIds, { fresh: true });
			phase = { kind: 'idle' };
		} catch (error) {
			phase = failedPhase(error);
		}
	}

	/** The watch of this device's execution in flight; stopped when the sheet goes. */
	let watching: AbortController | null = null;
	$effect(() => () => watching?.abort());

	async function onExecute() {
		if (!item || !actor || !members) return;
		// "Approve and execute": should only the execute part be unable to run
		// (before its passkey — a fee it cannot pay, a transaction that would
		// fail), the approval is still given and kept (r4-11).
		const approving =
			!actorApproved &&
			item.approved < item.threshold &&
			approvers.some((o) => sameAddress(o.address, actor.address));
		phase = { kind: 'busy', what: 'execute' };
		let result: Awaited<ReturnType<typeof execute>>;
		const by = actor.address;
		const executing = item;
		let sent: ReturnType<typeof factsOfExecution> | undefined;
		try {
			// The look on screen when it is fresh — its calls and fee are what is
			// signed — else a new one, said the same way. One that could not
			// check (or price) it is never reused: Execute looks again before
			// its passkey (round 5), bounded like every look.
			const input = executionInput(account, item, actor, members);
			const shown = preflight.get(input);
			const preview = await preflight.check(input, shown?.kind === 'priced' ? PREVIEW_FRESH_MS : 0);
			// What this execution counts, as it is sent — the approvals its
			// calls carry (r7-3): kept, so the transaction reads its own facts
			// later, never today's owners re-counted (r4a-8).
			if (preview.kind === 'priced') {
				sent = factsOfExecution(preview.carries, by, preview.executorApproves, item.threshold);
			}
			result = await execute(account, item, actor, members, preview);
		} catch (error) {
			// r5reg-21: its number was used meanwhile — what ran is said, not a failure.
			if (error instanceof ExecutionSpentError) {
				phase = { kind: 'idle' };
				await settleSpent(executing, by);
				return;
			}
			if (approving && stoppedBeforeSigning(error)) {
				const reason =
					error instanceof ExecutionWouldFailError ? whyText(m, error.why) : reasonOf(error);
				phase = { kind: 'busy', what: 'approve' };
				try {
					await approve(account, item, actor);
					await store.refresh(account, account.chainIds, { fresh: true });
					phase = { kind: 'partial', reason };
				} catch (again) {
					phase = failedPhase(again);
				}
				return;
			}
			phase = failedPhase(error);
			return;
		}
		if (candidates.length > 1) lastActor.note(account.address, by);
		// r4b-7: written down the moment the relay has it — a reload resumes
		// watching it, and it is not offered again while it may land.
		const flight = executions.start({
			chainId,
			multisig: account.address,
			safeTxHash: executing.safeTxHash,
			nonce: executing.nonce.toString(),
			userOpHash: result.userOpHash,
			by,
			maybeSent: result.maybeSent,
			startedMs: Date.now(),
			createdMs: executing.createdMs
		});
		// The submit's own wait for its receipt, beside the watch.
		let own: string | null = null;
		void result
			.waitForTxHash(180_000, (watching = new AbortController()).signal)
			.then((hash) => (own = hash))
			.catch(() => {});
		await follow(flight, executing, () => own, sent);
	}

	/**
	 * r4b-1: watch an execution sent from here to its end — the core's
	 * verdict on the chain's nonce, the relay's word and the transaction
	 * that executed it — within seconds of a race lost, bounded always.
	 */
	async function follow(
		flight: InFlight,
		executing: ProposalItem,
		own: () => string | null,
		sent?: ReturnType<typeof factsOfExecution>
	) {
		phase = { kind: 'sent', txHash: null, by: flight.by };
		const controller = watching ?? new AbortController();
		watching = controller;
		const verdict = await watchExecution(flight, { signal: controller.signal, own });
		if (controller.signal.aborted) return;
		controller.abort();
		watching = null;
		if (verdict.kind !== 'waiting') executions.end(chainId, account.address, flight.safeTxHash);
		await store.refresh(account, account.chainIds, { fresh: true }).catch(() => {});
		switch (verdict.kind) {
			case 'executed':
				if (sent) rememberExecution(chainId, account.address, flight.safeTxHash, sent);
				phase = { kind: 'sent', txHash: verdict.txHash, by: flight.by };
				// A rename: the record's new name is read from the chain now.
				if (executing.review.action?.kind === 'rename')
					multisig.send({ type: 'recheck_record', address: account.address });
				onexecuted?.({ item: executing, txHash: verdict.txHash, userOpHash: flight.userOpHash });
				// A dApp's transaction: its site gets the transaction, now.
				draft?.executed(verdict.txHash);
				return;
			case 'executedByOther':
				phase = { kind: 'ended', verdict, by: flight.by };
				// Executed all the same: the site waiting for it gets its hash.
				draft?.executed(verdict.txHash);
				return;
			case 'notSent':
				phase = { kind: 'failed', reason: m.proposal.notSent };
				return;
			case 'timedOut':
				phase = { kind: 'failed', reason: m.proposal.notLanded };
				return;
			case 'waiting':
				return;
			default:
				phase = { kind: 'ended', verdict, by: flight.by };
		}
	}

	// r4b-7: an execution this device sent before a reload (or with the sheet
	// closed) is watched again — never offered as if nothing was sent.
	const resumeKey = $derived(item && !drafting ? `${chainId}:${item.safeTxHash}` : '');
	$effect(() => {
		if (!resumeKey) return;
		untrack(() => {
			if (!item || phase.kind !== 'idle' || item.status === 'done') return;
			const flight = executions.get(chainId, account.address, item.safeTxHash);
			if (flight) void follow(flight, item, () => null);
		});
	});

	/*
	 * r5reg-21: the look found its number used on the chain — another member
	 * executed it (or another transaction at its number ran) while this sheet
	 * was open. Read what ran and say it the way a lost race is said — never
	 * "it would fail" — and refresh the list, so the sheet becomes the
	 * executed one.
	 */
	let settling: string | null = null;
	async function settleSpent(spent: ProposalItem, by: string) {
		if (settling === spent.safeTxHash) return;
		settling = spent.safeTxHash;
		try {
			const found = await findExecution(account.address, chainId, spent.safeTxHash, {
				sinceMs: spent.createdMs,
				final: true,
				nonce: spent.nonce
			});
			const verdict = await executionVerdict({
				nonceUsed: true,
				executedIn:
					found === undefined
						? { kind: 'unknown' }
						: found === null
							? { kind: 'none' }
							: { kind: 'found', txHash: found },
				// Nothing of this device's was sent.
				ourOp: { kind: 'unknown' },
				elapsedMs: 0,
				limitMs: 0
			});
			if (phase.kind !== 'idle' || item?.safeTxHash !== spent.safeTxHash) return;
			phase = { kind: 'ended', verdict, by };
			if (verdict.kind === 'executedByOther') draft?.executed(verdict.txHash);
			await store.refresh(account, account.chainIds, { fresh: true }).catch(() => {});
		} finally {
			settling = null;
		}
	}
	$effect(() => {
		if (actorPreview?.kind !== 'spent' || !item || !actor) return;
		const spent = item;
		const by = actor.address;
		untrack(() => {
			if (phase.kind === 'idle' && spent.status !== 'done') void settleSpent(spent, by);
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

	/** Owners still to approve, on no device here — while their approval is still needed. */
	const missingOwners = $derived(
		!item || item.status === 'done' || item.review.action === null || readOnly
			? []
			: (members?.owners ?? []).filter(
					(o) => !item.approvals.some((a) => a.counts && sameAddress(a.owner, o))
				)
	);
	/** Enough members approved: nobody else's approval is needed (r4D-6). */
	const enough = $derived(!!item && item.approved >= item.threshold);
	/** The proposals read here: a passkey approval on any of them says a Vela account (r4a-9). */
	const readItems = $derived([
		...(store.chain(chainId)?.items ?? []),
		...(sameAddress(proposals.followed, account.address) ? proposals.items : [])
	]);
	/**
	 * How each member still to approve signs (`signers.kindOf`) — code at its
	 * address on any of the multisig's networks says a Vela account, over the
	 * kind kept for it (r7: evidence beats the label).
	 */
	const kinds = $derived(
		new Map(missingOwners.map((o) => [o, signers.kindOf(chainId, o, readItems, account.chainIds)]))
	);
	/**
	 * Owners still to approve whose wallets are elsewhere: an outside wallet
	 * can sign for a plain account — a queued transaction too (its approval
	 * waits like any other). Never for a Vela account (ms-11, r4-9): one the
	 * index knows, one that approved with its passkey or this device keeps as
	 * one (r4a-9) — its key is a passkey, and it approves in Vela, from the
	 * link. Never for a smart contract account either (r7: code at its
	 * address, no index record) — it approves from its own app, which Vela
	 * can't collect yet, and that is said (`contractMissing`). One nothing says
	 * (`unsure`) is offered the Vela way first and the outside wallet's beside
	 * it — never alone: its row's "Send link", then "Approve with another
	 * wallet" (r7, restored beside it). Nothing is offered once enough have
	 * approved.
	 */
	const outside = $derived(
		enough
			? []
			: missingOwners.filter((o) => kinds.get(o) === 'wallet' || kinds.get(o) === 'unsure')
	);
	/** Vela members elsewhere still to approve: they approve in Vela, from the link. */
	const elsewhere = $derived(
		enough ? [] : missingOwners.filter((o) => kinds.get(o) === 'vela' || kinds.get(o) === 'unsure')
	);

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
	/** Members still to approve who approve in Vela (not this device's, not surely outside wallets). */
	const velaMissing = $derived(
		!item
			? []
			: (members?.owners ?? []).filter(
					(o) =>
						localOwners([o]).length === 0 &&
						kinds.get(o) !== 'contract' &&
						(!outside.some((x) => sameAddress(x, o)) || elsewhere.some((x) => sameAddress(x, o))) &&
						!item.approvals.some((a) => a.counts && sameAddress(a.owner, o))
				)
	);
	/**
	 * r7: smart contract members still to approve — they approve from their
	 * own app (EIP-1271), which Vela can't collect yet: said, and neither the
	 * Vela link nor an outside wallet offered for them.
	 */
	const contractMissing = $derived(
		enough ? [] : missingOwners.filter((o) => kinds.get(o) === 'contract')
	);
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
		try {
			await approveExternally(account, chainId, item.tx, outside);
			await store.refresh(account, account.chainIds, { fresh: true });
		} catch (error) {
			// The outside wallet's own refusal is its sheet's to say.
			if (!cancelled(error)) phase = failedPhase(error);
		}
	}

	async function onReject() {
		if (!item || !rejecter) return;
		phase = { kind: 'busy', what: 'reject' };
		try {
			await proposeCancel(account, item, rejecter);
			await store.refresh(account, account.chainIds, { fresh: true });
			rejecting = false;
			phase = { kind: 'idle' };
		} catch (error) {
			phase = failedPhase(error);
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
	$effect(() => {
		const key = doneKey;
		void historyTry;
		finished = null;
		unreadHistory = false;
		if (!key) return;
		let live = true;
		const address = untrack(() => account.address);
		const cancel = untrack(() => item?.cancelledBy ?? null);
		// Bounded by when it was proposed, and final: its number is used.
		const since = untrack(() => ({
			sinceMs: item?.createdMs ?? 0,
			final: true,
			nonce: item?.nonce
		}));
		let again: ReturnType<typeof setTimeout> | undefined;
		void findExecution(address, chainId, key, since).then(async (hash) => {
			if (!live) return;
			if (hash === undefined) {
				unreadHistory = true;
				// Round 5: a node a few blocks behind cannot say yet — asked
				// again by itself a few times (Retry stays on screen).
				if (autoTries < HISTORY_AUTO_TRIES) {
					autoTries += 1;
					again = setTimeout(() => (historyTry += 1), HISTORY_AUTO_MS);
				}
				return;
			}
			autoTries = 0;
			if (hash !== null) {
				finished = { txHash: hash };
				return;
			}
			const byCancel = cancel ? !!(await findExecution(address, chainId, cancel, since)) : false;
			if (live) finished = { txHash: null, byCancel };
		});
		return () => {
			live = false;
			clearTimeout(again);
		};
	});

	/**
	 * r4a-8: an executed transaction's facts as they were when it ran — whose
	 * approvals counted, the threshold then, who sent it — read once its
	 * execution is known; never today's owners re-counted.
	 */
	const executedTx = $derived(
		phase.kind === 'sent' && phase.txHash ? phase.txHash : (finished?.txHash ?? null)
	);
	const frozenKey = $derived(executedTx && item ? `${item.safeTxHash}:${executedTx}` : '');
	let frozen = $state<FrozenFacts | null>(null);
	$effect(() => {
		const key = frozenKey;
		frozen = null;
		if (!key) return;
		const [hash, tx] = key.split(':');
		let live = true;
		const address = untrack(() => account.address);
		void frozenFacts(chainId, address, hash, tx).then((facts) => {
			if (live) frozen = facts;
		});
		return () => {
			live = false;
		};
	});
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
	const kept = $derived(
		item?.unsent ? outbox.stateOf(chainId, account.address, item.safeTxHash) : null
	);
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
		try {
			await outbox.sendAgain({ chainId, safe: account.address, hash: item.safeTxHash });
			await store.refresh(account, account.chainIds);
		} finally {
			resending = false;
		}
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
				<Button variant="secondary" shape="rounded" onclick={() => (prebuilt ?? draft)?.decline()}
					>{m.external.decline}</Button
				>
				<Button variant="secondary" shape="rounded" onclick={() => void loadDraft()}
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
		{#if draft}
			<p class="site">{fillTemplate(m.proposal.siteAsks, { host: site })}</p>
		{/if}
		<header class="what">
			{#if !reading}
				<!-- r5reg-15: a call read by the signing sheet's machines heads itself
				     ("Send −0.05 USDC"): its verb is not said twice above it. -->
				<p class="title">{shownTitle || '…'}</p>
			{/if}
			{#if recipient && !(reading && item.review.action?.kind === 'transfer')}
				<!-- A transfer read by the signing sheet's machines names its own recipient (§3.6). -->
				{@const named = reading?.clear.result?.contract_name ?? localName(recipient)}
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
				<!-- r4a-1: who it adds, removes or replaces, the rule before and
				     after, on which network — said before anything else. -->
				<section class="change" data-testid="member-change">
					{#if item.status !== 'done'}
						<p class="change-caution" role="alert">
							<Icon icon={UTILITY_ICONS['triangle-alert']} size="sm" />
							<span>{change.caution}</span>
						</p>
					{/if}
					<ul class="change-list">
						{#each memberReading.changes as c, i (i)}
							<li>
								<span class="change-what">{change.changes[i]}</span>
								{#each [c.removed, c.added].filter((a) => a !== null) as address, j (j)}
									<span class="change-who">
										<Identicon
											svg={identiconSvgForClient(address ?? '')}
											size="compact"
											address={address ?? ''}
										/>
										<code class="change-address">{eip55(address ?? '')}</code>
									</span>
								{/each}
							</li>
						{/each}
					</ul>
					{#if item.status !== 'done'}
						<p class="change-rule" data-testid="member-change-rule">{change.rule}</p>
						{#if change.doesNotFit}
							<p class="warn" role="alert">{change.doesNotFit}</p>
						{/if}
					{/if}
				</section>
			{/if}
			{#if reading}
				<CallReview {reading} m={signing} badges={partyBadges} names={partyNames} />
			{/if}
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
			<ReviewTech {m} review={fullReview} {tech} advice={adviceModel} />
		{/if}

		{#if gate.kind === 'refused'}
			<!-- §3.5: its genesis refused here — read-only, nothing to sign. -->
			<AuthorityNotice {m} kind="refused" findings={gate.findings} readOnlyOn={[chainId]} />
		{:else if gate.kind === 'risk'}
			<!-- §3.5: more than its members can act for it here — each signature asks. -->
			<AuthorityNotice {m} kind="risk" findings={gate.findings} />
		{/if}

		{#if item.review.action === null}
			<p class="warn" role="alert">
				<Icon icon={UTILITY_ICONS['triangle-alert']} size="sm" />
				{fillTemplate(m.proposal.refused, { reason: item.review.refusal ?? '' })}
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
							{:else if !drafting && elsewhere.some((o) => sameAddress(o, owner))}
								<!-- A Vela member elsewhere approves in Vela, from the link (r4-9);
								     one that may be either is offered the Vela way first (r4a-9)
								     and its own wallet's beside it (r7). -->
								{m.proposal.waiting} ·
								<button
									type="button"
									class="external"
									title={fillTemplate(m.proposal.approvesInVela, { names: names.label(owner) })}
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
					names: contractMissing.map((o) => names.label(o)).join(', ')
				})}
			</p>
		{/if}

		{#if notCounted.length > 0 && item.status !== 'done'}
			<!-- The mailbox takes any valid signature; only this network's members count. -->
			<p class="fine" data-testid="not-counted">
				{fillTemplate(m.proposal.notCounted, {
					network: chainName(chainId),
					names: notCounted.map((o) => names.label(o)).join(', ')
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
						networks: startsOriginal.map((id) => chainName(id)).join(', ')
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
				{#if kept?.retryAt}
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
				<Button variant="secondary" shape="rounded" onclick={() => (historyTry += 1)}
					>{m.service.retry}</Button
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
							nonces: blocking.map((b) => `#${b.nonce}`).join(', ')
						})
					: fillTemplate(m.proposal.queuedNote, {
							nonce: (blocking[0]?.nonce ?? store.chain(chainId)?.members.nonce ?? 0n).toString()
						})}
			</p>
			{#if !source && blocking.some((b) => b.hash)}
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
			<ProposalTimeline
				{m}
				{chainId}
				steps={timelineOf(item, executedTx, frozen)}
				label={(a) => names.label(a)}
			/>
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
				<!-- r4D-13: a site's request is declined; the person's own proposal is withdrawn. -->
				<Button
					variant="secondary"
					shape="rounded"
					disabled={busy}
					onclick={() => (prebuilt ?? draft)?.decline()}
					>{prebuilt ? m.proposal.withdraw : m.external.decline}</Button
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
						historyTry += 1;
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
					{@const approveFirst = !actorApproved && item.approved < item.threshold}
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
								disabled={busy || reviewHolding || actorPreview === null}
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

		{#if draft && !drafting && item.status !== 'done'}
			<!-- A batch's site was given its id; a transaction's waits for its hash
			     (until the request's limit, when it is told it was proposed). -->
			{#if draft.method !== 'wallet_sendCalls' && !draft.siteTold && shownApproved < item.threshold}
				<p class="fine" role="status">
					{fillTemplate(m.proposal.stillNeeded, { missing: item.threshold - shownApproved })}
				</p>
			{/if}
			<p class="fine">
				{fillTemplate(
					draft.method === 'wallet_sendCalls'
						? m.proposal.siteFollows
						: draft.siteTold
							? m.proposal.siteTold
							: m.proposal.siteWaits,
					{ host: site }
				)}
			</p>
		{/if}

		<!-- Only while approvals are missing: not once it is being executed. -->
		{#if !drafting && item.status !== 'done' && phase.kind !== 'sent' && phase.kind !== 'ended' && shownApproved < item.threshold && (velaMissing.length > 0 || (outside.length === 0 && contractMissing.length === 0))}
			<section class="invite">
				<h3>{m.proposal.invite}</h3>
				<p class="fine">{m.proposal.inviteHint}</p>
				{#if elsewhere.length > 0}
					<!-- Vela members elsewhere: never an outside wallet — the link, in Vela (r4-9). -->
					<p class="fine" data-testid="approves-in-vela">
						{fillTemplate(m.proposal.approvesInVela, {
							names: elsewhere.map((o) => names.label(o)).join(', ')
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

		{#if !drafting && item.status !== 'done' && phase.kind !== 'sent' && phase.kind !== 'ended' && shownApproved < item.threshold && outsideLink}
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
					<p class="fine">{fillTemplate(m.proposal.rejectHint, { threshold: item.threshold })}</p>
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
							onclick={onReject}>{m.proposal.reject}</Button
						>
					</div>
				{:else}
					<button type="button" class="text-button" data-tap onclick={() => (rejecting = true)}
						>{m.proposal.reject}</button
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

	/* r4a-1: a member change, read before anything else. */
	.change {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
		margin: var(--space-md) 0 var(--space-lg);
		padding: var(--space-md);
		border-radius: var(--radius-lg);
		background: var(--color-error-soft);
	}

	.change-caution {
		display: flex;
		gap: var(--space-sm);
		align-items: flex-start;
		margin: 0;
		font-weight: var(--weight-semibold);
		color: var(--color-error-base);
	}

	.change-list {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.change-list li {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	.change-what {
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.change-who {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		min-width: 0;
	}

	.change-address {
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		overflow-wrap: anywhere;
	}

	.change-rule {
		margin: 0;
		font-variant-numeric: tabular-nums;
		color: var(--color-fg-base);
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
