<script lang="ts">
	/**
	 * Enter or leave the parallel space (spec 026 US4).
	 *
	 * The whole module graph behind this — the fixture keys and their P-256
	 * library — is reached through a DYNAMIC import, so no production page
	 * carries it in a startup chunk. English by design: this is a developer
	 * switch, not product chrome, and its words stay out of the corpus.
	 */
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { packagedHref } from '$lib/extension/page-url';
	import { parallelActive, parallelFlagSet } from '$lib/dev/parallel-flag.svelte';

	const locale = $derived(page.params.locale ?? 'en');
	const walletHref = $derived(resolve('/[locale]/wallet', { locale }));
	/** The DOCUMENT that route is stored as — the two navigations below leave
	 *  the page, and under the packaged extension a route path is not a file
	 *  (spec 027 D42). Identity on the hosted site. */
	const walletDocument = $derived(packagedHref(walletHref));

	let addresses = $state<{ name: string; address: string }[]>([]);
	let multi = $state('');
	let busy = $state(false);
	/** Leaving was refused: the connected sites set aside on the way in could
	 *  not be written back, so the space is still on (see `exitParallelSpace`). */
	let held = $state(false);

	onMount(async () => {
		// The core first: a fixture Safe is DERIVED, never stored, so nothing
		// here can be listed until the wasm is aboard.
		const [{ loadCore }, mod] = await Promise.all([
			import('$lib/core/client'),
			import('$lib/dev/parallel-space')
		]);
		await loadCore();
		mod.installParallelConsole();
		addresses = mod.fixtureStoredAccounts().map((a) => ({ name: a.name, address: a.address }));
		multi = addresses.at(-1)?.address ?? '';
	});

	async function enter(): Promise<void> {
		busy = true;
		const mod = await import('$lib/dev/parallel-space');
		await mod.enterParallelSpace();
		busy = false;
		// A full navigation, not a client one: every resident store must
		// re-hydrate from the swapped wallet rather than keep the real one.
		window.location.assign(walletDocument);
	}

	async function leave(): Promise<void> {
		busy = true;
		const mod = await import('$lib/dev/parallel-space');
		held = !(await mod.exitParallelSpace());
		busy = false;
		if (held) return;
		window.location.assign(walletDocument);
	}
</script>

<svelte:head><title>Parallel space</title></svelte:head>

<main class="page">
	<h1>Parallel space</h1>
	<p class="lede">
		The real app, with only the passkey faked. Chains, relay, storage and every screen are the ones
		that ship; signing uses a fixed test keyset instead of your device.
	</p>

	<p class="state">
		Status: <strong>{parallelActive() || parallelFlagSet() ? 'ACTIVE' : 'off'}</strong>
	</p>
	{#if held}
		<p class="warn" role="alert">
			Your connected sites could not be put back, so the parallel space is still on. Nothing was
			lost — press Leave again.
		</p>
	{/if}

	<div class="actions">
		<button type="button" onclick={enter} disabled={busy}>Enter (seed fixture wallet)</button>
		<button type="button" onclick={leave} disabled={busy}>Leave (restore real wallet)</button>
		<a href={walletHref}>Back to the wallet</a>
	</div>

	<h2>Fixture wallets</h2>
	<p class="warn">These private keys are public. Never send real money to these addresses.</p>
	<ul>
		{#each addresses as account (account.address)}
			<li><span>{account.name}</span><code>{account.address}</code></li>
		{/each}
	</ul>
	{#if multi}
		<p class="note">The last row founds one wallet on all three keys — the golden Safe.</p>
	{/if}
</main>

<style>
	.page {
		max-width: 42rem;
		margin: 0 auto;
		padding: 2rem 1.25rem 4rem;
		color: var(--color-fg-base);
		font-family: var(--font-sans);
	}

	h1 {
		font-size: var(--text-xl);
		margin: 0 0 0.5rem;
	}

	h2 {
		font-size: var(--text-lg);
		margin: 2rem 0 0.5rem;
	}

	.lede,
	.state,
	.note,
	.warn {
		color: var(--color-fg-muted);
		font-size: var(--text-sm);
		line-height: 1.5;
	}

	.warn {
		color: var(--color-error-base);
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.75rem;
		align-items: center;
		margin: 1.5rem 0;
	}

	button {
		padding: 0.5rem 0.9rem;
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-md);
		background: var(--color-bg-raised);
		color: var(--color-fg-base);
		font: inherit;
		font-size: var(--text-sm);
		cursor: pointer;
	}

	button:disabled {
		opacity: 0.5;
		cursor: default;
	}

	a {
		color: var(--color-fg-muted);
		font-size: var(--text-sm);
	}

	ul {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	li {
		display: flex;
		flex-direction: column;
		gap: 0.15rem;
	}

	li span {
		font-size: var(--text-sm);
	}

	code {
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		color: var(--color-fg-muted);
		word-break: break-all;
	}
</style>
