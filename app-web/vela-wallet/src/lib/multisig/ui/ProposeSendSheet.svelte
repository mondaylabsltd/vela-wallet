<script lang="ts">
	/**
	 * Proposing a transfer from the multisig: what to send, how much, to whom —
	 * then one passkey prompt, which is also the first approval. What the
	 * proposal is, is the core's (`transfersTx`, read back by `reviewTx` before
	 * anything is signed); the sheet hands over to the transaction's own sheet,
	 * where it is executed or the other members are asked.
	 *
	 * The whole balance can be proposed: the fee is the executor's, paid from
	 * its own account, never from the multisig (every gas field is zero).
	 */
	import Button from '$lib/ui/Button.svelte';
	import { fillTemplate } from '$lib/i18n/fill';
	import type { BalanceToken } from '$lib/core/generated/BalanceToken';
	import { balanceTokenId } from '$lib/wallet/live';
	import type { MultisigMessages } from '../messages';
	import { chainName, formatUnits, parseUnits } from '../core/display';
	import { reviewTx, transfersTx } from '../core/kernel';
	import { localOwners } from '../core/owners';
	import { proposals, propose } from '../core/proposals.svelte';
	import { multisig, type MultisigAccount } from '../core/store.svelte';
	import Sheet from './Sheet.svelte';

	interface Props {
		m: MultisigMessages;
		account: MultisigAccount;
		tokens: BalanceToken[];
		assetId?: string;
		wide: boolean;
		closeLabel: string;
		onclose: () => void;
		onproposed: (chainId: number, safeTxHash: string) => void;
	}

	let { m, account, tokens, assetId, wide, closeLabel, onclose, onproposed }: Props = $props();

	const holdings = $derived(tokens.filter((t) => !t.spam && Number(t.balance) > 0));
	let picked = $state<string | null>(null);
	const token = $derived(
		holdings.find((t) => balanceTokenId(t) === (picked ?? assetId)) ?? holdings[0] ?? null
	);
	let amountText = $state('');
	let recipient = $state('');
	let touched = $state(false);
	let failure = $state<string | null>(null);
	let busy = $state(false);

	const local = $derived(localOwners(account.owners));
	let ownerChoice = $state<string | null>(null);
	const owner = $derived(local.find((o) => o.address === ownerChoice) ?? local[0] ?? null);

	const balanceUnits = $derived(token ? parseUnits(token.balance, token.decimals) : null);
	const amount = $derived(token ? parseUnits(amountText, token.decimals) : null);
	const amountOk = $derived(amount !== null && amount > 0n && balanceUnits !== null && amount <= balanceUnits);
	const recipientOk = $derived(/^0x[0-9a-fA-F]{40}$/.test(recipient.trim()));
	const ready = $derived(!!token && amountOk && recipientOk && !!owner && !busy);

	function useMax() {
		if (token) amountText = token.balance;
	}

	async function submit() {
		touched = true;
		if (!ready || !token || amount === null || !owner) return;
		busy = true;
		failure = null;
		try {
			const chainId = token.chain_id;
			multisig.useChain(account.address, chainId);
			const chains = [...new Set([...account.chainIds, chainId])];
			await proposals.refresh(account, chains);
			const nonce = proposals.nextNonce(chainId);
			const tx = transfersTx(
				[{ token: token.token_address, to: recipient.trim(), amount: amount.toString() }],
				`0x${nonce.toString(16)}`
			);
			// The core reads back what it built before anything is signed.
			const review = reviewTx(chainId, account.address, tx);
			if (review.action === null) throw new Error(review.refusal ?? 'refused');
			const { safeTxHash } = await propose(account, chainId, tx, owner);
			await proposals.refresh(account, chains);
			onproposed(chainId, safeTxHash);
		} catch (error) {
			const name = error instanceof DOMException ? error.name : '';
			if (name !== 'NotAllowedError' && name !== 'AbortError') {
				failure = error instanceof Error ? error.message : String(error);
			}
		} finally {
			busy = false;
		}
	}
</script>

