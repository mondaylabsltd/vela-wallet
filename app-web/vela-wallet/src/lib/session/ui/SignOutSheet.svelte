<script lang="ts">
	/**
	 * The way back out of a signed-in wallet.
	 *
	 * **This exists because wiring a route guard without wiring its exit
	 * produces an app you cannot leave.** Spec 019 hit exactly that on the
	 * desktop and on both phones; the web reached its wallet last and inherits
	 * the fix rather than the wall.
	 *
	 * Two things here are the core's, not this sheet's:
	 *
	 * - **The warning.** `pendingUploadWarning` is the session machine's answer
	 *   after it asks storage whether any public key is still unconfirmed — not
	 *   this screen's guess. The dialog does not open until the machine has one,
	 *   which is why the caller renders on `sign_out !== null` rather than on a
	 *   local flag.
	 * - **What sign-out clears.** The account list and the active index, and
	 *   nothing else; contacts, history, tokens and settings belong to the
	 *   account, and the account comes back because its address derives from
	 *   the passkey.
	 *
	 * Shape follows `PromptSheet`: bottom sheet below the desktop breakpoint,
	 * centred card above it.
	 */
	import { MediaQuery } from 'svelte/reactivity';
	import Sheet from '$lib/ui/onboarding/Sheet.svelte';
	import Button from '$lib/ui/Button.svelte';
	import { BREAKPOINT_DESKTOP } from '$lib/tokens/tokens';
	import type { WalletMessages } from '$lib/wallet/messages';

	interface Props {
		copy: WalletMessages['signOut'];
		/** The core's answer about unconfirmed key uploads. */
		pendingUploadWarning: boolean;
		/** How many wallets go with it. One and six cannot read the same. */
		accountCount?: number;
		onConfirm: () => void;
		onDismiss: () => void;
	}

	let { copy, pendingUploadWarning, accountCount = 1, onConfirm, onDismiss }: Props = $props();

	const desktop = new MediaQuery(`(min-width: ${BREAKPOINT_DESKTOP}px)`, false);
	let sheet = $state<{ requestClose: () => void }>();

	/** A dismissal is a refusal; only the button confirms. */
	let confirmed = false;

	function confirm() {
		confirmed = true;
		close();
	}

	function close() {
		if (sheet) sheet.requestClose();
		else settle();
	}

	function settle() {
		if (confirmed) onConfirm();
		else onDismiss();
	}
</script>

{#snippet body()}
	<div class="signout">
		<h2 class="title">{copy.title}</h2>
		<!-- What this takes, when it is more than one wallet. `keeps` below is
		     true either way, and on a browser holding six it was ALSO how the
		     sheet managed to say nothing about signing in six times. -->
		{#if accountCount > 1}
			<p class="many">{copy.descMany.replace('{{count}}', String(accountCount))}</p>
		{/if}
		<p class="keeps">{copy.keeps}</p>

		{#if pendingUploadWarning}
			<p class="warning">{copy.warning}</p>
		{/if}

		<div class="actions">
			<!-- "Sign out anyway" when there is something to be anyway ABOUT;
			     plain "Sign out" otherwise. Wording the risk into the button is
			     what makes the warning above more than decoration. -->
			<!-- Red, like the fixture layer's `signOutSheet` declares (`tone:
			     'danger'`) and like the settings dialog this is now the only
			     sign-out confirm in place of. -->
			<Button variant="danger" shape="rounded" onclick={confirm}>
				{pendingUploadWarning ? copy.anyway : copy.button}
			</Button>
			<Button variant="secondary" shape="rounded" onclick={close}>{copy.cancel}</Button>
		</div>
	</div>
{/snippet}

{#if desktop.current}
	<div
		class="scrim"
		role="dialog"
		aria-modal="true"
		aria-label={copy.title}
		tabindex="-1"
		onkeydown={(event) => event.key === 'Escape' && close()}
	>
		<div class="card">{@render body()}</div>
	</div>
{:else}
	<Sheet bind:this={sheet} label={copy.title} onClose={settle}>
		{@render body()}
	</Sheet>
{/if}

<style>
	.scrim {
		position: fixed;
		inset: 0;
		z-index: 10;
		display: grid;
		place-items: center;
		padding: var(--space-3xl);
		background: var(--color-fixed-backdrop);
	}

	.card {
		width: 100%;
		max-width: var(--layout-promptCard);
		padding: var(--space-3xl);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-xl);
		background: var(--color-bg-raised);
		box-shadow: var(--shadow-lg);
	}

	.signout {
		display: flex;
		flex-direction: column;
		gap: var(--space-2xl);
	}

	.title {
		margin: 0;
		color: var(--color-fg-base);
		font-size: var(--text-xl);
		font-weight: var(--weight-bold);
		line-height: var(--leading-tight);
		letter-spacing: -0.01em;
	}

	.keeps {
		margin: 0;
		color: var(--color-fg-muted);
		font-size: var(--text-base);
		line-height: var(--leading-relaxed);
	}

	.warning {
		margin: 0;
		color: var(--color-fg-base);
		font-size: var(--text-sm);
		line-height: var(--leading-relaxed);
		border-left: var(--border-emphasis) solid var(--color-warning-base);
		padding-left: var(--space-lg);
	}

	.actions {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
	}
</style>
