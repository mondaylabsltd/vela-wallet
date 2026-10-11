<script lang="ts">
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import type { SiteMarkModel } from '../model';

	/**
	 * The ONE place a site's lock is drawn (device walk of 2026-10-11, item 11).
	 *
	 * Three marks, chosen by the core's `OriginLock` and by nothing else:
	 * the padlock for https, a neutral "i" for a page on this device or the
	 * person's own network, the warning triangle for public http. A padlock
	 * was drawn by four components, two of them unconditionally; none of them
	 * holds one now, and `site-mark.test.ts` keeps it so.
	 *
	 * The padlock and the local mark take the colour of where they stand, as
	 * the padlock always did (muted in an address bar, its status line's green
	 * beside "Secure site"). Only the warning brings its own: public http is
	 * the warning colour wherever it is drawn.
	 */
	interface Props {
		mark: SiteMarkModel;
		/**
		 * Name the mark for assistive technology. On where the mark stands
		 * alone (an address bar); off where its words are written beside it,
		 * which would otherwise be read twice.
		 */
		named?: boolean;
	}

	let { mark, named = false }: Props = $props();

	const ICONS = {
		secure: UTILITY_ICONS.lock,
		local: UTILITY_ICONS.info,
		insecure: UTILITY_ICONS['triangle-alert']
	} as const;
</script>

<span class="mark" data-lock={mark.lock} data-testid="site-mark">
	<Icon icon={ICONS[mark.lock]} size="xs" label={named ? mark.label : undefined} />
</span>

<style>
	.mark {
		display: flex;
		flex-shrink: 0;
	}

	.mark[data-lock='insecure'] {
		color: var(--color-warning-base);
	}
</style>
