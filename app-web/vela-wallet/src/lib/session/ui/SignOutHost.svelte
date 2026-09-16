<script lang="ts">
	/**
	 * Where the core's sign-out dialog is drawn.
	 *
	 * **The sheet has to live on every route that can ask for it.** The row
	 * that dispatches `session.signOut()` is the SETTINGS screen's, but
	 * `SignOutSheet` was rendered only inside the wallet route — so a confirm
	 * on settings set `sign_out` in the session view and then had nowhere to
	 * appear: the settings dialog simply sat there, the session stayed, and
	 * the sheet materialised — orphaned from the tap that caused it — the next
	 * time somebody opened the wallet (issue 214, web, reported 2026-09-15).
	 *
	 * Mounted once per signed-in route, like `IdenticonViewerHost`, which is
	 * the web's answer to the app-level host the phones get for free (Android
	 * draws it in `VelaNavHost`, iOS in `RootView`).
	 *
	 * The open condition is the CORE's `sign_out`, never a local flag: it is
	 * non-null only after the machine has asked storage whether any public key
	 * is still unconfirmed, so the warning the sheet carries is an answer
	 * rather than this screen's guess (session invariant ⑤).
	 */
	import { session } from '../core/session.svelte';
	import type { WalletMessages } from '$lib/wallet/messages';
	import SignOutSheet from './SignOutSheet.svelte';

	interface Props {
		copy: WalletMessages['signOut'];
	}

	let { copy }: Props = $props();

	const signOut = $derived(session.view.sign_out);
</script>

{#if signOut}
	<SignOutSheet
		{copy}
		pendingUploadWarning={signOut.pending_upload_warning}
		onConfirm={() => session.confirmSignOut()}
		onDismiss={() => session.dismissSignOut()}
	/>
{/if}
