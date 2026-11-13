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
		/** The note is a warning (spec 096 F12), drawn in the warning tone. */
		noteWarn?: boolean;
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
		noteWarn = false,
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
			<!-- vis-1: a textarea, not a one-line input — the address wraps and
			     is read whole, tail included, as the drawn lines are. One
			     logical line: Enter adds nothing, and a pasted line break is
			     dropped before the core sees the text. -->
			<textarea
				class="address entry"
				rows="2"
				spellcheck="false"
				autocomplete="off"
				autocapitalize="off"
				aria-label={label}
				placeholder={placeholder ?? ''}
				value={lines.join('')}
				onkeydown={(event) => {
					if (event.key === 'Enter') event.preventDefault();
				}}
				oninput={(event) => oninput(event.currentTarget.value.replace(/[\r\n]+/g, ''))}></textarea>
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
	{#if note !== undefined}<span class="note" class:warn={noteWarn}>{note}</span>{/if}
</div>

<style>
	.block {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	.label {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	/* The typed address reads as the drawn one: mono, the text's own colour
	   (a form control inherits neither), wrapped at any character, grown to
	   its content where the browser can. Focus is the well's `data-field`
	   edge (app.css). */
	.address.entry {
		display: block;
		flex: 1;
		min-width: 0;
		min-height: 2lh;
		border: none;
		background: none;
		padding: 0;
		resize: none;
		overflow-y: auto;
		field-sizing: content;
		font-family: var(--font-mono);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-base);
		word-break: break-all;
		overflow-wrap: anywhere;
	}

	.entry::placeholder {
		color: var(--color-fg-subtle);
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
		color: var(--color-fg-muted);
	}

	.note.warn {
		font-weight: var(--weight-medium);
		color: var(--color-warning-base);
	}
</style>
