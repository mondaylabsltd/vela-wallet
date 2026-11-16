<script lang="ts">
	/**
	 * Vela Connect's window (spec 109, `docs/vela-connect/PROTOCOL.md`): a site
	 * opened `wallet.getvela.app/dapp` to ask the web wallet for an account, a
	 * signature or a transaction.
	 *
	 * `ConnectHost` carries each request to its answer; the sheet is 026's,
	 * unchanged — the same machines, the same clear signing, the same guards a
	 * send from the wallet itself gets.
	 */
	import ConnectHost from '#lib/connect/ConnectHost.svelte';
	import SigningHost from '#lib/signing/SigningHost.svelte';
	import IdenticonViewerHost from '#lib/wallet/ui/IdenticonViewerHost.svelte';
	import { FeeQuote } from '#lib/flows/core/fee-quote.svelte.ts';
	import { browser } from '$app/env';
	import { followSharedTracker } from '#lib/wallet/core/tracker-resident.ts';

	let { data } = $props();

	// The signing resident needs the account's in-flight operations to hold a
	// second confirm; it follows the wallet's tracker when one runs.
	if (browser) followSharedTracker();

	/** ONE live fee session for this window (026's rule). */
	const feeQuote = new FeeQuote();

	let host = $state<ReturnType<typeof ConnectHost> | null>(null);
</script>

<svelte:head>
	<title>Vela</title>
	<meta name="robots" content="noindex" />
</svelte:head>

<ConnectHost
	bind:this={host}
	messages={data.connectMessages}
	request={data.requestMessages}
	locale={data.locale ?? 'en'}
/>

<SigningHost
	messages={data.signingMessages}
	fee={feeQuote}
	receipt={data.signingMessages.receipt}
	onlanding={() => host?.noteLanding()}
	onreceiptdone={() => host?.noteReceiptDone()}
/>

<IdenticonViewerHost copy={data.identiconViewer} />

<style>
	/*
	 * As in the extension's request window (dapp-14): nothing is behind a
	 * sheet here, so a sheet is the window, top to bottom.
	 */
	:global(html .root.signing > .sheet),
	:global(html .root.wallet > .sheet) {
		top: 0;
		max-height: none;
		border-radius: 0;
	}

	:global(html .root.signing > .scrim),
	:global(html .root.wallet > .scrim) {
		background: var(--color-bg-base);
	}
</style>
