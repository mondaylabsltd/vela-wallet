<script lang="ts">
	/**
	 * The identicon, big, next to the address that drew it (founder call,
	 * 2026-08-26).
	 *
	 * The artwork is a fingerprint of the address: the same address always
	 * draws the same pattern, which only becomes useful once a person has seen
	 * the two together often enough to recognise one from the other. A 40px
	 * avatar in a header never teaches that. This does — and it is reachable
	 * from every place the artwork appears, not from a settings page nobody
	 * visits.
	 *
	 * Shape follows `PromptSheet`: bottom sheet below the desktop breakpoint,
	 * centred card above it.
	 */
	import { MediaQuery } from 'svelte/reactivity';
	import Sheet from '$lib/ui/onboarding/Sheet.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Identicon from './Identicon.svelte';
	import { BREAKPOINT_DESKTOP } from '$lib/tokens/tokens';
	import type { WalletMessages } from '../messages';

	interface Props {
		copy: WalletMessages['identiconViewer'];
		/** The seed, verbatim: what the artwork was drawn from. */
		address: string;
		identiconSvg: string;
		onClose: () => void;
	}

	let { copy, address, identiconSvg, onClose }: Props = $props();

	const desktop = new MediaQuery(`(min-width: ${BREAKPOINT_DESKTOP}px)`, false);
	let sheet = $state<{ requestClose: () => void }>();
	let copied = $state(false);
	let copiedTimer: ReturnType<typeof setTimeout> | undefined;

	function close() {
		if (sheet) sheet.requestClose();
		else onClose();
	}

	async function copyAddress() {
		try {
			await navigator.clipboard.writeText(address);
		} catch {
			// A refused clipboard is not a failure worth a dialog: the address
			// is on screen in full, which is the fallback.
			return;
		}
		copied = true;
		clearTimeout(copiedTimer);
		copiedTimer = setTimeout(() => (copied = false), 1500);
	}
</script>

{#snippet body()}
	<div class="viewer">
		<div class="art"><Identicon svg={identiconSvg} size="viewer" /></div>
		<h2 class="title">{copy.title}</h2>
		<p class="caption">{copy.caption}</p>
		<p class="address">{address}</p>
		<div class="actions">
			<Button variant="primary" shape="rounded" onclick={copyAddress}>
				{copied ? copy.copied : copy.copyAddress}
			</Button>
			<Button variant="secondary" shape="rounded" onclick={close}>{copy.close}</Button>
		</div>
	</div>
{/snippet}

<!-- Its own stacking layer, above every sheet and dialog the artwork can be
     tapped in: the account switcher's dialog sits at z 11, and a viewer under
     the surface that opened it would be a tap that did nothing. -->
<div class="layer">
	{#if desktop.current}
		<div
			class="scrim"
			role="dialog"
			aria-modal="true"
			aria-label={copy.title}
			tabindex="-1"
			onkeydown={(event) => event.key === 'Escape' && close()}
		>
			<div class="card">{@render body()}</div>
		</div>
	{:else}
		<Sheet bind:this={sheet} label={copy.title} {onClose}>
			{@render body()}
		</Sheet>
	{/if}
</div>

<style>
	.layer {
		position: relative;
		z-index: 20;
	}

	.scrim {
		position: fixed;
		inset: 0;
		display: grid;
		place-items: center;
		padding: var(--space-3xl);
		background: var(--color-fixed-backdrop);
	}

	.card {
		width: 100%;
		max-width: var(--layout-promptCard);
		padding: var(--space-3xl);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-xl);
		background: var(--color-bg-raised);
		box-shadow: var(--shadow-lg);
	}

	.viewer {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-lg);
		text-align: center;
	}

	.art {
		margin-bottom: var(--space-md);
	}

	.title {
		margin: 0;
		color: var(--color-fg-base);
		font-size: var(--text-xl);
		font-weight: var(--weight-bold);
		line-height: var(--leading-tight);
	}

	.caption {
		margin: 0;
		color: var(--color-fg-muted);
		font-size: var(--text-base);
		line-height: var(--leading-relaxed);
	}

	/* The whole address, wrapped rather than truncated: a fingerprint you can
	   only see half of teaches half a habit. */
	.address {
		margin: 0;
		width: 100%;
		padding: var(--space-lg);
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
		color: var(--color-fg-base);
		font-family: var(--font-mono);
		font-size: var(--text-sm);
		line-height: var(--leading-relaxed);
		overflow-wrap: anywhere;
	}

	.actions {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
		width: 100%;
		margin-top: var(--space-md);
	}
</style>
