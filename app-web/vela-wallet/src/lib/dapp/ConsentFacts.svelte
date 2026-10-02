<script lang="ts">
	/**
	 * What a Connect shares (spec 096 F11): the account — its artwork, name and
	 * short address — and the network, as the in-app browsers' consent names
	 * them (desktop, iOS, Android: an account row, then a network row). The
	 * request window used to say only "Connect to <host>" and one sentence, so a
	 * person approved without seeing which account a site would get.
	 *
	 * Hairline rows, no card: the wallet's minimal language.
	 */
	import Identicon from '$lib/wallet/ui/Identicon.svelte';
	import RemoteLogo from '$lib/wallet/ui/RemoteLogo.svelte';

	interface Props {
		accountLabel: string;
		networkLabel: string;
		/** `null` when nobody is signed in — the row is then left out. */
		account: { name: string; address: string; short: string; identiconSvg: string } | null;
		network: { name: string; logoUrl?: string };
	}

	let { accountLabel, networkLabel, account, network }: Props = $props();
</script>

<dl class="facts" data-testid="consent-facts">
	{#if account}
		<div class="row">
			<dt>{accountLabel}</dt>
			<dd class="account">
				<Identicon svg={account.identiconSvg} size="inline" address={account.address} />
				<span class="who">
					{#if account.name !== ''}<span class="name">{account.name}</span>{/if}
					<span class="address">{account.short}</span>
				</span>
			</dd>
		</div>
	{/if}
	<div class="row">
		<dt>{networkLabel}</dt>
		<dd class="network">
			<span class="chain">
				<span class="dot"></span>
				<RemoteLogo urls={network.logoUrl === undefined ? undefined : [network.logoUrl]} />
			</span>
			{network.name}
		</dd>
	</div>
</dl>

<style>
	.facts {
		display: flex;
		flex-direction: column;
		margin: 0;
	}

	.row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-lg);
		padding-block: var(--space-lg);
		border-top: var(--border-hairline) solid var(--color-border-base);
	}

	.row:last-child {
		border-bottom: var(--border-hairline) solid var(--color-border-base);
	}

	dt {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	dd {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		margin: 0;
		min-width: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	.who {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		min-width: 0;
	}

	.name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.address {
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.chain {
		position: relative;
		display: grid;
		flex: none;
		place-items: center;
		width: var(--space-xl);
		height: var(--space-xl);
	}

	.dot {
		width: var(--space-md);
		height: var(--space-md);
		border-radius: var(--radius-full);
		background: var(--color-bg-sunken);
	}
</style>
