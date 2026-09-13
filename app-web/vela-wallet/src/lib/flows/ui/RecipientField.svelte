<script lang="ts">
	/**
	 * SD2's and SD2d's recipient field: the address in full over two lines,
	 * with the identicon that fingerprints it and the buttons that fill it.
	 *
	 * The identicon sits INSIDE the field, next to the characters it is drawn
	 * from. Address poisoning works by matching the first and last few
	 * characters of an address you have used before; the artwork is the part
	 * that does not match, and it only helps if it is where the eye already is.
	 */
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import Identicon from '$lib/wallet/ui/Identicon.svelte';

	interface Props {
		label: string;
		lines: [string, string];
		/** The whole address, when there is one: the artwork opens the viewer on it. */
		address?: string;
		identiconSvg: string;
		pickLabel: string;
		scanLabel?: string;
		note?: string;
		onpick?: () => void;
		onscan?: () => void;
		/**
		 * Present ⇒ the address can be typed or pasted here (spec 026). Absent,
		 * the field stays the two drawn lines the gallery renders.
		 */
		oninput?: (value: string) => void;
		placeholder?: string;
	}

	let {
		label,
		lines,
		address,
		identiconSvg,
		pickLabel,
		scanLabel,
		note,
		onpick,
		onscan,
		oninput,
		placeholder
	}: Props = $props();
</script>

<div class="block">
	<span class="label">{label}</span>
	<div class="field" data-field>
		<Identicon svg={identiconSvg} size="row" {address} />
		{#if oninput}
			<input
				class="address entry"
				spellcheck="false"
				autocomplete="off"
				aria-label={label}
				placeholder={placeholder ?? ''}
				value={lines.join('')}
				oninput={(event) => oninput(event.currentTarget.value)}
			/>
		{:else}
			<span class="address">
				{#each lines as line, i (i)}<span class="line">{line}</span>{/each}
			</span>
		{/if}
		<button type="button" aria-label={pickLabel} onclick={onpick}>
			<Icon icon={UTILITY_ICONS['user-round']} size="md" />
		</button>
		{#if scanLabel !== undefined}
			<button type="button" aria-label={scanLabel} onclick={onscan}>
				<Icon icon={UTILITY_ICONS['qr-code']} size="md" />
			</button>
		{/if}
	</div>
	{#if note !== undefined}<span class="note">{note}</span>{/if}
</div>

<style>
	.block {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	.label {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.entry {
		border: none;
		background: none;
		padding: 0;
		min-width: 0;
	}

	.entry:focus {
		outline: none;
	}

	.field {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		padding: var(--space-lg);
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
	}

	.address {
		display: flex;
		flex-direction: column;
		flex: 1;
		min-width: 0;
	}

	.line {
		font-family: var(--font-mono);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-base);
		/* Wrapping is the point — an address that ellipsises here hides the
		   characters an address-poisoning attack changes. */
		word-break: break-all;
	}

	button {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-control-sm);
		height: var(--size-control-sm);
		flex-shrink: 0;
		border: none;
		border-radius: var(--radius-full);
		background: none;
		color: var(--color-fg-muted);
		cursor: pointer;
	}

	button:hover {
		background: var(--color-bg-sunken);
		color: var(--color-fg-base);
	}

	.note {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}
</style>
