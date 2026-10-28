<script lang="ts">
	/**
	 * The Wallet side of an outside owner's session: this page joins the room
	 * in its link, and forwards what Vela asks — the SafeTx's typed data, a
	 * transaction, a network switch — to the wallet in this browser, which
	 * asks the owner. It keeps its sessions, so Vela can ask the same wallet
	 * again: reopening this page resumes them.
	 */
	import { replaceUrl } from '$lib/ui/history';
	import { onMount } from 'svelte';
	import Button from '$lib/ui/Button.svelte';
	import BrandMark from '$lib/ui/BrandMark.svelte';
	import { eip55 } from '$lib/multisig/external/eip55';
	import { shortenAddress } from '$lib/wallet/identity';
	import { chainName } from '$lib/services/networks';
	import { discoverProviders, type ProviderDetail } from '$lib/multisig/external/eip6963';
	import { Bridge, inviteFromHash, type SessionView } from '$lib/multisig/external/bridge';
	import { currentChain } from '$lib/multisig/external/injected';

	let { data } = $props();
	const m = $derived(data.multisigMessages.external);

	let providers = $state<ProviderDetail[]>([]);
	let discovered = $state(false);
	let bridge = $state<Bridge | undefined>();
	let sessions = $state<SessionView[]>([]);
	let tick = $state(0);
	let invite: string | null = null;
	let link = $state('');
	let copied = $state(false);
	let failure = $state('');

	onMount(async () => {
		// The invite is a bearer credential until paired: out of the address bar and history.
		invite = inviteFromHash(location.hash);
		if (invite) {
			link = location.href;
			replaceUrl(location.pathname);
		}
		providers = await discoverProviders();
		discovered = true;
		if (providers.length === 1) await start(providers[0]);
	});

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
			if (invite) b.join(invite);
			invite = null;
			bridge = b;
			refresh();
		} catch (error) {
			failure = error instanceof Error ? error.message : String(error);
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
	const activity = $derived.by(() => {
		void tick;
		return [...(bridge?.activity ?? [])];
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
	const live = $derived(sessions.filter((s) => s.approved));

	/** vis-20: a request's time to the minute, as every other time in the app. */
	function at(ms: number): string {
		return new Date(ms).toLocaleTimeString(data.locale, { hour: 'numeric', minute: '2-digit' });
	}

	function describe(method: string): string {
		if (method === 'eth_signTypedData_v4') return m.signTx;
		if (method === 'eth_sendTransaction') return m.sendTx;
		if (method === 'wallet_switchEthereumChain') return m.switchChain;
		return method;
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

	{#if !discovered}
		<p class="muted">{m.looking}</p>
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
				<div class="row">
					<Button variant="secondary" shape="rounded" onclick={() => prompt.answer(false)}
						>{m.decline}</Button
					>
					<Button variant="primary" shape="rounded" onclick={() => prompt.answer(true)}
						>{m.connect}</Button
					>
				</div>
			</section>
		{:else if live.length > 0}
			<section class="card">
				<p class="lead" data-testid="connect-connected">{m.keepOpen}</p>
				{#if account}
					<p class="who">
						<span class="mono">{shortenAddress(eip55(account))}</span>
						{#if network}<span class="muted">· {network}</span>{/if}
					</p>
				{/if}
			</section>
		{:else if sessions.some((s) => s.declined)}
			<!-- ms-12: the person said no here; Vela says the same. -->
			<p class="muted" data-testid="connect-declined">{m.declinedHere}</p>
		{:else if sessions.length > 0}
			<p class="muted">{m.waitingVela}</p>
		{:else}
			<p class="muted">{m.noSession}</p>
		{/if}

		{#if activity.length > 0}
			<section class="card">
				<h2>{m.requests}</h2>
				<ul class="activity">
					{#each activity as a (a.at + a.method)}
						<li>
							<span>{describe(a.method)}</span>
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

<style>
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
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
	}

	.muted {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
	}

	.who {
		margin: 0;
		display: flex;
		gap: var(--space-sm);
		align-items: baseline;
	}

	.mono {
		font-family: var(--font-mono);
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
		display: flex;
		justify-content: space-between;
		gap: var(--space-md);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
	}

	.error {
		margin: 0;
		color: var(--color-error-base);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
	}
</style>
