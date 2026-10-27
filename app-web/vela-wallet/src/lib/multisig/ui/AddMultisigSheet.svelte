<script lang="ts">
	/**
	 * Adding a multisig by its address — all another member needs. The
	 * genesis comes from the service and is accepted only if the core
	 * re-derives the address from it (`addMultisig`).
	 */
	import { untrack } from 'svelte';
	import Button from '$lib/ui/Button.svelte';
	import { spaceNetworksSync } from '$lib/services/networks';
	import type { MultisigMessages } from '../messages';
	import { addMultisig } from '../core/accounts';
	import Sheet from './Sheet.svelte';

	interface Props {
		m: MultisigMessages;
		prefill?: string;
		wide: boolean;
		closeLabel: string;
		onclose: () => void;
		onadded: (address: string) => void;
	}

	let { m, prefill = '', wide, closeLabel, onclose, onadded }: Props = $props();

	// The link's address seeds the field once; the person may edit it.
	let address = $state(untrack(() => prefill));
	let busy = $state(false);
	let failure = $state<string | null>(null);
	const valid = $derived(/^0x[0-9a-fA-F]{40}$/.test(address.trim()));

	async function add() {
		if (!valid) {
			failure = m.send.invalidAddress;
			return;
		}
		busy = true;
		failure = null;
		const outcome = await addMultisig(
			address.trim(),
			spaceNetworksSync().map((n) => n.chainId)
		);
		busy = false;
		if (outcome.ok) onadded(outcome.account.address);
		else failure = m.add[outcome.reason];
	}

	$effect(() => {
		if (prefill && valid) void add();
	});
</script>

<Sheet title={m.add.title} {closeLabel} {wide} {busy} {onclose}>
	<p class="intro">{m.add.intro}</p>
	<form
		onsubmit={(event) => {
			event.preventDefault();
			void add();
		}}
	>
		<input
			class="mono"
			placeholder={m.add.placeholder}
			autocomplete="off"
			spellcheck="false"
			bind:value={address}
			aria-invalid={failure !== null}
		/>
		{#if failure}<p class="error" role="alert">{failure}</p>{/if}
		<Button
			variant="primary"
			shape="rounded"
			loading={busy}
			disabled={busy}
			onclick={() => void add()}>{m.add.add}</Button
		>
	</form>
</Sheet>

<style>
	.intro {
		margin: 0 0 var(--space-lg);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	form {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}

	input {
		min-height: var(--size-control-lg);
		padding: 0 var(--space-md);
		border: var(--border-hairline) solid var(--color-border-strong);
		border-radius: var(--radius-lg);
		background: var(--color-bg-base);
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	.error {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-error-base);
	}
</style>
