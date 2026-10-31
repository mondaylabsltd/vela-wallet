<script lang="ts">
	/**
	 * The Wallet side of an outside owner's session: this page joins the room
	 * in its link, and forwards what Vela asks — the SafeTx's typed data, a
	 * transaction, a network switch — to the wallet in this browser, which
	 * asks the owner. It keeps its sessions, so Vela can ask the same wallet
	 * again: reopening this page resumes them.
	 *
	 * The core's page machine (`multisig_outside::page`, `OutsidePageHost`)
	 * decides all of it — what a request is, whether it is one of the link's
	 * members, how each ended, what the link's transaction is and who may act
	 * on it, what was signed and kept — and this page draws its view and says
	 * it in the corpus' words.
	 *
	 * ms-14: the link names the members Vela asks for (in its fragment, like
	 * the invite). The page says which account is connecting, and refuses —
	 * loudly, with what to do — an account that is not one of them.
	 *
	 * r4C-7 / r4D-2: nothing is signed blind. A request is shown first — which
	 * multisig, the network, what it does as the core reads it, its number,
	 * the hashes a hardware wallet shows — and goes to the wallet on the
	 * owner's tap; then "Signed ✓ — you can close this page", and the list of
	 * requests says how each ended (r4C-12). r4C-3: a wallet that does not
	 * answer is timed and said. r4C-1/4/15: the relay, and Vela's side, are
	 * said when they are not there — never an endless "Waiting for Vela…".
	 *
	 * r4C-2 / r4C-9: a link that names the transaction (`ms=`) lets the owner
	 * approve, execute or reject it from here alone: read from the mailbox,
	 * checked by the core, signed in the wallet.
	 *
	 * Round 6: a new link opened in this tab is read at once (r5reg-2); one
	 * transaction card at a time, and none for a transaction the pairing has
	 * handled or whose number is used (r5reg-7); one message for where the page
	 * stands — "Signed ✓" on a paired page says to keep it open, never also to
	 * close it; nothing to sign once the approvals are enough (r4C-14); Execute
	 * is not the main action for a wallet that cannot pay its fee (r5reg-13);
	 * the mailbox's refusals in plain words, what was signed kept and sent
	 * again (r5reg-12); each request named by what it was (r4C-12).
	 */
	import { replaceUrl } from '$lib/ui/history';
	import { onMount, untrack } from 'svelte';
	import Button from '$lib/ui/Button.svelte';
	import BrandMark from '$lib/ui/BrandMark.svelte';
	import { loadCore } from '$lib/core/client';
	import { eip55 } from '$lib/multisig/external/eip55';
	import { shortenAddress } from '$lib/wallet/identity';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { chainName, nativeSymbol } from '$lib/services/networks';
	import { discoverProviders, type ProviderDetail } from '$lib/multisig/external/eip6963';
	import { problemText } from '$lib/multisig/external/errors';
	import {
		connectLink,
		isExpected,
		OutsidePageHost,
		readConnectHash
	} from '$lib/multisig/shell/outside-page.svelte';
	import type { OutsideActivityEntry } from '$lib/multisig/generated/OutsideActivityEntry';
	import type { OutsideStep } from '$lib/multisig/generated/OutsideStep';
	import type { OutsideRefusalKind } from '$lib/multisig/generated/OutsideRefusalKind';
	import type { RelayStatus } from '$lib/multisig/generated/RelayStatus';
	import TxReview from '$lib/multisig/external/TxReview.svelte';
	import AuthorityNotice from '$lib/multisig/ui/AuthorityNotice.svelte';
	import { findingLines } from '$lib/multisig/ui/findings';
	import type { Finding } from '$lib/multisig/core/refusal';
	import { explorerTxUrl, formatUnits } from '$lib/multisig/core/display';
	import { rowLabelText, rowOutcomeText } from '$lib/multisig/external/words';
	import type { OutsideTxShown } from '$lib/multisig/generated/OutsideTxShown';
	import {
		safeTxHashes,
		verifyEcdsaApproval,
		type SafeTx,
		type SafeTxHashes
	} from '$lib/multisig/core/kernel';
	import { fillTemplate } from '$lib/i18n/fill';

	let { data } = $props();
	const m = $derived(data.multisigMessages.external);
	const all = $derived(data.multisigMessages);

	/** r4C-3: a wallet still silent after this long is said to be waiting, maybe behind this window. */
	const STILL_WAITING_MS = 20_000;
	/** r4C-4: Vela's side absent this long is said, not waited on in silence. */
	const VELA_ABSENT_MS = 10_000;
	/** r5reg-7: the link's transaction read again this often while it is shown, so its count is now's. */
	const REREAD_MS = 30_000;

	type ActivityEntry = OutsideActivityEntry;

	let providers = $state<ProviderDetail[]>([]);
	let discovered = $state(false);
	let coreReady = $state(false);
	let chosen = $state<ProviderDetail | null>(null);
	/** The core's page machine, once the core is loaded. */
	let host = $state<OutsidePageHost | null>(null);
	/** The wallet picked and handed to the machine. */
	let picked = $state(false);
	let invite: string | null = null;
	/** Whether this page came with an invite: "Connecting to Vela…" until it is in the room. */
	let invitedLink = $state(false);
	/** ms-14: the members Vela asked for (the link's), lowercase; empty for an older link. */
	let members = $state<string[]>([]);
	let link = $state('');
	let copied = $state(false);
	/** Round 6: the link's invite does not read — said as the link's, never the wallet's. */
	let damagedHash = $state(false);
	/** r4C-2: the link names a transaction, read and acted on from here. */
	let linked = $state(false);
	let now = $state(Date.now());

	onMount(() => {
		const timer = setInterval(() => (now = Date.now()), 1000);
		// r5reg-7: the link's transaction, read again while it is shown and idle.
		const reread = setInterval(() => {
			if (host && linked && step.kind === 'idle' && document.visibilityState === 'visible') {
				host.dispatch({ type: 'refresh' });
			}
		}, REREAD_MS);
		// r5reg-2: a new link opened in this tab — same page, another fragment.
		const opened = () => openLink(location.hash);
		window.addEventListener('hashchange', opened);
		void boot();
		return () => {
			clearInterval(timer);
			clearInterval(reread);
			window.removeEventListener('hashchange', opened);
			host?.dispose();
		};
	});

	const view = $derived(host?.view ?? null);

	/**
	 * What a link names, taken in: its members, the room's invite (out of the
	 * address bar and history — a bearer credential until paired; the members
	 * and the transaction stay, so a reload still has them), and its
	 * transaction, read afresh — never the one an earlier link named.
	 * Returns the invite, if it has one.
	 */
	function readLink(machine: OutsidePageHost, hash: string): string | null {
		const read = readConnectHash(hash);
		if (read.damaged) {
			damagedHash = true;
			invitedLink = false;
		}
		if (!read.invite && !read.tx) return null;
		damagedHash = read.damaged;
		members = read.members;
		if (read.invite) {
			invitedLink = true;
			link = location.href;
			replaceUrl(connectLink(location.pathname, null, members, read.tx));
		}
		const current = machine.view.link;
		const same =
			!!current &&
			linked &&
			!!read.tx &&
			current.chainId === read.tx.chainId &&
			current.safe.toLowerCase() === read.tx.multisig &&
			current.safeTxHash.toLowerCase() === read.tx.safeTxHash;
		rejecting = false;
		if (same) {
			// The same transaction opened again: read now, as it stands.
			if (step.kind === 'idle') machine.dispatch({ type: 'refresh' });
			return read.invite;
		}
		linked = !!read.tx;
		if (read.tx) {
			machine.dispatch({
				type: 'link',
				link: {
					chainId: read.tx.chainId,
					safe: read.tx.multisig,
					safeTxHash: read.tx.safeTxHash,
					forMembers: members
				}
			});
		}
		return read.invite;
	}

	/** r5reg-2: another link in the same tab: its transaction and its room, now. */
	function openLink(hash: string) {
		if (!host) return;
		const fresh = readLink(host, hash);
		if (!fresh) return;
		if (picked) joinRoom(host, fresh);
		else invite = fresh;
	}

	/**
	 * Round 6: the link's room, joined. An invite that does not read is the
	 * link's fault, said as such ("This link is damaged — ask for a new one"),
	 * never "the wallet couldn't sign: invalid invite" — and nothing waits on
	 * Vela for it (`joinFailed`).
	 */
	function joinRoom(machine: OutsidePageHost, fresh: string) {
		damagedHash = false;
		machine.dispatch({ type: 'join', invite: fresh, members });
	}

	async function boot() {
		const finding = discoverProviders();
		await loadCore().catch(() => {});
		const machine = new OutsidePageHost();
		host = machine;
		coreReady = true;
		invite = readLink(machine, location.hash);
		providers = await finding;
		discovered = true;
		if (providers.length === 1) await start(providers[0]);
	}

	async function start(detail: ProviderDetail) {
		if (!host) return;
		await host.useWallet(detail.provider, detail.info.name);
		chosen = detail;
		picked = true;
		if (invite) joinRoom(host, invite);
		invite = null;
	}

	// Keep the screen on while a session is open: a phone that sleeps drops the page.
	let wakeLock: { release(): Promise<void> } | undefined;
	async function holdScreen() {
		try {
			const nav = navigator as unknown as {
				wakeLock?: { request(type: 'screen'): Promise<{ release(): Promise<void> }> };
			};
			if (!wakeLock && nav.wakeLock && document.visibilityState === 'visible' && sessions.length) {
				wakeLock = await nav.wakeLock.request('screen');
			}
		} catch {
			// not offered: the page works while it stays in front
		}
	}
	const sessionCount = $derived(view?.sessions.length ?? 0);
	$effect(() => {
		if (sessionCount > 0) void holdScreen();
		const again = () => {
			wakeLock = undefined;
			void holdScreen();
		};
		document.addEventListener('visibilitychange', again);
		return () => document.removeEventListener('visibilitychange', again);
	});

	const sessions = $derived(view?.sessions ?? []);
	const prompt = $derived(view?.connectPrompt ?? null);
	const request = $derived(view?.request ?? null);
	const activity = $derived(view?.activity ?? []);
	const account = $derived(view?.accounts[0] ?? null);
	const network = $derived(view?.walletChainId ? chainName(view.walletChainId) : '');
	const relay = $derived<RelayStatus>(view?.relay ?? { kind: 'up' });
	/** Round 6: the invite did not open a room (PeerConnect refused it): the link's. */
	const damagedLink = $derived(damagedHash || !!view?.joinFailed);
	const invited = $derived(invitedLink && !view?.joinFailed);
	const walletName = $derived(chosen?.info.name || m.ownerWallet);
	const live = $derived(sessions.filter((s) => s.approved));
	/** ms-14: the wallet's account is not one of the members the link named. */
	const wrongAccount = $derived(!!account && coreReady && !isExpected(members, account));
	const memberList = $derived(members.map((a) => shortenAddress(eip55(a))).join(', '));
	/** A request handed to the wallet and not answered. */
	const inWallet = $derived(activity.find((a) => a.stage === 'wallet' && !a.outcome) ?? null);
	/**
	 * The latest request answered, if it was a signature: "Signed ✓" — said
	 * once (r5reg-7): with "keep this page open" while paired, else with
	 * "you can close this page".
	 */
	const lastSigned = $derived.by(() => {
		const last = activity.find((a) => a.outcome && a.method !== 'wallet_switchEthereumChain');
		return last?.method === 'eth_signTypedData_v4' && last.outcome === 'done' ? last : null;
	});
	/** Paired with Vela: a session it approved is here. */
	const paired = $derived(live.length > 0);
	/** r4C-15: a live session whose Vela side left while something was asked. */
	const velaGone = $derived(live.some((s) => s.state !== 'connected') && (!!request || !!inWallet));
	/** r4C-4: a session on the relay whose Vela side has not been there for a while. */
	const velaAbsent = $derived(
		!velaGone && sessions.some((s) => s.state === 'waiting' && now - s.stateSince > VELA_ABSENT_MS)
	);
	/** Joined with an invite, not in the room with Vela yet: "Connecting to Vela…". */
	const connectingVela = $derived(
		invited && !prompt && live.length === 0 && !sessions.some((s) => s.declined) && !velaAbsent
	);

	/** vis-20: a request's time to the minute, as every other time in the app. */
	function at(ms: number): string {
		return new Date(ms).toLocaleTimeString(data.locale, { hour: 'numeric', minute: '2-digit' });
	}

	function elapsed(since: number): string {
		const s = Math.max(0, Math.floor((now - since) / 1000));
		return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
	}

	function relayText(status: RelayStatus): string {
		if (status.kind !== 'retrying' && status.kind !== 'failed') return '';
		const reason =
			status.why === 'refused'
				? m.relayRefused
				: status.why === 'noToken'
					? m.relayNoToken
					: status.detail;
		if (status.kind === 'failed') return fillTemplate(m.relayFailed, { reason });
		if (status.at === null) return fillTemplate(m.relayUnreachable, { reason });
		return fillTemplate(m.relayRetrying, {
			reason,
			seconds: String(Math.max(1, Math.ceil((status.at - now) / 1000)))
		});
	}

	/** The hashes of a live request's SafeTx (the core's). */
	function hashesOf(entry: ActivityEntry) {
		const t = entry.safeTx;
		if (!t || !coreReady) return null;
		try {
			return safeTxHashes(t.chainId, t.safe, t.tx as SafeTx);
		} catch {
			return null;
		}
	}

	/** Whether a done signature recovers to the account here (checked by the core; Vela checks again). */
	function signatureChecks(entry: ActivityEntry): boolean {
		const hashes = hashesOf(entry);
		if (!hashes || !entry.result || !entry.account) return false;
		try {
			return (
				verifyEcdsaApproval(hashes.safeTxHash, {
					owner: entry.account,
					signature: entry.result
				}).owner.toLowerCase() === entry.account
			);
		} catch {
			return false;
		}
	}

	async function copy() {
		try {
			await navigator.clipboard.writeText(link);
			copied = true;
			setTimeout(() => (copied = false), 1600);
		} catch {
			// the link stays in the address the person opened
		}
	}

	// -- r4C-2 / r4C-9: the transaction the link names, from here ------------------

	const tx = $derived(linked ? (view?.tx ?? null) : null);
	const load = $derived(view?.load ?? { kind: 'idle' as const });
	const step = $derived<OutsideStep>(view?.step ?? { kind: 'idle' });
	/** What the wallet's account may do with it, as the core says. */
	const can = $derived(view?.actions ?? null);
	const txHashes = $derived(tx ? (tx.hashes as SafeTxHashes) : null);
	/** A live request for the same transaction is open: it is answered there, not twice. */
	const liveSame = $derived(
		!!tx &&
			activity.some(
				(a) =>
					!a.outcome &&
					a.safeTx &&
					coreReady &&
					hashesOf(a)?.safeTxHash.toLowerCase() === tx.item.safeTxHash.toLowerCase()
			)
	);
	let connecting = $state(false);
	function connectWallet() {
		if (!host || !chosen) return;
		connecting = true;
		host.dispatch({ type: 'connect_wallet' });
	}
	// The wallet answered: connected, or its refusal said.
	$effect(() => {
		if (account || view?.walletFailure) connecting = false;
	});
	const failure = $derived(
		view?.walletFailure ? problemText(view.walletFailure, m, fillTemplate, walletName, '') : ''
	);

	/** r5reg-13: the fee and the coin, read for the account that can execute it now. */
	const feeKey = $derived(
		tx && account && can?.canExecute
			? `${account}|${tx.item.safeTxHash}|${tx.item.approved}|${tx.item.status}`
			: ''
	);
	$effect(() => {
		const key = feeKey;
		const who = untrack(() => account);
		if (!key || !who || !host) return;
		const machine = host;
		untrack(() => {
			machine.dispatch({ type: 'estimate_fee', account: who });
			machine.dispatch({ type: 'balance', account: who });
		});
	});
	/** What executing costs (`null`: it cannot say; `undefined`: not read for this account). */
	const fee = $derived(
		feeKey && view?.fee && account && view.fee.account.toLowerCase() === account
			? view.fee.fee
			: undefined
	);
	/** r5reg-13: what the wallet holds of the network's coin, which pays an execution's fee. */
	const coin = $derived(
		feeKey && view?.balance && account && view.balance.account.toLowerCase() === account
			? view.balance.wei === null
				? null
				: BigInt(view.balance.wei)
			: undefined
	);
	/**
	 * r5reg-13: the wallet cannot pay for executing it — less than the fee, or
	 * nothing at all when the fee could not be worked out. Unknown is not "can't".
	 */
	const cannotPay = $derived(
		coin !== undefined &&
			coin !== null &&
			fee !== undefined &&
			(fee ? coin < BigInt(fee.wei) : coin === 0n)
	);
	const coinOf = (wei: bigint | string) =>
		tx ? `${formatUnits(wei.toString(), 18)} ${nativeSymbol(tx.chainId)}` : '';

	/** Waiting on the wallet, the chain or the mailbox for the link's transaction. */
	const outsideBusy = $derived(
		step.kind === 'checking' ||
			step.kind === 'switching' ||
			step.kind === 'wallet' ||
			step.kind === 'mining' ||
			step.kind === 'sending'
	);
	/** r5reg-7: the link's transaction was asked for, and answered, through the pairing. */
	const handledPaired = $derived(
		!!tx &&
			coreReady &&
			activity.some(
				(a) =>
					a.outcome &&
					a.safeTx &&
					hashesOf(a)?.safeTxHash.toLowerCase() === tx.item.safeTxHash.toLowerCase()
			)
	);
	/**
	 * r5reg-7: one transaction card. The link's is not shown while Vela's
	 * request is (unless the owner is acting on it from here), nor on a paired
	 * page once the pairing handled it or its number is used: it would only be
	 * a stale copy beside the current one.
	 */
	const showOutside = $derived(
		linked &&
			!liveSame &&
			(outsideBusy ||
				(!request &&
					!(inWallet && inWallet.method !== 'wallet_switchEthereumChain') &&
					!(paired && step.kind === 'idle' && (handledPaired || tx?.item.status === 'done'))))
	);
	// A request answered through the pairing moves the link's transaction on: read it again.
	let answered = 0;
	const doneCount = $derived(activity.filter((a) => a.outcome === 'done').length);
	$effect(() => {
		const done = doneCount;
		if (done > answered && linked && untrack(() => step.kind) === 'idle') {
			untrack(() => host?.dispatch({ type: 'refresh' }));
		}
		answered = done;
	});

	/**
	 * r4C-14: Vela's request, counted as the mailbox and the chain hold it now
	 * (the core's): its approvals already enough, nothing to sign — the Sign
	 * button is not offered (Decline is), whatever Vela still thinks.
	 */
	const requestEnough = $derived(view?.requestEnough ?? false);
	/**
	 * r5reg-23 / r7-11: Vela's request, reviewed as the link's is — the core
	 * read it (its title in facts, its hashes, its name, the one review): its
	 * title and hashes at once, the review once gathered.
	 */
	const requestReview = $derived(view?.requestReview ?? null);
	/** The link's transaction as the review draws it: the core reading of it. */
	const linkShown = $derived<OutsideTxShown | null>(
		tx
			? {
					chainId: tx.chainId,
					safe: tx.safe,
					nonce: tx.item.nonce,
					hashes: tx.hashes,
					name: tx.name,
					title: tx.title,
					review: tx.review,
					chainNonce: tx.members.nonce
				}
			: null
	);

	/**
	 * r4C-12: every row, newest first — Vela's requests and what was done
	 * here — each said by what it was ("Approve #7 · Send 0.02 USDC to 0x…",
	 * "Reject #8", "Execute #7") and how it ended, as the core read it: the
	 * page only words it.
	 */
	const rows = $derived(
		(view?.rows ?? []).map((r) => ({
			id: r.id,
			at: r.at,
			label: rowLabelText(all, r.label),
			outcome: rowOutcomeText(all, r.outcome, walletName),
			bad: r.bad
		}))
	);

	let rejecting = $state(false);
	/**
	 * §3.5: on a network where something besides its members can act for the
	 * multisig, the core asks before each action — named what was found — and
	 * nothing is remembered; "Don't sign" leaves it undone.
	 */
	const riskAsk = $derived(host?.risk ?? null);

	/** r5reg-12: the mailbox's refusal in plain words, never its raw text. */
	function refusedText(kind: OutsideRefusalKind): string {
		switch (kind) {
			case 'setupUnchecked':
				return m.refusedSetupUnchecked;
			case 'setup':
				return m.refusedSetup;
			case 'signature':
				return m.refusedSignature;
			case 'full':
				return m.refusedFull;
			case 'transaction':
				return m.refusedTransaction;
			default:
				return m.refusedOther;
		}
	}

	function stepFailure(s: Extract<OutsideStep, { kind: 'failed' }>): string {
		const network = tx ? chainName(tx.chainId) : '';
		if (s.problem) return problemText(s.problem, m, fillTemplate, walletName, network);
		const reason = s.reason;
		if (s.refused) return refusedText(s.refused);
		switch (reason?.kind) {
			case 'badSignature':
				return m.badSignature;
			case 'notSent':
				return m.notSent;
			case 'noEthSign':
				return m.mailboxNoEthSign;
			// §3.5: no deploy from a genesis the core does not find Verified there.
			case 'deployRefused':
				return fillTemplate(all.authority.deployRefused, { network });
			case 'deployUnverified':
				return fillTemplate(all.authority.deployUnverified, { network });
			case 'reverted':
				return m.reverted;
			// §3.5 / §3.6, read again right before the wallet was asked.
			case 'unchecked':
				return fillTemplate(all.authority.signUnchecked, { network });
			case 'readOnly':
				return fillTemplate(all.authority.refusedOn, { network });
			case 'blocked':
				return all.review.blocked;
			case 'notMember':
				return fillTemplate(m.notOwnerHere, {
					account: account ? shortenAddress(eip55(account)) : '—',
					network
				});
			case 'notLanded':
				return fillTemplate(m.walletFailed, {
					wallet: walletName,
					reason: `not on chain after 180 s: ${reason.txHash}`
				});
			case 'noTransaction':
				return fillTemplate(m.walletFailed, {
					wallet: walletName,
					reason: 'The wallet answered no transaction'
				});
			case 'refused':
				return refusedText('other');
			default:
				return fillTemplate(m.walletFailed, { wallet: walletName, reason: '' });
		}
	}

	const act = (event: Parameters<OutsidePageHost['dispatch']>[0]) => host?.dispatch(event);
