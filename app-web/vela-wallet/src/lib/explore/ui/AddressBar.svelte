<script lang="ts">
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import Identicon from '$lib/wallet/ui/Identicon.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import type { BrowserModel } from '../model';
	import type { ExploreMessages } from '../messages';

	/**
	 * The browsing top bar — the ONE bar the browser draws on a phone (spec 099
	 * navigation). The app's tab bar stays under the page, so everything the
	 * old bottom toolbar held moved up here or into the site menu:
	 *
	 *   ‹  [ 🔒 host ]  (account)  [n]  ⋯
	 *
	 * - ‹ walks the page's history; with none left it returns to the Explore
	 *   home, tab kept alive. It is never greyed: there is always somewhere
	 *   to go back to.
	 * - The pill shows the DOMAIN, never the full URL, and when it must be cut
	 *   it loses its START: the end of a host is the registrable domain, the
	 *   part that decides who you are talking to (`app.uniswap.org.evil.xyz`
	 *   must never read as `app.uniswap.org…`). A tap edits the full address
	 *   in place; the account, the count and ⋯ step aside while it does.
	 * - The account's green dot IS the connection state.
	 * - The boxed count opens the tab switcher.
	 */
	interface Props {
		browser: BrowserModel;
		copy: ExploreMessages;
		onback?: () => void;
		onaccount?: () => void;
		ontabs?: () => void;
		onmenu?: () => void;
		/** A typed address, opened in this tab. */
		onsubmit?: (value: string) => void;
	}

	let { browser, copy, onback, onaccount, ontabs, onmenu, onsubmit }: Props = $props();

	let editing = $state(false);
	let draft = $state('');

	function edit(): void {
		draft = browser.url;
		editing = true;
	}

	/** Puts the field away; the page under it is untouched. */
	function cancel(): void {
		editing = false;
	}

	function submit(): void {
		editing = false;
		if (draft.trim() !== '') onsubmit?.(draft);
	}

	/** Selects the whole address on arrival, ready to be replaced. */
	function focusSelected(node: HTMLInputElement): void {
		node.focus();
		node.select();
	}

	const accountLabel = $derived(
		browser.connected ? `${copy.account}, ${copy.connectedTag}` : copy.account
	);
</script>

<header class="bar">
	<!-- While the address is being edited, ‹ puts the field away and goes
	     nowhere. The press must not take the focus first: the field's blur
	     would end the edit and the click would then go back for real. -->
	<button
		type="button"
		class="icon"
		aria-label={copy.back}
		onmousedown={(event: MouseEvent) => {
			if (editing) event.preventDefault();
		}}
		onclick={() => (editing ? cancel() : onback?.())}
	>
		<Icon icon={UTILITY_ICONS['chevron-left']} size="lg" />
	</button>

	{#if editing}
		<input
			class="pill field"
			type="url"
			inputmode="url"
			autocomplete="off"
			autocapitalize="off"
			spellcheck="false"
			enterkeyhint="go"
			aria-label={copy.addressBar}
			bind:value={draft}
			use:focusSelected
			onblur={cancel}
			onkeydown={(event: KeyboardEvent) => {
				if (event.key === 'Enter') submit();
				else if (event.key === 'Escape') cancel();
			}}
		/>
	{:else}
		<button type="button" class="pill" title={copy.addressBar} onclick={edit}>
			{#if browser.secure}
				<span class="lock"><Icon icon={UTILITY_ICONS.lock} size="xs" /></span>
			{/if}
			<span class="host"><bdi>{browser.host}</bdi></span>
		</button>

		<span class="cluster">
			<button type="button" class="icon" aria-label={accountLabel} onclick={onaccount}>
				<span class="avatar">
					<Identicon svg={browser.account.identiconSvg} size="row" />
					{#if browser.connected}
						<span class="dot"></span>
					{/if}
				</span>
			</button>

			<button type="button" class="icon" aria-label={copy.tabs} onclick={ontabs}>
				<span class="count">{browser.tabCount}</span>
			</button>

			<button type="button" class="icon" aria-label={copy.siteMenu} onclick={onmenu}>
				<Icon icon={UTILITY_ICONS.ellipsis} size="lg" />
			</button>
		</span>
	{/if}
</header>

<style>
	/* Hit targets abut, so the 44s carry the spacing: the glyphs sit a
	   screen-padding's worth in from either edge without the bar padding it
	   twice, and the pill keeps every pixel the controls do not need. */
	.bar {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		padding: var(--space-md) var(--space-sm);
		background: var(--color-bg-base);
	}

	.icon {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		width: var(--size-hitTarget);
		height: var(--size-hitTarget);
		padding: 0;
		border: none;
		background: none;
		color: var(--color-fg-base);
		cursor: pointer;
	}

	.icon:active {
		transform: scale(var(--motion-press-button));
	}

	/* The account, the count and ⋯ read as one cluster: their 44s touch. */
	.cluster {
		display: flex;
		flex-shrink: 0;
	}

	.pill {
		display: flex;
		flex: 1;
		align-items: center;
		justify-content: center;
		gap: var(--space-md);
		min-width: 0;
		height: var(--size-addressPill);
		padding-inline: var(--space-lg);
		border: none;
		border-radius: var(--radius-full);
		background: var(--color-bg-raised);
		font-family: var(--font-ui);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		color: var(--color-fg-base);
		cursor: text;
	}

	.lock {
		display: flex;
		flex-shrink: 0;
		color: var(--color-fg-muted);
	}

	/* Cut from the START (see the doc above). The paragraph runs right to
	   left so the ellipsis lands on the left edge; the host inside is
	   isolated left to right, so a port or a digit never reorders. */
	.host {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		direction: rtl;
	}

	.host bdi {
		direction: ltr;
		unicode-bidi: isolate;
	}

	/* The field takes the bar, and ends a gutter in from the edge as ‹'s
	   glyph starts one in from the other. Its caret is its focus, as in the
	   home's search field. */
	.field {
		justify-content: flex-start;
		margin-inline-end: var(--space-lg);
		outline: none;
	}

	.avatar {
		position: relative;
		display: flex;
	}

	/* The connection, as one green dot on the account — ringed in the bar's
	   own colour so it reads on any artwork. */
	.dot {
		position: absolute;
		inset-inline-end: calc(var(--border-emphasis) * -1);
		bottom: calc(var(--border-emphasis) * -1);
		width: var(--space-md);
		height: var(--space-md);
		border: var(--border-emphasis) solid var(--color-bg-base);
		border-radius: var(--radius-full);
		background: var(--color-success-base);
		box-sizing: content-box;
	}

	.count {
		display: flex;
		align-items: center;
		justify-content: center;
		box-sizing: border-box;
		min-width: var(--size-tabCount);
		height: var(--size-tabCount);
		padding-inline: var(--space-sm);
		border: var(--border-emphasis) solid currentColor;
		border-radius: var(--radius-sm);
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		font-variant-numeric: tabular-nums;
		line-height: 1;
	}
</style>
