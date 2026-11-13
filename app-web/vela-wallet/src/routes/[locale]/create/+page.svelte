<script lang="ts">
	/**
	 * Creating a wallet, on its own URL.
	 *
	 * The v2 design draws the flow as a full page rather than a panel, and a
	 * full page deserves a route: back works, a reload does not strand anyone
	 * mid-ceremony, and the Welcome page stays a landing page.
	 *
	 * The wasm this needs is fetched by `CreateFlow` on mount. Reaching this
	 * route IS the commitment that justifies 3.4 MB, which is why nothing above
	 * it loads the core.
	 */
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import type { PageProps } from './$types';
	import CreateFlow from '$lib/ui/onboarding/v2/CreateFlow.svelte';
	import AppPrompt from '$lib/app-prompt/AppPrompt.svelte';
	import PromptSheet from '$lib/ui/onboarding/v2/PromptSheet.svelte';
	import IdenticonViewerHost from '$lib/wallet/ui/IdenticonViewerHost.svelte';
	import PageHandoffSheet from '$lib/signing/ui/PageHandoffSheet.svelte';
	import { fillTemplate } from '$lib/i18n/fill';
	import { pluralTemplate } from '$lib/i18n/plural';
	import { promptCopy, type PromptCopy } from '$lib/onboarding/core/copy';
	import { onMount } from 'svelte';
	import { session } from '$lib/session/core/session.svelte';
	import type { CompletionMode } from '$lib/onboarding/generated/CompletionMode';
	import type { PromptKind } from '$lib/onboarding/generated/PromptKind';

	let { data }: PageProps = $props();

	// A plural key (the done screen's line, issue 409) has its form picked by
	// the core from `params.count`. Its forms come first: a key the other
	// shells still read bare (the progress line, ob-10) keeps that value too,
	// and the bare one must not win here ("from 1 keys").
	const strings = (key: string, params?: Record<string, string | number>) =>
		fillTemplate(
			(typeof params?.count === 'number'
				? pluralTemplate((k) => data.flow[k], data.locale, key, params.count)
				: undefined) ??
				data.flow[key] ??
				key,
			params
		);

	const home = $derived(resolve('/[locale]', { locale: data.locale }));
	const wallet = $derived(resolve('/[locale]/wallet', { locale: data.locale }));

	// ob-2: a reload of the retry screen lands here, on the form, with the
	// wallet already saved. The session's restore is what sends its pending
	// publish again, so it boots here too — the same wasm the flow fetches.
	onMount(() => {
		void session.boot();
	});

	/** The prompt currently on screen, and who is waiting for its answer. */
	let pending = $state<{ copy: PromptCopy; resolve: (accepted: boolean) => void } | null>(null);

	// The executor also passes `confirmable`, and it is deliberately unread:
	// the same fact is already in the copy — only the prompt whose answer
	// changes the flow carries a confirm pair — and PromptSheet reads that
	// rather than a second flag that could disagree with it.
	function prompt(kind: PromptKind): Promise<boolean> {
		return new Promise((settle) => {
			pending = { copy: promptCopy(kind, strings), resolve: settle };
		});
	}

	async function complete(mode: CompletionMode): Promise<void> {
		await session.boot();
		session.accountEstablished(mode);
		// Into the wallet, not back to Welcome: the journey ended in a wallet
		// and the person should be standing in it, as they are on all three
		// native clients. `replaceState` keeps Back out of a finished flow.
		await goto(wallet, { replaceState: true });
	}
</script>

<svelte:head><title>{data.messages.createWallet}</title></svelte:head>

<main class="page">
	<CreateFlow
		{strings}
		privacyUrl="https://getvela.app/privacy"
		termsUrl="https://getvela.app/terms"
		{prompt}
		{complete}
		onExit={() => goto(home)}
	>
		{#snippet doneAside()}
			<!-- Once per device, and nothing until the store links exist. -->
			<AppPrompt placement="create_done" messages={data.appPrompt} />
		{/snippet}
	</CreateFlow>
</main>

<!-- Spec 102 R3/R8: a founding key minted, or confirmed, on the signing page
     (every one in the Safari package, whose pages cannot use a getvela.app
     passkey) — the hand-off card over this flow, which stays exactly as it
     was underneath. -->
<PageHandoffSheet
	m={data.handoff}
	locale={data.locale}
	wide={false}
	closeLabel={strings('common.cancel')}
	kind="ceremony"
/>

<!-- The done screen's artwork opens the viewer, as every artwork does. -->
<IdenticonViewerHost copy={data.identiconViewer} />

{#if pending}
	<PromptSheet
		copy={pending.copy}
		dismissLabel={strings('common.gotIt')}
		onAnswer={(accepted) => {
			pending?.resolve(accepted);
			pending = null;
		}}
	/>
{/if}

<style>
	/* The phone's gutters (issue 475): the screen's text and its rows ran to
	   both edges of a 390 px screen — the page's padding had been commented
	   out to let the desktop rail reach the window's edges. The welcome page's
	   answer, which this copies: the gutters below the desktop breakpoint,
	   none at and above it, where the rail brings its own. */
	.page {
		display: flex;
		flex-direction: column;
		min-height: 100dvh;
		padding: var(--space-4xl) var(--layout-screenPaddingX) var(--space-4xl);
	}

	@media (min-width: 1280px) {
		.page {
			padding: 0;
		}
	}
</style>
