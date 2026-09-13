<script lang="ts">
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';

	/**
	 * The one input on the start page: a search box and an address bar at the
	 * same time, because "type a name" and "type a URL" are the same act to
	 * everyone except a browser engineer.
	 */
	interface Props {
		placeholder: string;
		scanLabel: string;
		onscan?: () => void;
		onsubmit?: (value: string) => void;
	}

	let { placeholder, scanLabel, onscan, onsubmit }: Props = $props();
	let value = $state('');
</script>

<div class="field">
	<label class="box" data-field>
		<Icon icon={UTILITY_ICONS.search} size="base" />
		<input
			type="text"
			inputmode="url"
			autocomplete="off"
			spellcheck="false"
			{placeholder}
			bind:value
			onkeydown={(event: KeyboardEvent) => {
				if (event.key === 'Enter') onsubmit?.(value);
			}}
		/>
	</label>
	<button type="button" class="scan" aria-label={scanLabel} onclick={onscan}>
		<Icon icon={UTILITY_ICONS['scan-line']} size="lg" />
	</button>
</div>

<style>
	.field {
		display: flex;
		align-items: center;
		gap: var(--space-md);
	}

	.box {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		flex: 1;
		min-width: 0;
		height: var(--size-searchField);
		padding-inline: var(--space-xl);
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
		color: var(--color-fg-subtle);
	}

	input {
		flex: 1;
		min-width: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		color: var(--color-fg-base);
		outline: none;
	}

	input::placeholder {
		color: var(--color-fg-subtle);
	}

	.scan {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-hitTarget);
		height: var(--size-hitTarget);
		border: none;
		background: none;
		color: var(--color-fg-muted);
		cursor: pointer;
	}

	.scan:active {
		transform: scale(var(--motion-press-button));
	}
</style>
