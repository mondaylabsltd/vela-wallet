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
	import PromptSheet from '$lib/ui/onboarding/v2/PromptSheet.svelte';
	import { fillTemplate } from '$lib/i18n/fill';
	import { promptCopy, type PromptCopy } from '$lib/onboarding/core/copy';
	import { session } from '$lib/session/core/session.svelte';
	import type { CompletionMode } from '$lib/onboarding/generated/CompletionMode';
	import type { PromptKind } from '$lib/onboarding/generated/PromptKind';

	let { data }: PageProps = $props();

	const strings = (key: string, params?: Record<string, string | number>) =>
		fillTemplate(data.flow[key] ?? key, params);

	const home = $derived(resolve('/[locale]', { locale: data.locale }));
	const wallet = $derived(resolve('/[locale]/wallet', { locale: data.locale }));

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
	/>
</main>

{#if pending}
	<PromptSheet
		copy={pending.copy}
		dismissLabel={strings('onboarding.common.back')}
		onAnswer={(accepted) => {
			pending?.resolve(accepted);
			pending = null;
		}}
	/>
{/if}

<style>
	.page {
		display: flex;
		flex-direction: column;
		min-height: 100dvh;
		/* padding: var(--space-4xl) var(--layout-screenPaddingX) var(--space-4xl); */
	}
</style>
