<script lang="ts">
	/**
	 * A surface standing aside for the one its window's wallet is in — one
	 * wallet per window (`one-surface.svelte.ts`). The side panel says Vela is
	 * open in a tab; a tab says Vela is open in the side panel. Instead of a
	 * second live wallet that could say something else, it says where the
	 * wallet is and offers the two ways on: go there, or use this one instead.
	 *
	 * The wallet stays mounted beneath, inert: a send half-filled here is
	 * where the person left it when they come back — "Use here instead"
	 * closes nothing. A dApp's request still rises over it in the panel (the
	 * layout lifts this while one is owed).
	 */
	import Button from '$lib/ui/Button.svelte';

	interface Props {
		title: string;
		body: string;
		go: string;
		here: string;
		ongo: () => void;
		onhere: () => void;
		testid: string;
	}

	let { title, body, go, here, ongo, onhere, testid }: Props = $props();
</script>

<section class="aside" aria-labelledby="step-aside-title" data-testid={testid}>
	<div class="card">
		<h1 id="step-aside-title">{title}</h1>
		<p>{body}</p>
		<div class="actions">
			<Button variant="primary" shape="rounded" onclick={ongo}>{go}</Button>
			<Button variant="secondary" shape="rounded" onclick={onhere}>{here}</Button>
		</div>
	</div>
</section>

<style>
	.aside {
		position: fixed;
		inset: 0;
		z-index: 65;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: var(--layout-screenPaddingX);
		background: var(--color-bg-base);
	}

	.card {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
		inline-size: min(100%, var(--layout-settingsDialogW));
		text-align: center;
	}

	h1 {
		margin: 0;
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	p {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.actions {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
		margin-block-start: var(--space-md);
	}
</style>
