<script lang="ts">
	/**
	 * The spaces over every page (spec 105).
	 *
	 * - In the testnet space, a bar across the top of every screen: nothing
	 *   here has real value. In the flow of the page, so it pushes the page
	 *   down rather than covering its header.
	 * - The switch prompt, opened by the header's space label or Settings →
	 *   Networks: it says what the other space is, and the person's answer
	 *   goes through the same intent an agent's request takes
	 *   (`set_network_space`). The intents — and the dApp channel they
	 *   reach — are imported when the person answers yes, not with the layer:
	 *   it is over every page, and Welcome carries neither (`budgets.e2e.ts`).
	 */
	import PromptSheet from '$lib/ui/onboarding/v2/PromptSheet.svelte';
	import type { NetworkSpaceMessages } from './messages';
	import { networkSpace } from './network-space.svelte';

	interface Props {
		messages: NetworkSpaceMessages;
	}

	let { messages }: Props = $props();

	$effect(() => {
		networkSpace.words = messages;
	});

	const to = $derived(networkSpace.testnet ? 'mainnet' : 'testnet');

	/**
	 * home-10: the bar is in the flow of the page, and every screen frames
	 * itself at the viewport's height — so on a phone the bar pushed the tab
	 * bar off the bottom. Its height is published as `--space-bar`, and the
	 * frames are the viewport less it.
	 */
	let barHeight = $state(0);
	$effect(() => {
		const root = document.documentElement;
		if (!networkSpace.testnet || barHeight === 0) {
			root.style.removeProperty('--space-bar');
			return;
		}
		root.style.setProperty('--space-bar', `${barHeight}px`);
		return () => root.style.removeProperty('--space-bar');
	});
</script>

{#if networkSpace.testnet}
	<div class="bar" role="status" data-testid="testnet-bar" bind:offsetHeight={barHeight}>
		<button type="button" onclick={() => networkSpace.openSwitcher()}>{messages.bar}</button>
	</div>
{/if}

{#if networkSpace.switcherOpen}
	<PromptSheet
		copy={{
			title: messages.title,
			message: to === 'testnet' ? messages.testnetHint : messages.mainnetHint,
			confirm: {
				confirmLabel: to === 'testnet' ? messages.toTestnet : messages.toMainnet,
				cancelLabel: messages.cancel
			}
		}}
		dismissLabel={messages.cancel}
		onAnswer={(switching) => {
			networkSpace.switcherOpen = false;
			if (!switching) return;
			const space = to;
			void import('$lib/intents/index.svelte').then(({ intents }) =>
				intents.invoke('person', { type: 'set_network_space', space })
			);
		}}
	/>
{/if}

<style>
	.bar {
		position: sticky;
		top: 0;
		z-index: 60;
		display: flex;
		justify-content: center;
		padding: var(--space-xs) var(--space-lg);
		padding-top: calc(var(--space-xs) + env(safe-area-inset-top, 0px));
		background: var(--color-warning-soft);
		border-bottom: var(--border-hairline) solid var(--color-warning-border);
	}

	.bar button {
		border: none;
		background: none;
		padding: 0;
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: 600;
		color: var(--color-fg-base);
		cursor: pointer;
		text-align: center;
	}
</style>
