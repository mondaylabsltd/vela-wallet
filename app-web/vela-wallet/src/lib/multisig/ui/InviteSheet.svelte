<script lang="ts">
	/**
	 * Inviting the other members (r4D-16): a code to scan and a link to send
	 * — Share where the browser has it — that adds the multisig in Vela with
	 * its networks and its members' labels (r4D-4, r4D-5), so whoever opens
	 * it is never asked to guess a network. The address is all a member
	 * strictly needs; the link only saves typing it and guessing the rest.
	 */
	import Button from '$lib/ui/Button.svelte';
	import { fillTemplate } from '$lib/i18n/fill';
	import { chainName } from '$lib/services/networks';
	import { encodeQr } from '$lib/wallet/qr';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import Identicon from '$lib/wallet/ui/Identicon.svelte';
	import { identiconSvgForClient } from '$lib/wallet/identicon';
	import type { MultisigMessages } from '../messages';
	import { multisigName } from '../core/display';
	import { inviteLink } from '../core/invite';
	import { shareOrigin } from '../core/link';
	import { multisig, type MultisigAccount } from '../core/store.svelte';
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

	const shown = $derived(multisig.find(account.address) ?? account);
	const link = $derived(
		typeof location === 'undefined' ? '' : inviteLink(shareOrigin(location.origin), locale, shown)
	);
	const qr = $derived.by(() => {
		if (!link) return null;
		try {
			return encodeQr(link);
		} catch {
			return null;
		}
	});
	/**
	 * Its record has not reached the genesis service yet: another member who
	 * adds it by this address may not find it until it has. Read from the
	 * accounts machine's row, so it changes the moment a retry lands.
	 */
	const unshared = $derived(multisig.shared(account.address) === false);
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

	function share() {
		navigator
			.share({ title: multisigName(shown, m), text: m.invite.shareText, url: link })
			.catch(() => {});
	}
</script>

<Sheet title={m.panel.invite} {closeLabel} {wide} {onclose}>
	<div class="hero">
		{#if qr}
			<svg
				class="qr"
				viewBox="-2 -2 {qr.modules + 4} {qr.modules + 4}"
				role="img"
				aria-label={m.invite.qrLabel}
				data-testid="invite-qr"
			>
				<rect x="-2" y="-2" width={qr.modules + 4} height={qr.modules + 4} class="qr-bg" />
				<path d={qr.path} class="qr-fg" />
			</svg>
		{/if}
		<p class="who">
			<Identicon
				svg={identiconSvgForClient(account.address)}
				size="row"
				address={account.address}
			/><span class="name">{multisigName(shown, m)}</span>
		</p>
		<p class="address">{account.address}</p>
		{#if shown.chainIds.length > 0}
			<p class="network" data-testid="invite-network">
				{fillTemplate(m.add.usedOn, { networks: shown.chainIds.map(chainName).join(', ') })}
			</p>
		{/if}
		<p class="hint">{m.invite.body}</p>
		{#if unshared}
			<p class="unshared" role="status">{m.panel.notShared}</p>
		{/if}
	</div>
	<div class="actions">
		{#if canShare}
			<Button variant="primary" shape="rounded" onclick={share}>
				<span class="label"
					><Icon icon={UTILITY_ICONS['share-2']} size="sm" />{m.proposal.share}</span
				>
			</Button>
		{/if}
		<Button variant="secondary" shape="rounded" onclick={() => copy('link')}>
			<span class="label"
				><Icon
					icon={copied === 'link' ? UTILITY_ICONS.check : UTILITY_ICONS.copy}
					size="sm"
				/>{copied === 'link' ? m.proposal.copied : m.proposal.copyLink}</span
			>
		</Button>
		<Button variant="secondary" shape="rounded" onclick={() => copy('address')}>
			<span class="label"
				><Icon
					icon={copied === 'address' ? UTILITY_ICONS.check : UTILITY_ICONS.copy}
					size="sm"
				/>{copied === 'address' ? m.proposal.copied : m.create.copyAddress}</span
			>
		</Button>
	</div>
	<p class="outside">{m.invite.outsideHint}</p>
</Sheet>

<style>
	.hero {
		display: flex;
		flex-direction: column;
		align-items: center;
		text-align: center;
		padding-block: var(--space-md) var(--space-lg);
	}

	.qr {
		display: block;
		width: min(var(--size-qrCard), 70%);
		margin-block-end: var(--space-md);
	}

	.qr-bg {
		fill: var(--color-bg-base);
	}

	.qr-fg {
		fill: var(--color-fg-base);
	}

	.who {
		display: inline-flex;
		align-items: center;
		gap: var(--space-sm);
		margin: 0;
	}

	.name {
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
		overflow-wrap: anywhere;
	}

	.address {
		margin: var(--space-xs) 0 0;
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-base);
		overflow-wrap: anywhere;
	}

	.network {
		margin: var(--space-sm) 0 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.hint,
	.outside {
		margin: var(--space-sm) 0 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
	}

	.outside {
		margin-block-start: var(--space-lg);
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
