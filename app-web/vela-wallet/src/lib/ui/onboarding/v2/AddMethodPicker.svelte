<script lang="ts">
	/**
	 * The three ways to mint a founding key, expanded in place.
	 *
	 * On the web all three are the browser's to present: `navigator.credentials`
	 * shows its own picker covering this device, a nearby device and a security
	 * key. So every option here dispatches the SAME ceremony and differs only in
	 * what the core records as the person's choice — which is what labels the
	 * key row afterwards.
	 *
	 * That is a web-only truth. The native shells run the ceremony themselves
	 * and must honour the choice, which is why it travels through the core
	 * rather than being decided here.
	 */
	import type { KeyMethod } from '$lib/onboarding/generated/KeyMethod';
	import { methodCopy } from '$lib/onboarding/core/copy';
	import { methodIcon } from '$lib/onboarding/passkey-icons';
	import PasskeyMethodIcon from '$lib/ui/onboarding/PasskeyMethodIcon.svelte';

	interface Props {
		open: boolean;
		strings: (key: string) => string;
		onPick: (method: KeyMethod) => void;
	}

	let { open, strings, onPick }: Props = $props();

	const METHODS: KeyMethod[] = ['platform', 'hybrid', 'security_key'];
</script>

{#if open}
	<ul class="methods">
		{#each METHODS as method (method)}
			{@const copy = methodCopy(method)}
			<li>
				<button class="method" type="button" onclick={() => onPick(method)}>
					<PasskeyMethodIcon id={methodIcon(method)} />
					<span class="text">
						<span class="name">{strings(copy.title)}</span>
						<span class="caption">{strings(copy.body)}</span>
					</span>
				</button>
			</li>
		{/each}
	</ul>
{/if}

<style>
	.methods {
		display: flex;
		flex-direction: column;
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.method {
		display: flex;
		gap: var(--space-lg);
		align-items: center;
		width: 100%;
		padding-block: var(--space-lg);
		border: 0;
		border-bottom: var(--border-hairline) solid var(--color-border-base);
		background: none;
		font-family: var(--font-ui);
		text-align: start;
		cursor: pointer;
	}

	.method:hover {
		background: var(--color-bg-sunken);
	}

	.text {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	.name {
		color: var(--color-fg-base);
		font-size: var(--text-base);
		font-weight: var(--weight-semibold);
	}

	.caption {
		color: var(--color-fg-muted);
		font-size: var(--text-sm);
	}
</style>
