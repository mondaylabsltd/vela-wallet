<script lang="ts">
	/**
	 * What the packaged extension has to say about itself, above every page
	 * (spec 094). Draws nothing anywhere else — on the hosted site, in a test —
	 * and nothing when there is nothing to say.
	 *
	 * - **Site access limited (S2).** A person can withhold the extension's host
	 *   permissions after install. Then sites cannot find the wallet and the
	 *   passkey cannot be used, and before this nothing said so: a dApp showed
	 *   "no wallet", a ceremony failed with Chrome's own SecurityError. Said in
	 *   plain words, with the one-click grant beside them — the button calls
	 *   `chrome.permissions.request` inside its own click, as it must.
	 * - **Just installed, with tabs already open (S3).** Chrome put no provider
	 *   into pages that were open before the install, and this extension does
	 *   not ask for the `scripting` permission to put one in; the welcome tab
	 *   the worker opened says to reload them, once.
	 */
	import { onMount } from 'svelte';
	import Button from '$lib/ui/Button.svelte';
	import { INSTALLED_KEY } from '../../../extension/lib/locales.js';
	import type { ExtensionMessages } from './messages';
	import { isPackagedApp } from './page-url';
	import { onSiteAccessChange, requestSiteAccess, siteAccessGranted } from './site-access';

	interface Props {
		/** `undefined` on a page outside `[locale]` — then nothing is drawn. */
		messages: ExtensionMessages | undefined;
	}

	let { messages }: Props = $props();

	let accessLimited = $state(false);
	let installed = $state(false);
	let asking = $state(false);

	async function readAccess(): Promise<void> {
		accessLimited = (await siteAccessGranted()) === false;
	}

	onMount(() => {
		if (!isPackagedApp()) return;
		try {
			installed = sessionStorage.getItem(INSTALLED_KEY) === '1';
		} catch {
			installed = false;
		}
		void readAccess();
		return onSiteAccessChange(() => void readAccess());
	});

	function allow(): void {
		// No `await` before the request: the click's user gesture is what lets
		// Chrome show its prompt at all.
		asking = true;
		void requestSiteAccess().then(async () => {
			asking = false;
			await readAccess();
		});
	}

	function dismissInstalled(): void {
		installed = false;
		try {
			sessionStorage.removeItem(INSTALLED_KEY);
		} catch {
			/* nothing kept, nothing to drop */
		}
	}
</script>

{#if messages && accessLimited}
	<div class="notice warning" role="status" data-testid="ext-site-access">
		<p>{messages.accessNote}</p>
		<div class="action">
			<Button variant="primary" shape="rounded" loading={asking} onclick={allow}>
				{messages.accessAllow}
			</Button>
		</div>
	</div>
{/if}
{#if messages && installed}
	<div class="notice" role="status" data-testid="ext-installed">
		<p>{messages.installedNote}</p>
		<div class="action">
			<Button variant="secondary" shape="rounded" onclick={dismissInstalled}>
				{messages.dismiss}
			</Button>
		</div>
	</div>
{/if}

<style>
	/* An open strip on the page's own surface, a hairline under it — the
	   design language's notice, not a card. */
	.notice {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: center;
		gap: var(--space-xs) var(--space-md);
		padding: var(--space-sm) var(--layout-screenPaddingX);
		border-bottom: var(--border-hairline) solid var(--color-border-base);
		font-size: var(--text-sm);
		color: var(--color-fg-muted);
	}
	.notice p {
		margin: 0;
		flex: 1 1 16rem;
		max-width: 40rem;
		text-align: start;
	}
	/* The shared Button, at the size of a line of text: a notice's action,
	   not a page's. */
	.action {
		flex: 0 0 auto;
	}
	.action :global(.button) {
		width: auto;
		min-height: var(--size-control-sm);
		padding-inline: var(--space-lg);
		padding-block: var(--space-xs);
		font-size: var(--text-sm);
	}
	.notice.warning {
		background: var(--color-warning-soft, var(--color-bg-sunken));
		color: var(--color-warning-base);
	}
</style>
