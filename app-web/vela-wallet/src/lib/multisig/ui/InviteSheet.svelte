<script lang="ts">
	/**
	 * Inviting the other members: the multisig's address, and a link that adds
	 * it in Vela with one tap. The address is all a member needs; the link only
	 * saves typing it.
	 */
	import Button from '$lib/ui/Button.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import Identicon from '$lib/wallet/ui/Identicon.svelte';
	import { identiconSvgForClient } from '$lib/wallet/identicon';
	import type { MultisigMessages } from '../messages';
	import { addLink, shareOrigin } from '../core/link';
	import { isShared, multisig, type MultisigAccount } from '../core/store.svelte';
	import Sheet from './Sheet.svelte';

	interface Props {
		m: MultisigMessages;
		account: MultisigAccount;
		locale: string;
		wide: boolean;
		closeLabel: string;
		onclose: () => void;
	}

	let { m, account, locale, wide, closeLabel, onclose }: Props = $props();

	const link = $derived(
		typeof location === 'undefined'
			? ''
			: addLink(shareOrigin(location.origin), locale, account.address)
	);
	/**
	 * Its record has not reached the genesis service yet: another member who
	 * adds it by this address may not find it until it has. Read from the
	 * store, so it changes the moment a retry lands.
	 */
	const unshared = $derived(!isShared(multisig.find(account.address) ?? account));
	let copied = $state<'address' | 'link' | null>(null);
	const canShare = typeof navigator !== 'undefined' && typeof navigator.share === 'function';

	async function copy(what: 'address' | 'link') {
		try {
			await navigator.clipboard.writeText(what === 'address' ? account.address : link);
			copied = what;
			setTimeout(() => (copied = null), 1600);
		} catch {
			// it stays on screen
		}
	}
</script>

<Sheet title={m.panel.invite} {closeLabel} {wide} {onclose}>
	<div class="hero">
		<Identicon
			svg={identiconSvgForClient(account.address)}
			size="detail"
			address={account.address}
		/>
		<p class="address">{account.address}</p>
		<p class="hint">{m.create.createdBody}</p>
		{#if unshared}
			<p class="unshared" role="status">{m.panel.notShared}</p>
		{/if}
	</div>
	<div class="actions">
		<Button variant="secondary" shape="rounded" onclick={() => copy('address')}>
			<span class="label"
				><Icon
					icon={copied === 'address' ? UTILITY_ICONS.check : UTILITY_ICONS.copy}
					size="sm"
				/>{copied === 'address' ? m.proposal.copied : m.create.copyAddress}</span
			>
		</Button>
		<Button variant="secondary" shape="rounded" onclick={() => copy('link')}>
			<span class="label"
				><Icon
					icon={copied === 'link' ? UTILITY_ICONS.check : UTILITY_ICONS.copy}
					size="sm"
				/>{copied === 'link' ? m.proposal.copied : m.proposal.copyLink}</span
			>
		</Button>
		{#if canShare}
			<Button
				variant="secondary"
				shape="rounded"
				onclick={() => navigator.share({ url: link }).catch(() => {})}>{m.proposal.share}</Button
			>
		{/if}
	</div>
</Sheet>

<style>
	.hero {
		display: flex;
		flex-direction: column;
		align-items: center;
		text-align: center;
		padding-block: var(--space-lg);
	}

	.address {
		margin: var(--space-md) 0 0;
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-base);
		overflow-wrap: anywhere;
	}

	.hint {
		margin: var(--space-sm) 0 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.unshared {
		margin: var(--space-md) 0 0;
		padding: var(--space-md);
		border-radius: var(--radius-lg);
		background: var(--color-warning-soft);
		border: var(--border-hairline) solid var(--color-warning-border);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-base);
		text-align: start;
	}

	.actions {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	.label {
		display: inline-flex;
		align-items: center;
		gap: var(--space-sm);
	}
</style>
