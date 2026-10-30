<script lang="ts">
	/**
	 * A payment link, opened in the web wallet (spec 039 Part B):
	 * `/<locale>/pay?to=…&chain=…&token=…&amount=…`.
	 *
	 * The core reads the link (`payment_request`'s `link_opened` — one rule, the
	 * one iOS, Android and the desktop read theirs with). Then one of three:
	 *
	 * - **refused** — the link is not one this wallet can honour. Said, in the
	 *   corpus's words for the core's reason, with the way home. Nothing is
	 *   kept: a broken link is not a request.
	 * - **a wallet on this device** — the request is kept and the wallet takes
	 *   it (`$lib/pay/kept`), opening Send locked to the link's network, token
	 *   and amount. Send has the last word on what the wallet can do with it —
	 *   a network it lacks, a token nobody can describe — and the person
	 *   reviews and confirms before anything is signed. This page signs
	 *   nothing and moves nothing.
	 * - **no wallet yet** — said, with Welcome's two ways in; the request is
	 *   kept for the wallet that is about to exist.
	 *
	 * Prerendered: the shape below is the page until the core has ruled, and
	 * the query is never read on the server (there is none).
	 */
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import type { PageProps } from './$types';
	import BrandMark from '$lib/ui/BrandMark.svelte';
	import Button from '$lib/ui/Button.svelte';
	import { hasSignedInWallet, SIGN_IN_PARAM, STAY_PARAM } from '$lib/intro/gate';
	import { validatePayQuery } from '$lib/flows/core/payment-request';
	import { payQueryOf, refusalBody } from '$lib/pay/link';
	import { keepPay } from '$lib/pay/kept';

	let { data }: PageProps = $props();

	const m = $derived(data.pay);
	const locale = $derived(data.locale);

	type Stage =
		| { kind: 'reading' }
		| { kind: 'refused'; body: string }
		| { kind: 'no_wallet'; recipient: string };

	let stage = $state<Stage>({ kind: 'reading' });

	const home = $derived(resolve('/[locale]', { locale }));
	const walletHref = $derived(resolve('/[locale]/wallet', { locale }));
	const createHref = $derived(resolve('/[locale]/create', { locale }));
	// Welcome with its way-in sheet up — `stay` so a half-signed-in browser is
	// not bounced back here (ob-6).
	const signInHref = $derived(`${home}?${STAY_PARAM}&${SIGN_IN_PARAM}`);

	onMount(() => {
		let live = true;
		const query = payQueryOf(location.search);
		void validatePayQuery(query).then((verdict) => {
			if (!live) return;
			if (!verdict.valid) {
				stage = { kind: 'refused', body: refusalBody(verdict.refusal, m) };
				return;
			}
			keepPay(query);
			if (hasSignedInWallet()) {
				void goto(walletHref, { replaceState: true });
				return;
			}
			stage = { kind: 'no_wallet', recipient: verdict.pay.recipient };
		});
		return () => {
			live = false;
		};
	});

	const heading = $derived(stage.kind === 'refused' ? m.invalidTitle : m.title);
</script>

<svelte:head>
	<title>{heading}</title>
	<!-- A link's landing, with someone's address in it: not a page to index. -->
	<meta name="robots" content="noindex" />
</svelte:head>

<main class="pay" aria-busy={stage.kind === 'reading'}>
	<div class="column">
		<header class="brand">
			<BrandMark size={44} />
			<span class="wordmark">VELA WALLET</span>
		</header>

		<section class="body" aria-live="polite">
			<h1 class="title">{heading}</h1>
			{#if stage.kind === 'refused'}
				<p class="lead">{stage.body}</p>
			{:else if stage.kind === 'no_wallet'}
				<p class="lead">{m.noWalletBody}</p>
				<dl class="request">
					<dt>{m.recipient}</dt>
					<dd class="address">{stage.recipient}</dd>
				</dl>
			{:else}
				<div class="reading" aria-hidden="true"><span class="spinner"></span></div>
			{/if}
		</section>

		{#if stage.kind === 'refused'}
			<div class="actions">
				<Button variant="primary" shape="rounded" href={home}>{m.home}</Button>
			</div>
		{:else if stage.kind === 'no_wallet'}
			<div class="actions">
				<Button variant="primary" shape="rounded" href={createHref}>{m.createWallet}</Button>
				<Button variant="secondary" shape="rounded" href={signInHref}>
					{m.alreadyHaveWallet}
				</Button>
			</div>
		{/if}
	</div>
</main>

<style>
	.pay {
		display: flex;
		justify-content: center;
		min-height: 100dvh;
		padding: var(--space-4xl) var(--layout-screenPaddingX) var(--space-5xl);
		background: var(--color-bg-base);
	}

	/* The Welcome column's shape: brand and words at the top, the ways on at
	   the bottom within a thumb on a phone; centred on a desktop. */
	.column {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: var(--space-4xl);
		width: 100%;
		max-width: var(--layout-flowColumn);
	}

	@media (min-width: 1280px) {
		.column {
			flex: 0 1 auto;
			align-self: center;
		}
	}

	.brand {
		display: flex;
		gap: var(--space-md);
		align-items: center;
	}

	.wordmark {
		color: var(--color-fg-base);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		letter-spacing: 0.11em;
	}

	.body {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: var(--space-lg);
	}

	@media (min-width: 1280px) {
		.body {
			flex: 0 1 auto;
		}
	}

	.title {
		margin: 0;
		color: var(--color-fg-base);
		font-size: calc(var(--text-3xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		line-height: var(--leading-tight);
	}

	.lead {
		margin: 0;
		color: var(--color-fg-muted);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		line-height: var(--leading-normal);
	}

	.request {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		margin: 0;
		padding: var(--space-lg);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
	}

	.request dt {
		color: var(--color-fg-muted);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
	}

	.request dd {
		margin: 0;
		color: var(--color-fg-base);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
	}

	.address {
		font-family: var(--font-mono);
		overflow-wrap: anywhere;
	}

	.reading {
		display: flex;
		padding-block: var(--space-2xl);
	}

	.spinner {
		width: 1.5em;
		height: 1.5em;
		border: var(--border-emphasis) solid var(--color-border-base);
		border-top-color: var(--color-fg-muted);
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
		}
	}

	.actions {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}
</style>
