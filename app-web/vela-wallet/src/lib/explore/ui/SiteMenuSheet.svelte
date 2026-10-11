<script lang="ts">
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS, type UtilityIconId } from '$lib/wallet/icons';
	import LetterAvatar from '$lib/ui/LetterAvatar.svelte';
	import type { MenuItemModel, SiteMarkModel, SiteModel } from '../model';
	import SiteMark from './SiteMark.svelte';

	interface Props {
		site: SiteModel;
		/**
		 * The open page's mark (the core's `OriginLock`). Absent on a history
		 * row's menu, whose line is a host and no claim about a connection —
		 * this sheet drew a padlock there too.
		 */
		mark?: SiteMarkModel;
		statusLine: string;
		items: MenuItemModel[];
		closeLabel: string;
		onclose?: () => void;
		onpick?: (id: string) => void;
	}

	let { site, mark, statusLine, items, closeLabel, onclose, onpick }: Props = $props();
</script>

<header class="head">
	<LetterAvatar letter={site.letter} tint={site.tint} size={40} />
	<span class="who">
		<span class="host">{site.host}</span>
		<span class="status" data-lock={mark?.lock}>
			{#if mark}<SiteMark {mark} />{/if}
			{statusLine}
		</span>
	</span>
	<button type="button" class="close" aria-label={closeLabel} onclick={onclose}>
		<Icon icon={UTILITY_ICONS.x} size="lg" />
	</button>
</header>

<ul>
	{#each items as item (item.id)}
		<li>
			<button
				type="button"
				class="item"
				class:danger={item.danger}
				disabled={item.disabled}
				onclick={() => onpick?.(item.id)}
			>
				<Icon icon={UTILITY_ICONS[item.icon as UtilityIconId]} size="lg" />
				<span>{item.label}</span>
			</button>
		</li>
	{/each}
</ul>

<style>
	.head {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		padding-block: var(--space-lg) var(--space-xl);
	}

	.who {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		flex: 1;
		min-width: 0;
	}

	.host {
		font-size: calc(var(--text-xl) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.status {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	/* The words wear their mark's tone: green for a padlock's "Secure site",
	   the warning for public http. A local page, and a line that is only a
	   host, stay plain. */
	.status[data-lock='secure'] {
		color: var(--color-success-base);
	}

	.status[data-lock='insecure'] {
		color: var(--color-warning-base);
	}

	.close {
		display: flex;
		border: none;
		background: none;
		color: var(--color-fg-muted);
		cursor: pointer;
	}

	ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	li + li {
		border-top: var(--border-hairline) solid var(--color-border-base);
	}

	.item {
		display: flex;
		align-items: center;
		gap: var(--space-xl);
		width: 100%;
		padding-block: var(--space-xl);
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		color: var(--color-fg-base);
		cursor: pointer;
		text-align: start;
	}

	.item:active:not(:disabled) {
		transform: scale(var(--motion-press-row));
	}

	/* Greyed, never hidden: the row keeps its place under the thumb. */
	.item:disabled {
		color: var(--color-fg-subtle);
		opacity: var(--opacity-disabled);
		cursor: default;
	}

	.danger {
		color: var(--color-error-base);
	}
</style>
