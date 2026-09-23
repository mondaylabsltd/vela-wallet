<script lang="ts">
	/**
	 * The dedicated window a dApp's request is answered in (spec 027 D34).
	 *
	 * ONE door now. A request a person clicked for opens the side PANEL, and
	 * since spec 077 the panel is the wallet (`extension/panel.js`), with the
	 * request raised over it — so this page is what is left: the window a page
	 * fired a request into with no user gesture, which cannot have a panel
	 * because `chrome.sidePanel.open` needs a gesture.
	 *
	 * It is not the action popup, which would be dismissed the moment a passkey
	 * prompt takes focus (D34), and not an in-page sheet the site could style,
	 * cover or scroll.
	 *
	 * The lifecycle — ask the core, draw the consent card, hand a signature to
	 * `sign_request`, answer exactly once, settle on teardown with the core's
	 * code — is `DappRequestHost`, the same component the wallet mounts for the
	 * panel. Two surfaces, one answer path.
	 *
	 * The window does not get a landing. `background.js`'s `settle()` calls
	 * `chrome.windows.remove` the moment the answer goes out, and a page has no
	 * say in that (spec 077 FR-003, measured 2026-09-23: the receipt drew, and
	 * the window vanished around it).
	 */
	import DappRequestHost from '$lib/dapp/DappRequestHost.svelte';
	import SigningHost from '$lib/signing/SigningHost.svelte';
	import { FeeQuote } from '$lib/flows/core/fee-quote.svelte';

	let { data } = $props();

	/**
	 * ONE live fee session for this window (026's rule): the quote the core
	 * pre-checks against, the quote on screen and the quote that is signed are
	 * one object with one owner.
	 */
	const feeQuote = new FeeQuote();

	let host = $state<ReturnType<typeof DappRequestHost> | null>(null);
</script>

<svelte:head><title>Vela</title></svelte:head>

<DappRequestHost
	bind:this={host}
	mode="window"
	messages={data.requestMessages}
	locale={data.locale ?? 'en'}
/>

<!--
	026's sheet, unchanged: the same four machines, the same 13 block kinds, the
	same never-unlimited guard. Dismissing it rejects, and the core answers this
	window's transport.
-->
<SigningHost
	messages={data.signingMessages}
	fee={feeQuote}
	receipt={data.signingMessages.receipt}
	onlanding={() => host?.noteLanding()}
	onreceiptdone={() => host?.noteReceiptDone()}
/>
