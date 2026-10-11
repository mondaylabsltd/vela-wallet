<script lang="ts">
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import LetterAvatar from '$lib/ui/LetterAvatar.svelte';
	import type { ConnectionModel } from '../model';
	import SiteMark from './SiteMark.svelte';

	/**
	 * What a connected site can and cannot do (E7 / DE3), in that order: who
	 * it is, which account it sees, which network, then the sentence that
	 * says a connection is not a permission to move money.
	 */
	interface Props {
		connection: ConnectionModel;
		closeLabel?: string;
		onclose?: () => void;
		ondisconnect?: () => void;
		onswitch?: () => void;
	}

	let { connection, closeLabel, onclose, ondisconnect, onswitch }: Props = $props();
</script>

<div class="panel">
	<header class="site">
		<LetterAvatar letter={connection.site.letter} tint={connection.site.tint} size={40} />
		<span class="who">
			<span class="host">{connection.site.host}</span>
			<span class="status" data-lock={connection.mark.lock}>
				<SiteMark mark={connection.mark} />
				{connection.statusLine}
			</span>
		</span>
		{#if onclose}
			<button type="button" class="close" aria-label={closeLabel} onclick={onclose}>
				<Icon icon={UTILITY_ICONS.x} size="lg" />
			</button>
		{/if}
	</header>

	<button type="button" class="account" onclick={onswitch}>
		<span class="identicon"
			><!-- eslint-disable-next-line svelte/no-at-html-tags -- trusted vela-core output, no user content -->
			{@html connection.account.identiconSvg}</span
		>
		<span class="who">
			<span class="name">{connection.account.name}</span>
			<span class="address">{connection.account.address}</span>
		</span>
		<span class="switch">
			{connection.switchLabel}
			<Icon icon={UTILITY_ICONS['chevron-right']} size="sm" />
		</span>
	</button>

	<div class="network">
		<span class="label">{connection.networkLabel}</span>
		<span class="value">
			<span class="dot" style:background={connection.network.dot}></span>
			{connection.network.name}
		</span>
	</div>

	<p class="explainer">{connection.explainer}</p>

	<button type="button" class="disconnect" onclick={ondisconnect}>{connection.disconnect}</button>
	<p class="footnote">{connection.footnote}</p>
</div>

<style>
	.panel {
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
		padding-block: var(--space-lg) var(--space-xl);
	}

	.site,
	.account {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		width: 100%;
		padding: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		text-align: start;
	}

	.account {
		cursor: pointer;
		padding-block: var(--space-lg);
		border-top: var(--border-hairline) solid var(--color-border-base);
		border-bottom: var(--border-hairline) solid var(--color-border-base);
	}

	.who {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		flex: 1;
		min-width: 0;
	}

	.host,
	.name {
		font-size: calc(var(--text-xl) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.name {
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
	}

	.status {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	/* The words wear their mark's tone (see `SiteMark`): green beside a
	   padlock, the warning for public http, plain for a local page. */
	.status[data-lock='secure'] {
		color: var(--color-success-base);
	}

	.status[data-lock='insecure'] {
		color: var(--color-warning-base);
	}

	.address {
		font-family: var(--font-mono);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.identicon {
		display: flex;
		width: var(--icon-xl);
		height: var(--icon-xl);
	}

	.identicon :global(svg) {
		width: 100%;
		height: 100%;
	}

	.switch {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		flex-shrink: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.close {
		display: flex;
		border: none;
		background: none;
		color: var(--color-fg-muted);
		cursor: pointer;
	}

	.network {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.label {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.value {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	.dot {
		width: var(--space-md);
		height: var(--space-md);
		border-radius: var(--radius-full);
	}

	.explainer {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
	}

	.disconnect {
		height: var(--size-control-lg);
		border: var(--border-hairline) solid var(--color-border-strong);
		border-radius: var(--radius-lg);
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
		cursor: pointer;
	}

	.disconnect:active {
		transform: scale(var(--motion-press-button));
	}

	.footnote {
		margin: 0;
		text-align: center;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}
</style>