<Sheet title={m.send.title} {closeLabel} {wide} {busy} {onclose}>
	{#if holdings.length === 0}
		<p class="quiet">{m.send.noAssets}</p>
	{:else}
		<form
			onsubmit={(event) => {
				event.preventDefault();
				void submit();
			}}
		>
			<fieldset class="assets">
				<legend>{m.send.asset}</legend>
				{#each holdings as t (balanceTokenId(t))}
					{@const on = token !== null && balanceTokenId(t) === balanceTokenId(token)}
					<label class:on>
						<input type="radio" name="asset" checked={on} onchange={() => (picked = balanceTokenId(t))} />
						<span class="glyph" aria-hidden="true">{t.symbol.slice(0, 3)}</span>
						<span class="asset-text">
							<span class="symbol">{t.symbol}</span>
							<span class="chain">{chainName(t.chain_id)}</span>
						</span>
						<span class="held">{formatUnits(parseUnits(t.balance, t.decimals) ?? 0n, t.decimals)}</span>
					</label>
				{/each}
			</fieldset>

			<label class="field">
				<span class="field-label">{m.send.amount}</span>
				<span class="amount-row">
					<input
						inputmode="decimal"
						autocomplete="off"
						placeholder="0"
						bind:value={amountText}
						aria-invalid={touched && !amountOk}
					/>
					<span class="unit">{token?.symbol}</span>
					<button type="button" class="max" onclick={useMax}>{m.send.max}</button>
				</span>
				{#if token}
					<span class="hint">{fillTemplate(m.send.balance, { amount: `${formatUnits(balanceUnits ?? 0n, token.decimals)} ${token.symbol}` })}</span>
				{/if}
				{#if touched && !amountOk}<span class="error">{m.send.invalidAmount}</span>{/if}
			</label>

			<label class="field">
				<span class="field-label">{m.send.recipient}</span>
				<input
					class="mono"
					autocomplete="off"
					spellcheck="false"
					placeholder={m.send.recipientPlaceholder}
					bind:value={recipient}
					aria-invalid={touched && !recipientOk}
				/>
				{#if touched && !recipientOk}<span class="error">{m.send.invalidAddress}</span>{/if}
			</label>

			{#if local.length > 1}
				<fieldset class="actor">
					<legend>{m.proposal.as}</legend>
					{#each local as o (o.address)}
						<label class:on={owner?.address === o.address}>
							<input type="radio" name="owner" checked={owner?.address === o.address} onchange={() => (ownerChoice = o.address)} />
							{o.name}
						</label>
					{/each}
				</fieldset>
			{/if}

			<p class="hint">{fillTemplate(m.send.hint, { threshold: account.threshold })}</p>
			{#if failure}<p class="error" role="alert">{fillTemplate(m.proposal.failed, { reason: failure })}</p>{/if}

			<Button variant="primary" shape="rounded" loading={busy} disabled={busy || !owner} onclick={() => void submit()}>
				{m.send.propose}
			</Button>
		</form>
	{/if}
</Sheet>

<style>
	form {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
	}

	fieldset {
		margin: 0;
		padding: 0;
		border: 0;
	}

	legend,
	.field-label {
		display: block;
		margin-block-end: var(--space-sm);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.assets {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		max-height: calc(var(--size-siteRow) * 4);
		overflow-y: auto;
	}

	.assets label {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		padding: var(--space-sm) var(--space-md);
		border-radius: var(--radius-lg);
		border: var(--border-hairline) solid var(--color-border-base);
		cursor: pointer;
	}

	.assets label.on {
		border-color: var(--color-border-strong);
		background: var(--color-bg-sunken);
	}

	input[type='radio'] {
		position: absolute;
		opacity: 0;
		pointer-events: none;
	}

	.glyph {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		flex: none;
		width: var(--size-control-sm);
		height: var(--size-control-sm);
		border-radius: var(--radius-full);
		background: var(--color-bg-sunken);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-muted);
	}

	.asset-text {
		display: flex;
		flex-direction: column;
		flex: 1;
		min-width: 0;
	}

	.symbol {
		font-weight: var(--weight-medium);
		color: var(--color-fg-base);
	}

	.chain {
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.held {
		font-variant-numeric: tabular-nums;
		color: var(--color-fg-muted);
	}

	.field {
		display: flex;
		flex-direction: column;
	}

	.field input,
	.amount-row {
		width: 100%;
		box-sizing: border-box;
		min-height: var(--size-control-lg);
		padding: 0 var(--space-md);
		border: var(--border-hairline) solid var(--color-border-strong);
		border-radius: var(--radius-lg);
		background: var(--color-bg-base);
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	.amount-row {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
	}

	.amount-row input {
		flex: 1;
		min-width: 0;
		min-height: 0;
		padding: 0;
		border: 0;
		background: none;
		font-size: calc(var(--text-xl) * var(--text-scale, 1));
		font-variant-numeric: tabular-nums;
	}

	.amount-row input:focus {
		outline: none;
	}

	.unit {
		color: var(--color-fg-muted);
	}

	.max {
		padding: var(--space-xs) var(--space-sm);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-full);
		background: var(--color-bg-sunken);
		font-family: var(--font-ui);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-base);
		cursor: pointer;
	}

	.mono {
		font-family: var(--font-mono) !important;
		font-size: calc(var(--text-sm) * var(--text-scale, 1)) !important;
	}

	.hint {
		margin: var(--space-xs) 0 0;
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.error {
		margin: var(--space-xs) 0 0;
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-error-base);
	}

	.actor {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-sm);
	}

	.actor label {
		display: inline-flex;
		padding: var(--space-sm) var(--space-md);
		border-radius: var(--radius-full);
		border: var(--border-hairline) solid var(--color-border-base);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		cursor: pointer;
	}

	.actor label.on {
		border-color: var(--color-border-strong);
		background: var(--color-bg-sunken);
	}

	.quiet {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}
</style>
