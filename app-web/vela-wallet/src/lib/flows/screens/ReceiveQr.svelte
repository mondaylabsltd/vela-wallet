<script lang="ts">
	/**
	 * R2 / R3 / DR2L / DR3L — the address, as a code.
	 *
	 * One body, two hosts: a bottom sheet on the phone, the third column on the
	 * desktop. The only difference between R2 and R3 is the contract line —
	 * a named asset has one, a network does not.
	 */
	import Button from '$lib/ui/Button.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import AddressCard from '../ui/AddressCard.svelte';
	import QRCard from '../ui/QRCard.svelte';
	import TokenIcon from '$lib/wallet/ui/TokenIcon.svelte';
	import { copyText } from '$lib/services/clipboard';
	import { saveShareImage } from '../share-image';
	import { shortenAddress } from '$lib/wallet/identity';
	import type { ReceiveQrModel } from '../model';

	interface Props {
		model: ReceiveQrModel;
		onsave?: () => void;
		onexplorer?: () => void;
	}

	let { model, onsave, onexplorer }: Props = $props();

	let copied = $state<'address' | 'contract' | null>(null);
	let timer: ReturnType<typeof setTimeout> | undefined;
	let saving = $state(false);

	/** 保存图片: the live card saves itself; the drawn one (the gallery) only reports the tap. */
	async function save() {
		if (model.share === undefined) {
			onsave?.();
			return;
		}
		saving = true;
		try {
			const address = model.account.lines.join('');
			await saveShareImage(model.share, `vela-${shortenAddress(address)}.png`);
		} finally {
			saving = false;
		}
	}

	function copy(what: 'address' | 'contract') {
		void copyText(
			what === 'address'
				? model.account.lines.join('')
				: (model.contract?.copyValue ?? model.contract?.value ?? '')
		);
		copied = what;
		clearTimeout(timer);
		timer = setTimeout(() => (copied = null), 150);
	}
</script>

<div class="qr">
	<h3>{model.title}</h3>

	{#if model.contract !== undefined}
		<p class="contract">
			<span class="contract-label">{model.contract.label}</span>
			<span class="contract-value">{model.contract.value}</span>
			<!-- The same copy affordance the address row carries, one size
			     down: a contract is a detail ABOUT the code below, not the
			     thing being received. -->
			<button
				type="button"
				aria-label={model.contract.copyLabel}
				class:copied={copied === 'contract'}
				onclick={() => copy('contract')}
			>
				<Icon icon={copied === 'contract' ? UTILITY_ICONS.check : UTILITY_ICONS.copy} size="sm" />
			</button>
		</p>
	{/if}

	<AddressCard
		account={model.account}
		copied={copied === 'address'}
		oncopy={() => copy('address')}
	/>

	<div class="code">
		<QRCard label={model.title} code={model.code}>
			{#snippet centre()}
				{#if model.centre.logoUrls !== undefined}
					<!-- Live: the chain's (R2) or the token's (R3) own logo, the same
					     mark the asset rows draw; the lettered badge stands until it loads. -->
					<TokenIcon
						ticker={model.centre.ticker}
						badgeColor={model.centre.badgeColor}
						logoUrls={model.centre.logoUrls}
						badgeLogoUrl={model.centre.badgeLogoUrl}
						badgeHidden={model.centre.badgeHidden}
					/>
				{:else}
					<span class="mark" style:background={model.centre.badgeColor}>{model.centre.ticker}</span>
				{/if}
			{/snippet}
		</QRCard>
	</div>

	<p class="warning">{model.warning}</p>

	<div class="actions">
		<Button variant="secondary" loading={saving} onclick={save}>{model.saveImage}</Button>
		{#if model.explorerUrl !== undefined}
			<Button variant="secondary" href={model.explorerUrl} external>{model.viewOnExplorer}</Button>
		{:else}
			<Button variant="secondary" onclick={onexplorer}>{model.viewOnExplorer}</Button>
		{/if}
	</div>
</div>

<style>
	.qr {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}

	h3 {
		margin: 0;
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.contract {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		margin: 0;
	}

	.contract-label {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.contract-value {
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	.contract button {
		display: flex;
		align-items: center;
		border: none;
		background: none;
		color: var(--color-fg-subtle);
		cursor: pointer;
	}

	.contract .copied {
		color: var(--color-success-base);
	}

	.code {
		display: flex;
		justify-content: center;
		padding-block: var(--space-md);
	}

	.warning {
		margin: 0;
		text-align: center;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.actions {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
		padding-top: var(--space-lg);
	}

	.mark {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--icon-3xl);
		height: var(--icon-3xl);
		border-radius: var(--radius-full);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-onAccent);
	}
</style>
