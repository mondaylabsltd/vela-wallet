<script lang="ts">
	/**
	 * One plain line under the account's keys (owner, 2026-10-29): "This
	 * account is a standard Safe — you can also open it in Safe{Wallet}". A
	 * Vela account and a multisig are both Safes, so the line is the same under
	 * either; the link leaves for Safe{Wallet} in a new tab.
	 */
	import type { SafeWalletLineModel } from '../model';

	interface Props {
		line: SafeWalletLineModel;
	}

	let { line }: Props = $props();
</script>

<p class="safe-wallet">
	{line.before}<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- Safe{Wallet}, not an app route --><a
		href={line.href}
		target="_blank"
		rel="noreferrer noopener">{line.app}</a
	>{line.after}
</p>

<style>
	.safe-wallet {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		overflow-wrap: break-word;
	}

	a {
		color: var(--color-fg-base);
		text-decoration: underline;
		text-underline-offset: var(--space-xs);
	}
</style>
