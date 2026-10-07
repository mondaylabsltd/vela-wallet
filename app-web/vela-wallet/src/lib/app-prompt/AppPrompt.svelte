<script lang="ts">
	/**
	 * "Get Vela on your phone" — a dismissible card on the wallet home, and a
	 * one-time suggestion on the screen that says a new wallet is ready.
	 *
	 * Drawn only when there is a listing for the person's platform
	 * (`STORE_LINKS` in `config.ts`, empty until the apps are listed — so this
	 * draws nothing today) and only until it is closed. On an iPhone it offers
	 * the App Store, on Android Google Play; on a desktop browser and in the
	 * extension both, each with a code the person scans with the phone they
	 * mean to install it on — the receive flow's own code card.
	 *
	 * Decided on mount, never in a prerender: the platform, the listings and
	 * the dismissal are all facts about this browser.
	 */
	import { onMount } from 'svelte';
	import { track } from '$lib/analytics';
	import QRCard from '$lib/flows/ui/QRCard.svelte';
	import Button from '$lib/ui/Button.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import { encodeQr } from '$lib/wallet/qr';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { storeLinks, type StoreLinks } from './config';
	import type { AppPromptMessages } from './messages';
	import {
		detectPlatform,
		dismissPrompt,
		firstShowThisVisit,
		notePromptShown,
		promptAllowed,
		promptPlan,
		type AppPlatform,
		type AppStore,
		type Placement,
		type PromptPlan
	} from './plan';

	interface Props {
		/** The wallet home is `extension` in the extension's build. */
		placement: 'home' | 'create_done';
		messages: AppPromptMessages;
		/** The listings — the configured ones (`config.ts`) unless a test passes its own. */
		links?: StoreLinks;
		/** Detected from the browser unless a test pins it. */
		platform?: AppPlatform;
	}

	let { placement: drawnAt, messages, links, platform }: Props = $props();

	const placement: Placement = $derived(
		drawnAt === 'home' && __VELA_EXTENSION__ ? 'extension' : drawnAt
	);

	let plan = $state<PromptPlan | null>(null);

	function storage(): Storage | undefined {
		try {
			return localStorage;
		} catch {
			return undefined;
		}
	}

	onMount(() => {
		const on =
			platform ?? detectPlatform(navigator.userAgent, navigator.maxTouchPoints, __VELA_EXTENSION__);
		const next = promptPlan(on, links ?? storeLinks(placement));
		if (next === null || !promptAllowed(placement, storage())) return;
		plan = next;
		notePromptShown(placement, storage());
		if (firstShowThisVisit(placement))
			track('store_prompt_shown', { placement, platform: next.platform });
	});

	function close(): void {
		dismissPrompt(storage());
		track('store_prompt_dismissed', { placement });
		plan = null;
	}

	const storeName = (store: AppStore): string =>
		store === 'app_store' ? messages.appStore : messages.googlePlay;

	/**
	 * Count a store link being followed. Attached to the link's wrapper
	 * because `Button` renders an `<a>` that takes no click handler of its
	 * own; the click bubbles here on its way to opening the listing.
	 */
	const counted = (store: AppStore) => (node: HTMLElement) => {
		const followed = (event: Event) => {
			if ((event.target as Element | null)?.closest('a') !== null)
				track('store_click', { store, placement });
		};
		node.addEventListener('click', followed);
		return () => node.removeEventListener('click', followed);
	};
</script>

{#snippet storeLink(entry: PromptPlan['stores'][number])}
	<span class="store" {@attach counted(entry.store)}>
		<Button variant="secondary" href={entry.url} external>{storeName(entry.store)}</Button>
	</span>
{/snippet}

{#if plan !== null}
	<section class="prompt" data-testid="app-prompt">
		<div class="head">
			<span class="glyph"><Icon icon={UTILITY_ICONS.smartphone} size="lg" /></span>
			<div class="text">
				<h2 class="title">{messages.title}</h2>
				<p class="body">{messages.body}</p>
			</div>
			<button type="button" class="close" aria-label={messages.close} onclick={close}>
				<Icon icon={UTILITY_ICONS.x} size="sm" />
			</button>
		</div>

		{#if plan.qr}
			<div class="codes">
				{#each plan.stores as entry (entry.store)}
					<div class="code">
						<span class="qr"
							><QRCard label={storeName(entry.store)} code={encodeQr(entry.url)} /></span
						>
						{@render storeLink(entry)}
					</div>
				{/each}
			</div>
			<p class="hint">{messages.scanHint}</p>
		{:else}
			<div class="stores">
				{#each plan.stores as entry (entry.store)}
					{@render storeLink(entry)}
				{/each}
			</div>
		{/if}
	</section>
{/if}

<style>
	.prompt {
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
		padding: var(--space-xl);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-xl);
		background: var(--color-bg-raised);
		color: var(--color-fg-base);
		font-family: var(--font-ui);
	}

	.head {
		display: flex;
		align-items: flex-start;
		gap: var(--space-lg);
	}

	.glyph {
		display: flex;
		flex-shrink: 0;
		color: var(--color-fg-muted);
	}

	.text {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: var(--space-xs);
		min-width: 0;
	}

	.title {
		margin: 0;
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		overflow-wrap: break-word;
	}

	.body,
	.hint {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		overflow-wrap: break-word;
	}

	.hint {
		text-align: center;
	}

	.close {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		width: var(--size-control-sm);
		height: var(--size-control-sm);
		margin: calc(var(--space-md) * -1) calc(var(--space-md) * -1) 0 0;
		padding: 0;
		border: none;
		border-radius: var(--radius-full);
		background: none;
		color: var(--color-fg-subtle);
		cursor: pointer;
	}

	.close:focus-visible {
		outline: var(--border-emphasis) solid var(--color-border-strong);
	}

	@media (hover: hover) {
		.close:hover {
			color: var(--color-fg-base);
		}
	}

	.codes {
		display: flex;
		flex-wrap: wrap;
		justify-content: center;
		gap: var(--space-xl);
	}

	.code {
		display: flex;
		flex-direction: column;
		align-items: stretch;
		gap: var(--space-md);
		/* Small enough for two side by side in the extension's side panel,
		   big enough for a phone's camera at arm's length. */
		width: var(--size-identiconViewer);
	}

	.qr {
		display: flex;
		justify-content: center;
	}

	.stores {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-md);
	}

	.store {
		display: flex;
	}

	.stores > .store {
		flex: 1 1 0;
	}
</style>
