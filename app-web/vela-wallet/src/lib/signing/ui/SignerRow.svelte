<script lang="ts">
	/**
	 * "Signing as · ◉ name" — the account that will sign. The artwork is the
	 * same identicon the wallet header draws, and it opens the same viewer
	 * (founder call, 2026-09-05): the signing sheet is the one place a person
	 * most needs to be sure whose key is about to be used.
	 */
	import Identicon from '$lib/wallet/ui/Identicon.svelte';

	interface Props {
		label: string;
		name: string;
		identiconSvg: string;
		/** The account's address; present, the artwork opens the viewer on it. */
		address?: string;
	}

	let { label, name, identiconSvg, address }: Props = $props();
</script>

<div class="signer">
	<span class="label">{label}</span>
	<span class="who">
		<Identicon svg={identiconSvg} size="inline" {address} />
		<span class="name">{name}</span>
	</span>
</div>

<style>
	.signer {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-lg);
	}

	.label {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.who {
		display: flex;
		align-items: center;
		gap: var(--space-md);
	}

	.name {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}
</style>
