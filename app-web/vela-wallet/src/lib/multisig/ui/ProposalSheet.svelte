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
	import { CallReading, callTitle, readableCalls } from '../core/call-review.svelte';
	import { names } from '../core/names.svelte';
	import {
		actionRecipient,
		actionTitle,
		chainName,
		explorerTxUrl,
		formatUnits,
		localName,
		memberChanges,
		shortenAddress,
		tokenInfo
	} from '../core/display';
	import { ExecutorCannotPayError } from '../core/execute';
	import { recheckRecord } from '../core/accounts';
	import { findExecution } from '../core/records';
	import { ChainUnavailableError } from '../core/chain';
	import { executions, watchExecution, type InFlight } from '../core/execution-watch.svelte';
	import { progressLine } from '../core/progress';
	import { multisigUi } from '../core/ui.svelte';
	import { PasskeyCheckError } from '../core/owners';
	import { ServiceUnavailableError } from '../core/service';
	import { PasskeyError } from '$lib/onboarding/core/passkey';
	import type { ExecutionVerdict } from '../core/kernel';
	import { memberDrift, sameAddress, transfersOf } from '../core/kernel';
	import { proposalLink, shareOrigin } from '../core/link';
	import { localOwners, type LocalOwner } from '../core/owners';
	import {
		approve,
		approveExternally,
		draftCalls,
		execute,
		MailboxRefusedError,
		MailboxUnreadableError,
		outbox,
		proposals,
		proposeCancel,
		type ChainState,
		type ProposalItem
	} from '../core/proposals.svelte';
	import type { MultisigAccount } from '../core/store.svelte';
	import type { DappProposal } from '../core/ui.svelte';
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
		source = null,
		watch = null
	}: Props = $props();

	/** The shown multisig's transactions, or the link's own source (ms-21). */
	const store = $derived<ProposalSource>(source ?? proposals);
	// r4b-4: a link's transaction is read again while it is shown, as the
	// shown multisig's are — an execution elsewhere is seen here too.
	$effect(() => source?.watch?.());

	/** A dApp's transaction not proposed yet: the review, before any passkey. */
	const drafting = $derived(!!draft && !draft.safeTxHash);
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

	onMount(() => {
		if (draft && !draft.safeTxHash) void loadDraft();
	});

	const item = $derived(
		drafting
			? (drafted?.item ?? null)
			: (store
					.chain(chainId)
					?.items.find((i) => i.safeTxHash.toLowerCase() === safeTxHash.toLowerCase()) ?? null)
	);
	const members = $derived(
		drafting ? (drafted?.chain.members ?? null) : (store.chain(chainId)?.members ?? null)
	);
	const site = $derived(draft ? hostLabel(draft.origin) : '');

	let title = $state('');
	let lines = $state<{ label: string; to: string }[]>([]);
	$effect(() => {
		const current = item;
		if (!current) return;
		void actionTitle(m, chainId, current.review.action, current.nonce).then((t) => (title = t));
		const action = current.review.action;
		// A batch of transfers alone keeps its lines; one with calls is read
		// call by call below, as the signing sheet reads a batch.
		if (action?.kind === 'batch' && readableCalls(action) === null) {
			void Promise.all(
				transfersOf(action).map(async (t) => {
					const info = await tokenInfo(chainId, t.token).catch(() => ({
						symbol: '',
						decimals: 18
					}));
					return { label: `${formatUnits(t.amount, info.decimals)} ${info.symbol}`, to: t.to };
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
		const calls = readableCalls(item?.review.action ?? null);
		return calls
			? `${chainId}:${account.address}:${calls.map((c) => `${c.to}:${c.value}:${c.data}`).join(';')}`
			: '';
	});
	let reading = $state<CallReading | null>(null);
	$effect(() => {
		if (!callKey) return;
		const { action, chain, address } = untrack(() => ({
			action: item?.review.action,
			chain: chainId,
			address: account.address
		}));
		const calls = readableCalls(action ?? null);
		if (!calls) return;
		const next = new CallReading(chain, address, calls);
		reading = next;
		void next.start();
		return () => {
			next.dispose();
			if (reading === next) reading = null;
		};
	});
	/** A batch's calls into the multisig itself: each one changes who controls it, said loudly. */
	const changes = $derived(memberChanges(m, item?.review.action ?? null));
	/** The core's verb for a call it could read ("Approve"); else the multisig's line. */
	const shownTitle = $derived((reading && callTitle(reading.clear, signing)) || title);

	/** Whose hand does the work: the person picks among this device's members. */
	let chosen = $state<string | null>(null);
	const executors = $derived(item?.canExecute ?? []);
	const approvers = $derived(item?.canApprove ?? []);
	const candidates = $derived(executors.length > 0 ? executors : approvers);
	const actor = $derived<LocalOwner | null>(
		candidates.find((o) => sameAddress(o.address, chosen)) ?? candidates[0] ?? null
	);
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
		| { kind: 'failed'; reason: string };
	let phase = $state<Phase>({ kind: 'idle' });
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
	function reasonOf(error: unknown): string {
		console.warn('[multisig] not done:', error);
		if (error instanceof ExecutorCannotPayError && actor) {
			const symbol = nativeSymbol(chainId);
			return fillTemplate(m.proposal.cannotPay, {
				name: names.label(actor.address),
				symbol,
				fee: `${formatUnits(error.need, 18)} ${symbol}`,
				balance: `${formatUnits(error.have, 18)} ${symbol}`
			});
		}
		if (error instanceof MailboxUnreadableError) {
			return offline() ? m.proposal.reason.offline : m.proposal.mailboxDown;
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

	/**
	 * A dApp's transaction that the proposer's own approval makes ready (a
	 * one-approval multisig, nothing ahead of it): proposed and executed in
	 * one go, so the site gets its transaction — not a wait for approvals
	 * nobody else has to give.
	 */
	const executeNow = $derived(drafting && !!item && item.canExecute.length > 0);

	/** The person confirmed the dApp's transaction: propose it, approved by this device's member. */
	async function onPropose() {
		if (!draft || !item) return;
		const thenExecute = executeNow;
		phase = { kind: 'busy', what: 'propose' };
		try {
			await draft.propose();
			phase = { kind: 'idle' };
		} catch (error) {
			phase = { kind: 'failed', reason: reasonOf(error) };
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
			await store.refresh(account, account.chainIds);
			phase = { kind: 'idle' };
		} catch (error) {
			phase = { kind: 'failed', reason: reasonOf(error) };
		}
	}

	/** The watch of this device's execution in flight; stopped when the sheet goes. */
	let watching: AbortController | null = null;
	$effect(() => () => watching?.abort());

	async function onExecute() {
		if (!item || !actor || !members) return;
		phase = { kind: 'busy', what: 'execute' };
		let result: Awaited<ReturnType<typeof execute>>;
		const by = actor.address;
		const executing = item;
		try {
			result = await execute(account, item, actor, members);
		} catch (error) {
			phase = { kind: 'failed', reason: reasonOf(error) };
			return;
		}
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
		await follow(flight, executing, () => own);
	}

	/**
	 * r4b-1: watch an execution sent from here to its end — the core's
	 * verdict on the chain's nonce, the relay's word and the transaction
	 * that executed it — within seconds of a race lost, bounded always.
	 */
	async function follow(flight: InFlight, executing: ProposalItem, own: () => string | null) {
		phase = { kind: 'sent', txHash: null, by: flight.by };
		const controller = watching ?? new AbortController();
		watching = controller;
		const verdict = await watchExecution(flight, { signal: controller.signal, own });
		if (controller.signal.aborted) return;
		controller.abort();
		watching = null;
		if (verdict.kind !== 'waiting') executions.end(chainId, account.address, flight.safeTxHash);
		await store.refresh(account, account.chainIds).catch(() => {});
		switch (verdict.kind) {
			case 'executed':
				phase = { kind: 'sent', txHash: verdict.txHash, by: flight.by };
				// A rename: the record's new name is read from the chain now.
				if (executing.review.action?.kind === 'rename') void recheckRecord(account.address);
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
	 * Owners still to approve whose wallets are elsewhere: an outside wallet
	 * can sign for any — a queued one too (its approval waits like any other).
	 * Not a Vela account on another device (ms-11): its key is a passkey, and
	 * it approves in Vela, from the link.
	 */
	const outside = $derived(
		!item || item.status === 'done' || item.review.action === null
			? []
			: (members?.owners ?? []).filter(
					(o) =>
						localOwners([o]).length === 0 &&
						!names.isVela(o) &&
						!item.approvals.some((a) => a.counts && sameAddress(a.owner, o))
				)
	);

	async function onExternal() {
		if (!item) return;
		try {
			await approveExternally(account, chainId, item.tx, outside);
			await store.refresh(account, account.chainIds);
		} catch (error) {
			// The outside wallet's own refusal is its sheet's to say.
			if (!cancelled(error)) phase = { kind: 'failed', reason: reasonOf(error) };
		}
	}

	async function onReject() {
		if (!item || !rejecter) return;
		phase = { kind: 'busy', what: 'reject' };
		try {
			await proposeCancel(account, item, rejecter);
			await store.refresh(account, account.chainIds);
			rejecting = false;
			phase = { kind: 'idle' };
		} catch (error) {
			phase = { kind: 'failed', reason: reasonOf(error) };
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
		void findExecution(address, chainId, key, since).then(async (hash) => {
			if (!live) return;
			if (hash === undefined) {
				unreadHistory = true;
				return;
			}
			if (hash !== null) {
				finished = { txHash: hash };
				return;
			}
			const byCancel = cancel ? !!(await findExecution(address, chainId, cancel, since)) : false;
			if (live) finished = { txHash: null, byCancel };
		});
		return () => {
			live = false;
		};
	});

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
				<Button variant="secondary" shape="rounded" onclick={() => draft?.decline()}
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
			<p class="title">{shownTitle || '…'}</p>
			{#if recipient}
				{@const named = reading?.clear.result?.contract_name ?? localName(recipient)}
				<p class="to">
					<Identicon svg={identiconSvgForClient(recipient)} size="compact" address={recipient} />
					<!-- A call names the contract it calls, in the signing sheet's word. -->
					<span class="to-label">{reading ? signing.labelInteracting : m.action.to}</span>
					{#if named}<span class="to-who">{named}</span>{/if}
					<span class="to-address">{shortenAddress(recipient)}</span>
				</p>
			{/if}
			{#if reading}
				<CallReview {reading} m={signing} alerts={changes} />
			{/if}
			{#if lines.length > 0}
				<ul class="lines">
					{#each lines as line, i (i)}
						<li>
							<span>{line.label}</span><span class="to-address">{shortenAddress(line.to)}</span>
						</li>
					{/each}
				</ul>
			{/if}
			<dl class="facts">
				<div>
					<dt>{m.action.network}</dt>
					<dd>{chainName(chainId)}</dd>
				</div>
				<div>
					<dt>{m.action.number}</dt>
					<dd>{item.nonce.toString()}</dd>
				</div>
				<div>
					<dt>{m.proposal.hash}</dt>
					<dd class="mono">{item.safeTxHash.slice(0, 10)}…{item.safeTxHash.slice(-8)}</dd>
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

		{#if item.review.action === null}
			<p class="warn" role="alert">
				<Icon icon={UTILITY_ICONS['triangle-alert']} size="sm" />
				{fillTemplate(m.proposal.refused, { reason: item.review.refusal ?? '' })}
			</p>
		{/if}

		<section class="approvals" aria-label={m.proposal.approvals}>
			<h3>
				{m.proposal.approvals}
				<span class="count"
					>{finished
						? finished.txHash
							? m.proposal.executed
							: m.proposal.voided
						: progressLine(m, shownApproved, item.threshold)}</span
				>
			</h3>
			<div class="bar" aria-hidden="true">
				<span
					class="fill"
					style:width="{finished?.txHash
						? 100
						: Math.min(100, (shownApproved / Math.max(1, item.threshold)) * 100)}%"
				></span>
			</div>
			<ul>
				{#each members?.owners ?? [] as owner (owner)}
					{@const row = item.approvals.find((a) => a.counts && sameAddress(a.owner, owner))}
					{@const executedBy =
						phase.kind === 'sent' && !!phase.txHash && sameAddress(phase.by, owner)}
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
							{:else if !drafting && outside.some( (o) => sameAddress(o, owner) ) && phase.kind === 'idle'}
								<button type="button" class="external" onclick={onExternal}
									>{m.external.signWithWallet}</button
								>
							{:else if item.status !== 'done'}
								{m.proposal.waiting}
							{/if}
						</span>
					</li>
				{/each}
			</ul>
		</section>

		{#if notCounted.length > 0}
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

		{#if drafting}
			<!-- The review of what the site asks: nothing is signed before this. -->
			{#if phase.kind === 'failed'}
				<p class="warn" role="alert" data-testid="proposal-failed">{phase.reason}</p>
			{/if}
			<div class="two">
				<Button variant="secondary" shape="rounded" disabled={busy} onclick={() => draft?.decline()}
					>{m.external.decline}</Button
				>
				{#if item.review.action !== null}
					<Button
						variant="value"
						shape="rounded"
						loading={busy && phase.kind === 'busy' && phase.what === 'propose'}
						disabled={busy}
						onclick={onPropose}>{executeNow ? m.proposal.approveAndExecute : m.send.propose}</Button
					>
				{/if}
			</div>
			{#if executeNow && item.review.action !== null}
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
		{:else if item.status !== 'done' && item.review.action !== null}
			{#if candidates.length > 1}
				<fieldset class="actor">
					<legend>{m.proposal.as}</legend>
					{#each candidates as owner (owner.address)}
						<label class:on={actor?.address === owner.address}>
							<input
								type="radio"
								name="actor"
								value={owner.address}
								checked={actor?.address === owner.address}
								onchange={() => (chosen = owner.address)}
							/>
							{owner.name}
						</label>
					{/each}
				</fieldset>
			{/if}

			{#if phase.kind === 'failed'}
				<p class="warn" role="alert" data-testid="proposal-failed">{phase.reason}</p>
			{/if}

			<div class="primary">
				{#if executors.length > 0 && actor}
					{@const approveFirst = !actorApproved && item.approved < item.threshold}
					<div class:two={approveFirst && canApproveOnly}>
						{#if approveFirst && canApproveOnly}
							<!-- Executing pays the fee; approving alone does not. -->
							<Button
								variant="secondary"
								shape="rounded"
								loading={busy && phase.kind === 'busy' && phase.what === 'approve'}
								disabled={busy}
								onclick={onApprove}
							>
								{m.proposal.approve}
							</Button>
						{/if}
						<Button
							variant="value"
							shape="rounded"
							loading={busy && phase.kind === 'busy' && phase.what === 'execute'}
							disabled={busy}
							onclick={onExecute}
						>
							{approveFirst ? m.proposal.approveAndExecute : m.proposal.execute}
						</Button>
					</div>
					<p class="fine">{m.proposal.feeNote}</p>
				{:else if approvers.length > 0 && actor}
					<Button
						variant="value"
						shape="rounded"
						loading={busy && phase.kind === 'busy' && phase.what === 'approve'}
						disabled={busy}
						onclick={onApprove}
					>
						{m.proposal.approve}
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
		{#if !drafting && item.status !== 'done' && phase.kind !== 'sent' && phase.kind !== 'ended' && shownApproved < item.threshold}
			<section class="invite">
				<h3>{m.proposal.invite}</h3>
				<p class="fine">{m.proposal.inviteHint}</p>
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

		<!-- Any member here may withdraw it, the proposer included, approved or not. -->
		{#if !drafting && item.status !== 'done' && item.review.action?.kind !== 'cancel' && !item.cancelledBy && rejecter && phase.kind !== 'sent' && phase.kind !== 'ended'}
			<div class="reject">
				{#if rejecting}
					<p class="fine">{fillTemplate(m.proposal.rejectHint, { threshold: item.threshold })}</p>
					<Button
						variant="danger"
						shape="rounded"
						loading={busy && phase.kind === 'busy' && phase.what === 'reject'}
						disabled={busy}
						onclick={onReject}>{m.proposal.reject}</Button
					>
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

	.external {
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
		color: var(--color-fg-subtle);
	}

	.to-who {
		color: var(--color-fg-base);
		font-weight: var(--weight-medium);
	}

	.to-address,
	.mono {
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.lines {
		list-style: none;
		margin: 0 0 var(--space-md);
		padding: 0;
	}

	.lines li {
		display: flex;
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
		color: var(--color-fg-subtle);
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

	.approvals li {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		padding-block: var(--space-sm);
	}

	.who {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	.here {
		color: var(--color-fg-subtle);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
	}

	.state {
		display: inline-flex;
		align-items: center;
		gap: var(--space-xs);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	li.done .state {
		color: var(--color-success-base);
	}

	.quiet {
		margin: var(--space-lg) 0 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
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
		color: var(--color-fg-subtle);
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

	.fine {
		margin: var(--space-sm) 0 0;
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.fine.subtle {
		margin-block-start: var(--space-xl);
		color: var(--color-fg-subtle);
	}

	.sent {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		margin-block-start: var(--space-xl);
		padding: var(--space-md);
		border-radius: var(--radius-lg);
		background: var(--color-success-soft);
		color: var(--color-success-base);
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
		color: var(--color-fg-subtle);
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
