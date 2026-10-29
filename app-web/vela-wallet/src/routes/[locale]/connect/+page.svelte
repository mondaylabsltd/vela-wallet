<script lang="ts">
	/**
	 * The Wallet side of an outside owner's session: this page joins the room
	 * in its link, and forwards what Vela asks — the SafeTx's typed data, a
	 * transaction, a network switch — to the wallet in this browser, which
	 * asks the owner. It keeps its sessions, so Vela can ask the same wallet
	 * again: reopening this page resumes them.
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
	 * approve, execute or reject it from here alone (`OutsideTransaction`):
	 * read from the mailbox, checked by the core, signed in the wallet.
	 */
	import { replaceUrl } from '$lib/ui/history';
	import { onMount } from 'svelte';
	import Button from '$lib/ui/Button.svelte';
	import BrandMark from '$lib/ui/BrandMark.svelte';
	import { loadCore } from '$lib/core/client';
	import { eip55 } from '$lib/multisig/external/eip55';
	import { shortenAddress } from '$lib/wallet/identity';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { chainName, nativeSymbol } from '$lib/services/networks';
	import { discoverProviders, type ProviderDetail } from '$lib/multisig/external/eip6963';
	import { Bridge, type ActivityEntry, type SessionView } from '$lib/multisig/external/bridge';
	import { connectLink, isExpected, readConnectHash } from '$lib/multisig/external/connect-link';
	import { currentChain } from '$lib/multisig/external/injected';
	import { problemText, walletProblem } from '$lib/multisig/external/errors';
	import {
		OutsideTransaction,
		type ExecutionFee,
		type OutsideStep
	} from '$lib/multisig/external/outside.svelte';
	import type { RelayStatus } from '$lib/multisig/external/relay';
	import TxReview from '$lib/multisig/external/TxReview.svelte';
	import AuthorityNotice from '$lib/multisig/ui/AuthorityNotice.svelte';
	import { findingLines } from '$lib/multisig/ui/findings';
	import { explorerTxUrl, formatUnits } from '$lib/multisig/core/display';
	import { safeTxHashes, verifyEcdsaApproval } from '$lib/multisig/core/kernel';
	import { fillTemplate } from '$lib/i18n/fill';

	let { data } = $props();
	const m = $derived(data.multisigMessages.external);
	const all = $derived(data.multisigMessages);

	/** r4C-3: a wallet still silent after this long is said to be waiting, maybe behind this window. */
	const STILL_WAITING_MS = 20_000;
	/** r4C-4: Vela's side absent this long is said, not waited on in silence. */
	const VELA_ABSENT_MS = 10_000;

	let providers = $state<ProviderDetail[]>([]);
	let discovered = $state(false);
	let coreReady = $state(false);
	let chosen = $state<ProviderDetail | null>(null);
	let bridge = $state<Bridge | undefined>();
	let sessions = $state<SessionView[]>([]);
	let tick = $state(0);
	let invite: string | null = null;
	/** Whether this page came with an invite: "Connecting to Vela…" until it is in the room. */
	let invited = $state(false);
	/** ms-14: the members Vela asked for (the link's), lowercase; empty for an older link. */
	let members = $state<string[]>([]);
	let link = $state('');
	let copied = $state(false);
	let failure = $state('');
	/** r4C-2: the transaction the link names, read and acted on from here. */
	let outside = $state<OutsideTransaction | null>(null);
	let now = $state(Date.now());

	onMount(() => {
		const timer = setInterval(() => (now = Date.now()), 1000);
		void boot();
		return () => clearInterval(timer);
	});

	async function boot() {
		// The invite is a bearer credential until paired: out of the address bar
		// and history. The members and the transaction stay, so a reload still has them.
		const read = readConnectHash(location.hash);
		invite = read.invite;
		invited = !!invite;
		members = read.members;
		if (invite) {
			link = location.href;
			replaceUrl(connectLink(location.pathname, null, members, read.tx));
		}
		// The core reads every request before the wallet sees it.
		const core = loadCore().then(() => (coreReady = true));
		if (read.tx) {
			const tx = new OutsideTransaction(read.tx);
			outside = tx;
			void core.then(() => tx.refresh());
		}
		providers = await discoverProviders();
		discovered = true;
		await core.catch(() => {});
		if (providers.length === 1) await start(providers[0]);
	}

	async function start(detail: ProviderDetail) {
		failure = '';
		try {
			const accounts =
				((await detail.provider.request({ method: 'eth_accounts' })) as string[]) ?? [];
			const chain = await currentChain(detail.provider);
			const b = new Bridge(
				detail.provider,
				accounts.map((a) => a.toLowerCase()),
				chain,
				detail.info.name
			);
			const refresh = () => {
				sessions = b.sessions;
				tick++;
			};
			b.addEventListener('change', refresh);
			b.addEventListener('activity', refresh);
			b.addEventListener('prompt', refresh);
			b.resumeAll();
			if (invite) b.join(invite, members);
			invite = null;
			chosen = detail;
			bridge = b;
			refresh();
		} catch (error) {
			failure = problemText(walletProblem(error), m, fillTemplate, walletName, '');
		}
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
	$effect(() => {
		if (sessions.length > 0) void holdScreen();
		const again = () => {
			wakeLock = undefined;
			void holdScreen();
		};
		document.addEventListener('visibilitychange', again);
		return () => document.removeEventListener('visibilitychange', again);
	});

	const prompt = $derived.by(() => {
		void tick;
		return bridge?.prompt;
	});
	const request = $derived.by(() => {
		void tick;
		const r = bridge?.request;
		return r ? { entry: { ...r.entry }, answer: r.answer } : undefined;
	});
	const activity = $derived.by(() => {
		void tick;
		// Copies: the bridge changes its entries in place, and a list keyed by
		// id redraws an entry only when it is another object.
		return (bridge?.activity ?? []).map((a) => ({ ...a }));
	});
	const account = $derived.by(() => {
		void tick;
		return bridge?.signer.accounts[0] ?? null;
	});
	const network = $derived.by(() => {
		void tick;
		const chain = bridge?.signer.chain;
		return chain ? chainName(Number(chain.split(':')[1])) : '';
	});
	const relay = $derived.by<RelayStatus>(() => {
		void tick;
		return bridge?.relay ?? { kind: 'up' };
	});
	const walletName = $derived(chosen?.info.name || m.ownerWallet);
	const live = $derived(sessions.filter((s) => s.approved));
	/** ms-14: the wallet's account is not one of the members the link named. */
	const wrongAccount = $derived(!!account && !isExpected(members, account));
	const memberList = $derived(members.map((a) => shortenAddress(eip55(a))).join(', '));
	/** A request handed to the wallet and not answered. */
	const inWallet = $derived(activity.find((a) => a.stage === 'wallet' && !a.outcome) ?? null);
	/** The latest answered signature: "Signed ✓". */
	const lastSigned = $derived(
		activity.find((a) => a.method === 'eth_signTypedData_v4' && a.outcome === 'done') ?? null
	);
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

	function describe(method: string): string {
		if (method === 'eth_signTypedData_v4') return m.signTx;
		if (method === 'eth_sendTransaction') return m.sendTx;
		if (method === 'wallet_switchEthereumChain') return m.switchChain;
		return method;
	}

	function outcomeOf(a: ActivityEntry): string {
		switch (a.outcome) {
			case 'done':
				return a.method === 'eth_sendTransaction'
					? m.outcomeSent
					: a.method === 'wallet_switchEthereumChain'
						? m.outcomeSwitched
						: a.form === 'ethSign'
							? m.outcomeSignedHash
							: m.outcomeDone;
			case 'withdrawn':
				return m.outcomeWithdrawn;
			case 'rejected':
				return a.problem?.kind === 'declined' ? m.walletDeclined : m.outcomeDeclined;
			case 'failed':
				return a.problem
					? problemText(a.problem, m, fillTemplate, walletName, '')
					: m.outcomeFailed;
			default:
				return a.stage === 'wallet' ? m.outcomeInWallet : m.outcomeWaiting;
		}
	}

	function relayText(status: RelayStatus): string {
		if (status.kind !== 'retrying' && status.kind !== 'failed') return '';
		const reason =
			status.why === 'refused'
				? m.relayRefused
				: status.why === 'noToken'
					? fillTemplate(m.relayNoToken, { detail: status.detail })
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
			return safeTxHashes(t.chainId, t.safe, t.tx);
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

	const view = $derived(outside?.view ?? null);
	const step = $derived<OutsideStep>(outside?.step ?? { kind: 'idle' });
	/** A live request for the same transaction is open: it is answered there, not twice. */
	const liveSame = $derived(
		!!view &&
			activity.some(
				(a) =>
					!a.outcome &&
					a.safeTx &&
					coreReady &&
					hashesOf(a)?.safeTxHash.toLowerCase() === view.item.safeTxHash.toLowerCase()
			)
	);
	let connecting = $state(false);
	async function connectWallet() {
		if (!chosen || !bridge) return;
		connecting = true;
		failure = '';
		try {
			const accounts = (await chosen.provider.request({
				method: 'eth_requestAccounts'
			})) as string[];
			bridge.signer.accounts.splice(
				0,
				bridge.signer.accounts.length,
				...accounts.map((a) => a.toLowerCase())
			);
			tick++;
		} catch (error) {
			failure = problemText(walletProblem(error), m, fillTemplate, walletName, '');
		} finally {
			connecting = false;
		}
	}

	let fee = $state<ExecutionFee | null | undefined>(undefined);
	$effect(() => {
		const tx = outside;
		const who = account;
		if (!tx || !who || !view || !tx.canExecute(who)) {
			fee = undefined;
			return;
		}
		let alive = true;
		void tx.fee(who).then((f) => {
			if (alive) fee = f;
		});
		return () => {
			alive = false;
		};
	});

	let rejecting = $state(false);
	/**
	 * §3.5: on a network where something besides its members can act for the
	 * multisig, each action from here asks first — named what was found — and
	 * nothing is remembered; "Don't sign" leaves it undone.
	 */
	let riskAsk = $state<(() => Promise<void>) | null>(null);
	function act(run: () => Promise<void>) {
		if (outside?.gate?.kind === 'risk') riskAsk = run;
		else void run();
	}
	async function riskAnswer(yes: boolean) {
		const run = riskAsk;
		riskAsk = null;
		if (yes && run) await run();
	}

	function stepFailure(s: Extract<OutsideStep, { kind: 'failed' }>): string {
		if (s.problem)
			return problemText(
				s.problem,
				m,
				fillTemplate,
				walletName,
				view ? chainName(view.chainId) : ''
			);
		if (s.reason === 'badSignature') return m.badSignature;
		if (s.reason === 'notSent') return m.notSent;
		if (s.reason === 'noEthSign') return m.mailboxNoEthSign;
		// §3.5: no deploy from a genesis the core does not find Verified there.
		if (s.reason === 'deployRefused' || s.reason === 'deployUnverified') {
			const network = view ? chainName(view.chainId) : '';
			return s.reason === 'deployRefused'
				? fillTemplate(all.authority.deployRefused, { network })
				: fillTemplate(all.authority.deployUnverified, { network });
		}
		if (s.reason.startsWith('reverted')) return m.reverted;
		if (s.reason.startsWith('refused')) {
			return fillTemplate(m.mailboxRefused, { reason: s.reason.replace(/^refused: /, '') });
		}
		return fillTemplate(m.walletFailed, { wallet: walletName, reason: s.reason });
	}
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

	{#if relay.kind === 'retrying' || relay.kind === 'failed'}
		<!-- r4C-1: this page's own way onto the relay, said with what to do. -->
		<section class="card trouble" role="alert" data-testid="connect-relay">
			<p class="error">{relayText(relay)}</p>
			<Button variant="secondary" shape="rounded" onclick={() => bridge?.retryRelay()}
				>{m.retry}</Button
			>
		</section>
	{/if}

	{#if !discovered || !coreReady}
		<!-- r4D-16: from the first frame (prerendered), a spinner and what it waits for. -->
		<p class="waiting" aria-busy="true" data-testid="connect-booting">
			<span class="spinner" aria-hidden="true"></span>
			{#if invited || outside}
				{invited || !outside ? m.connectingVela : m.loadingTx}
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
	{:else if !bridge}
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
					<Button variant="secondary" shape="rounded" onclick={() => prompt.answer(false)}
						>{m.decline}</Button
					>
					<!-- ms-14: an account that is not a member is not connected. -->
					<Button
						variant="primary"
						shape="rounded"
						disabled={wrongAccount}
						onclick={() => prompt.answer(true)}>{m.connect}</Button
					>
				</div>
			</section>
		{/if}

		{#if request}
			{@const entry = request.entry}
			{@const hashes = hashesOf(entry)}
			<!-- r4C-7: Vela's request, read before the wallet is asked. -->
			<section class="card" aria-live="polite" data-testid="connect-request">
				{#if entry.safeTx && hashes}
					<TxReview
						m={all}
						signing={data.signingMessages}
						chainId={entry.safeTx.chainId}
						safe={entry.safeTx.safe}
						tx={entry.safeTx.tx}
						{hashes}
					/>
				{:else if entry.method === 'eth_signTypedData_v4'}
					<p class="error" role="alert">{m.notSafeTx}</p>
				{:else if entry.send}
					<p class="lead">{m.sendTx}</p>
					<p class="mono">{entry.send.to}</p>
				{/if}
				{#if entry.stage === 'fallback' && hashes}
					<!-- r4C-16: no typed data in this wallet: the hash itself, on a tap. -->
					<p class="muted" data-testid="connect-fallback">
						{fillTemplate(m.typedUnsupported, { wallet: walletName })}
						{fillTemplate(m.ethSignOffer, { hash: hashes.safeTxHash })}
					</p>
				{/if}
				<div class="row">
					<Button variant="secondary" shape="rounded" onclick={() => request.answer(false)}
						>{m.decline}</Button
					>
					{#if entry.method !== 'eth_signTypedData_v4' || (entry.safeTx && hashes)}
						<Button
							variant="value"
							shape="rounded"
							disabled={wrongAccount}
							onclick={() => request.answer(true)}
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
		{:else if lastSigned && !activity.some((a) => !a.outcome)}
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
		{:else if live.length > 0 && !request && !inWallet}
			<section class="card">
				{#if wrongAccount}
					{@render who()}
				{:else}
					<p class="lead" data-testid="connect-connected">{m.keepOpen}</p>
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
		{:else if velaAbsent && !outside}
			<p class="muted" role="status" data-testid="connect-vela-absent">{m.velaNotConnected}</p>
		{:else if connectingVela && relay.kind !== 'failed'}
			<p class="waiting" aria-busy="true" data-testid="connect-waiting-vela">
				<span class="spinner" aria-hidden="true"></span>{m.connectingVela}
			</p>
		{:else if sessions.length === 0 && !outside}
			<p class="muted">{m.noSession}</p>
		{/if}

		{#if outside && !liveSame}
			{@render fromHere(outside)}
		{/if}

		{#if activity.length > 0}
			<section class="card">
				<h2>{m.requests}</h2>
				<ul class="activity" data-testid="connect-activity">
					{#each activity as a (a.id)}
						<li>
							<span>{describe(a.method)}</span>
							<span class="outcome" class:bad={a.outcome === 'failed'}>{outcomeOf(a)}</span>
							<time class="muted">{at(a.at)}</time>
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
					<Button variant="secondary" shape="rounded" onclick={() => bridge?.end(s.id)}
						>{m.disconnect}</Button
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
{#snippet fromHere(tx: OutsideTransaction)}
	<section class="card" data-testid="connect-outside">
		{#if tx.load.kind === 'loading'}
			<p class="waiting" aria-busy="true">
				<span class="spinner" aria-hidden="true"></span>{m.loadingTx}
			</p>
		{:else if tx.load.kind === 'missing'}
			<p class="muted" data-testid="outside-missing">{m.missingTx}</p>
			<Button variant="secondary" shape="rounded" onclick={() => tx.refresh()}>{m.retry}</Button>
		{:else if tx.load.kind === 'mismatch'}
			<p class="error" role="alert" data-testid="outside-mismatch">{m.mismatchTx}</p>
		{:else if tx.load.kind === 'unavailable'}
			<p class="error" role="alert" data-testid="outside-unavailable">
				{fillTemplate(m.txUnavailable, {
					what: tx.load.what === 'mailbox' ? m.whatMailbox : m.whatChain
				})}
			</p>
			<Button variant="secondary" shape="rounded" onclick={() => tx.refresh()}>{m.retry}</Button>
		{:else if view}
			<TxReview
				m={all}
				signing={data.signingMessages}
				chainId={view.chainId}
				safe={view.safe}
				tx={view.tx}
				hashes={view.hashes}
				genesis={view.genesis}
				chainNonce={view.members.nonce}
			/>
			<!-- §3.5: who can act for it on this network, as Vela judges it. -->
			{#if tx.gate?.kind === 'refused'}
				<AuthorityNotice
					m={all}
					kind="refused"
					findings={tx.gate.findings}
					readOnlyOn={[view.chainId]}
				/>
			{:else if tx.gate?.kind === 'risk'}
				<AuthorityNotice m={all} kind="risk" findings={tx.gate.findings} />
			{:else if tx.gate?.kind === 'unchecked' && view.item.status !== 'done'}
				<p class="error" role="alert" data-testid="outside-unchecked">
					{fillTemplate(all.authority.signUnchecked, { network: chainName(view.chainId) })}
				</p>
				<Button variant="secondary" shape="rounded" onclick={() => tx.refresh()}>{m.retry}</Button>
			{/if}
			<!-- Counted while it is open; once its number is used, what happened is said instead. -->
			{#if view.item.status !== 'done' && step.kind !== 'executed'}
				<p class="approvals" data-testid="outside-approvals">
					{m.approvalsLabel} ·
					{fillTemplate(all.status.progress, {
						approved: view.item.approved,
						threshold: view.item.threshold
					})}
				</p>
			{/if}
			{#if view.item.status === 'done' && step.kind !== 'executed'}
				<p class="muted" data-testid="outside-done">{m.doneTx}</p>
			{/if}

			{#if riskAsk && tx.gate?.kind === 'risk'}
				<div class="risk-ask" role="alert" data-testid="outside-risk-confirm">
					<p>{fillTemplate(all.authority.signRiskBody, { network: chainName(view.chainId) })}</p>
					<ul>
						{#each findingLines(all, tx.gate.findings) as line (line)}
							<li class="mono">{line}</li>
						{/each}
					</ul>
					<div class="row">
						<Button variant="secondary" shape="rounded" onclick={() => riskAnswer(false)}
							>{all.authority.signRiskCancel}</Button
						>
						<Button variant="danger" shape="rounded" onclick={() => riskAnswer(true)}
							>{all.authority.signRiskConfirm}</Button
						>
					</div>
				</div>
			{/if}
			{#if step.kind === 'approved'}
				<p class="lead ok" data-testid="outside-approved">
					{m.approvedClose}
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
					{#if explorerTxUrl(view.chainId, step.txHash)}
						·
						<!-- eslint-disable svelte/no-navigation-without-resolve -- an external block explorer -->
						<a
							href={explorerTxUrl(view.chainId, step.txHash)}
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
						chain: chainName(view.chainId)
					})}
				</p>
			{:else if step.kind === 'wallet' || step.kind === 'mining'}
				<!-- r4C-3: timed, and said where to look; Cancel stops waiting here. -->
				<div class="wait" data-testid="outside-waiting">
					<p class="waiting">
						<span class="spinner" aria-hidden="true"></span>
						{step.kind === 'mining'
							? fillTemplate(m.mining, {
									network: chainName(view.chainId),
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
						<Button variant="secondary" shape="rounded" onclick={() => tx.cancel()}
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
				{/if}
				{#if step.kind === 'failed' && step.unsent}
					<Button variant="value" shape="rounded" onclick={() => tx.sendAgain()}
						>{m.sendAgain}</Button
					>
				{:else if step.kind === 'failed' && step.reason === 'noEthSign' && account && view.members.deployed}
					<!-- r4C-16: the hash form is not taken there: approving on chain is. -->
					<Button
						variant="value"
						shape="rounded"
						onclick={() => act(() => tx.approveOnChain(chosen!.provider, account))}
						>{m.approveOnChain}</Button
					>
				{:else if !account}
					<Button variant="primary" shape="rounded" loading={connecting} onclick={connectWallet}
						>{fillTemplate(m.connectWallet, { wallet: walletName })}</Button
					>
				{:else if !tx.isOwner(account) && view.item.status !== 'done'}
					<p class="error" role="alert" data-testid="outside-not-owner">
						{fillTemplate(m.notOwnerHere, {
							account: shortenAddress(eip55(account)),
							network: chainName(view.chainId)
						})}
					</p>
				{:else if step.kind === 'unsupported'}
					<!-- r4C-16: no typed data: the hash itself, or on chain. -->
					<p class="muted" data-testid="outside-unsupported">
						{fillTemplate(m.typedUnsupported, { wallet: walletName })}
						{fillTemplate(m.ethSignOffer, { hash: view.hashes.safeTxHash })}
					</p>
					<Button
						variant="value"
						shape="rounded"
						onclick={() => act(() => tx.approve(chosen!.provider, account, 'ethSign'))}
						>{m.signHash}</Button
					>
					{#if view.members.deployed}
						<p class="muted">{m.approveOnChainOffer}</p>
						<Button
							variant="secondary"
							shape="rounded"
							onclick={() => act(() => tx.approveOnChain(chosen!.provider, account))}
							>{m.approveOnChain}</Button
						>
					{/if}
				{:else}
					{#if tx.hasApproved(account)}
						<p class="muted" data-testid="outside-already">
							{fillTemplate(m.alreadyApproved, { account: shortenAddress(eip55(account)) })}
						</p>
					{/if}
					{#if tx.canExecute(account)}
						<!-- r4C-9: the owner's own wallet executes it, and pays: said first. -->
						<p class="muted" data-testid="outside-fee">
							{fee
								? fillTemplate(m.executeFee, {
										fee: `${formatUnits(fee.wei.toString(), 18)} ${nativeSymbol(view.chainId)}`,
										account: shortenAddress(eip55(account))
									})
								: m.executeFeeUnknown}
							{#if fee && fee.count > 1}{fillTemplate(m.executeSteps, {
									network: chainName(view.chainId)
								})}{/if}
						</p>
						<Button
							variant="value"
							shape="rounded"
							onclick={() => act(() => tx.execute(chosen!.provider, account))}
							>{fillTemplate(m.executeWith, { wallet: walletName })}</Button
						>
					{/if}
					{#if tx.canApprove(account) && !tx.canExecute(account)}
						<Button
							variant="value"
							shape="rounded"
							onclick={() => act(() => tx.approve(chosen!.provider, account))}
							>{fillTemplate(m.signInWallet, { wallet: walletName })}</Button
						>
					{:else if tx.canApprove(account)}
						<Button
							variant="secondary"
							shape="rounded"
							onclick={() => act(() => tx.approve(chosen!.provider, account))}
							>{m.approveOnly}</Button
						>
					{/if}
					{#if tx.canReject(account)}
						{#if rejecting}
							<p class="muted">
								{fillTemplate(all.proposal.rejectHint, { threshold: view.item.threshold })}
							</p>
							<div class="row">
								<Button variant="secondary" shape="rounded" onclick={() => (rejecting = false)}
									>{m.keep}</Button
								>
								<Button
									variant="danger"
									shape="rounded"
									onclick={() => act(() => tx.reject(chosen!.provider, account))}
									>{all.proposal.reject}</Button
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