</script>

<svelte:head><title>{m.pageTitle}</title></svelte:head>

<main class="page">
	<!-- vis-20: whose page this is, before anything else — the outside
	     member opened it from a link, in a browser Vela has never drawn in. -->
	<header class="brand">
		<BrandMark size={32} />
		<span class="wordmark">VELA WALLET</span>
	</header>
	<h1>{m.pageTitle}</h1>

	{#if damagedLink}
		<!-- Round 6: an invite that does not read is the link's — what to do, plainly. -->
		<p class="error" role="alert" data-testid="connect-link-damaged">{m.linkDamaged}</p>
	{/if}

	{#if relay.kind === 'retrying' || relay.kind === 'failed'}
		<!-- r4C-1: this page's own way onto the relay, said with what to do. -->
		<section class="card trouble" role="alert" data-testid="connect-relay">
			<p class="error">{relayText(relay)}</p>
			<Button variant="secondary" shape="rounded" onclick={() => host?.retryRelay()}
				>{m.retry}</Button
			>
		</section>
	{/if}

	{#if !discovered || !coreReady}
		<!-- r4D-16: from the first frame (prerendered), a spinner and what it waits for. -->
		<p class="waiting" aria-busy="true" data-testid="connect-booting">
			<span class="spinner" aria-hidden="true"></span>
			{#if invited || linked}
				{invited || !linked ? m.connectingVela : m.loadingTx}
			{:else}
				<!-- Round 5: before this page runs, `app.html` read the fragment — an
				     approve-later link says "Reading the transaction…" from its first frame. -->
				<span class="boot-vela">{m.connectingVela}</span><span class="boot-tx">{m.loadingTx}</span>
			{/if}
		</p>
	{:else if providers.length === 0}
		<section class="card">
			<h2>{m.noWallet}</h2>
			<p class="muted">{m.noWalletBody}</p>
			{#if link}
				<Button variant="secondary" shape="rounded" onclick={copy}
					>{copied ? m.copied : m.copyLink}</Button
				>
			{/if}
		</section>
	{:else if !picked}
		<section class="card">
			<h2>{m.chooseWallet}</h2>
			<ul class="wallets">
				{#each providers as p (p.info.uuid)}
					<li>
						<button type="button" class="wallet" onclick={() => start(p)}>
							{#if p.info.icon}<img src={p.info.icon} alt="" />{/if}
							<span>{p.info.name}</span>
						</button>
					</li>
				{/each}
			</ul>
		</section>
	{:else}
		{#if prompt}
			<section class="card ask" aria-live="polite">
				<p class="lead">{m.askConnect}</p>
				<p class="muted">{m.askNote}</p>
				{@render who()}
				<div class="row">
					<Button
						variant="secondary"
						shape="rounded"
						onclick={() => act({ type: 'answer_connect', session: prompt.session, approve: false })}
						>{m.decline}</Button
					>
					<!-- ms-14: an account that is not a member is not connected. -->
					<Button
						variant="primary"
						shape="rounded"
						disabled={wrongAccount}
						onclick={() => act({ type: 'answer_connect', session: prompt.session, approve: true })}
						>{m.connect}</Button
					>
				</div>
			</section>
		{/if}

		{#if request}
			{@const entry = request}
			{@const hashes = hashesOf(entry)}
			<!-- r4C-7: Vela's request, read before the wallet is asked. -->
			<section class="card" aria-live="polite" data-testid="connect-request">
				{#if entry.safeTx && hashes && requestReview}
					<TxReview
						m={all}
						signing={data.signingMessages}
						shown={requestReview}
						tx={entry.safeTx.tx as SafeTx}
					/>
				{:else if entry.safeTx && hashes}
					<!-- The core reads it in a moment: never a reading of the page's own meanwhile. -->
					<p class="waiting" aria-busy="true" data-testid="connect-request-reading">
						<span class="spinner" aria-hidden="true"></span>{m.loadingTx}
					</p>
				{:else if entry.method === 'eth_signTypedData_v4'}
					<p class="error" role="alert">{m.notSafeTx}</p>
				{:else if entry.send}
					<p class="lead">{m.sendTx}</p>
					<p class="mono">{entry.send.to}</p>
				{/if}
				{#if requestEnough}
					<p class="muted" data-testid="connect-request-enough">{m.enoughApprovals}</p>
				{/if}
				{#if entry.stage === 'fallback' && hashes}
					<!-- r4C-16: no typed data in this wallet: the hash itself, on a tap. -->
					<p class="muted" data-testid="connect-fallback">
						{fillTemplate(m.typedUnsupported, { wallet: walletName })}
						{fillTemplate(m.ethSignOffer, { hash: hashes.safeTxHash })}
					</p>
				{/if}
				<div class="row">
					<Button
						variant="secondary"
						shape="rounded"
						onclick={() =>
							act({ type: 'answer_request', session: entry.session, id: entry.id, go: false })}
						>{m.decline}</Button
					>
					{#if !requestEnough && (entry.method !== 'eth_signTypedData_v4' || (entry.safeTx && hashes))}
						<Button
							variant="value"
							shape="rounded"
							disabled={wrongAccount}
							onclick={() =>
								act({ type: 'answer_request', session: entry.session, id: entry.id, go: true })}
							>{entry.stage === 'fallback'
								? m.signHash
								: fillTemplate(
										entry.method === 'eth_sendTransaction' ? m.sendInWallet : m.signInWallet,
										{
											wallet: walletName
										}
									)}</Button
						>
					{/if}
				</div>
			</section>
		{:else if inWallet && inWallet.method !== 'wallet_switchEthereumChain'}
			<!-- r4C-3: handed to the wallet, timed; after a while, where to look. -->
			<section class="card" aria-live="polite" data-testid="connect-in-wallet">
				<p class="waiting">
					<span class="spinner" aria-hidden="true"></span>
					{fillTemplate(m.inWallet, {
						wallet: walletName,
						elapsed: elapsed(inWallet.walletAt ?? inWallet.at)
					})}
				</p>
				{#if now - (inWallet.walletAt ?? inWallet.at) > STILL_WAITING_MS}
					<p class="muted" data-testid="connect-still-waiting">
						{fillTemplate(m.stillWaiting, { wallet: walletName })}
					</p>
				{/if}
			</section>
		{:else if lastSigned && !paired && !activity.some((a) => !a.outcome)}
			<!-- r5reg-7: no pairing left to keep open: done, and the page may close. -->
			<section class="card done" data-testid="connect-signed">
				<p class="lead">
					{m.signedClose}
				</p>
				<p class="muted">
					{lastSigned.form === 'ethSign' ? m.signedHashForm : m.signedTypedForm}
					{#if !signatureChecks(lastSigned)}{m.badSignature}{/if}
				</p>
			</section>
		{/if}

		{#if activity.some((a) => a.outcome === 'withdrawn' && a.stage === 'wallet')}
			<p class="muted" role="status">{m.withdrawnInWallet}</p>
		{/if}

		{#if velaGone}
			<!-- r4C-15: Vela's side left mid-request; it reconnects on its own. -->
			<p class="waiting" role="status" data-testid="connect-vela-gone">
				<span class="spinner" aria-hidden="true"></span>{m.velaOffline}
			</p>
		{:else if paired && !request && !inWallet}
			<section class="card">
				{#if wrongAccount}
					{@render who()}
				{:else}
					{#if lastSigned}
						<!-- r5reg-7: one state, one message — signed, and the pairing stays open. -->
						<p class="lead" data-testid="connect-signed">{m.signedPaired}</p>
						<p class="muted">
							{lastSigned.form === 'ethSign' ? m.signedHashForm : m.signedTypedForm}
							{#if !signatureChecks(lastSigned)}{m.badSignature}{/if}
						</p>
					{/if}
					<p class={lastSigned ? 'muted' : 'lead'} data-testid="connect-connected">
						{m.keepOpen}
					</p>
					{#if account}
						<p class="who">
							<span class="mono">{shortenAddress(eip55(account))}</span>
							{#if network}<span class="muted">· {network}</span>{/if}
						</p>
					{/if}
				{/if}
			</section>
		{:else if sessions.some((s) => s.declined)}
			<!-- ms-12: the person said no here; Vela says the same. -->
			<p class="muted" data-testid="connect-declined">{m.declinedHere}</p>
		{:else if velaAbsent && !linked}
			<p class="muted" role="status" data-testid="connect-vela-absent">{m.velaNotConnected}</p>
		{:else if connectingVela && relay.kind !== 'failed'}
			<p class="waiting" aria-busy="true" data-testid="connect-waiting-vela">
				<span class="spinner" aria-hidden="true"></span>{m.connectingVela}
			</p>
		{:else if sessions.length === 0 && !linked}
			<p class="muted">{m.noSession}</p>
		{/if}

		{#if showOutside}
			{@render fromHere()}
		{:else if linked && !request && tx?.item.status === 'done'}
			<!-- r8-24: a link to a transaction that already ran says so — never a page with nothing on it. -->
			<section class="card" data-testid="connect-link-done">
				<p class="muted" data-testid="outside-done">{m.doneTx}</p>
			</section>
		{/if}

		{#if rows.length > 0}
			<section class="card">
				<h2>{m.requests}</h2>
				<ul class="activity" data-testid="connect-activity">
					{#each rows as row (row.id)}
						<li>
							<span>{row.label}</span>
							<span class="outcome" class:bad={row.bad}>{row.outcome}</span>
							<time class="muted">{at(row.at)}</time>
						</li>
					{/each}
				</ul>
			</section>
		{/if}

		<!-- Only a connection there is can be ended (vis-20): a session still
		     waiting for its answer has the Decline above. -->
		{#if live.length > 0}
			<div class="end">
				{#each live as s (s.id)}
					<Button
						variant="secondary"
						shape="rounded"
						onclick={() => act({ type: 'end', session: s.id })}>{m.disconnect}</Button
					>
				{/each}
			</div>
		{/if}
	{/if}

	{#if failure}<p class="error" role="alert">{failure}</p>{/if}
</main>

<!-- ms-14: who is connecting, and whom Vela asked for — a stranger refused aloud. -->
{#snippet who()}
	{#if account}
		<p class="who" data-testid="connect-account">
			<span class="muted"
				>{fillTemplate(m.connectingAs, { account: shortenAddress(eip55(account)) })}</span
			>
		</p>
	{/if}
	{#if wrongAccount}
		<p class="error" role="alert" data-testid="connect-wrong-account">
			{fillTemplate(m.notExpectedMember, {
				account: shortenAddress(eip55(account ?? '')),
				accounts: memberList
			})}
		</p>
	{:else if members.length > 0}
		<p class="muted">{fillTemplate(m.expectedMember, { accounts: memberList })}</p>
	{/if}
{/snippet}

<!-- r4C-2 / r4C-9: the link's transaction, approved, executed or rejected from here. -->
{#snippet fromHere()}
	<section class="card" data-testid="connect-outside">
		{#if load.kind === 'loading' || load.kind === 'idle'}
			<p class="waiting" aria-busy="true">
				<span class="spinner" aria-hidden="true"></span>{m.loadingTx}
			</p>
		{:else if load.kind === 'missing'}
			<p class="muted" data-testid="outside-missing">{m.missingTx}</p>
			<Button variant="secondary" shape="rounded" onclick={() => act({ type: 'refresh' })}
				>{m.retry}</Button
			>
		{:else if load.kind === 'mismatch'}
			<p class="error" role="alert" data-testid="outside-mismatch">{m.mismatchTx}</p>
		{:else if load.kind === 'unavailable'}
			<p class="error" role="alert" data-testid="outside-unavailable">
				{fillTemplate(m.txUnavailable, {
					what: load.what === 'mailbox' ? m.whatMailbox : m.whatChain
				})}
			</p>
			<Button variant="secondary" shape="rounded" onclick={() => act({ type: 'refresh' })}
				>{m.retry}</Button
			>
		{:else if tx && txHashes}
			{@const gate = tx.gate}
			{#if linkShown}
				<TxReview m={all} signing={data.signingMessages} shown={linkShown} tx={tx.tx as SafeTx} />
			{/if}
			<!-- §3.5: who can act for it on this network, as Vela judges it. -->
			{#if gate.kind === 'refused'}
				<AuthorityNotice
					m={all}
					kind="refused"
					findings={gate.findings as Finding[]}
					readOnlyOn={[tx.chainId]}
				/>
			{:else if gate.kind === 'risk'}
				<AuthorityNotice m={all} kind="risk" findings={gate.findings as Finding[]} />
			{:else if gate.kind === 'unchecked' && tx.item.status !== 'done'}
				<p class="error" role="alert" data-testid="outside-unchecked">
					{fillTemplate(all.authority.signUnchecked, { network: chainName(tx.chainId) })}
				</p>
				<Button variant="secondary" shape="rounded" onclick={() => act({ type: 'refresh' })}
					>{m.retry}</Button
				>
			{:else if gate.kind === 'blocked' && tx.item.status !== 'done'}
				<!-- §3.6: the core does not sign what it reads as taking the multisig over. -->
				<p class="error" role="alert" data-testid="outside-blocked">{all.review.blocked}</p>
			{/if}
			<!-- Counted while it is open; once its number is used, what happened is said instead. -->
			{#if tx.item.status !== 'done' && step.kind !== 'executed'}
				<p class="approvals" data-testid="outside-approvals">
					{m.approvalsLabel} ·
					{fillTemplate(all.status.progress, {
						approved: tx.item.approved,
						threshold: tx.item.threshold
					})}
				</p>
			{/if}
			{#if tx.item.status === 'done' && step.kind !== 'executed'}
				<p class="muted" data-testid="outside-done">{m.doneTx}</p>
			{/if}

			{#if riskAsk}
				<div class="risk-ask" role="alert" data-testid="outside-risk-confirm">
					<p>{fillTemplate(all.authority.signRiskBody, { network: chainName(tx.chainId) })}</p>
					<ul>
						{#each findingLines(all, riskAsk.findings as Finding[]) as line (line)}
							<li class="mono">{line}</li>
						{/each}
					</ul>
					<div class="row">
						<Button variant="secondary" shape="rounded" onclick={() => riskAsk.answer(false)}
							>{all.authority.signRiskCancel}</Button
						>
						<Button variant="danger" shape="rounded" onclick={() => riskAsk.answer(true)}
							>{all.authority.signRiskConfirm}</Button
						>
					</div>
				</div>
			{/if}
			{#if step.kind === 'approved'}
				<p class="lead ok" data-testid="outside-approved">
					{#if paired}
						<!-- r5reg-7: the pairing stays open: not "you can close this page" too. -->
						<Icon icon={UTILITY_ICONS.check} size="md" />{all.proposal.approved}
					{:else}
						{m.approvedClose}
					{/if}
				</p>
				<p class="muted">
					{step.form === 'ethSign'
						? m.signedHashForm
						: step.form === 'onChain'
							? m.signedOnChain
							: m.signedTypedForm}
				</p>
			{:else if step.kind === 'executed'}
				<p class="lead ok" data-testid="outside-executed">
					<Icon icon={UTILITY_ICONS.check} size="md" />{all.proposal.executed}
					{#if explorerTxUrl(tx.chainId, step.txHash)}
						·
						<!-- eslint-disable svelte/no-navigation-without-resolve -- an external block explorer -->
						<a
							href={explorerTxUrl(tx.chainId, step.txHash)}
							target="_blank"
							rel="noopener noreferrer">{all.proposal.explorer}</a
						>
						<!-- eslint-enable svelte/no-navigation-without-resolve -->
					{/if}
				</p>
				<p class="mono" data-testid="outside-tx-hash">{step.txHash}</p>
			{:else if step.kind === 'rejected'}
				<p class="lead ok" data-testid="outside-rejected">{m.rejectedHere}</p>
			{:else if step.kind === 'switching'}
				<p class="waiting">
					<span class="spinner" aria-hidden="true"></span>{fillTemplate(all.external.switching, {
						chain: chainName(tx.chainId)
					})}
				</p>
			{:else if step.kind === 'wallet' || step.kind === 'mining'}
				<!-- r4C-3: timed, and said where to look; Cancel stops waiting here. -->
				<div class="wait" data-testid="outside-waiting">
					<p class="waiting">
						<span class="spinner" aria-hidden="true"></span>
						{step.kind === 'mining'
							? fillTemplate(m.mining, {
									network: chainName(tx.chainId),
									elapsed: elapsed(step.since)
								})
							: fillTemplate(m.inWallet, { wallet: walletName, elapsed: elapsed(step.since) })}
					</p>
					{#if step.kind === 'wallet' && step.total > 1}
						<p class="muted">{fillTemplate(m.stepOf, { index: step.index, total: step.total })}</p>
					{/if}
					{#if step.kind === 'wallet' && now - step.since > STILL_WAITING_MS}
						<p class="muted" data-testid="outside-still-waiting">
							{fillTemplate(m.stillWaiting, { wallet: walletName })}
						</p>
					{/if}
					{#if step.kind === 'wallet'}
						<Button variant="secondary" shape="rounded" onclick={() => act({ type: 'cancel' })}
							>{m.stopWaiting}</Button
						>
					{/if}
				</div>
			{:else if step.kind === 'sending'}
				<p class="waiting" role="status" data-testid="outside-sending">
					<span class="spinner" aria-hidden="true"></span>
					{step.retryAt
						? fillTemplate(m.mailboxBusy, {
								seconds: String(Math.max(1, Math.ceil((step.retryAt - now) / 1000)))
							})
						: m.sendingMailbox}
				</p>
			{:else}
				{#if step.kind === 'failed'}
					<p class="error" role="alert" data-testid="outside-failed">{stepFailure(step)}</p>
					{#if step.unsent && step.refused}
						<!-- r5reg-12: refused, and still kept: nothing is signed twice. -->
						<p class="muted" data-testid="outside-kept">{m.keptSigned}</p>
					{/if}
				{/if}
				{#if step.kind === 'failed' && step.unsent}
					<Button variant="value" shape="rounded" onclick={() => act({ type: 'send_again' })}
						>{m.sendAgain}</Button
					>
				{:else if step.kind === 'failed' && step.reason?.kind === 'noEthSign' && account && tx.members.deployed}
					<!-- r4C-16: the hash form is not taken there: approving on chain is. -->
					<Button
						variant="value"
						shape="rounded"
						onclick={() => act({ type: 'approve_on_chain', account })}>{m.approveOnChain}</Button
					>
				{:else if !account}
					<Button variant="primary" shape="rounded" loading={connecting} onclick={connectWallet}
						>{fillTemplate(m.connectWallet, { wallet: walletName })}</Button
					>
				{:else if !can?.isOwner && tx.item.status !== 'done'}
					<p class="error" role="alert" data-testid="outside-not-owner">
						{fillTemplate(m.notOwnerHere, {
							account: shortenAddress(eip55(account)),
							network: chainName(tx.chainId)
						})}
					</p>
				{:else if step.kind === 'unsupported'}
					<!-- r4C-16: no typed data: the hash itself, or on chain. -->
					<p class="muted" data-testid="outside-unsupported">
						{fillTemplate(m.typedUnsupported, { wallet: walletName })}
						{fillTemplate(m.ethSignOffer, { hash: txHashes.safeTxHash })}
					</p>
					<Button
						variant="value"
						shape="rounded"
						onclick={() => act({ type: 'approve', account, form: 'ethSign' })}>{m.signHash}</Button
					>
					{#if tx.members.deployed}
						<p class="muted">{m.approveOnChainOffer}</p>
						<Button
							variant="secondary"
							shape="rounded"
							onclick={() => act({ type: 'approve_on_chain', account })}>{m.approveOnChain}</Button
						>
					{/if}
				{:else}
					{#if can?.hasApproved}
						<p class="muted" data-testid="outside-already">
							{fillTemplate(m.alreadyApproved, { account: shortenAddress(eip55(account)) })}
						</p>
					{:else if can?.enough && can.open}
						<!-- r4C-14: the approvals are enough: nothing to sign, only executing is left. -->
						<p class="muted" data-testid="outside-enough">{m.enoughApprovals}</p>
					{/if}
					{#if can?.canExecute}
						<!-- r4C-9: the owner's own wallet executes it, and pays: said first. -->
						{#if cannotPay}
							<!-- r5reg-13: a wallet that can't pay is told what it needs; Execute steps back. -->
							<p class="error" role="alert" data-testid="outside-cannot-pay">
								{fee
									? fillTemplate(m.executeNeedsGas, {
											fee: coinOf(fee.wei),
											balance: coinOf(coin ?? 0n)
										})
									: fillTemplate(m.executeNoGas, { symbol: nativeSymbol(tx.chainId) })}
							</p>
						{:else}
							<p class="muted" data-testid="outside-fee">
								<!-- r8-15: the estimate and the most it can take, never less than paid. -->
								{fee
									? fee.maxWei
										? fillTemplate(m.executeFeeMax, {
												fee: coinOf(fee.wei),
												max: coinOf(fee.maxWei),
												account: shortenAddress(eip55(account))
											})
										: fillTemplate(m.executeFee, {
												fee: coinOf(fee.wei),
												account: shortenAddress(eip55(account))
											})
									: m.executeFeeUnknown}
								{#if fee && fee.count > 1}{fillTemplate(m.executeSteps, {
										network: chainName(tx.chainId)
									})}{/if}
							</p>
							{#if fee && fee.count > 1}
								<!-- r8-15: how many wallet prompts are coming, said before the first. -->
								<p class="muted" data-testid="outside-confirms">
									{fillTemplate(m.executeConfirms, { total: fee.count })}
								</p>
							{/if}
						{/if}
						<Button
							variant={cannotPay ? 'secondary' : 'value'}
							shape="rounded"
							disabled={cannotPay}
							onclick={() => act({ type: 'execute', account })}
							>{fillTemplate(m.executeWith, { wallet: walletName })}</Button
						>
					{/if}
					{#if can?.canApprove && (!can.canExecute || cannotPay)}
						<Button
							variant="value"
							shape="rounded"
							onclick={() => act({ type: 'approve', account, form: null })}
							>{fillTemplate(m.signInWallet, { wallet: walletName })}</Button
						>
					{:else if can?.canApprove}
						<Button
							variant="secondary"
							shape="rounded"
							onclick={() => act({ type: 'approve', account, form: null })}>{m.approveOnly}</Button
						>
					{/if}
					{#if can?.canReject}
						{#if rejecting}
							<p class="muted">
								{fillTemplate(all.proposal.rejectHint, { threshold: tx.item.threshold })}
							</p>
							<div class="row">
								<Button variant="secondary" shape="rounded" onclick={() => (rejecting = false)}
									>{m.keep}</Button
								>
								<Button
									variant="danger"
									shape="rounded"
									onclick={() => act({ type: 'reject', account })}>{all.proposal.reject}</Button
								>
							</div>
						{:else}
							<button type="button" class="text-button" onclick={() => (rejecting = true)}
								>{all.proposal.reject}</button
							>
						{/if}
					{/if}
				{/if}
			{/if}
		{/if}
	</section>
{/snippet}

<style>
	/* Round 5: which booting line the first frame shows (`app.html`, connect-boot). */
	.boot-tx,
	:global(html[data-connect-tx]) .boot-vela {
		display: none;
	}

	:global(html[data-connect-tx]) .boot-tx {
		display: inline;
	}

	.risk-ask {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
		padding: var(--space-lg);
		border-radius: var(--radius-lg);
		border: var(--border-hairline) solid var(--color-error-base);
		background: var(--color-bg-sunken);
	}

	.risk-ask p,
	.risk-ask ul {
		margin: 0;
	}

	/* vis-20: the app's screen gutter and type scale — the page had a gutter
	   half the app's and body copy smaller than anything else Vela draws. */
	.page {
		box-sizing: border-box;
		max-width: calc(var(--layout-onboardingColumn) + var(--layout-screenPaddingX) * 2);
		margin: 0 auto;
		padding: var(--space-3xl) var(--layout-screenPaddingX);
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
		font-family: var(--font-ui);
		color: var(--color-fg-base);
	}

	.brand {
		display: flex;
		gap: var(--space-md);
		align-items: center;
	}

	.wordmark {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		letter-spacing: 0.11em;
	}

	h1 {
		margin: 0;
		font-size: calc(var(--text-3xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		line-height: var(--leading-tight);
	}

	h2 {
		margin: 0 0 var(--space-sm);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
	}

	.card {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
		padding: var(--space-xl);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
	}

	.lead {
		margin: 0;
		display: flex;
		gap: var(--space-sm);
		align-items: center;
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
	}

	.muted {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
	}

	.waiting {
		margin: 0;
		display: flex;
		gap: var(--space-md);
		align-items: center;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.spinner {
		flex: none;
		width: var(--icon-sm);
		height: var(--icon-sm);
		border: var(--border-emphasis) solid currentColor;
		border-top-color: transparent;
		border-radius: var(--radius-full);
		animation: spin 800ms linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(1turn);
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.spinner {
			animation: none;
			border-top-color: currentColor;
			opacity: var(--opacity-dim);
		}
	}

	.wait {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}

	.who {
		margin: 0;
		display: flex;
		gap: var(--space-sm);
		align-items: baseline;
	}

	.mono {
		margin: 0;
		font-family: var(--font-mono);
		overflow-wrap: anywhere;
	}

	.approvals {
		margin: 0;
		font-weight: var(--weight-semibold);
	}

	.row,
	.end {
		display: flex;
		gap: var(--space-sm);
	}

	.row > :global(*) {
		flex: 1;
	}

	.wallets,
	.activity {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	.wallet {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		width: 100%;
		min-height: var(--size-control-lg);
		padding: 0 var(--space-md);
		border: var(--border-hairline) solid var(--color-border-strong);
		border-radius: var(--radius-lg);
		background: var(--color-bg-base);
		font: inherit;
		color: inherit;
		cursor: pointer;
	}

	.wallet img {
		width: var(--icon-lg);
		height: var(--icon-lg);
	}

	.activity li {
		display: grid;
		grid-template-columns: 1fr auto;
		gap: var(--space-xs) var(--space-md);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
	}

	.activity .outcome {
		grid-column: 1;
		color: var(--color-fg-muted);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
	}

	.activity .outcome.bad {
		color: var(--color-error-base);
	}

	.activity time {
		grid-row: 1;
		grid-column: 2;
	}

	.text-button {
		align-self: flex-start;
		min-height: var(--size-control-sm);
		padding: 0;
		border: none;
		background: none;
		color: var(--color-error-base);
		font: inherit;
		cursor: pointer;
	}

	.error {
		margin: 0;
		color: var(--color-error-base);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
	}
</style>
