<script lang="ts">
	/**
	 * "Sign with · Automatic ›" — where the passkey that signs this request is.
	 *
	 * Creating a wallet and signing in both let a person say whether their key
	 * is on this machine, on their phone, or on a security key. Signing did not:
	 * it handed the browser every credential and the browser, finding one here,
	 * went straight to Touch ID (founder, 2026-09-19). The choice is per
	 * request and defaults to what the browser would do on its own.
	 *
	 * Opens in place, like the fee selector: a sheet over the signing sheet is a
	 * modal under a modal.
	 *
	 * The Trusted Signer (spec 071) is the one choice that is not a place a
	 * passkey is, so it carries a line saying what it does instead.
	 */
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import type { SigningModel } from '../model';

	interface Props {
		signWith: NonNullable<SigningModel['signWith']>;
		onselect?: (id: string | null) => void;
	}

	let { signWith, onselect }: Props = $props();
</script>

<div class="sign-with">
	<button type="button" class="row" aria-expanded={signWith.open} onclick={() => onselect?.(null)}>
		<span class="label">{signWith.label}</span>
		<span class="value">{signWith.value}</span>
		<span class="chevron" class:open={signWith.open}>
			<Icon icon={UTILITY_ICONS['chevron-down']} size="sm" />
		</span>
	</button>

	{#if signWith.open}
		<div class="options" role="radiogroup" aria-label={signWith.label}>
			{#each signWith.options as option (option.id)}
				<button
					type="button"
					class="option"
					role="radio"
					aria-checked={option.selected}
					class:selected={option.selected}
					onclick={() => onselect?.(option.id)}
				>
					<span class="text">
						<span>{option.title}</span>
						{#if option.detail !== undefined}
							<span class="detail">{option.detail}</span>
						{/if}
					</span>
					{#if option.selected}
						<span class="check"><Icon icon={UTILITY_ICONS.check} size="sm" /></span>
					{/if}
				</button>
			{/each}
		</div>
	{/if}
</div>

<style>
	.sign-with {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}

	.row,
	.option {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		width: 100%;
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		text-align: start;
		cursor: pointer;
	}

	.row {
		justify-content: space-between;
		padding: 0;
	}

	.label {
		flex: 1;
		color: var(--color-fg-muted);
	}

	.value {
		color: var(--color-fg-base);
	}

	.chevron {
		display: grid;
		place-items: center;
		color: var(--color-fg-muted);
		transition: transform var(--motion-fast, 120ms) ease-out;
	}

	.chevron.open {
		transform: rotate(180deg);
	}

	@media (prefers-reduced-motion: reduce) {
		.chevron {
			transition: none;
		}
	}

	.options {
		display: flex;
		flex-direction: column;
		padding: var(--space-sm);
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
	}

	.option {
		justify-content: space-between;
		padding: var(--space-lg) var(--space-xl);
		border-radius: var(--radius-md);
		color: var(--color-fg-muted);
	}

	.option:hover,
	.option.selected {
		color: var(--color-fg-base);
	}

	.text {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		min-width: 0;
	}

	/* Quieter than the name: the name is the choice, this says what it does. */
	.detail {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-subtle);
	}

	.check {
		display: grid;
		place-items: center;
		color: var(--color-accent-base);
	}
</style>
